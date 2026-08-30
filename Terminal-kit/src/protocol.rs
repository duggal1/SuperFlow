use std::ops::Range;
use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::model::SessionState;

/// Top-level directive parsed from agent PTY output.
/// Agents emit these as single-line JSON prefixed by `SAPPHIRE_`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SapphireDirective {
    /// Worker state update (progress, blockers, completion)
    Status(StatusDirective),
    /// Inter-worker communication (requests, reviews, blockers)
    Mail(MailDirective),
    /// Mail acknowledgment (acked, done, cannot_comply)
    Ack(AckDirective),
    /// File ownership claim (must emit before editing)
    Lease(LeaseDirective),
}

/// Worker state update — emitted when a worker's state changes.
/// Required: `state`, `summary`. Optional: `files`, `commands`, `risks`, `overlap`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusDirective {
    /// One of the 16 lifecycle states (progressing, blocked, done_claimed, validated, failed, etc.)
    pub state: String,
    /// What the worker is currently doing or why state changed
    pub summary: String,
    /// File paths the worker has touched
    #[serde(default)]
    pub files: Vec<String>,
    /// Commands the worker has run
    #[serde(default)]
    pub commands: Vec<String>,
    /// Risks the worker has identified
    #[serde(default)]
    pub risks: Vec<String>,
    /// Potential file conflicts with other workers
    #[serde(default)]
    pub overlap: Option<String>,
}

impl StatusDirective {
    pub fn session_state(&self) -> Option<SessionState> {
        SessionState::from_directive(&self.state)
    }
}

/// Inter-worker communication directive — structured terminal-to-terminal mail.
/// Persisted to SQLite before injection into recipient PTY.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MailDirective {
    /// Unique mail ID for tracking (optional, generated if absent)
    #[serde(default)]
    pub mail_id: Option<String>,
    /// Mail ID this is replying to (for threading)
    #[serde(default)]
    pub reply_to: Option<String>,
    /// Conversation thread ID (optional)
    #[serde(default)]
    pub thread_id: Option<String>,
    /// Target worker display name (e.g. "Engineer-2", "Supervisor")
    pub to: String,
    /// Additional recipients (e.g. ["Supervisor"] for visibility)
    #[serde(default)]
    pub cc: Vec<String>,
    /// Message type: task, reply, notification, escalation, scavenge
    /// Legacy types (dependency_request, review_request, blocker, etc.) are normalized.
    pub message_type: String,
    /// Priority: urgent, high, normal, low
    pub priority: String,
    /// Delivery mode: interrupt (inject immediately) or queue (normal delivery)
    #[serde(default)]
    pub delivery_mode: String,
    /// Brief subject line
    pub subject: String,
    /// Background context for the recipient
    #[serde(default)]
    pub context: String,
    /// What the sender needs from the recipient
    pub request: String,
    /// What action the sender expects
    pub expected_action: String,
    /// Whether this mail requires explicit acknowledgment
    #[serde(default)]
    pub requires_ack: bool,
    /// Delivery state tracking (optional)
    #[serde(default)]
    pub delivery_state: Option<String>,
    /// Pinned mail — survives auto-archival
    #[serde(default)]
    pub pinned: bool,
    /// Suppress all notifications for this mail
    #[serde(default)]
    pub suppress_notify: bool,
}

/// Mail acknowledgment — confirms receipt and intent to act on a mail message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AckDirective {
    /// The mail ID being acknowledged
    pub mail_id: String,
    /// Response status: acked (received), done (actioned), cannot_comply (unable)
    pub status: String,
    /// Brief summary of what will be done or why compliance isn't possible
    pub summary: String,
}

/// File ownership claim — workers must emit this before editing files.
/// Conflicting claims on the same path trigger contradiction handling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaseDirective {
    /// File paths to claim or release (relative to repo root)
    #[serde(default)]
    pub paths: Vec<String>,
    /// Intent: read, edit, or review
    pub intent: String,
    /// Action: claim or release
    pub status: String,
}

pub fn consume_directives(buffer: &mut String, chunk: &str) -> Vec<SapphireDirective> {
    buffer.push_str(&sanitize_output(chunk));
    let mut directives = Vec::new();
    let mut cursor = 0;
    let mut consumed_until = 0;

    while let Some((directive, next_cursor)) = parse_next_directive(buffer, cursor) {
        directives.push(directive);
        consumed_until = next_cursor;
        cursor = next_cursor;
    }

    if consumed_until > 0 {
        buffer.drain(..consumed_until);
    } else if buffer.len() > 128_000 {
        let keep_from = previous_char_boundary(buffer, buffer.len() - 64_000);
        buffer.drain(..keep_from);
    }

    directives
}

pub fn sanitize_output(text: &str) -> String {
    ansi_escape_regex()
        .replace_all(text, "")
        .replace('\r', "\n")
}

fn parse_next_directive(buffer: &str, start_at: usize) -> Option<(SapphireDirective, usize)> {
    let mut search_from = previous_char_boundary(buffer, start_at);
    while let Some(relative_start) = buffer[search_from..].find("SAPPHIRE_") {
        let absolute_start = search_from + relative_start;
        let rest = &buffer[absolute_start..];
        let captures = directive_prefix_regex().captures(rest)?;
        let kind = captures.get(1)?.as_str();
        let json_start = absolute_start + captures.get(0)?.end();
        let json_range = extract_json_object_range(buffer, json_start)?;
        let json = &buffer[json_range.clone()];

        let directive = match kind {
            "STATUS" => serde_json::from_str::<StatusDirective>(json)
                .ok()
                .filter(|directive| !status_looks_like_prompt_example(directive))
                .map(SapphireDirective::Status),
            "MAIL" => serde_json::from_str::<MailDirective>(json)
                .ok()
                .filter(|directive| !mail_looks_like_prompt_example(directive))
                .map(SapphireDirective::Mail),
            "ACK" => serde_json::from_str::<AckDirective>(json)
                .ok()
                .filter(|directive| !ack_looks_like_prompt_example(directive))
                .map(SapphireDirective::Ack),
            "LEASE" => serde_json::from_str::<LeaseDirective>(json)
                .ok()
                .filter(|directive| !lease_looks_like_prompt_example(directive))
                .map(SapphireDirective::Lease),
            _ => None,
        };

        if let Some(directive) = directive {
            return Some((directive, json_range.end));
        }

        search_from = absolute_start + "SAPPHIRE_".len();
    }

    None
}

fn looks_like_placeholder_text(value: &str) -> bool {
    let lowered = value.trim().to_ascii_lowercase();
    lowered.is_empty()
        || lowered == "..."
        || lowered.contains('<')
        || lowered.contains('>')
        || lowered.contains('|')
        || lowered.contains("your-display-name")
        || lowered.contains("one short sentence")
        || lowered.contains("one short instruction")
        || lowered.contains("paths touched")
        || lowered.contains("commands ran")
        || lowered.contains("what you are doing")
        || lowered.contains("optional")
}

fn status_looks_like_prompt_example(directive: &StatusDirective) -> bool {
    looks_like_placeholder_text(&directive.state)
        || looks_like_placeholder_text(&directive.summary)
        || directive
            .files
            .iter()
            .any(|value| looks_like_placeholder_text(value))
        || directive
            .commands
            .iter()
            .any(|value| looks_like_placeholder_text(value))
        || directive
            .risks
            .iter()
            .any(|value| looks_like_placeholder_text(value))
        || directive
            .overlap
            .as_deref()
            .map(looks_like_placeholder_text)
            .unwrap_or(false)
}

fn mail_looks_like_prompt_example(directive: &MailDirective) -> bool {
    directive
        .mail_id
        .as_deref()
        .map(looks_like_placeholder_text)
        .unwrap_or(false)
        || directive
            .reply_to
            .as_deref()
            .map(looks_like_placeholder_text)
            .unwrap_or(false)
        || looks_like_placeholder_text(&directive.to)
        || directive
            .cc
            .iter()
            .any(|value| looks_like_placeholder_text(value))
        || looks_like_placeholder_text(&directive.message_type)
        || looks_like_placeholder_text(&directive.priority)
        || looks_like_placeholder_text(&directive.subject)
        || looks_like_placeholder_text(&directive.context)
        || looks_like_placeholder_text(&directive.request)
        || looks_like_placeholder_text(&directive.expected_action)
}

pub(crate) fn ack_looks_like_prompt_example(directive: &AckDirective) -> bool {
    looks_like_placeholder_text(&directive.mail_id)
        || looks_like_placeholder_text(&directive.status)
        || looks_like_placeholder_text(&directive.summary)
}

pub(crate) fn lease_looks_like_prompt_example(directive: &LeaseDirective) -> bool {
    looks_like_placeholder_text(&directive.intent)
        || looks_like_placeholder_text(&directive.status)
        || directive
            .paths
            .iter()
            .any(|value| looks_like_placeholder_text(value) || value == "src/path.rs")
}

fn previous_char_boundary(text: &str, index: usize) -> usize {
    let mut boundary = index.min(text.len());
    while boundary > 0 && !text.is_char_boundary(boundary) {
        boundary -= 1;
    }
    boundary
}

fn extract_json_object_range(buffer: &str, start_at: usize) -> Option<Range<usize>> {
    let bytes = buffer.as_bytes();
    let mut index = start_at;
    while let Some(byte) = bytes.get(index) {
        if !byte.is_ascii_whitespace() {
            break;
        }
        index += 1;
    }
    if bytes.get(index).copied()? != b'{' {
        return None;
    }

    let mut depth = 0_i32;
    let mut in_string = false;
    let mut escaped = false;
    let start = index;

    for (offset, byte) in bytes[index..].iter().enumerate() {
        let ch = *byte as char;
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    let end = index + offset + 1;
                    return Some(start..end);
                }
            }
            _ => {}
        }
    }

    None
}

fn directive_prefix_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"SAPPHIRE_(STATUS|MAIL|ACK|LEASE)\s+").expect("valid directive prefix regex")
    })
}

fn ansi_escape_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"\x1B(?:[@-Z\\-_]|\[[0-?]*[ -/]*[@-~])").expect("valid ansi regex")
    })
}

#[cfg(test)]
mod tests {
    use super::{MailDirective, SapphireDirective, consume_directives, sanitize_output};

    #[test]
    fn parses_status_mail_and_leases_from_line_buffer() {
        let mut buffer = String::new();
        let chunk = concat!(
            "noise\n",
            "SAPPHIRE_STATUS {\"state\":\"progressing\",\"summary\":\"working\",\"files\":[\"src/main.rs\"],\"commands\":[],\"risks\":[]}\n",
            "SAPPHIRE_MAIL {\"to\":\"Engineer-2\",\"message_type\":\"dependency_request\",\"priority\":\"high\",\"subject\":\"confirm contract\",\"context\":\"need response shape\",\"request\":\"share fields\",\"expected_action\":\"reply\"}\n",
            "SAPPHIRE_ACK {\"mail_id\":\"m-1\",\"status\":\"acked\",\"summary\":\"will reply\"}\n",
            "SAPPHIRE_LEASE {\"paths\":[\"src/main.rs\"],\"intent\":\"edit\",\"status\":\"claim\"}\n"
        );

        let directives = consume_directives(&mut buffer, chunk);
        assert_eq!(directives.len(), 4);
        assert!(matches!(directives[0], SapphireDirective::Status(_)));
        assert!(matches!(directives[2], SapphireDirective::Ack(_)));
        assert!(matches!(directives[3], SapphireDirective::Lease(_)));

        let mail = match &directives[1] {
            SapphireDirective::Mail(mail) => mail,
            _ => panic!("expected mail directive"),
        };
        assert_eq!(mail.to, "Engineer-2");
    }

    #[test]
    fn keeps_partial_line_buffer_until_complete() {
        let mut buffer = String::new();
        let first = "SAPPHIRE_MAIL {\"to\":\"Engineer-2\"";
        let second = ",\"message_type\":\"dependency_request\",\"priority\":\"high\",\"subject\":\"s\",\"context\":\"c\",\"request\":\"r\",\"expected_action\":\"e\"}\n";
        let none = consume_directives(&mut buffer, first);
        assert!(none.is_empty());

        let directives = consume_directives(&mut buffer, second);
        let mail = match &directives[0] {
            SapphireDirective::Mail(mail) => mail,
            _ => panic!("expected mail directive"),
        };
        assert_eq!(
            mail.to,
            MailDirective {
                to: "Engineer-2".to_owned(),
                mail_id: None,
                reply_to: None,
                thread_id: None,
                cc: Vec::new(),
                message_type: "dependency_request".to_owned(),
                priority: "high".to_owned(),
                delivery_mode: String::new(),
                subject: "s".to_owned(),
                context: "c".to_owned(),
                request: "r".to_owned(),
                expected_action: "e".to_owned(),
                requires_ack: false,
                delivery_state: None,
                pinned: false,
                suppress_notify: false,
            }
            .to
        );
    }

    #[test]
    fn parses_multiline_directives_with_nested_json() {
        let mut buffer = String::new();
        let chunk = concat!(
            "noise before\n",
            "SAPPHIRE_MAIL {\n",
            "  \"to\": \"Engineer-2\",\n",
            "  \"message_type\": \"dependency_request\",\n",
            "  \"priority\": \"high\",\n",
            "  \"subject\": \"confirm contract\",\n",
            "  \"context\": \"need response shape\",\n",
            "  \"request\": \"share nested fields {if any}\",\n",
            "  \"expected_action\": \"reply\",\n",
            "  \"requires_ack\": true\n",
            "}\n",
            "more noise\n",
            "SAPPHIRE_STATUS {\n",
            "  \"state\": \"blocked\",\n",
            "  \"summary\": \"waiting on Engineer-2\",\n",
            "  \"files\": [\"src/main.rs\"],\n",
            "  \"commands\": [],\n",
            "  \"risks\": [\"dependency gap\"]\n",
            "}\n"
        );

        let directives = consume_directives(&mut buffer, chunk);
        assert_eq!(directives.len(), 2);
        assert!(matches!(directives[0], SapphireDirective::Mail(_)));
        assert!(matches!(directives[1], SapphireDirective::Status(_)));
        assert!(buffer.trim().is_empty());
    }

    #[test]
    fn preserves_carriage_return_line_structure_for_screen_reader_output() {
        let sanitized = sanitize_output("STATE: progressing\rSUMMARY: ready\rFILES: NONE");
        assert_eq!(sanitized, "STATE: progressing\nSUMMARY: ready\nFILES: NONE");
    }

    #[test]
    fn buffer_trimming_keeps_utf8_boundaries() {
        let mut buffer = "é".repeat(70_000);
        let directives = consume_directives(&mut buffer, "");
        assert!(directives.is_empty());
        assert!(buffer.is_char_boundary(buffer.len()));
        assert!(std::str::from_utf8(buffer.as_bytes()).is_ok());
    }

    #[test]
    fn ignores_malformed_json_directive() {
        let mut buffer = String::new();
        let chunk = concat!(
            "SAPPHIRE_STATUS {broken json}\n",
            "SAPPHIRE_STATUS {\"state\":\"progressing\",\"summary\":\"ok\",\"files\":[],\"commands\":[],\"risks\":[]}\n"
        );
        let directives = consume_directives(&mut buffer, chunk);
        // Malformed JSON is skipped; valid directive is parsed
        assert_eq!(directives.len(), 1);
        assert!(matches!(directives[0], SapphireDirective::Status(_)));
    }

    #[test]
    fn ignores_unknown_directive_type() {
        let mut buffer = String::new();
        let chunk = "SAPPHIRE_UNKNOWN {\"key\":\"value\"}\n";
        let directives = consume_directives(&mut buffer, chunk);
        assert!(directives.is_empty());
    }

    #[test]
    fn handles_ansi_escapes_around_directive() {
        let mut buffer = String::new();
        let chunk = concat!(
            "\x1b[32mSAPPHIRE_STATUS ",
            "{\"state\":\"progressing\",\"summary\":\"colored\",\"files\":[],\"commands\":[],\"risks\":[]}",
            "\x1b[0m\n"
        );
        let directives = consume_directives(&mut buffer, chunk);
        assert_eq!(directives.len(), 1);
        let status = match &directives[0] {
            SapphireDirective::Status(s) => s,
            _ => panic!("expected status directive"),
        };
        assert_eq!(status.summary, "colored");
    }

    #[test]
    fn handles_json_with_escaped_quotes() {
        let mut buffer = String::new();
        let chunk = concat!(
            "SAPPHIRE_STATUS {",
            "\"state\":\"progressing\",",
            "\"summary\":\"He said \\\"hello\\\"\",",
            "\"files\":[],\"commands\":[],\"risks\":[]",
            "}\n"
        );
        let directives = consume_directives(&mut buffer, chunk);
        assert_eq!(directives.len(), 1);
        let status = match &directives[0] {
            SapphireDirective::Status(s) => s,
            _ => panic!("expected status directive"),
        };
        assert_eq!(status.summary, "He said \"hello\"");
    }

    #[test]
    fn handles_multiple_directives_on_same_line() {
        let mut buffer = String::new();
        let chunk = concat!(
            "SAPPHIRE_STATUS {\"state\":\"progressing\",\"summary\":\"a\",\"files\":[],\"commands\":[],\"risks\":[]}",
            " SAPPHIRE_ACK {\"mail_id\":\"x\",\"status\":\"acked\",\"summary\":\"b\"}\n"
        );
        let directives = consume_directives(&mut buffer, chunk);
        assert_eq!(directives.len(), 2);
        assert!(matches!(directives[0], SapphireDirective::Status(_)));
        assert!(matches!(directives[1], SapphireDirective::Ack(_)));
    }

    #[test]
    fn handles_empty_chunk() {
        let mut buffer = String::new();
        let directives = consume_directives(&mut buffer, "");
        assert!(directives.is_empty());
        assert!(buffer.is_empty());
    }

    #[test]
    fn handles_directive_with_missing_optional_fields() {
        let mut buffer = String::new();
        let chunk = "SAPPHIRE_STATUS {\"state\":\"blocked\",\"summary\":\"no files\"}\n";
        let directives = consume_directives(&mut buffer, chunk);
        assert_eq!(directives.len(), 1);
        let status = match &directives[0] {
            SapphireDirective::Status(s) => s,
            _ => panic!("expected status directive"),
        };
        assert_eq!(status.files, Vec::<String>::new());
        assert_eq!(status.commands, Vec::<String>::new());
        assert_eq!(status.risks, Vec::<String>::new());
        assert!(status.overlap.is_none());
    }

    #[test]
    fn handles_partial_directive_prefix_without_json() {
        let mut buffer = String::new();
        let chunk = "SAPPHIRE_STATUS \n";
        let directives = consume_directives(&mut buffer, chunk);
        assert!(directives.is_empty());
    }

    #[test]
    fn handles_lease_with_empty_paths() {
        let mut buffer = String::new();
        let chunk = "SAPPHIRE_LEASE {\"paths\":[],\"intent\":\"read\",\"status\":\"claim\"}\n";
        let directives = consume_directives(&mut buffer, chunk);
        assert_eq!(directives.len(), 1);
        let lease = match &directives[0] {
            SapphireDirective::Lease(l) => l,
            _ => panic!("expected lease directive"),
        };
        assert!(lease.paths.is_empty());
    }

    #[test]
    fn consumes_parsed_directives_and_clears_buffer() {
        let mut buffer = String::new();
        let chunk = concat!(
            "prefix noise\n",
            "SAPPHIRE_STATUS {\"state\":\"progressing\",\"summary\":\"ok\",\"files\":[],\"commands\":[],\"risks\":[]}\n",
            "trailing noise\n"
        );
        let directives = consume_directives(&mut buffer, chunk);
        assert_eq!(directives.len(), 1);
        // Buffer should have the directive prefix consumed
        assert!(!buffer.contains("SAPPHIRE_STATUS"));
    }

    #[test]
    fn handles_sapphire_keyword_in_regular_output() {
        let mut buffer = String::new();
        let chunk = "The SAPPHIRE_STATUS endpoint is not working properly\n";
        let directives = consume_directives(&mut buffer, chunk);
        assert!(directives.is_empty());
    }

    #[test]
    fn handles_nested_braces_in_string_values() {
        let mut buffer = String::new();
        let chunk = concat!(
            "SAPPHIRE_MAIL {",
            "\"to\":\"Engineer-1\",",
            "\"message_type\":\"dependency_request\",",
            "\"priority\":\"high\",",
            "\"subject\":\"nested {braces} here\",",
            "\"context\":\"data with {nested: {deep: 1}}\",",
            "\"request\":\"r\",",
            "\"expected_action\":\"e\"",
            "}\n"
        );
        let directives = consume_directives(&mut buffer, chunk);
        assert_eq!(directives.len(), 1);
        let mail = match &directives[0] {
            SapphireDirective::Mail(m) => m,
            _ => panic!("expected mail directive"),
        };
        assert_eq!(mail.subject, "nested {braces} here");
    }

    #[test]
    fn ignores_prompt_example_directives_with_placeholder_values() {
        let mut buffer = String::new();
        let chunk = concat!(
            "SAPPHIRE_LEASE {\"paths\":[\"src/path.rs\"],\"intent\":\"read|edit|review\",\"status\":\"claim|release\"}\n",
            "SAPPHIRE_ACK {\"mail_id\":\"...\",\"status\":\"acked|done|cannot_comply\",\"summary\":\"...\"}\n",
            "SAPPHIRE_STATUS {\"state\":\"progressing\",\"summary\":\"working\",\"files\":[\"src/main.rs\"],\"commands\":[],\"risks\":[]}\n",
        );
        let directives = consume_directives(&mut buffer, chunk);
        assert_eq!(directives.len(), 1);
        assert!(matches!(directives[0], SapphireDirective::Status(_)));
    }
}
