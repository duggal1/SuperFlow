//! Snapshot capture entrypoint — ties detector, browser, and classify
//! together into the frozen [`ContextSnapshot`] contract.

use super::types::{now_millis, ContextSnapshot, Surface};

#[cfg(target_os = "macos")]
pub fn capture_snapshot() -> ContextSnapshot {
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
pub fn capture_snapshot() -> ContextSnapshot {
    ContextSnapshot::other("Unknown")
}

#[cfg(test)]
mod tests {
    #[test]
    fn capture_never_panics() {
        // Runs in CI on macOS runners and locally; detection may find the test
        // host process. Contract: always returns a snapshot.
        let snapshot = super::capture_snapshot();
        assert!(!snapshot.app_name.is_empty());
    }
}
