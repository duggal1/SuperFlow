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
    Cline,
    Kilo,
}

impl AgentKind {
    /// Binary launched inside each tmux pane.
    pub fn executable(self) -> &'static str {
        match self {
            AgentKind::Claude => "claude",
            AgentKind::Codex => "codex",
            AgentKind::OpenCode => "opencode",
            AgentKind::Cline => "cline",
            AgentKind::Kilo => "kilo",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            AgentKind::Claude => "Claude Code",
            AgentKind::Codex => "Codex",
            AgentKind::OpenCode => "OpenCode",
            AgentKind::Cline => "Cline",
            AgentKind::Kilo => "Kilo",
        }
    }

    /// Canonical lowercase id shared with the AI-interpreted JSON (`terminal`).
    pub fn json_id(self) -> &'static str {
        self.executable()
    }

    /// Map a canonical id from AI-interpreted JSON back to an agent.
    pub fn from_json_id(id: &str) -> Option<AgentKind> {
        AgentKind::all()
            .into_iter()
            .find(|kind| kind.json_id() == id)
    }

    pub fn all() -> [AgentKind; 5] {
        [
            AgentKind::Claude,
            AgentKind::Codex,
            AgentKind::OpenCode,
            AgentKind::Cline,
            AgentKind::Kilo,
        ]
    }
}

#[derive(Debug, Clone)]
pub struct ParsedCommand {
    pub agent: AgentKind,
    /// Worker count requested from the supervisor (>= 1).
    pub workers: usize,
    /// Everything spoken after the agent name; empty for a bare launch order.
    pub mission: String,
}

/// Absolute cap on workers delegated by one supervisor.
pub(crate) const MAX_WORKERS: usize = 16;

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
/// These cover common ASR misrecognitions ("cluade", "clawed", "kline") and
/// hyphenated/compound spellings — normalization already splits on hyphens.
fn agent_alias_len_at(tokens: &[String], index: usize) -> Option<(AgentKind, usize)> {
    let pairs: &[(&[&str], AgentKind)] = &[
        (&["claude", "code"], AgentKind::Claude),
        (&["claud", "code"], AgentKind::Claude),
        (&["cloud", "code"], AgentKind::Claude),
        (&["clawed", "code"], AgentKind::Claude),
        (&["cluade", "code"], AgentKind::Claude),
        (&["claudecode"], AgentKind::Claude),
        (&["opencode"], AgentKind::OpenCode),
        (&["open", "code"], AgentKind::OpenCode),
        (&["kilo", "code"], AgentKind::Kilo),
        (&["kilocode"], AgentKind::Kilo),
        (&["codex", "cli"], AgentKind::Codex),
        (&["claude"], AgentKind::Claude),
        (&["claud"], AgentKind::Claude),
        (&["cluade"], AgentKind::Claude),
        (&["clawed"], AgentKind::Claude),
        (&["codex"], AgentKind::Codex),
        (&["codec"], AgentKind::Codex),
        (&["codecs"], AgentKind::Codex),
        (&["cline"], AgentKind::Cline),
        (&["kline"], AgentKind::Cline),
        (&["clyne"], AgentKind::Cline),
        (&["kilo"], AgentKind::Kilo),
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

/// Maximum tokens between the spoken verb "open" and the agent alias. The
/// verb and the agent name must belong to the same command phrase — a wide
/// gap means the two words appeared independently in normal speech.
const MAX_VERB_DISTANCE: usize = 6;

/// Parse a finalized transcript into a launch command, or `None`.
pub fn parse(transcription: &str) -> Option<ParsedCommand> {
    let tokens = normalize(transcription);

    // The spoken verb must be its own token so "opening" never triggers.
    let verb_index = tokens.iter().position(|t| t == "open")?;
    let rest = &tokens[verb_index + 1..];

    // Find the first agent alias within the same phrase as the verb.
    for i in 0..rest.len().min(MAX_VERB_DISTANCE) {
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
            mission: mission_tokens.join(" ").trim().to_string(),
        });
    }

    None
}

/// Cheap pre-filter for the AI-interpreted path: does this transcript mention
/// any known agent alias at all? Used to skip the LLM roundtrip entirely for
/// normal speech that cannot possibly name a terminal agent.
pub fn mentions_agent(transcription: &str) -> bool {
    let tokens = normalize(transcription);
    (0..tokens.len()).any(|i| agent_alias_len_at(&tokens, i).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(text: &str) -> ParsedCommand {
        parse(text).expect("should parse")
    }

    #[test]
    fn bare_single_open_defaults_to_one_worker() {
        let cmd = parsed("Please open claude code");
        assert_eq!(cmd.agent, AgentKind::Claude);
        assert_eq!(cmd.workers, 1);
        assert!(cmd.mission.is_empty());
    }

    #[test]
    fn spoken_count_is_preserved_for_the_supervisor() {
        let cmd = parsed("Please open a four Claude Code terminal");
        assert_eq!(cmd.agent, AgentKind::Claude);
        assert_eq!(cmd.workers, 4);
        assert!(cmd.mission.is_empty());
    }

    #[test]
    fn digit_count_and_mission() {
        let cmd = parsed("Open 3 codex terminals refactor the auth module");
        assert_eq!(cmd.agent, AgentKind::Codex);
        assert_eq!(cmd.workers, 3);
        assert_eq!(cmd.mission, "refactor the auth module");
    }

    #[test]
    fn opencode_two_word_alias() {
        let cmd = parsed("please open open code to fix the crash");
        assert_eq!(cmd.agent, AgentKind::OpenCode);
        assert_eq!(cmd.workers, 1);
        assert_eq!(cmd.mission, "to fix the crash");
    }

    #[test]
    fn filler_words_after_agent_are_stripped() {
        let cmd = parsed("please open five opencode windows");
        assert_eq!(cmd.agent, AgentKind::OpenCode);
        assert_eq!(cmd.workers, 5);
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

    #[test]
    fn every_supported_agent_parses_with_its_mission() {
        let cases = [
            (
                "open Cline and inspect the implementation",
                AgentKind::Cline,
                1,
            ),
            ("open Kilo and validate the fix", AgentKind::Kilo, 1),
            (
                "please open Codex and review this code",
                AgentKind::Codex,
                1,
            ),
            (
                "open 3 OpenCode and review these issues",
                AgentKind::OpenCode,
                3,
            ),
            ("open Claude Code and fix the backend", AgentKind::Claude, 1),
        ];
        for (text, agent, workers) in cases {
            let cmd = parse(text).unwrap_or_else(|| panic!("should parse: {text}"));
            assert_eq!(cmd.agent, agent, "{text}");
            assert_eq!(cmd.workers, workers, "{text}");
            assert!(!cmd.mission.is_empty(), "{text}");
        }
    }

    #[test]
    fn asr_misrecognitions_still_resolve() {
        assert_eq!(parsed("open cluade-code").agent, AgentKind::Claude);
        assert_eq!(parsed("open clawed code").agent, AgentKind::Claude);
        assert_eq!(parsed("open claudecode").agent, AgentKind::Claude);
        assert_eq!(parsed("please open kline").agent, AgentKind::Cline);
        assert_eq!(parsed("open kilocode").agent, AgentKind::Kilo);
        assert_eq!(parsed("open kilo code").agent, AgentKind::Kilo);
    }

    #[test]
    fn multi_worker_requests_parse_counts() {
        let cmd = parsed("open 5 Claude Code and fix the backend frontend and run tests");
        assert_eq!(cmd.agent, AgentKind::Claude);
        assert_eq!(cmd.workers, 5);
        assert_eq!(cmd.mission, "fix the backend frontend and run tests");
    }

    #[test]
    fn verb_and_agent_far_apart_do_not_trigger() {
        assert!(parse("open the project files and tell me what claude thinks about it").is_none());
        assert!(
            parse("I said open the notes, and later we should ask claude code for help").is_none()
        );
    }

    #[test]
    fn mentions_agent_prefilter_matches_aliases_only() {
        assert!(mentions_agent("can claude code fix this?"));
        assert!(mentions_agent("open kilo"));
        assert!(mentions_agent("cluade-code misheard"));
        assert!(!mentions_agent("what is the weather today"));
        assert!(!mentions_agent("cloud infrastructure costs"));
    }
}
