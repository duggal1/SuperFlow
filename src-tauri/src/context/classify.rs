//! Pure surface classification — no OS calls, fully unit-testable.
//!
//! Classification precedence: explicit URL host > native bundle id >
//! conservative title markers. Anything unproven stays [`Surface::Other`];
//! false positives would route dictation into awareness mode wrongly, while
//! misses merely skip it.

use super::types::Surface;

/// Native Slack.app bundle identifier (macOS).
pub const SLACK_BUNDLE_ID: &str = "com.tinyspeck.slackmacgap";

/// Ghostty terminal bundle identifier (macOS).
pub const GHOSTTY_BUNDLE_ID: &str = "com.mitchellh.ghostty";

/// Apple Terminal bundle identifier (macOS).
pub const APPLE_TERMINAL_BUNDLE_ID: &str = "com.apple.Terminal";

/// Terminal bundle identifiers classified as [`Surface::Terminal`]. Kept as a
/// superset with `file_refs::TERMINAL_BUNDLE_IDS`: a terminal that classifies
/// as Other would bail out of smart file references before its own bundle
/// list is ever consulted.
pub const TERMINAL_BUNDLE_IDS: &[&str] = &[
    GHOSTTY_BUNDLE_ID,
    APPLE_TERMINAL_BUNDLE_ID,
    "com.googlecode.iterm2",
    "dev.warp.Warp-Stable",
    "net.kovidgoyal.kitty",
    "io.alacritty",
    "com.github.wez.wezterm",
    "org.wezterm",
];

/// Bundle-id prefixes of code editors whose focused text we can inspect.
pub const EDITOR_BUNDLE_PREFIXES: &[&str] = &[
    "com.microsoft.VSCode",
    "com.vscodium.codium",
    "com.todesktop.230313mzl4w4u92", // Cursor
];

/// Classify a native (non-browser) bundle id into a developer surface.
pub fn classify_native_bundle(bundle_id: Option<&str>) -> Option<Surface> {
    let bundle = bundle_id?;
    if TERMINAL_BUNDLE_IDS.contains(&bundle) {
        return Some(Surface::Terminal);
    }
    if EDITOR_BUNDLE_PREFIXES
        .iter()
        .any(|prefix| bundle.starts_with(prefix))
    {
        return Some(Surface::Editor);
    }
    None
}

/// Bundle-id prefixes of browsers whose tabs we can inspect.
pub const CHROMIUM_BUNDLE_PREFIXES: &[&str] = &[
    "com.google.Chrome",
    "company.thebrowser.Browser", // Arc
    "com.microsoft.edgemac",
    "com.brave.Browser",
];

pub const SAFARI_BUNDLE_ID: &str = "com.apple.Safari";

pub fn is_known_browser(bundle_id: Option<&str>) -> bool {
    let Some(bundle) = bundle_id else {
        return false;
    };
    bundle == SAFARI_BUNDLE_ID
        || CHROMIUM_BUNDLE_PREFIXES
            .iter()
            .any(|prefix| bundle.starts_with(prefix))
}

/// Extract the host from a URL string without pulling in a URL crate.
/// Returns lowercase host without port, or `None` when absent.
pub fn url_host(url: &str) -> Option<String> {
    let after_scheme = url.split_once("://")?.1;
    let authority = after_scheme.split(['/', '?', '#']).next()?;
    let host = authority.rsplit_once(':').map_or(authority, |(h, _)| h);
    let host = host.trim().to_ascii_lowercase();
    (!host.is_empty()).then_some(host)
}

fn host_is(host: &str, domains: &[&str]) -> bool {
    domains
        .iter()
        .any(|domain| host == *domain || host.ends_with(&format!(".{domain}")))
}

pub fn classify(bundle_id: Option<&str>, url: Option<&str>, title: Option<&str>) -> Surface {
    if let Some(url) = url {
        if let Some(host) = url_host(url) {
            if host_is(&host, &["gmail.com", "mail.google.com"]) {
                return Surface::Gmail;
            }
            if host_is(&host, &["slack.com"]) {
                return Surface::Slack;
            }
        }
    }

    if bundle_id == Some(SLACK_BUNDLE_ID) {
        return Surface::Slack;
    }

    // Developer terminals and editors are recognized by bundle id alone —
    // their window titles are arbitrary shell/editor state.
    if let Some(native) = classify_native_bundle(bundle_id) {
        return native;
    }

    // Conservative title markers for cases where the URL was unreadable but
    // the browser/webview still exposes a recognizable page title.
    if let Some(title) = title {
        let trimmed = title.trim_end();
        if trimmed.ends_with("- Gmail") || trimmed.ends_with("– Gmail") {
            return Surface::Gmail;
        }
        if trimmed.ends_with(" | Slack") || trimmed.ends_with(" - Slack") {
            return Surface::Slack;
        }
    }

    Surface::Other
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gmail_urls_classify() {
        assert_eq!(
            classify(None, Some("https://mail.google.com/mail/u/0/#inbox"), None),
            Surface::Gmail
        );
        assert_eq!(
            classify(None, Some("https://gmail.com/"), None),
            Surface::Gmail
        );
    }

    #[test]
    fn slack_urls_and_native_bundle_classify() {
        assert_eq!(
            classify(None, Some("https://app.slack.com/client/T01/C02"), None),
            Surface::Slack
        );
        assert_eq!(
            classify(Some(SLACK_BUNDLE_ID), None, Some("#general")),
            Surface::Slack
        );
    }

    #[test]
    fn title_fallback_when_url_missing() {
        assert_eq!(
            classify(
                Some("com.google.Chrome"),
                None,
                Some("Inbox (2) - me@gmail.com - Gmail")
            ),
            Surface::Gmail
        );
        assert_eq!(
            classify(
                Some("com.google.Chrome"),
                None,
                Some("Design | My Team - Slack")
            ),
            Surface::Slack
        );
    }

    #[test]
    fn lookalikes_stay_other() {
        assert_eq!(
            classify(None, Some("https://evil-gmail.example.com/"), None),
            Surface::Other
        );
        assert_eq!(
            classify(None, None, Some("reading about Gmail tips")),
            Surface::Other
        );
    }

    #[test]
    fn host_extraction_handles_ports_and_paths() {
        assert_eq!(
            url_host("https://mail.google.com:443/mail/u/0").as_deref(),
            Some("mail.google.com")
        );
        assert_eq!(
            url_host("http://SLACK.com/client").as_deref(),
            Some("slack.com")
        );
        assert_eq!(url_host("not-a-url"), None);
    }

    #[test]
    fn browser_detection() {
        assert!(is_known_browser(Some("com.google.Chrome")));
        assert!(is_known_browser(Some("com.google.Chrome.canary")));
        assert!(is_known_browser(Some("company.thebrowser.Browser")));
        assert!(is_known_browser(Some(SAFARI_BUNDLE_ID)));
        assert!(!is_known_browser(Some(SLACK_BUNDLE_ID)));
        assert!(!is_known_browser(None));
    }

    #[test]
    fn developer_bundles_classify() {
        assert_eq!(
            classify_native_bundle(Some("com.mitchellh.ghostty")),
            Some(Surface::Terminal)
        );
        assert_eq!(
            classify_native_bundle(Some("com.apple.Terminal")),
            Some(Surface::Terminal)
        );
        assert_eq!(
            classify_native_bundle(Some("com.googlecode.iterm2")),
            Some(Surface::Terminal)
        );
        assert_eq!(
            classify_native_bundle(Some("dev.warp.Warp-Stable")),
            Some(Surface::Terminal)
        );
        assert_eq!(
            classify_native_bundle(Some("com.microsoft.VSCode")),
            Some(Surface::Editor)
        );
        assert_eq!(
            classify_native_bundle(Some("com.microsoft.VSCodeInsiders")),
            Some(Surface::Editor)
        );
        assert_eq!(
            classify_native_bundle(Some("com.vscodium.codium")),
            Some(Surface::Editor)
        );
        assert_eq!(
            classify(Some("com.mitchellh.ghostty"), None, None),
            Surface::Terminal
        );
        assert_eq!(
            classify(Some("com.microsoft.VSCode"), None, None),
            Surface::Editor
        );
        assert_eq!(classify_native_bundle(Some(SLACK_BUNDLE_ID)), None);
        assert_eq!(classify_native_bundle(None), None);
    }
}
