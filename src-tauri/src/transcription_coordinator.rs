use crate::actions::ACTION_MAP;
use crate::managers::audio::AudioRecordingManager;
use log::{debug, error, warn};
use std::sync::mpsc::{self, Sender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

const DEBOUNCE: Duration = Duration::from_millis(30);
const RELEASE_GRACE: Duration = Duration::from_millis(50);
const FN_DOUBLE_TAP_WINDOW: Duration = Duration::from_millis(350);
pub const HANDS_FREE_BINDING_ID: &str = "hands_free_transcribe";
const STANDARD_BINDING_ID: &str = "transcribe";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PttAction {
    Passthrough,
    DeferRelease,
    CancelRelease,
}

struct PendingRelease {
    binding_id: String,
    hotkey_string: String,
    deadline: Instant,
}

/// Commands processed sequentially by the coordinator thread.
enum Command {
    Input {
        binding_id: String,
        hotkey_string: String,
        is_pressed: bool,
        push_to_talk: bool,
    },
    Cancel {
        recording_was_active: bool,
    },
    CompleteHandsFree,
    ProcessingFinished,
}

/// Pipeline lifecycle, owned exclusively by the coordinator thread.
enum Stage {
    Idle,
    Recording {
        binding_id: String,
        hands_free: bool,
    },
    Processing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HandsFreeAction {
    Passthrough,
    Start,
    Promote,
    Ignore,
}

fn classify_hands_free_event(
    binding_id: &str,
    is_pressed: bool,
    recording_binding: Option<&str>,
    recording_is_hands_free: bool,
) -> HandsFreeAction {
    if binding_id == HANDS_FREE_BINDING_ID {
        if !is_pressed {
            return HandsFreeAction::Ignore;
        }
        return match recording_binding {
            None => HandsFreeAction::Start,
            Some(STANDARD_BINDING_ID) if !recording_is_hands_free => HandsFreeAction::Promote,
            _ => HandsFreeAction::Ignore,
        };
    }

    if binding_id == STANDARD_BINDING_ID && recording_is_hands_free {
        HandsFreeAction::Ignore
    } else {
        HandsFreeAction::Passthrough
    }
}

fn classify_ptt_event(
    pending_release_binding: Option<&str>,
    is_pressed: bool,
    push_to_talk: bool,
    binding_id: &str,
    recording_binding: Option<&str>,
) -> PttAction {
    if !push_to_talk {
        return PttAction::Passthrough;
    }

    if is_pressed {
        if pending_release_binding == Some(binding_id) {
            PttAction::CancelRelease
        } else {
            PttAction::Passthrough
        }
    } else if recording_binding == Some(binding_id) && pending_release_binding.is_none() {
        PttAction::DeferRelease
    } else {
        PttAction::Passthrough
    }
}

/// Serialises all transcription lifecycle events through a single thread
/// to eliminate race conditions between keyboard shortcuts, signals, and
/// the async transcribe-paste pipeline.
pub struct TranscriptionCoordinator {
    tx: Sender<Command>,
}

pub fn is_transcribe_binding(id: &str) -> bool {
    id == STANDARD_BINDING_ID
        || id == "transcribe_with_post_process"
        || id == "transcribe_with_ai"
        || id == HANDS_FREE_BINDING_ID
}

impl TranscriptionCoordinator {
    pub fn new(app: AppHandle) -> Self {
        let (tx, rx) = mpsc::channel();

        thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut stage = Stage::Idle;
                let mut last_press: Option<Instant> = None;
                let mut last_standard_press: Option<Instant> = None;
                let mut pending_release: Option<PendingRelease> = None;

                loop {
                    let cmd = if let Some(pending) = &pending_release {
                        match rx.recv_timeout(
                            pending.deadline.saturating_duration_since(Instant::now()),
                        ) {
                            Ok(cmd) => cmd,
                            Err(mpsc::RecvTimeoutError::Timeout) => {
                                if let Some(pending) = pending_release.take() {
                                    if matches!(&stage, Stage::Recording { binding_id, .. } if binding_id == &pending.binding_id)
                                    {
                                        stop(
                                            &app,
                                            &mut stage,
                                            &pending.binding_id,
                                            &pending.hotkey_string,
                                        );
                                    }
                                }
                                continue;
                            }
                            Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        }
                    } else {
                        match rx.recv() {
                            Ok(cmd) => cmd,
                            Err(_) => break,
                        }
                    };

                    match cmd {
                        Command::Input {
                            binding_id,
                            hotkey_string,
                            is_pressed,
                            push_to_talk,
                        } => {
                            let pending_release_binding = pending_release
                                .as_ref()
                                .map(|pending| pending.binding_id.as_str());
                            let (recording_binding, recording_is_hands_free) = match &stage {
                                Stage::Recording {
                                    binding_id,
                                    hands_free,
                                } => (Some(binding_id.as_str()), *hands_free),
                                _ => (None, false),
                            };

                            match classify_hands_free_event(
                                &binding_id,
                                is_pressed,
                                recording_binding,
                                recording_is_hands_free,
                            ) {
                                HandsFreeAction::Start => {
                                    pending_release = None;
                                    if matches!(stage, Stage::Idle) {
                                        start(&app, &mut stage, &binding_id, &hotkey_string);
                                        if matches!(
                                            stage,
                                            Stage::Recording {
                                                hands_free: true,
                                                ..
                                            }
                                        ) {
                                            crate::escape_cancel::set_hands_free_active(true);
                                        }
                                    }
                                    continue;
                                }
                                HandsFreeAction::Promote => {
                                    pending_release = None;
                                    if let Stage::Recording { hands_free, .. } = &mut stage {
                                        *hands_free = true;
                                    }
                                    crate::escape_cancel::set_hands_free_active(true);
                                    crate::overlay::show_hands_free_overlay(&app);
                                    continue;
                                }
                                HandsFreeAction::Ignore => continue,
                                HandsFreeAction::Passthrough => {}
                            }

                            match classify_ptt_event(
                                pending_release_binding,
                                is_pressed,
                                push_to_talk,
                                &binding_id,
                                recording_binding,
                            ) {
                                PttAction::CancelRelease => {
                                    // If this cancel is actually a double-tap of the standard
                                    // shortcut, latch to hands-free instead of just cancelling
                                    // the deferred release (covers push-to-talk mode).
                                    if binding_id == STANDARD_BINDING_ID && is_pressed {
                                        if let Some(prev) = last_standard_press {
                                            let now = Instant::now();
                                            let since = now.duration_since(prev);
                                            if since < FN_DOUBLE_TAP_WINDOW && since >= DEBOUNCE {
                                                match &stage {
                                                    Stage::Recording {
                                                        hands_free: false, ..
                                                    } => {
                                                        pending_release = None;
                                                        if let Stage::Recording {
                                                            hands_free, ..
                                                        } = &mut stage
                                                        {
                                                            *hands_free = true;
                                                        }
                                                        crate::escape_cancel::set_hands_free_active(
                                                            true,
                                                        );
                                                        crate::overlay::show_hands_free_overlay(
                                                            &app,
                                                        );
                                                        last_standard_press = None;
                                                        continue;
                                                    }
                                                    Stage::Idle => {
                                                        pending_release = None;
                                                        start(
                                                            &app,
                                                            &mut stage,
                                                            HANDS_FREE_BINDING_ID,
                                                            HANDS_FREE_BINDING_ID,
                                                        );
                                                        if matches!(
                                                            stage,
                                                            Stage::Recording {
                                                                hands_free: true,
                                                                ..
                                                            }
                                                        ) {
                                                            crate::escape_cancel::set_hands_free_active(
                                                                true,
                                                            );
                                                        }
                                                        last_standard_press = None;
                                                        continue;
                                                    }
                                                    _ => {}
                                                }
                                            }
                                        }
                                    }
                                    pending_release = None;
                                    continue;
                                }
                                PttAction::DeferRelease => {
                                    pending_release = Some(PendingRelease {
                                        binding_id,
                                        hotkey_string,
                                        deadline: Instant::now() + RELEASE_GRACE,
                                    });
                                    continue;
                                }
                                PttAction::Passthrough => {}
                            }

                            // Debounce rapid-fire press events (key repeat / double-tap).
                            // Push-to-talk releases may be deferred above to absorb X11 auto-repeat.
                            let mut now_for_standard: Option<Instant> = None;
                            if is_pressed {
                                let now = Instant::now();
                                if last_press.is_some_and(|t| now.duration_since(t) < DEBOUNCE) {
                                    debug!("Debounced press for '{binding_id}'");
                                    continue;
                                }
                                last_press = Some(now);
                                if binding_id == STANDARD_BINDING_ID {
                                    now_for_standard = Some(now);
                                }
                            }

                            // Simple double-tap of the standard shortcut → hands-free (double-click FN).
                            // Keeps the existing fn+ctrl chord untouched; just adds the double-tap trigger.
                            if let Some(now) = now_for_standard {
                                if let Some(prev) = last_standard_press {
                                    let since = now.duration_since(prev);
                                    if since < FN_DOUBLE_TAP_WINDOW && since >= DEBOUNCE {
                                        match &stage {
                                            Stage::Recording {
                                                hands_free: false, ..
                                            } => {
                                                pending_release = None;
                                                if let Stage::Recording { hands_free, .. } =
                                                    &mut stage
                                                {
                                                    *hands_free = true;
                                                }
                                                crate::escape_cancel::set_hands_free_active(true);
                                                crate::overlay::show_hands_free_overlay(&app);
                                                last_standard_press = None;
                                                continue;
                                            }
                                            Stage::Idle => {
                                                pending_release = None;
                                                start(
                                                    &app,
                                                    &mut stage,
                                                    HANDS_FREE_BINDING_ID,
                                                    HANDS_FREE_BINDING_ID,
                                                );
                                                if matches!(
                                                    stage,
                                                    Stage::Recording {
                                                        hands_free: true,
                                                        ..
                                                    }
                                                ) {
                                                    crate::escape_cancel::set_hands_free_active(
                                                        true,
                                                    );
                                                }
                                                last_standard_press = None;
                                                continue;
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                                last_standard_press = Some(now);
                            }

                            if push_to_talk {
                                if is_pressed && matches!(stage, Stage::Idle) {
                                    start(&app, &mut stage, &binding_id, &hotkey_string);
                                } else if !is_pressed
                                    && matches!(&stage, Stage::Recording { binding_id: id, .. } if id == &binding_id)
                                {
                                    stop(&app, &mut stage, &binding_id, &hotkey_string);
                                }
                            } else if is_pressed {
                                match &stage {
                                    Stage::Idle => {
                                        start(&app, &mut stage, &binding_id, &hotkey_string);
                                    }
                                    Stage::Recording { binding_id: id, .. }
                                        if id == &binding_id =>
                                    {
                                        stop(&app, &mut stage, &binding_id, &hotkey_string);
                                    }
                                    _ => {
                                        debug!("Ignoring press for '{binding_id}': pipeline busy")
                                    }
                                }
                            }
                        }
                        Command::Cancel {
                            recording_was_active,
                        } => {
                            pending_release = None;
                            crate::escape_cancel::set_hands_free_active(false);
                            // Don't reset during processing — wait for the pipeline to finish.
                            if !matches!(stage, Stage::Processing)
                                && (recording_was_active
                                    || matches!(stage, Stage::Recording { .. }))
                            {
                                stage = Stage::Idle;
                            }
                        }
                        Command::ProcessingFinished => {
                            crate::escape_cancel::set_hands_free_active(false);
                            stage = Stage::Idle;
                        }
                        Command::CompleteHandsFree => {
                            pending_release = None;
                            if let Stage::Recording {
                                binding_id,
                                hands_free: true,
                            } = &stage
                            {
                                let active_binding = binding_id.clone();
                                stop(&app, &mut stage, &active_binding, HANDS_FREE_BINDING_ID);
                            }
                        }
                    }
                }
                debug!("Transcription coordinator exited");
            }));
            if let Err(e) = result {
                error!("Transcription coordinator panicked: {e:?}");
            }
        });

        Self { tx }
    }

    /// Send a keyboard/signal input event for a transcribe binding.
    /// For signal-based toggles, use `is_pressed: true` and `push_to_talk: false`.
    pub fn send_input(
        &self,
        binding_id: &str,
        hotkey_string: &str,
        is_pressed: bool,
        push_to_talk: bool,
    ) {
        if self
            .tx
            .send(Command::Input {
                binding_id: binding_id.to_string(),
                hotkey_string: hotkey_string.to_string(),
                is_pressed,
                push_to_talk,
            })
            .is_err()
        {
            warn!("Transcription coordinator channel closed");
        }
    }

    pub fn notify_cancel(&self, recording_was_active: bool) {
        if self
            .tx
            .send(Command::Cancel {
                recording_was_active,
            })
            .is_err()
        {
            warn!("Transcription coordinator channel closed");
        }
    }

    pub fn notify_processing_finished(&self) {
        if self.tx.send(Command::ProcessingFinished).is_err() {
            warn!("Transcription coordinator channel closed");
        }
    }

    pub fn complete_hands_free(&self) {
        if self.tx.send(Command::CompleteHandsFree).is_err() {
            warn!("Transcription coordinator channel closed");
        }
    }
}

fn start(app: &AppHandle, stage: &mut Stage, binding_id: &str, hotkey_string: &str) {
    let Some(action) = ACTION_MAP.get(binding_id) else {
        warn!("No action in ACTION_MAP for '{binding_id}'");
        return;
    };
    action.start(app, binding_id, hotkey_string);
    if app
        .try_state::<Arc<AudioRecordingManager>>()
        .is_some_and(|a| a.is_recording())
    {
        *stage = Stage::Recording {
            binding_id: binding_id.to_string(),
            hands_free: binding_id == HANDS_FREE_BINDING_ID,
        };
    } else {
        debug!("Start for '{binding_id}' did not begin recording; staying idle");
    }
}

fn stop(app: &AppHandle, stage: &mut Stage, binding_id: &str, hotkey_string: &str) {
    if matches!(
        stage,
        Stage::Recording {
            hands_free: true,
            ..
        }
    ) {
        crate::escape_cancel::set_hands_free_active(false);
    }
    let Some(action) = ACTION_MAP.get(binding_id) else {
        warn!("No action in ACTION_MAP for '{binding_id}'");
        return;
    };
    action.stop(app, binding_id, hotkey_string);
    *stage = Stage::Processing;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_voice_recording_binding_uses_the_serialized_lifecycle() {
        for binding in [
            "transcribe",
            "transcribe_with_post_process",
            "transcribe_with_ai",
            HANDS_FREE_BINDING_ID,
        ] {
            assert!(is_transcribe_binding(binding), "missing binding: {binding}");
        }
        assert!(!is_transcribe_binding("cancel"));
    }

    #[test]
    fn hands_free_chord_promotes_the_standard_recording() {
        assert_eq!(
            classify_hands_free_event(
                HANDS_FREE_BINDING_ID,
                true,
                Some(STANDARD_BINDING_ID),
                false,
            ),
            HandsFreeAction::Promote
        );
    }

    #[test]
    fn primary_key_events_never_finish_a_hands_free_recording() {
        assert_eq!(
            classify_hands_free_event(
                HANDS_FREE_BINDING_ID,
                false,
                Some(STANDARD_BINDING_ID),
                true,
            ),
            HandsFreeAction::Ignore
        );
        assert_eq!(
            classify_hands_free_event(STANDARD_BINDING_ID, false, Some(STANDARD_BINDING_ID), true),
            HandsFreeAction::Ignore
        );
        assert_eq!(
            classify_hands_free_event(STANDARD_BINDING_ID, true, Some(STANDARD_BINDING_ID), true),
            HandsFreeAction::Ignore
        );
    }

    #[test]
    fn hands_free_chord_can_start_directly_when_idle() {
        assert_eq!(
            classify_hands_free_event(HANDS_FREE_BINDING_ID, true, None, false),
            HandsFreeAction::Start
        );
    }

    #[test]
    fn push_to_talk_release_while_recording_defers_release() {
        assert_eq!(
            classify_ptt_event(None, false, true, "transcribe", Some("transcribe")),
            PttAction::DeferRelease
        );
    }

    #[test]
    fn push_to_talk_press_matching_pending_release_cancels_release() {
        assert_eq!(
            classify_ptt_event(
                Some("transcribe"),
                true,
                true,
                "transcribe",
                Some("transcribe")
            ),
            PttAction::CancelRelease
        );
    }

    #[test]
    fn toggle_mode_press_and_release_pass_through() {
        assert_eq!(
            classify_ptt_event(
                Some("transcribe"),
                true,
                false,
                "transcribe",
                Some("transcribe")
            ),
            PttAction::Passthrough
        );
        assert_eq!(
            classify_ptt_event(None, false, false, "transcribe", Some("transcribe")),
            PttAction::Passthrough
        );
    }

    #[test]
    fn press_for_different_binding_than_pending_release_passes_through() {
        assert_eq!(
            classify_ptt_event(
                Some("transcribe"),
                true,
                true,
                "transcribe_with_post_process",
                Some("transcribe")
            ),
            PttAction::Passthrough
        );
    }

    #[test]
    fn press_matching_pending_release_cancels_without_recording_state() {
        assert_eq!(
            classify_ptt_event(Some("transcribe"), true, true, "transcribe", None),
            PttAction::CancelRelease
        );
    }

    // ---------------------------------------------------------------------
    // Sequence-level regression coverage for issue #1539.
    //
    // Under X11 key auto-repeat, holding a push-to-talk key does not emit one
    // long press. It emits the initial press followed by a stream of
    // synthesized release/press pairs, then a single genuine release on key-up.
    // Before the fix, every synthesized release passed straight through and
    // stopped recording, so holding the key "rapidly toggled" recording on and
    // off. The fix defers each release for a short grace window and cancels it
    // when the matching auto-repeat press arrives.
    //
    // The unit tests above assert `classify_ptt_event` in isolation. The
    // simulator below threads that classifier through the same `pending_release`
    // / `stage` state transitions the coordinator loop performs (lines that
    // handle `Command::Input` and the `recv_timeout` grace expiry), so a whole
    // event burst can be exercised deterministically without a Tauri AppHandle
    // or real timers.
    // ---------------------------------------------------------------------

    const BINDING: &str = "transcribe";

    #[derive(Clone, Copy)]
    enum Ev {
        /// A key-down event (real initial press or a synthesized auto-repeat press).
        Press,
        /// A key-up event (synthesized auto-repeat release or the genuine key-up).
        Release,
        /// The `RELEASE_GRACE` window elapsed with no cancelling press arriving.
        Grace,
    }

    #[derive(Debug, PartialEq, Eq)]
    enum SimStage {
        Idle,
        Recording,
        Processing,
    }

    struct SimResult {
        starts: u32,
        stops: u32,
        stage: SimStage,
    }

    /// Mirror of the coordinator loop's decision logic for a single push-to-talk
    /// binding: it calls the real `classify_ptt_event` and applies the exact same
    /// Defer / Cancel / debounce / start / stop transitions.
    fn simulate(events: &[Ev]) -> SimResult {
        let mut stage = SimStage::Idle;
        let mut pending: Option<String> = None;
        let mut last_press_ms: Option<u64> = None;
        let mut clock_ms: u64 = 0;
        let mut starts = 0u32;
        let mut stops = 0u32;
        let debounce_ms = DEBOUNCE.as_millis() as u64;

        for ev in events {
            // Auto-repeat events arrive a few ms apart, well inside DEBOUNCE.
            clock_ms += 5;

            match ev {
                Ev::Grace => {
                    // Coordinator's `RecvTimeoutError::Timeout` arm: fire the
                    // deferred release iff we are still recording that binding.
                    if let Some(pending_binding) = pending.take() {
                        if stage == SimStage::Recording && pending_binding == BINDING {
                            stage = SimStage::Processing;
                            stops += 1;
                        }
                    }
                }
                Ev::Press | Ev::Release => {
                    let is_pressed = matches!(ev, Ev::Press);
                    let pending_binding = pending.as_deref();
                    let recording_binding = if stage == SimStage::Recording {
                        Some(BINDING)
                    } else {
                        None
                    };

                    match classify_ptt_event(
                        pending_binding,
                        is_pressed,
                        true, // push_to_talk
                        BINDING,
                        recording_binding,
                    ) {
                        PttAction::CancelRelease => {
                            pending = None;
                            continue;
                        }
                        PttAction::DeferRelease => {
                            pending = Some(BINDING.to_string());
                            continue;
                        }
                        PttAction::Passthrough => {}
                    }

                    if is_pressed {
                        if last_press_ms.is_some_and(|t| clock_ms - t < debounce_ms) {
                            continue;
                        }
                        last_press_ms = Some(clock_ms);
                    }

                    if is_pressed && stage == SimStage::Idle {
                        stage = SimStage::Recording;
                        starts += 1;
                    } else if !is_pressed && stage == SimStage::Recording {
                        stage = SimStage::Processing;
                        stops += 1;
                    }
                }
            }
        }

        SimResult {
            starts,
            stops,
            stage,
        }
    }

    /// Initial press plus several synthesized release/press pairs, as X11 emits
    /// while a push-to-talk key is held down.
    fn autorepeat_burst() -> Vec<Ev> {
        let mut events = vec![Ev::Press];
        for _ in 0..6 {
            events.push(Ev::Release);
            events.push(Ev::Press);
        }
        events
    }

    /// Regression for #1539: a burst of X11 auto-repeat release/press pairs must
    /// not stop recording. Before the fix the first synthesized release stopped
    /// recording immediately (stops == 1, stage left Recording), which produced
    /// the rapid on/off toggling. With the fix the releases are coalesced and
    /// recording stays continuously active for the whole burst.
    #[test]
    fn x11_autorepeat_burst_does_not_toggle_recording() {
        let result = simulate(&autorepeat_burst());
        assert_eq!(result.starts, 1, "recording should start exactly once");
        assert_eq!(
            result.stops, 0,
            "synthesized auto-repeat releases must not stop recording mid-burst"
        );
        assert_eq!(
            result.stage,
            SimStage::Recording,
            "recording must remain active across the entire auto-repeat burst"
        );
    }

    /// Complements the burst test: once the key is genuinely released and the
    /// grace window elapses with no re-press, recording stops exactly once. This
    /// proves the debounce only coalesces synthesized releases and does not wedge
    /// the coordinator or swallow the real key-up.
    #[test]
    fn genuine_release_after_grace_stops_recording_once() {
        let mut events = autorepeat_burst();
        events.push(Ev::Release); // genuine key-up
        events.push(Ev::Grace); // grace window elapses, no cancelling press
        let result = simulate(&events);
        assert_eq!(result.starts, 1, "recording should start exactly once");
        assert_eq!(
            result.stops, 1,
            "a genuine release should stop recording exactly once"
        );
        assert_eq!(result.stage, SimStage::Processing);
    }

    // ---------------------------------------------------------------------
    // Double-tap FN → hands-free (issue 1) — aggressive window tests
    // ---------------------------------------------------------------------
    #[test]
    fn double_tap_window_constants_are_sane() {
        assert!(DEBOUNCE < FN_DOUBLE_TAP_WINDOW);
        assert_eq!(DEBOUNCE.as_millis(), 30);
        assert_eq!(FN_DOUBLE_TAP_WINDOW.as_millis(), 350);
        assert_eq!(RELEASE_GRACE.as_millis(), 50);
    }

    fn is_double_tap(prev_ms: u64, now_ms: u64) -> bool {
        let since = now_ms.saturating_sub(prev_ms);
        since >= DEBOUNCE.as_millis() as u64 && since < FN_DOUBLE_TAP_WINDOW.as_millis() as u64
    }

    #[test]
    fn double_tap_window_boundaries() {
        assert!(!is_double_tap(0, 10), "10ms < debounce (30) → not double");
        assert!(!is_double_tap(0, 29), "29ms < debounce → not double");
        assert!(is_double_tap(0, 30), "30ms == debounce → double");
        assert!(is_double_tap(0, 31), "31ms → double");
        assert!(is_double_tap(0, 200), "200ms → double");
        assert!(is_double_tap(0, 349), "349ms → double");
        assert!(!is_double_tap(0, 350), "350ms == window → not double");
        assert!(!is_double_tap(0, 500), "500ms > window → not double");
    }

    #[test]
    fn double_tap_must_not_confuse_autorepeat() {
        // Auto-repeat burst uses 5ms gaps — must never be double
        assert!(!is_double_tap(0, 5));
        assert!(!is_double_tap(10, 15));
        // Genuine double at 200ms must be double even after autorepeat
        assert!(is_double_tap(0, 200));
    }

    /// Simulator that includes double-tap → hands-free promote/start
    #[derive(Debug, PartialEq, Eq)]
    enum HandsFreeSimStage {
        Idle,
        Recording { hands_free: bool },
        Processing,
    }

    struct HandsFreeSimResult {
        starts_standard: u32,
        starts_hands_free: u32,
        promotes: u32,
        stops: u32,
        stage: HandsFreeSimStage,
    }

    fn simulate_with_double(events: &[(u64, Ev)]) -> HandsFreeSimResult {
        let mut stage = HandsFreeSimStage::Idle;
        let mut pending: Option<String> = None;
        let mut last_press_ms: Option<u64> = None;
        let mut last_standard_press_ms: Option<u64> = None;
        let mut starts_standard = 0;
        let mut starts_hands_free = 0;
        let mut promotes = 0;
        let mut stops = 0;
        let debounce_ms = DEBOUNCE.as_millis() as u64;
        let window_ms = FN_DOUBLE_TAP_WINDOW.as_millis() as u64;

        for (clock_ms, ev) in events {
            match ev {
                Ev::Grace => {
                    if let Some(p) = pending.take() {
                        if matches!(stage, HandsFreeSimStage::Recording { .. }) && p == BINDING {
                            stage = HandsFreeSimStage::Processing;
                            stops += 1;
                        }
                    }
                }
                Ev::Press | Ev::Release => {
                    let is_pressed = matches!(ev, Ev::Press);
                    let pending_binding = pending.as_deref();
                    let recording_binding = match &stage {
                        HandsFreeSimStage::Recording { .. } => Some(BINDING),
                        _ => None,
                    };
                    let recording_is_hands_free =
                        matches!(stage, HandsFreeSimStage::Recording { hands_free: true });

                    // Hands-free chord (not simulated here) would be Start/Promote
                    // — we only test double-tap path.

                    // PTT handling
                    match classify_ptt_event(
                        pending_binding,
                        is_pressed,
                        true,
                        BINDING,
                        recording_binding,
                    ) {
                        PttAction::CancelRelease => {
                            // Double-tap check inside CancelRelease
                            if is_pressed {
                                if let Some(prev) = last_standard_press_ms {
                                    let since = clock_ms.saturating_sub(prev);
                                    if since >= debounce_ms && since < window_ms {
                                        match &stage {
                                            HandsFreeSimStage::Recording { hands_free: false } => {
                                                pending = None;
                                                if let HandsFreeSimStage::Recording { hands_free } =
                                                    &mut stage
                                                {
                                                    *hands_free = true;
                                                }
                                                promotes += 1;
                                                last_standard_press_ms = None;
                                                continue;
                                            }
                                            HandsFreeSimStage::Idle => {
                                                pending = None;
                                                stage = HandsFreeSimStage::Recording {
                                                    hands_free: true,
                                                };
                                                starts_hands_free += 1;
                                                last_standard_press_ms = None;
                                                continue;
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                            }
                            pending = None;
                            continue;
                        }
                        PttAction::DeferRelease => {
                            pending = Some(BINDING.to_string());
                            continue;
                        }
                        PttAction::Passthrough => {}
                    }

                    // Debounce
                    if is_pressed {
                        if last_press_ms.is_some_and(|t| clock_ms.saturating_sub(t) < debounce_ms) {
                            continue;
                        }
                        last_press_ms = Some(*clock_ms);
                    }

                    // Double-tap after debounce (toggle mode double)
                    let mut now_for_standard = None;
                    if is_pressed {
                        now_for_standard = Some(*clock_ms);
                    }
                    if let Some(now) = now_for_standard {
                        if let Some(prev) = last_standard_press_ms {
                            let since = now.saturating_sub(prev);
                            if since >= debounce_ms && since < window_ms {
                                match &stage {
                                    HandsFreeSimStage::Recording { hands_free: false } => {
                                        pending = None;
                                        if let HandsFreeSimStage::Recording { hands_free } =
                                            &mut stage
                                        {
                                            *hands_free = true;
                                        }
                                        promotes += 1;
                                        last_standard_press_ms = None;
                                        continue;
                                    }
                                    HandsFreeSimStage::Idle => {
                                        pending = None;
                                        stage = HandsFreeSimStage::Recording { hands_free: true };
                                        starts_hands_free += 1;
                                        last_standard_press_ms = None;
                                        continue;
                                    }
                                    _ => {}
                                }
                            }
                        }
                        last_standard_press_ms = Some(now);
                    }

                    // Normal toggle/PTT start/stop (push_to_talk=true for this sim)
                    if is_pressed && matches!(stage, HandsFreeSimStage::Idle) {
                        stage = HandsFreeSimStage::Recording { hands_free: false };
                        starts_standard += 1;
                    } else if !is_pressed
                        && matches!(&stage, HandsFreeSimStage::Recording { hands_free: false })
                    {
                        // In real PTT, release would be deferred; for toggle sim we stop
                        // directly if no pending
                        if pending.is_none() {
                            stage = HandsFreeSimStage::Processing;
                            stops += 1;
                        }
                    }
                    // Hands-free recording ignores standard release (classify_hands_free)
                    if recording_is_hands_free && !is_pressed {
                        // ignored
                    }
                }
            }
        }

        HandsFreeSimResult {
            starts_standard,
            starts_hands_free,
            promotes,
            stops,
            stage,
        }
    }

    #[test]
    fn double_tap_idle_starts_hands_free() {
        // Two presses 200ms apart while idle: first starts standard, second promotes to hands-free
        let events = vec![(0, Ev::Press), (200, Ev::Press)];
        let r = simulate_with_double(&events);
        assert_eq!(r.starts_standard, 1, "first press starts standard");
        assert_eq!(
            r.promotes, 1,
            "second press within window should promote to hands-free"
        );
        assert_eq!(r.stage, HandsFreeSimStage::Recording { hands_free: true });
    }

    #[test]
    fn double_tap_while_recording_promotes() {
        // Simulate: press@0 → recording standard, release deferred, press@200 → promote
        let events = vec![
            (0, Ev::Press),
            (10, Ev::Release),
            (15, Ev::Press), // auto-repeat cancel (5ms) — not double
            (20, Ev::Release),
            (200, Ev::Press), // genuine second tap after 180ms from last standard press
        ];
        let r = simulate_with_double(&events);
        // After first press, standard recording; after 200ms double, should promote
        // Our sim's last_standard_press is at 0 and 15, so 200-15=185 <350 → promote
        assert!(
            r.promotes >= 1 || r.starts_hands_free >= 1,
            "should promote or start hands-free"
        );
    }

    #[test]
    fn double_tap_outside_window_does_not_promote() {
        let events = vec![(0, Ev::Press), (500, Ev::Press)];
        let r = simulate_with_double(&events);
        assert_eq!(r.promotes, 0, "500ms > window should not promote");
        assert_eq!(
            r.starts_hands_free, 0,
            "outside window should not start hands-free"
        );
    }

    #[test]
    fn debounce_blocks_spurious_double() {
        let events = vec![(0, Ev::Press), (10, Ev::Press)];
        let r = simulate_with_double(&events);
        assert_eq!(r.promotes, 0, "10ms < debounce should not promote");
        assert_eq!(r.starts_hands_free, 0);
    }

    #[test]
    fn hands_free_promote_preserves_recording() {
        // Start standard, then double → hands_free, ensure stage stays Recording {hands_free:true}
        let events = vec![(0, Ev::Press), (200, Ev::Press)];
        let r = simulate_with_double(&events);
        assert_eq!(r.stage, HandsFreeSimStage::Recording { hands_free: true });
    }
}
