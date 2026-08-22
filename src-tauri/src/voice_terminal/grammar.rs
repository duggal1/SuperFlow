//! Deterministic parser for spoken terminal-launch commands.
//!
//! Grammar (case-, punctuation-, and filler-insensitive):
//!   `[please] open [a|an] [<count>] <agent> [code|terminals|instances|...] [mission...]`
//!
//! Examples that parse:
//!   "please open claude code"
//!   "open codex"
//!   "please open opencode"
//!   "please open a four Claude Code terminal"
//!   "open three opencode terminals refactor the auth module"
//!
//! Anything that does not contain both the spoken verb "open" and a known
//! agent name returns `None` so normal dictation is never hijacked.

/// Terminal coding agent CLIs this feature can launch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentKind {
    Claude,
    Codex,
    OpenCode,
}

impl AgentKind {
    /// Binary launched inside each tmux pane.
    pub fn executable(self) -> &'static str {
        match self {
            AgentKind::Claude => "claude",
            AgentKind::Codex => "codex",
            AgentKind::OpenCode => "opencode",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            AgentKind::Claude => "Claude Code",
            AgentKind::Codex => "Codex",
            AgentKind::OpenCode => "OpenCode",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParsedCommand {
    pub agent: AgentKind,
    /// Worker terminal count (>= 1). One extra brain terminal is spawned when
    /// `brain` is set.
    pub workers: usize,
    /// True when the speaker said an explicit count → spawn the extra brain.
    pub brain: bool,
    /// Everything spoken after the agent name; empty for a bare launch order.
    pub mission: String,
}

/// Absolute cap on worker terminals (two full pages of eight).
const MAX_WORKERS: usize = 16;

/// Words that carry a count between the verb and the agent name.
fn count_from_token(token: &str) -> Option<usize> {
    let counts: &[(&str, usize)] = &[
        ("one", 1),
        ("two", 2),
        ("three", 3),
        ("four", 4),
        ("five", 5),
        ("six", 6),
        ("seven", 7),
        ("eight", 8),
        ("nine", 9),
        ("ten", 10),
        ("eleven", 11),
        ("twelve", 12),
    ];
    if let Ok(n) = token.parse::<usize>() {
        if (1..=MAX_WORKERS).contains(&n) {
            return Some(n);
        }
    }
    counts
        .iter()
        .find(|(word, _)| *word == token)
        .map(|(_, n)| *n)
}

/// Multi-token aliases are listed first so they win over their prefixes.
fn agent_alias_len_at(tokens: &[String], index: usize) -> Option<(AgentKind, usize)> {
    let pairs: &[(&[&str], AgentKind)] = &[
        (&["claude", "code"], AgentKind::Claude),
        (&["claud", "code"], AgentKind::Claude),
        (&["cloud", "code"], AgentKind::Claude),
        (&["opencode"], AgentKind::OpenCode),
        (&["open", "code"], AgentKind::OpenCode),
        (&["claude"], AgentKind::Claude),
        (&["claud"], AgentKind::Claude),
        (&["codex"], AgentKind::Codex),
        (&["codec"], AgentKind::Codex),
        (&["codecs"], AgentKind::Codex),
    ];
    for (alias, kind) in pairs {
        if index + alias.len() <= tokens.len()
            && alias
                .iter()
                .enumerate()
                .all(|(k, word)| tokens[index + k] == *word)
        {
            return Some((*kind, alias.len()));
        }
    }
    None
}

fn strip_leading_fillers(tokens: &[String]) -> &[String] {
    let fillers = [
        "terminal",
        "terminals",
        "instance",
        "instances",
        "window",
        "windows",
        "session",
        "sessions",
        "tab",
        "tabs",
        "pane",
        "panes",
        "and",
    ];
    let mut end = 0;
    while end < tokens.len() && fillers.contains(&tokens[end].as_str()) {
        end += 1;
    }
    &tokens[end..]
}

fn normalize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !(c.is_alphanumeric() || c == '\''))
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Parse a finalized transcript into a launch command, or `None`.
pub fn parse(transcription: &str) -> Option<ParsedCommand> {
    let tokens = normalize(transcription);

    // The spoken verb must be its own token so "opening" never triggers.
    let verb_index = tokens.iter().position(|t| t == "open")?;
    let rest = &tokens[verb_index + 1..];

    // Find the first agent alias after the verb.
    for i in 0..rest.len() {
        let Some((agent, width)) = agent_alias_len_at(rest, i) else {
            continue;
        };

        // Look backwards from the alias for a spoken count, skipping articles.
        let mut count = None;
        for token in rest[..i].iter().rev().take(4) {
            if matches!(token.as_str(), "a" | "an" | "the") {
                continue;
            }
            if let Some(n) = count_from_token(token.as_str()) {
                count = Some(n);
            }
            break;
        }

        let mission_tokens = strip_leading_fillers(&rest[i + width..]);
        return Some(ParsedCommand {
            agent,
            workers: count.unwrap_or(1).min(MAX_WORKERS),
            brain: count.is_some(),
            mission: mission_tokens.join(" ").trim().to_string(),
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(text: &str) -> ParsedCommand {
        parse(text).expect("should parse")
    }

    #[test]
    fn bare_single_open_defaults_to_one_worker_without_brain() {
        let cmd = parsed("Please open claude code");
        assert_eq!(cmd.agent, AgentKind::Claude);
        assert_eq!(cmd.workers, 1);
        assert!(!cmd.brain);
        assert!(cmd.mission.is_empty());
    }

    #[test]
    fn spoken_count_spawns_brain_and_keeps_mission() {
        let cmd = parsed("Please open a four Claude Code terminal");
        assert_eq!(cmd.agent, AgentKind::Claude);
        assert_eq!(cmd.workers, 4);
        assert!(cmd.brain);
        assert!(cmd.mission.is_empty());
    }

    #[test]
    fn digit_count_and_mission() {
        let cmd = parsed("Open 3 codex terminals refactor the auth module");
        assert_eq!(cmd.agent, AgentKind::Codex);
        assert_eq!(cmd.workers, 3);
        assert!(cmd.brain);
        assert_eq!(cmd.mission, "refactor the auth module");
    }

    #[test]
    fn opencode_two_word_alias() {
        let cmd = parsed("please open open code to fix the crash");
        assert_eq!(cmd.agent, AgentKind::OpenCode);
        assert_eq!(cmd.workers, 1);
        assert!(!cmd.brain);
        assert_eq!(cmd.mission, "to fix the crash");
    }

    #[test]
    fn filler_words_after_agent_are_stripped() {
        let cmd = parsed("please open five opencode windows");
        assert_eq!(cmd.agent, AgentKind::OpenCode);
        assert_eq!(cmd.workers, 5);
        assert!(cmd.brain);
        assert!(cmd.mission.is_empty());
    }

    #[test]
    fn normal_dictation_does_not_match_without_agent_name() {
        assert!(parse("please open the door for me").is_none());
        assert!(parse("I was opening files all day").is_none());
        assert!(parse("open questions about cloud infrastructure").is_none());
    }

    #[test]
    fn punctuation_and_case_are_ignored() {
        let cmd = parsed("Um, Please Open... Four, Claude Code terminals!");
        assert_eq!(cmd.workers, 4);
        assert_eq!(cmd.agent, AgentKind::Claude);
    }
}
