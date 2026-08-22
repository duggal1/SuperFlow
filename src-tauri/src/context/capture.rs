//! Snapshot capture entrypoint — ties detector, browser, and classify
//! together into the frozen [`ContextSnapshot`] contract.
//!
//! Threading: the macOS Accessibility client is bound to the main thread's
//! run loop; calling `AXUIElementCopyAttributeValue` from a background worker
//! traps (`_AXUIElementValidate` → EXC_BREAKPOINT). Every capture therefore
//! dispatches through [`capture_snapshot`], which hops to the main thread via
//! the app handle registered at startup and never blocks longer than
//! [`CAPTURE_TIMEOUT`] before degrading to a passive snapshot.

use std::sync::OnceLock;
use std::time::Duration;

use super::types::{now_millis, ContextSnapshot, Surface};

/// How long a non-main caller waits for the main-thread capture before giving
/// up. Generous: AX round-trips are single-digit milliseconds when healthy.
const CAPTURE_TIMEOUT: Duration = Duration::from_millis(800);

static MAIN_DISPATCHER: OnceLock<tauri::AppHandle> = OnceLock::new();

/// Register the handle used to hop to the main thread. Called once from
/// `initialize_core_logic`.
pub fn set_main_dispatcher(app: tauri::AppHandle) {
    let _ = MAIN_DISPATCHER.set(app);
}

#[cfg(target_os = "macos")]
extern "C" {
    /// Returns 1 when the calling thread is the process main thread.
    fn pthread_main_np() -> i32;
}

fn on_main_thread() -> bool {
    #[cfg(target_os = "macos")]
    unsafe {
        pthread_main_np() == 1
    }
    #[cfg(not(target_os = "macos"))]
    false
}

/// Safe entrypoint for every caller. On the main thread this runs inline;
/// anywhere else it marshals to main and waits briefly, degrading to a
/// passive snapshot on timeout or dispatcher loss.
pub fn capture_snapshot() -> ContextSnapshot {
    match MAIN_DISPATCHER.get() {
        // Already on main (or no dispatcher yet, e.g. unit tests): run direct.
        _ if on_main_thread() || MAIN_DISPATCHER.get().is_none() => capture_snapshot_impl(),
        Some(app) => {
            let (tx, rx) = std::sync::mpsc::sync_channel(1);
            let dispatched = app.run_on_main_thread(move || {
                let _ = tx.send(capture_snapshot_impl());
            });
            if dispatched.is_err() {
                return ContextSnapshot::other("Unknown");
            }
            rx.recv_timeout(CAPTURE_TIMEOUT)
                .unwrap_or_else(|_| ContextSnapshot::other("Unknown"))
        }
        None => ContextSnapshot::other("Unknown"),
    }
}

/// The actual OS-touching capture. MUST only run on the main thread.
fn capture_snapshot_impl() -> ContextSnapshot {
    #[cfg(target_os = "macos")]
    {
        use super::{browser, classify, detector, focused_text};

        let Some(app) = detector::frontmost_app() else {
            return ContextSnapshot::other("Unknown");
        };

        let bundle_id = app.bundle_id.clone();

        // Tab inspection only runs for supported browsers; native apps contribute
        // their bundle id (and developer surfaces their focused-element text).
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

        // Focused text only for developer surfaces (terminal buffer / editor
        // content), and never while macOS Secure Input is active — reading other
        // apps' text must not touch password fields.
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

        if surface == Surface::Other && !surface.is_aware_surface() {
            return ContextSnapshot {
                surface,
                app_name: app.name,
                bundle_id,
                url: tab.and_then(|t| t.url),
                title: None,
                focused_text: None,
                captured_at_ms: now_millis(),
            };
        }

        ContextSnapshot {
            surface,
            app_name: app.name,
            bundle_id,
            url: tab.as_ref().and_then(|t| t.url.clone()),
            title: tab.and_then(|t| t.title),
            focused_text,
            captured_at_ms: now_millis(),
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        ContextSnapshot::other("Unknown")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_never_panics_without_dispatcher() {
        // No dispatcher registered in tests → direct path, still safe.
        let snapshot = super::capture_snapshot();
        assert!(!snapshot.app_name.is_empty());
    }
}
