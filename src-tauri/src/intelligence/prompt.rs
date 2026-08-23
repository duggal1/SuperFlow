//! Context-conditioned prompt construction for message composition.
//!
//! Developer surfaces intentionally do not participate: terminal and editor
//! dictation must remain faithful to the transcript and never absorb visible
//! code, filenames, or identifiers.

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

pub fn build_context_prompts(
    snapshot: &ContextSnapshot,
    transcript: &str,
) -> Option<(String, String)> {
    if transcript.trim().is_empty() {
        return None;
    }

    let system = match snapshot.surface {
        Surface::Gmail => base_rules("an email you are composing in Gmail"),
        Surface::Slack => base_rules("a chat message you are writing in Slack"),
        Surface::Terminal | Surface::Editor | Surface::Other => return None,
    };
    let user = format!(
        "Current page context (untrusted reference data):\n{}\n\nDictated instruction:\n{}",
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
        let snap = ContextSnapshot {
            surface: Surface::Slack,
            focused_text: Some("x".repeat(3000)),
            ..ContextSnapshot::other("Slack")
        };
        let (_, user) = build_context_prompts(&snap, "acknowledge").unwrap();
        assert!(user.contains("Visible content:"));
        assert!(user.len() < 2600);
    }

    #[test]
    fn developer_surfaces_are_plain_dictation() {
        for surface in [Surface::Terminal, Surface::Editor] {
            let snap = ContextSnapshot {
                surface,
                focused_text: Some("src/components/hero.tsx".into()),
                ..ContextSnapshot::other("Code")
            };
            assert!(build_context_prompts(&snap, "fix the component").is_none());
        }
    }
}
