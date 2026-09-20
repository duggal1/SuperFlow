//! Voice-triggered coding-agent launches are temporarily disabled.
//! Both entry points return false so utterances remain ordinary dictation.
//! The former launcher files are retained for a future deliberate re-enable.

use crate::settings::AppSettings;

pub fn try_handle_voice_command(_transcription: &str) -> bool {
    false
}

pub async fn try_handle_ai_command(_instruction: &str, _settings: &AppSettings) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voice_agent_commands_are_never_consumed_or_launched() {
        for spoken in [
            "open codex",
            "open claude code",
            "open 4 codex terminals",
            "Open Cloud Code, open Codex",
            "please open a thousand Claude Code terminals",
            "Use Codex to describe my existing code without launching anything",
        ] {
            assert!(!try_handle_voice_command(spoken), "{spoken}");
        }
    }

    #[tokio::test]
    async fn ai_agent_commands_are_never_interpreted_or_launched() {
        let settings = AppSettings::default();
        assert!(!try_handle_ai_command("Open Claude Code and Codex", &settings).await);
    }
}
