//! Snapshot capture — process-isolated.
//!
//! The main app NEVER executes Accessibility code. [`capture_snapshot`]
//! spawns this same binary with `--context-agent`; the child performs one
//! capture on its own main thread, prints a JSON [`ContextSnapshot`], and
//! exits. If the child faults (AX traps are uncatchable SIGTRAPs), hangs, or
//! times out, only the subprocess dies: the supervisor degrades to a passive
//! snapshot and dictation continues untouched.
//!
//! A circuit breaker stops respawning after repeated consecutive failures so
//! a broken system state cannot turn into a fork storm; it re-arms after a
//! cool-down.

use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::Duration;

use super::types::{now_millis, ContextSnapshot, Surface};
use super::RecordingContext;
use serde::{Deserialize, Serialize};

/// Hard ceiling for one helper round-trip. AX captures are single-digit ms
/// when healthy; this exists for pathological cases only.
const CAPTURE_TIMEOUT: Duration = Duration::from_millis(900);

/// Consecutive failures before the breaker opens.
const BREAKER_THRESHOLD: u32 = 4;

/// How long an open breaker stays open before probing again.
const BREAKER_COOLDOWN_MS: u64 = 60_000;

static CONSECUTIVE_FAILURES: AtomicU32 = AtomicU32::new(0);
static BREAKER_OPEN_UNTIL_MS: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SelectionSnapshot {
    pub pid: i32,
    pub app_name: String,
    pub text: String,
}

pub fn capture_recording_context(
    resolve_project: bool,
    capture_developer_context: bool,
) -> RecordingContext {
    let snapshot = capture_snapshot();
    let project_root = resolve_project
        .then(|| crate::file_refs::project_root_for_snapshot(&snapshot))
        .flatten();
    let developer = capture_developer_context
        .then(|| {
            project_root
                .as_deref()
                .map(super::developer::DeveloperContext::capture)
        })
        .flatten();
    RecordingContext {
        snapshot,
        project_root,
        developer,
    }
}

fn breaker_open() -> bool {
    now_millis() < BREAKER_OPEN_UNTIL_MS.load(Ordering::Relaxed)
}

fn record_success() {
    CONSECUTIVE_FAILURES.store(0, Ordering::Relaxed);
}

fn record_failure() {
    let failures = CONSECUTIVE_FAILURES.fetch_add(1, Ordering::Relaxed) + 1;
    if failures >= BREAKER_THRESHOLD {
        BREAKER_OPEN_UNTIL_MS.store(now_millis() + BREAKER_COOLDOWN_MS, Ordering::Relaxed);
        CONSECUTIVE_FAILURES.store(0, Ordering::Relaxed);
    }
}

#[cfg(target_os = "macos")]
pub fn capture_snapshot() -> ContextSnapshot {
    // Unit tests have no real executable context to spawn; degrade quietly.
    if cfg!(test) || breaker_open() {
        return ContextSnapshot::other("Unknown");
    }

    let Ok(exe) = std::env::current_exe() else {
        record_failure();
        return ContextSnapshot::other("Unknown");
    };

    let Ok(mut child) = Command::new(exe)
        .arg("--context-agent")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .stdin(Stdio::null())
        .spawn()
    else {
        record_failure();
        return ContextSnapshot::other("Unknown");
    };

    let mut stdout = child.stdout.take().expect("stdout was piped");
    let reader = std::thread::spawn(move || {
        let mut out = String::new();
        let _ = stdout.read_to_string(&mut out);
        out
    });

    let finished = match wait_with_timeout(&mut child, CAPTURE_TIMEOUT) {
        Ok(()) => true,
        Err(()) => {
            let _ = child.kill();
            let _ = child.wait();
            false
        }
    };

    if !finished {
        record_failure();
        return ContextSnapshot::other("Unknown");
    }

    let output = match reader.join() {
        Ok(out) => out,
        Err(_) => {
            record_failure();
            return ContextSnapshot::other("Unknown");
        }
    };

    // The agent prints exactly one JSON line on stdout.
    let json = output.lines().last().unwrap_or("");
    match serde_json::from_str::<ContextSnapshot>(json) {
        Ok(snapshot) => {
            record_success();
            snapshot
        }
        Err(_) => {
            record_failure();
            ContextSnapshot::other("Unknown")
        }
    }
}

#[cfg(target_os = "macos")]
pub fn capture_selected_text() -> Option<SelectionSnapshot> {
    if cfg!(test) || crate::secure_input::is_enabled_now() {
        return None;
    }
    // Pin the target before spawning the helper. Re-detecting the frontmost
    // app inside the subprocess can resolve the invoking terminal or
    // SuperFlow instead of the browser that owns the selection.
    let target = super::detector::frontmost_app()?;
    let exe = std::env::current_exe().ok()?;
    let mut child = Command::new(exe)
        .arg("--selected-text-agent")
        .arg("--selected-text-pid")
        .arg(target.pid.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .stdin(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdout = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut out = String::new();
        let _ = stdout.read_to_string(&mut out);
        out
    });
    if wait_with_timeout(&mut child, CAPTURE_TIMEOUT).is_err() {
        let _ = child.kill();
        let _ = child.wait();
        return None;
    }
    let output = reader.join().ok()?;
    let text: Option<String> = serde_json::from_str(output.lines().last().unwrap_or("")).ok()?;
    text.map(|text| SelectionSnapshot {
        pid: target.pid,
        app_name: target.name,
        text,
    })
}

#[cfg(not(target_os = "macos"))]
pub fn capture_selected_text() -> Option<SelectionSnapshot> {
    None
}

#[cfg(target_os = "macos")]
fn wait_with_timeout(child: &mut std::process::Child, timeout: Duration) -> Result<(), ()> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return Ok(()),
            Ok(None) => {}
            Err(_) => return Err(()),
        }
        if std::time::Instant::now() >= deadline {
            return Err(());
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[cfg(not(target_os = "macos"))]
pub fn capture_snapshot() -> ContextSnapshot {
    ContextSnapshot::other("Unknown")
}

/// Entry point of the `--context-agent` subprocess. Runs on the child's main
/// thread by construction (single-threaded bare process), which is also where
/// the Accessibility client belongs. Prints exactly one JSON line and exits.
///
/// Deliberately minimal: no logging targets are initialized here, so stdout
/// carries nothing but the JSON payload.
#[cfg(target_os = "macos")]
pub fn run_context_agent() {
    use super::{browser, classify, detector, focused_text};

    let snapshot = detector::frontmost_app().map_or_else(
        || ContextSnapshot::other("Unknown"),
        |app| {
            let bundle_id = app.bundle_id.clone();

            let tab = if classify::is_known_browser(bundle_id.as_deref()) {
                browser::frontmost_tab(bundle_id.as_deref(), app.pid)
            } else {
                None
            };

            let surface = classify::classify(
                bundle_id.as_deref(),
                tab.as_ref().and_then(|t| t.url.as_deref()),
                tab.as_ref().and_then(|t| t.title.as_deref()),
            );

            let focused_text = match surface {
                Surface::Terminal | Surface::Editor => {
                    if crate::secure_input::is_enabled_now() {
                        None
                    } else {
                        focused_text::focused_element_text(app.pid)
                    }
                }
                _ => None,
            };

            ContextSnapshot {
                surface,
                app_name: app.name,
                bundle_id,
                url: tab.as_ref().and_then(|t| t.url.clone()),
                title: tab.and_then(|t| t.title),
                focused_text,
                captured_at_ms: now_millis(),
            }
        },
    );

    if let Ok(json) = serde_json::to_string(&snapshot) {
        println!("{json}");
    }
}

#[cfg(target_os = "macos")]
pub fn run_selected_text_agent(pid: Option<i32>) {
    let text = pid.and_then(super::focused_text::selected_text);
    if let Ok(json) = serde_json::to_string(&text) {
        println!("{json}");
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn capture_degrades_without_spawning_in_tests() {
        // In test builds the supervisor short-circuits to the passive
        // snapshot — the contract callers rely on (never panics, never
        // blocks) still holds.
        let snapshot = super::capture_snapshot();
        assert!(!snapshot.app_name.is_empty());
    }
}
