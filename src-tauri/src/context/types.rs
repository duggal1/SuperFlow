//! Shared context contracts for Intelligence Awareness.
//!
//! This module is the frozen boundary between two parallel work lanes:
//! Lane A (context detection engine) produces [`ContextSnapshot`] values;
//! Lane B (intelligence router, settings UI, send adapters) consumes them.
//! Both lanes compile against these types alone — neither needs the other's
//! internals. Do not add fields casually; every addition is a cross-lane
//! contract change.

use serde::{Deserialize, Serialize};

/// The product surface the user is currently working in, classified from the
/// frontmost app / browser URL at recording start.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Surface {
    /// gmail.com in any supported browser.
    Gmail,
    /// slack.com in a browser, or the native Slack.app.
    Slack,
    /// A developer terminal: Ghostty or Apple Terminal.
    Terminal,
    /// A code editor: VS Code (including Insiders builds).
    Editor,
    /// Everything else — awareness stays passive.
    Other,
}

impl Surface {
    pub fn as_str(&self) -> &'static str {
        match self {
            Surface::Gmail => "gmail",
            Surface::Slack => "slack",
            Surface::Terminal => "terminal",
            Surface::Editor => "editor",
            Surface::Other => "other",
        }
    }

    pub fn is_aware_surface(&self) -> bool {
        matches!(
            self,
            Surface::Gmail | Surface::Slack | Surface::Terminal | Surface::Editor
        )
    }

    /// Whether the surface should be treated as Gmail for voice-command
    /// routing. A named predicate so Gmail-like hosts can be added here
    /// without touching call sites.
    pub fn is_gmail_like(&self) -> bool {
        matches!(self, Surface::Gmail)
    }
}

/// Immutable snapshot of "where the user is" captured the moment a recording
/// starts. Detection failures degrade fields to `None` — a snapshot is always
/// constructible and never blocks dictation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextSnapshot {
    pub surface: Surface,
    /// Frontmost application display name (e.g. "Google Chrome", "Slack").
    pub app_name: String,
    /// Frontmost application bundle identifier when resolvable.
    pub bundle_id: Option<String>,
    /// Browser tab URL when the frontmost app is a supported browser.
    pub url: Option<String>,
    /// Window or tab title (page `<title>` for browsers).
    pub title: Option<String>,
    /// Visible text of the focused element for surfaces that expose it
    /// (terminal buffer, editor content). Capped; `None` when unreadable,
    /// unsupported, or macOS Secure Input is active.
    pub focused_text: Option<String>,
    /// Wall-clock capture time (unix millis).
    pub captured_at_ms: u64,
}

impl ContextSnapshot {
    /// A passive snapshot for surfaces where detection found nothing useful.
    pub fn other(app_name: impl Into<String>) -> Self {
        Self {
            surface: Surface::Other,
            app_name: app_name.into(),
            bundle_id: None,
            url: None,
            title: None,
            focused_text: None,
            captured_at_ms: now_millis(),
        }
    }

    /// Whether this snapshot's surface participates in Intelligence Awareness.
    pub fn is_aware_surface(&self) -> bool {
        self.surface.is_aware_surface()
    }
}

pub fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_always_constructible() {
        let s = ContextSnapshot::other("Finder");
        assert_eq!(s.surface, Surface::Other);
        assert!(!s.is_aware_surface());
        assert!(s.url.is_none());
    }

    #[test]
    fn aware_surfaces_classify() {
        assert!(Surface::Gmail.is_aware_surface());
        assert!(Surface::Slack.is_aware_surface());
        assert!(Surface::Terminal.is_aware_surface());
        assert!(Surface::Editor.is_aware_surface());
        assert!(!Surface::Other.is_aware_surface());
    }

    #[test]
    fn serializes_snake_case() {
        let json = serde_json::to_string(&Surface::Gmail).unwrap();
        assert_eq!(json, r#""gmail""#);
        let terminal = serde_json::to_string(&Surface::Terminal).unwrap();
        assert_eq!(terminal, r#""terminal""#);
    }
}
