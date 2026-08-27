//! Deterministic, case-preserving Gmail voice-command parser.

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum GmailIntent {
    Reply,
    Compose,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TerminalAction {
    #[default]
    None,
    Send,
    Cancel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GmailVoiceCommand {
    pub intent: GmailIntent,
    pub instruction: String,
    pub recipient_hint: Option<String>,
    pub terminal_action: TerminalAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GmailVoiceInput {
    Start(GmailVoiceCommand),
    SessionAction(TerminalAction),
}

const REPLY_ALIASES: &[&str] = &[
    "please reply to this email",
    "reply to this email",
    "please reply to this",
    "reply to this",
    "please reply",
    "reply",
];

const COMPOSE_ALIASES: &[&str] = &[
    "please draft me an email to",
    "draft me an email to",
    "please draft an email to",
    "draft an email to",
    "please write me an email to",
    "write me an email to",
    "please write an email to",
    "write an email to",
    "please compose an email to",
    "compose an email to",
];

// Terminal-position send aliases. Bare "send" is deliberately terminal-only:
// a standalone utterance of just "send" stays ordinary dictation, while
// "… and send." at the end of a Gmail command is a send instruction.
const SEND_ALIASES: &[&str] = &["send it now", "send this email", "send this", "send it"];
const CANCEL_ALIASES: &[&str] = &["cancel", "never mind", "stop that", "abort"];
const RECIPIENT_TERMINATORS: &[&str] = &[
    "telling",
    "saying",
    "asking",
    "thanking",
    "reminding",
    "explaining",
    "that",
    "about",
    "letting",
    "and tell",
    "and ask",
];

fn trim_outer_command_punctuation(value: &str) -> &str {
    value.trim().trim_matches(|character: char| {
        character.is_whitespace() || matches!(character, '.' | ',' | ':' | ';' | '!' | '?')
    })
}

fn equals_alias(value: &str, alias: &str) -> bool {
    let value = trim_outer_command_punctuation(value).to_lowercase();
    let value = value.strip_prefix("please ").unwrap_or(&value);
    let value = value.strip_suffix(" please").unwrap_or(value);
    value == alias
}

fn strip_leading_alias<'a>(value: &'a str, alias: &str) -> Option<&'a str> {
    let value = value.trim_start();
    let prefix = value.get(..alias.len())?;
    if !prefix.eq_ignore_ascii_case(alias) {
        return None;
    }
    let rest = value.get(alias.len()..)?;
    if rest
        .chars()
        .next()
        .is_some_and(|character| character.is_alphanumeric() || character == '@')
    {
        return None;
    }
    Some(rest.trim_start_matches(|character: char| {
        character.is_whitespace()
            || matches!(
                character,
                '.' | ',' | ':' | ';' | '!' | '?' | '-' | '–' | '—'
            )
    }))
}

const SEND_ALIASES_TERMINAL: &[&str] = &[
    "send it now",
    "send this email",
    "send it",
    "send this",
    "send",
];

fn strip_terminal_send(value: &str) -> Option<String> {
    let without_terminal_punctuation = value
        .trim_end_matches(|character: char| {
            character.is_whitespace() || matches!(character, '.' | '!' | '?' | ',' | ';')
        })
        .trim_end_matches(|c: char| c.is_whitespace() || c == ',')
        .trim_end_matches(" please")
        .trim();
    let lower = without_terminal_punctuation.to_lowercase();

    for alias in SEND_ALIASES_TERMINAL {
        let alias_lower = alias.to_lowercase();
        if lower.len() < alias_lower.len() {
            continue;
        }
        if !lower.ends_with(&alias_lower) {
            continue;
        }
        let before_len = lower.len() - alias_lower.len();
        let before = without_terminal_punctuation.get(..before_len).unwrap_or("").trim_end();
        // Handle " please" suffix already trimmed above, but also handle bare alias
        if before.is_empty() {
            return Some(String::new());
        }
        let before_lower = before.to_lowercase();
        // False-positive guard: embedded "send it tomorrow" etc. must stay content
        if before_lower.contains("send it tomorrow")
            || before_lower.contains("send this tomorrow")
            || before_lower.ends_with("will send")
            || before_lower.ends_with("to send")
        {
            continue;
        }
        // Accept sentence boundary OR conjunction "and"/"or" before alias
        let last_char = before.chars().last();
        let ends_with_boundary = matches!(last_char, Some('.' | '!' | '?' | ';' | ','));
        let ends_with_and_or = before_lower.ends_with(" and") || before_lower.ends_with(" or");
        if ends_with_boundary || ends_with_and_or || before.is_empty() {
            let mut instruction = before
                .trim_end_matches(|character: char| {
                    character.is_whitespace()
                        || matches!(character, '.' | '!' | '?' | ';' | ',' | ':')
                })
                .trim_end_matches(" and")
                .trim_end_matches(" or")
                .trim()
                .to_string();
            // Remove trailing "and" left after alias strip (e.g. "Tell him X and" -> "Tell him X")
            if instruction.to_lowercase().ends_with(" and") {
                instruction = instruction[..instruction.len() - 4].trim().to_string();
            }
            return Some(instruction);
        }
    }
    None
}

fn extract_compose_parts(instruction: &str) -> (Option<String>, String) {
    let lowered = instruction.to_lowercase();
    let boundary = RECIPIENT_TERMINATORS
        .iter()
        .filter_map(|terminator| {
            lowered
                .match_indices(terminator)
                .find(|(index, _)| {
                    *index > 0
                        && lowered[..*index]
                            .chars()
                            .last()
                            .is_some_and(char::is_whitespace)
                })
                .map(|(index, _)| (index, *terminator))
        })
        .min_by_key(|(index, _)| *index);
    let end = boundary
        .map(|(index, _)| index)
        .unwrap_or(instruction.len());
    let candidate = instruction[..end]
        .trim()
        .trim_end_matches(|character: char| matches!(character, '!' | ',' | '.' | '?' | '-'))
        .trim();
    let recipient = (!candidate.is_empty()).then(|| candidate.to_string());
    let remaining = boundary
        .map(|(index, _)| instruction[index..].trim())
        .unwrap_or("");
    let remaining = [
        ("telling", "Tell"),
        ("saying", "Say"),
        ("asking", "Ask"),
        ("thanking", "Thank"),
        ("reminding", "Remind"),
        ("explaining", "Explain"),
        ("letting", "Let"),
    ]
    .into_iter()
    .find_map(|(prefix, replacement)| {
        remaining
            .get(..prefix.len())
            .filter(|value| value.eq_ignore_ascii_case(prefix))
            .and_then(|_| remaining.get(prefix.len()..))
            .map(|rest| format!("{replacement}{rest}"))
    })
    .unwrap_or_else(|| remaining.to_string());
    (recipient, remaining)
}

pub fn literal_recipient_email(recipient_hint: Option<&str>) -> Option<String> {
    let candidate = recipient_hint?.trim().trim_matches(['<', '>', '(', ')']);
    let (local, domain) = candidate.split_once('@')?;
    let valid_local = !local.is_empty()
        && local.chars().all(|character| {
            character.is_ascii_alphanumeric() || ".!#$%&'*+/=?^_`{|}~-".contains(character)
        });
    let valid_domain = domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && domain
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '.' | '-'));
    (valid_local && valid_domain).then(|| candidate.to_string())
}

pub fn parse(transcription: &str) -> Option<GmailVoiceInput> {
    for alias in SEND_ALIASES {
        if equals_alias(transcription, alias) {
            return Some(GmailVoiceInput::SessionAction(TerminalAction::Send));
        }
    }
    for alias in CANCEL_ALIASES {
        if equals_alias(transcription, alias) {
            return Some(GmailVoiceInput::SessionAction(TerminalAction::Cancel));
        }
    }

    for (aliases, intent) in [
        (REPLY_ALIASES, GmailIntent::Reply),
        (COMPOSE_ALIASES, GmailIntent::Compose),
    ] {
        for alias in aliases {
            let Some(rest) = strip_leading_alias(transcription, alias) else {
                continue;
            };
            let (instruction, terminal_action) = match strip_terminal_send(rest) {
                Some(instruction) => (instruction, TerminalAction::Send),
                None => (rest.trim().to_string(), TerminalAction::None),
            };
            let (recipient_hint, instruction) = if intent == GmailIntent::Compose {
                let (recipient, compose_instruction) = extract_compose_parts(&instruction);
                (recipient, compose_instruction)
            } else {
                (None, instruction)
            };
            return Some(GmailVoiceInput::Start(GmailVoiceCommand {
                intent,
                instruction,
                recipient_hint,
                terminal_action,
            }));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command(value: &str) -> GmailVoiceCommand {
        let GmailVoiceInput::Start(command) = parse(value).expect("expected Gmail command") else {
            panic!("expected start command");
        };
        command
    }

    #[test]
    fn preserves_instruction_case_and_punctuation() {
        let parsed = command(
            "Please reply to this email. Tell Alex I'll join at 10 AM, but only for 30 minutes.",
        );
        assert_eq!(parsed.intent, GmailIntent::Reply);
        assert_eq!(
            parsed.instruction,
            "Tell Alex I'll join at 10 AM, but only for 30 minutes."
        );
    }

    #[test]
    fn inline_send_requires_a_sentence_boundary() {
        let parsed = command("Reply. Tell Alex I'll be there tomorrow. Send it.");
        assert_eq!(parsed.terminal_action, TerminalAction::Send);
        assert_eq!(parsed.instruction, "Tell Alex I'll be there tomorrow");

        let ambiguous = command("Reply saying tell Alex I'll send it");
        assert_eq!(ambiguous.terminal_action, TerminalAction::None);
        assert!(ambiguous.instruction.ends_with("send it"));

        let compose = command(
            "Draft an email to alex@company.com thanking him for the update. Send it.",
        );
        assert_eq!(compose.intent, GmailIntent::Compose);
        assert_eq!(compose.terminal_action, TerminalAction::Send);
        assert_eq!(compose.recipient_hint.as_deref(), Some("alex@company.com"));

        // Case-insensitive variants of "send it" must all trigger
        for variant in ["SEND IT", "Send It", "sEnD iT", "send it"] {
            let cmd = command(&format!("Reply. Hello. {}", variant));
            assert_eq!(cmd.terminal_action, TerminalAction::Send, "{variant}");
        }
    }

    #[test]
    fn false_positive_send_phrases_remain_content() {
        for value in [
            "Reply. Tell Alex I'll send it tomorrow.",
            "Reply. Ask him whether he can send it.",
            "Reply. Say don't send it yet.",
            "Reply. Say ‘send it’ was his exact wording.",
        ] {
            assert_eq!(
                command(value).terminal_action,
                TerminalAction::None,
                "{value}"
            );
        }
    }

    #[test]
    fn standalone_actions_accept_terminal_punctuation() {
        for value in ["Send it.", "SEND IT", "Send It!", "send it?", "send it."] {
            assert_eq!(
                parse(value),
                Some(GmailVoiceInput::SessionAction(TerminalAction::Send)),
                "{value}"
            );
        }
        assert_eq!(
            parse("Cancel."),
            Some(GmailVoiceInput::SessionAction(TerminalAction::Cancel))
        );
    }

    #[test]
    fn ordinary_dictation_and_prefix_lookalikes_are_not_commands() {
        for value in [
            "Tell Alex I'll send it tomorrow.",
            "Please open the door",
            "Replying to this email now",
            "Draft an email today",
            "send",
            "send it send it send it",
        ] {
            assert!(parse(value).is_none(), "{value}");
        }
    }

    #[test]
    fn compose_extracts_earliest_recipient_hint_without_losing_case() {
        let parsed = command(
            "Draft an email to Alexander Chen asking whether the build is ready and tell me later.",
        );
        assert_eq!(parsed.intent, GmailIntent::Compose);
        assert_eq!(parsed.recipient_hint.as_deref(), Some("Alexander Chen"));
        assert_eq!(
            parsed.instruction,
            "Ask whether the build is ready and tell me later."
        );
    }

    #[test]
    fn literal_email_resolution_is_strict() {
        assert_eq!(
            literal_recipient_email(Some("alex@company.com")),
            Some("alex@company.com".to_string())
        );
        assert!(literal_recipient_email(Some("Alex")).is_none());
        assert!(literal_recipient_email(Some("alex@localhost")).is_none());
    }

    #[test]
    fn terminal_send_supports_the_full_alias_set() {
        for (value, expected_instruction) in [
            ("Reply. The build is ready. Send it.", "The build is ready"),
            (
                "Reply. The build is ready. Send it now.",
                "The build is ready",
            ),
            ("Reply. The build is ready. Send this.", "The build is ready"),
            (
                "Reply. The build is ready. Send this email.",
                "The build is ready",
            ),
            ("Reply. The build is ready. Send.", "The build is ready"),
        ] {
            let parsed = command(value);
            assert_eq!(parsed.terminal_action, TerminalAction::Send, "{value}");
            assert_eq!(parsed.instruction, expected_instruction, "{value}");
        }
    }

    #[test]
    fn standalone_send_accepts_terminal_aliases_but_not_bare_send() {
        for value in ["Send it.", "Send it now!", "Send this.", "Send this email."] {
            assert_eq!(
                parse(value),
                Some(GmailVoiceInput::SessionAction(TerminalAction::Send)),
                "{value}"
            );
        }
        assert_eq!(parse("send"), None);
    }

    #[test]
    fn full_alias_set_false_positives_remain_content() {
        for value in [
            "Reply. Tell Alex I'll send it now and then leave.",
            "Reply. Ask him whether he can send this email tomorrow.",
            "Reply. Say don't send this yet.",
        ] {
            assert_eq!(
                command(value).terminal_action,
                TerminalAction::None,
                "{value}"
            );
        }
    }
}
