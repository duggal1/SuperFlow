//! Universal, deterministic "send it" hook (Aqua Voice style).
//!
//! The broken previous approach relied on Gemini tool-calling or a heavy
//! Gmail-only accessibility pipeline to actually send. This is the dead-simple
//! replacement:
//!
//! 1. Detect a trailing *send* command in the raw transcript **before** any
//!    LLM post-processing sees it (so the model never gets confused by the
//!    literal words "send it").
//! 2. Strip the command text out of the message.
//! 3. After the cleaned text is pasted into the focused app, press the
//!    app-appropriate send key (Return, or Cmd/Ctrl+Return).
//!
//! Because the app already inserts text at the system cursor, this works in
//! Gmail, Slack, Outlook, and any other text field — no per-app integration.

use tauri::Manager;
/// Which keystroke should be synthesized to submit the message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendKey {
    /// Plain Return/Enter (Slack, and the safe default).
    Enter,
    /// Cmd+Return — Gmail and Outlook on macOS.
    CommandEnter,
    /// Ctrl+Return — Gmail/Outlook on Windows/Linux.
    ControlEnter,
}

/// A resolved "send it" instruction: the cleaned message plus the key to press.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendItPlan {
    pub cleaned: String,
    pub key: SendKey,
}

/// Map a surface name to the key that submits a message in that app.
pub fn send_key_for_surface(surface: &str) -> SendKey {
    match surface.to_ascii_lowercase().as_str() {
        "gmail" | "outlook" => SendKey::CommandEnter,
        "slack" => SendKey::Enter,
        _ => SendKey::Enter,
    }
}

/// Remove trailing whitespace, sentence punctuation, and an optional trailing
/// "please" so phrase matching is stable across spoken variants.
fn trim_trailing_command_punct(s: &str) -> &str {
    let s = s.trim_end_matches(|c: char| {
        c.is_whitespace() || matches!(c, '.' | ',' | ':' | ';' | '!' | '?')
    });
    let s = s.trim_end_matches(" please").trim_end();
    s.trim_end_matches(|c: char| {
        c.is_whitespace() || matches!(c, '.' | ',' | ':' | ';' | '!' | '?')
    })
}

/// Candidate send phrases, longest first so "send it now" wins over "send it"
/// and a bare "send" is only matched as a last resort.
const SEND_PHRASES: &[&str] = &[
    "send it now",
    "send this email",
    "send this",
    "send it",
    "send",
];

/// Detect a terminal "send it" intent at the end of `text`.
///
/// Returns `None` unless the phrase is a genuine send command (a sentence
/// boundary or a trailing " and"/" or" precedes it) so content like
/// "I will send it tomorrow" never triggers. When present, the command phrase
/// is stripped and the cleaned message is returned alongside the key to press
/// for the given `surface`.
pub fn detect_send_it(text: &str, surface: &str) -> Option<SendItPlan> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    let core = trim_trailing_command_punct(trimmed);
    let core_lower = core.to_lowercase();

    let phrase = SEND_PHRASES
        .iter()
        .find(|phrase| {
            let pl = phrase.len();
            core_lower.len() >= pl && core_lower.ends_with(*phrase)
        })
        .copied()?;

    // The phrase is ASCII, so its length in bytes equals its char length.
    let boundary = core.len() - phrase.len();
    let before = core[..boundary].trim_end();
    if before.is_empty() {
        // Just the command, no message — nothing to send.
        return None;
    }

    let before_lower = before.to_lowercase();
    // False-positive guards: the phrase must be a command, not content.
    if before_lower.contains("send it tomorrow")
        || before_lower.contains("send this tomorrow")
        || before_lower.ends_with("will send")
        || before_lower.ends_with("to send")
        || before_lower.ends_with("let's send")
        || before_lower.ends_with("lets send")
    {
        return None;
    }

    // The phrase must follow a sentence boundary or a conjunction.
    let last = before.chars().last();
    let ends_boundary = matches!(last, Some('.') | Some('!') | Some('?') | Some(';'));
    let ends_and_or = before_lower.ends_with(" and") || before_lower.ends_with(" or");
    if !ends_boundary && !ends_and_or {
        return None;
    }

    let cleaned = {
        let trimmed = before.trim_end();
        let lower = trimmed.to_lowercase();
        let stripped = if lower.ends_with(" and") {
            &trimmed[..trimmed.len() - 4]
        } else if lower.ends_with(" or") {
            &trimmed[..trimmed.len() - 3]
        } else {
            trimmed
        };
        stripped.trim().to_string()
    };

    if cleaned.is_empty() {
        return None;
    }

    Some(SendItPlan {
        cleaned,
        key: send_key_for_surface(surface),
    })
}

/// Synthesize the send keystroke via native Swift CGEvent (not Enigo).
/// Swift is reliable, modern, and simple — Enigo's single-instance
/// limitation and silent drops are why the previous Rust path never worked.
/// Works for both normal transcription and AI Super Wispr.
/// 8-level / 256-node style logging for diagnosability.
#[cfg(target_os = "macos")]
pub fn inject_send_key(app: &tauri::AppHandle, key: SendKey) {
    let _injected_key_guard = crate::shortcut::InjectedKeyGuard::acquire();
    log::info!(target: "send_it", "inject_send_key: {:?} — paste already completed, posting CGEvent immediately (no 80ms wait)", key);

    let result = {
        extern "C" {
            fn superflow_send_key(key: i32) -> bool;
        }
        let code = match key {
            SendKey::Enter => 0,
            SendKey::CommandEnter => 1,
            SendKey::ControlEnter => 2,
        };
        // SAFETY: Swift side uses CGEvent with kVK_Return and proper flags; no
        // allocation, no callback, just a synchronous key post.
        let ok = unsafe { superflow_send_key(code) };
        if ok {
            Ok(())
        } else {
            Err("Swift CGEvent send failed — 8 levels / 256 nodes: CGEvent tap may be denied, check Accessibility permission".to_string())
        }
    };

    match &result {
        Ok(()) => log::info!(target: "send_it", "sent {:?} key via Swift — 8 levels / 256 nodes success", key),
        Err(error) => log::warn!(target: "send_it", "send key injection failed: {error} — 8 levels / 256 nodes check, key={key:?}", error = error, key = key),
    }
    if let Err(error) = result {
        log::warn!(target: "send_it", "send key injection failed: {error}");
    } else {
        log::info!(target: "send_it", "sent {:?} key via Swift", key);
    }
    let _ = app;
}

#[cfg(not(target_os = "macos"))]
pub fn inject_send_key(_app: &tauri::AppHandle, _key: SendKey) {
    log::warn!(target: "send_it", "send key injection is only supported on macOS");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(text: &str, surface: &str) -> Option<(String, SendKey)> {
        detect_send_it(text, surface).map(|p| (p.cleaned, p.key))
    }

    #[test]
    fn strips_trailing_send_it_gmail() {
        assert_eq!(
            plan(
                "Hey SuperFlow, draft me an email to Alex. Thanks for the update. Send it.",
                "gmail"
            ),
            Some((
                "Hey SuperFlow, draft me an email to Alex. Thanks for the update.".to_string(),
                SendKey::CommandEnter
            ))
        );
    }

    #[test]
    fn all_casings_trigger() {
        for variant in ["send it", "SEND IT", "Send It", "sEnD iT", "Send it."] {
            assert_eq!(
                plan(&format!("Meeting at 3. {variant}"), "slack"),
                Some(("Meeting at 3.".to_string(), SendKey::Enter)),
                "{variant}"
            );
        }
    }

    #[test]
    fn send_it_now_and_send_this_work() {
        assert_eq!(
            plan("The build is ready. Send it now.", "gmail"),
            Some(("The build is ready.".to_string(), SendKey::CommandEnter))
        );
        assert_eq!(
            plan("Reply. Send this email.", "outlook"),
            Some(("Reply.".to_string(), SendKey::CommandEnter))
        );
    }

    #[test]
    fn and_send_it_strips_conjunction() {
        assert_eq!(
            plan("Tell Alex I'll be there. And send it.", "slack"),
            Some(("Tell Alex I'll be there.".to_string(), SendKey::Enter))
        );
    }

    #[test]
    fn content_that_mentions_send_does_not_trigger() {
        assert_eq!(plan("Tell Alex I'll send it tomorrow.", "gmail"), None);
        assert_eq!(plan("Ask him whether he can send it.", "gmail"), None);
        assert_eq!(
            plan("Please open the door and send it later.", "gmail"),
            None
        );
        assert_eq!(plan("I will send it.", "gmail"), None);
    }

    #[test]
    fn bare_send_only_as_terminal() {
        assert_eq!(
            plan("Thanks for the update. Send.", "gmail"),
            Some(("Thanks for the update.".to_string(), SendKey::CommandEnter))
        );
        assert_eq!(plan("Please send the file.", "gmail"), None);
    }

    #[test]
    fn only_the_command_yields_nothing() {
        assert_eq!(plan("Send it.", "slack"), None);
        assert_eq!(plan("hey superflow send it", "gmail"), None);
    }

    #[test]
    fn outlook_and_slack_keys_differ() {
        assert_eq!(
            detect_send_it("Hi team. Send it.", "outlook").unwrap().key,
            SendKey::CommandEnter
        );
        assert_eq!(
            detect_send_it("Hi team. Send it.", "slack").unwrap().key,
            SendKey::Enter
        );
    }

    #[test]
    fn repeated_command_is_treated_as_content() {
        // "send it send it send it" has no sentence boundary before the second
        // and third occurrence, so it is not a terminal command.
        assert_eq!(plan("Done. Send it send it send it", "gmail"), None);
    }
}
