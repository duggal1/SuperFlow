//! Surface-aware prompt construction for Intelligence Awareness.
//!
//! Pure functions only: snapshot + transcript in, prompts out. The system
//! prompt carries the persona and output contract; the user content carries
//! the page context and the verbatim dictated instruction.

use crate::context::types::{ContextSnapshot, Surface};

fn context_block(snapshot: &ContextSnapshot) -> String {
    let mut lines = vec![format!("Application: {}", snapshot.app_name)];
    if let Some(url) = &snapshot.url {
        lines.push(format!("URL: {url}"));
    }
    if let Some(title) = &snapshot.title {
        lines.push(format!("Page title: {title}"));
    }
    if let Some(focused) = &snapshot.focused_text {
        // Visible page text (thread/message content when the AX adapter can
        // read it). Hard cap keeps prompts bounded.
        const MAX_FOCUSED_CHARS: usize = 2000;
        let mut text = focused.trim().to_string();
        if text.len() > MAX_FOCUSED_CHARS {
            text.truncate(MAX_FOCUSED_CHARS);
            text.push('…');
        }
        if !text.is_empty() {
            lines.push(format!("Visible content:\n{text}"));
        }
    }
    lines.join("\n")
}

fn base_rules(kind: &str) -> String {
    format!(
        "You are SuperFlow's writing assistant, currently helping inside {kind}. \
         The user dictates a short INSTRUCTION about what they want written — \
         you produce the finished text itself.\n\
         Rules:\n\
         - Output ONLY the final {kind} text. No preamble, no explanation, no quotes around it.\n\
         - Never repeat or mention the instruction.\n\
         - Write in the language the instruction was spoken in.\n\
         - Keep it natural, concise, and appropriate to what you can see of the page.\n"
    )
}

fn developer_rules(kind: &str) -> String {
    format!(
        "You are SuperFlow's dictation assistant, currently helping inside {kind}. \
         The user dictated text that will be inserted at the cursor position.\n\
         Rules:\n\
         - Output ONLY the final dictated text, ready to insert. No preamble, no explanation, no quotes around it.\n\
         - Resolve spoken file paths, directory names, commands, flags, symbols, and identifiers against what is visible on screen; prefer an exact visible match over a phonetic guess.\n\
         - Keep code tokens' exact spelling and casing once resolved.\n\
         - Otherwise preserve the user's words, word order, and language exactly.\n"
    )
}

/// Build (system, user) prompts for the detected surface. Returns `None` when
/// the surface is not awareness-enabled or the instruction is empty — callers
/// then fall back to plain dictation.
pub fn build_context_prompts(
    snapshot: &ContextSnapshot,
    transcript: &str,
) -> Option<(String, String)> {
    if !snapshot.is_aware_surface() {
        return None;
    }
    if transcript.trim().is_empty() {
        return None;
    }

    let system = match snapshot.surface {
        Surface::Gmail => base_rules("an email you are composing in Gmail"),
        Surface::Slack => base_rules("a chat message you are writing in Slack"),
        Surface::Terminal => developer_rules("a terminal"),
        Surface::Editor => developer_rules("a code editor"),
        Surface::Other => return None,
    };

    let user = format!(
        "Current page context:\n{}\n\nDictated instruction:\n{}",
        context_block(snapshot),
        transcript.trim()
    );

    Some((system, user))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn other_surface_is_skipped() {
        let snap = ContextSnapshot::other("Finder");
        assert!(build_context_prompts(&snap, "say hi").is_none());
    }

    #[test]
    fn blank_transcript_is_skipped() {
        let snap = ContextSnapshot {
            surface: Surface::Gmail,
            ..ContextSnapshot::other("Google Chrome")
        };
        assert!(build_context_prompts(&snap, "   ").is_none());
    }

    #[test]
    fn gmail_prompt_carries_url_and_instruction() {
        let snap = ContextSnapshot {
            surface: Surface::Gmail,
            url: Some("https://mail.google.com/mail/u/0/#inbox".into()),
            title: Some("Inbox - me@gmail.com - Gmail".into()),
            ..ContextSnapshot::other("Google Chrome")
        };
        let (system, user) = build_context_prompts(&snap, "reply informally please").unwrap();
        assert!(system.contains("Gmail"));
        assert!(user.contains("https://mail.google.com/mail/u/0/#inbox"));
        assert!(user.contains("reply informally please"));
    }

    #[test]
    fn focused_text_is_included_and_capped() {
        let long = "x".repeat(3000);
        let snap = ContextSnapshot {
            surface: Surface::Slack,
            focused_text: Some(long),
            ..ContextSnapshot::other("Slack")
        };
        let (_, user) = build_context_prompts(&snap, "acknowledge").unwrap();
        assert!(user.contains("Visible content:"));
        assert!(user.len() < 2600);
    }

    #[test]
    fn slack_native_snapshot_without_url_still_composes() {
        let snap = ContextSnapshot {
            surface: Surface::Slack,
            ..ContextSnapshot::other("Slack")
        };
        let (_, user) = build_context_prompts(&snap, "tell the team deploy is done").unwrap();
        assert!(user.contains("deploy is done"));
    }

    #[test]
    fn terminal_prompt_resolves_against_visible_context() {
        let snap = ContextSnapshot {
            surface: Surface::Terminal,
            focused_text: Some("~/app/src/components/hero.tsx\n$ ".into()),
            ..ContextSnapshot::other("Ghostty")
        };
        let (system, user) = build_context_prompts(&snap, "open components hero dot tsx").unwrap();
        assert!(system.contains("terminal"));
        assert!(system.contains("visible on screen"));
        assert!(user.contains("hero.tsx"));
    }

    #[test]
    fn editor_prompt_resolves_against_visible_context() {
        let snap = ContextSnapshot {
            surface: Surface::Editor,
            focused_text: Some("import {{ Button }} from \"./components/Button\"".into()),
            ..ContextSnapshot::other("Code")
        };
        let (system, _) = build_context_prompts(&snap, "add the button import").unwrap();
        assert!(system.contains("code editor"));
    }
}
