//! Health state tracking for session liveness and mass death detection.
//!
//! Extracted from Gas Town's decon.md (AgentHealthState) and supervise.md
//! (mass death detection, zombie debounce) patterns.
//!
//! Capabilities:
//! - Per-session consecutive failure tracking with cooldown
//! - Zombie consecutive count debounce (don't kill on first detection)
//! - Sliding window mass death detection

// Real implementations — some methods not yet called from orchestrator.
#![allow(dead_code)]

use std::collections::VecDeque;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerLivenessState {
    Assigned,
    PromptDelivered,
    Booting,
    AliveUnconfirmed,
    AliveConfirmed,
    Executing,
    Reporting,
    Blocked,
    Stalled,
    Nonresponsive,
    Failed,
    Done,
}

impl WorkerLivenessState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Assigned => "assigned",
            Self::PromptDelivered => "prompt_delivered",
            Self::Booting => "booting",
            Self::AliveUnconfirmed => "alive_unconfirmed",
            Self::AliveConfirmed => "alive_confirmed",
            Self::Executing => "executing",
            Self::Reporting => "reporting",
            Self::Blocked => "blocked",
            Self::Stalled => "stalled",
            Self::Nonresponsive => "nonresponsive",
            Self::Failed => "failed",
            Self::Done => "done",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncidentScope {
    None,
    Local,
    Systemic,
}

impl IncidentScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Local => "local",
            Self::Systemic => "systemic",
        }
    }
}

pub fn first_status_systemic_threshold(worker_count: usize) -> usize {
    match worker_count {
        0 | 1 => worker_count,
        2 | 3 => 2,
        count => count.div_ceil(2),
    }
}

pub fn is_systemic_first_status_incident(worker_count: usize, overdue_workers: usize) -> bool {
    worker_count >= 2
        && overdue_workers >= first_status_systemic_threshold(worker_count)
        && overdue_workers * 2 >= worker_count
}

// ─── Health Check State (from Gas Town decon.md:182-310) ──────────────────

/// Tracks per-session health check outcomes.
/// Distinct from stall detection — this tracks explicit health probe results.
#[derive(Debug, Clone)]
pub struct SessionHealthState {
    /// When we last sent a health probe (nudge/ping)
    pub last_probe_at: Option<Instant>,
    /// When the session last responded to a health probe
    pub last_response_at: Option<Instant>,
    /// Consecutive health probe failures (reset on any response)
    pub consecutive_probe_failures: usize,
    /// Total force interventions (stall escalations, force-fails)
    pub intervention_count: usize,
    /// When the last intervention occurred
    pub last_intervention_at: Option<Instant>,
    /// Cooldown until which no new interventions should be sent
    pub cooldown_until: Option<Instant>,
}

impl SessionHealthState {
    pub fn new() -> Self {
        Self {
            last_probe_at: None,
            last_response_at: None,
            consecutive_probe_failures: 0,
            intervention_count: 0,
            last_intervention_at: None,
            cooldown_until: None,
        }
    }

    /// Record that a health probe was sent.
    pub fn record_probe(&mut self) {
        self.last_probe_at = Some(Instant::now());
    }

    /// Record that the session responded to a health probe.
    /// Resets the consecutive failure counter.
    pub fn record_response(&mut self) {
        self.last_response_at = Some(Instant::now());
        self.consecutive_probe_failures = 0;
    }

    /// Record that a health probe failed (no response).
    pub fn record_failure(&mut self) {
        self.consecutive_probe_failures += 1;
    }

    /// Record a watchdog intervention (stall prompt, redirect, etc.).
    pub fn record_intervention(&mut self, _intervention_type: &str) {
        self.intervention_count += 1;
        self.last_intervention_at = Some(Instant::now());
        // Cooldown: 30s × count, capped at 120s
        let cooldown_secs = (30 * self.intervention_count).min(120) as u64;
        self.cooldown_until = Some(Instant::now() + Duration::from_secs(cooldown_secs));
    }

    /// Returns true if the session is currently in cooldown.
    pub fn is_in_cooldown(&self) -> bool {
        self.cooldown_until
            .is_some_and(|until| Instant::now() < until)
    }

    /// Clear cooldown — called when the session produces output after intervention.
    pub fn clear_cooldown(&mut self) {
        self.cooldown_until = None;
    }

    /// Returns true if consecutive failures exceed the threshold.
    pub fn should_escalate(&self, threshold: usize) -> bool {
        self.consecutive_probe_failures >= threshold
    }
}

// ─── Zombie Debounce (from Gas Town supervise.md:1568-1600) ─────────────

/// Tracks consecutive zombie detections to debounce transient gaps.
/// Gas Town pattern: don't kill on first zombie detection — wait 3 cycles.
#[derive(Debug, Clone)]
pub struct ZombieDebounce {
    /// Consecutive cycles where the session appeared as a zombie
    pub consecutive_zombie_count: usize,
    /// Threshold before triggering zombie restart (default: 3)
    pub threshold: usize,
}

impl ZombieDebounce {
    pub fn new(threshold: usize) -> Self {
        Self {
            consecutive_zombie_count: 0,
            threshold,
        }
    }

    /// Call when zombie is detected. Returns true if threshold exceeded.
    pub fn record_zombie(&mut self) -> bool {
        self.consecutive_zombie_count += 1;
        self.consecutive_zombie_count >= self.threshold
    }

    /// Call when session is confirmed alive. Resets the counter.
    pub fn record_alive(&mut self) {
        self.consecutive_zombie_count = 0;
    }

    /// Returns true if we should trigger zombie restart.
    pub fn should_restart(&self) -> bool {
        self.consecutive_zombie_count >= self.threshold
    }
}

impl Default for ZombieDebounce {
    fn default() -> Self {
        Self::new(3)
    }
}

// ─── Mass Death Detection (from Gas Town supervise.md:2389-2436) ────────

/// Session death record for sliding window tracking.
#[derive(Debug, Clone)]
struct SessionDeath {
    session_name: String,
    timestamp: Instant,
}

/// Detects mass session deaths within a sliding time window.
/// Gas Town pattern: if N sessions die within M seconds, emit critical event.
#[derive(Debug)]
pub struct MassDeathDetector {
    /// Recent session deaths in chronological order
    recent_deaths: VecDeque<SessionDeath>,
    /// Time window for detection (default: 30s)
    window: Duration,
    /// Number of deaths to trigger mass death event (default: 3)
    threshold: usize,
    /// Whether a mass death event has already been emitted for the current window
    last_emitted_at: Option<Instant>,
    /// Minimum time between mass death events (prevents spam)
    emit_cooldown: Duration,
}

impl MassDeathDetector {
    pub fn new(window: Duration, threshold: usize) -> Self {
        Self {
            recent_deaths: VecDeque::new(),
            window,
            threshold,
            last_emitted_at: None,
            emit_cooldown: Duration::from_secs(60),
        }
    }

    /// Record a session death. Returns true if mass death threshold exceeded.
    pub fn record_death(&mut self, session_name: &str) -> Option<MassDeathEvent> {
        let now = Instant::now();
        self.recent_deaths.push_back(SessionDeath {
            session_name: session_name.to_owned(),
            timestamp: now,
        });

        // Prune deaths outside the window
        let cutoff = now.checked_sub(self.window)?;
        while self
            .recent_deaths
            .front()
            .is_some_and(|d| d.timestamp < cutoff)
        {
            self.recent_deaths.pop_front();
        }

        // Check threshold
        if self.recent_deaths.len() >= self.threshold {
            // Check emit cooldown
            if self
                .last_emitted_at
                .is_some_and(|last| now.duration_since(last) < self.emit_cooldown)
            {
                return None;
            }

            let names: Vec<&str> = self
                .recent_deaths
                .iter()
                .map(|d| d.session_name.as_str())
                .collect();
            self.last_emitted_at = Some(now);

            Some(MassDeathEvent {
                dead_sessions: names.into_iter().map(String::from).collect(),
                count: self.recent_deaths.len(),
                window: self.window,
            })
        } else {
            None
        }
    }

    /// Reset the detector (e.g., after handling a mass death event).
    pub fn reset(&mut self) {
        self.recent_deaths.clear();
        self.last_emitted_at = None;
    }

    /// Current death count within the window.
    pub fn death_count(&self) -> usize {
        self.recent_deaths.len()
    }
}

impl Default for MassDeathDetector {
    fn default() -> Self {
        Self::new(Duration::from_secs(30), 3)
    }
}

/// Mass death event — emitted when N sessions die within M seconds.
#[derive(Debug)]
pub struct MassDeathEvent {
    pub dead_sessions: Vec<String>,
    pub count: usize,
    pub window: Duration,
}

// ─── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_state_resets_failures_on_response() {
        let mut state = SessionHealthState::new();
        state.record_failure();
        state.record_failure();
        assert_eq!(state.consecutive_probe_failures, 2);
        state.record_response();
        assert_eq!(state.consecutive_probe_failures, 0);
    }

    #[test]
    fn health_state_cooldown_prevents_rapid_interventions() {
        let mut state = SessionHealthState::new();
        state.record_intervention("stall_prompt");
        assert!(state.is_in_cooldown());
    }

    #[test]
    fn zombie_debounce_requires_three_consecutive() {
        let mut debounce = ZombieDebounce::new(3);
        assert!(!debounce.record_zombie()); // 1
        assert!(!debounce.record_zombie()); // 2
        assert!(debounce.record_zombie()); // 3 → threshold exceeded
        debounce.record_alive();
        assert!(!debounce.should_restart());
    }

    #[test]
    fn mass_death_detector_fires_at_threshold() {
        let mut detector = MassDeathDetector::new(Duration::from_secs(30), 3);
        detector.record_death("session-1");
        detector.record_death("session-2");
        assert!(detector.record_death("session-3").is_some());
    }

    #[test]
    fn mass_death_detector_respects_cooldown() {
        let mut detector = MassDeathDetector::new(Duration::from_secs(30), 3);
        detector.record_death("session-1");
        detector.record_death("session-2");
        assert!(detector.record_death("session-3").is_some());
        // Immediate second event should be suppressed by cooldown
        detector.record_death("session-4");
        assert!(detector.record_death("session-5").is_none());
    }
}
