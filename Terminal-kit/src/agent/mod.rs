use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;
use std::time::Duration;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use crate::model::SessionState;
use crate::runtime::{ProcessLaunchSpec, StartupAutomationRule, SubmitMode};

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum AgentKind {
    Qwen,
    Forge,
    Codex,
    Claude,
}

impl AgentKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Qwen => "qwen",
            Self::Forge => "forge",
            Self::Codex => "codex",
            Self::Claude => "claude",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "qwen" => Some(Self::Qwen),
            "forge" => Some(Self::Forge),
            "codex" => Some(Self::Codex),
            "claude" => Some(Self::Claude),
            _ => None,
        }
    }

    fn display_name(self) -> &'static str {
        match self {
            Self::Qwen => "Qwen Code",
            Self::Forge => "Forge",
            Self::Codex => "OpenAI Codex",
            Self::Claude => "Claude Code",
        }
    }

    pub fn build_launch_spec(
        self,
        repo: &Path,
        state_dir: &Path,
        extra_args: &[String],
    ) -> ProcessLaunchSpec {
        let mut env = BTreeMap::new();
        env.insert(
            "SAPPHIRE_SESSION_ROOT".to_owned(),
            state_dir.to_string_lossy().into_owned(),
        );
        if matches!(self, Self::Forge) {
            let forge_home = state_dir.join("forge-home");
            let xdg_data = forge_home.join(".local/share");
            let xdg_config = forge_home.join(".config");
            env.insert("HOME".to_owned(), forge_home.to_string_lossy().into_owned());
            env.insert(
                "XDG_DATA_HOME".to_owned(),
                xdg_data.to_string_lossy().into_owned(),
            );
            env.insert(
                "XDG_CONFIG_HOME".to_owned(),
                xdg_config.to_string_lossy().into_owned(),
            );
        }

        let (program, base_args, prompt_delay, startup_input, startup_rules, submit_mode) =
            match self {
                Self::Qwen => (
                    "qwen",
                    vec!["--approval-mode".to_owned(), "yolo".to_owned()],
                    Duration::from_millis(14000),
                    Some((Duration::from_millis(8000), "\n".to_owned())),
                    vec![StartupAutomationRule::new(
                        "qwen_ide_prompt",
                        "Do you want to connect IDE to Qwen Code?",
                        "2\n",
                    )],
                    SubmitMode::CarriageReturn,
                ),
                Self::Forge => (
                    "forge",
                    Vec::new(),
                    Duration::from_millis(1200),
                    Some((Duration::from_millis(400), "\n".to_owned())),
                    Vec::new(),
                    SubmitMode::LineFeed,
                ),
                Self::Codex => (
                    "codex",
                    vec![
                        "--no-alt-screen".to_owned(),
                        "-m".to_owned(),
                        "gpt-5.4-mini".to_owned(),
                        "-c".to_owned(),
                        "model_reasoning_effort=low".to_owned(),
                    ],
                    Duration::from_millis(1800),
                    Some((Duration::from_millis(400), "\n".to_owned())),
                    vec![StartupAutomationRule::new(
                        "codex_trust_prompt",
                        "directory?",
                        "1\n",
                    )],
                    SubmitMode::CarriageReturn,
                ),
                Self::Claude => (
                    "claude",
                    Vec::new(),
                    Duration::from_millis(3200),
                    Some((Duration::from_millis(600), "\n".to_owned())),
                    vec![StartupAutomationRule::new(
                        "claude_trust_prompt",
                        "Yes, I trust this folder",
                        "\n",
                    )],
                    SubmitMode::CarriageReturn,
                ),
            };

        let mut args = base_args;
        args.extend(extra_args.iter().cloned());

        ProcessLaunchSpec {
            program: program.to_owned(),
            args,
            cwd: repo.to_path_buf(),
            env,
            prompt_delay,
            startup_input,
            startup_rules,
            surface_label: self.display_name().to_owned(),
            submit_mode,
        }
    }

    #[allow(dead_code)]
    pub fn protocol_nudge(self) -> String {
        match self {
            Self::Qwen => "Reply with exactly one raw line that starts with SAPPHIRE_STATUS followed by compact JSON. Do not wrap it in markdown.".to_owned(),
            Self::Forge => "Emit one plain SAPPHIRE_STATUS line with compact JSON. Keep it machine-readable and on a single line.".to_owned(),
            Self::Codex => "Print a single-line SAPPHIRE_STATUS JSON record now. No code fence, no prose before it.".to_owned(),
            Self::Claude => "Output exactly one SAPPHIRE_STATUS line with compact JSON, not a markdown block.".to_owned(),
        }
    }

    #[allow(dead_code)]
    pub fn infer_state_from_output(self, text: &str) -> Option<HeuristicSignal> {
        let lowered = text.to_ascii_lowercase();
        if contains_any(
            &lowered,
            &[
                "you are worker-",
                "mission:",
                "# role",
                "sapphire_status",
                "sapphire_mail",
                "sapphire_lease",
                "definition of done",
                "communication rules",
                "validation standard",
                "supervision strategy",
            ],
        ) {
            return None;
        }

        let state = if contains_any(
            &lowered,
            &["validation passed", "validated", "all checks passed"],
        ) {
            SessionState::Validated
        } else if contains_any(
            &lowered,
            &[
                "i'm done",
                "im done",
                "completed the task",
                "finished the task",
                "task is complete",
                "implemented the requested",
                "done with",
            ],
        ) {
            SessionState::DoneClaimed
        } else if contains_any(
            &lowered,
            &[
                "blocked",
                "can't proceed",
                "cannot proceed",
                "waiting on",
                "need clarification",
                "dependency",
            ],
        ) {
            SessionState::Blocked
        } else if contains_any(
            &lowered,
            &[
                "conflict",
                "overlap",
                "ownership unclear",
                "contradiction",
                "someone else changed",
            ],
        ) {
            SessionState::Contradictory
        } else if contains_any(
            &lowered,
            &[
                "investigating",
                "reproducing",
                "working on",
                "running tests",
                "profiling",
                "reviewing",
            ],
        ) {
            SessionState::Progressing
        } else {
            return None;
        };

        Some(HeuristicSignal {
            state,
            summary: truncate_summary(text),
        })
    }
}

impl fmt::Display for AgentKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct HeuristicSignal {
    pub state: SessionState,
    pub summary: String,
}

#[allow(dead_code)]
fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

#[allow(dead_code)]
fn truncate_summary(text: &str) -> String {
    let compact = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut out = compact.chars().take(180).collect::<String>();
    if compact.chars().count() > 180 {
        out.push_str("...");
    }
    out
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::time::Duration;

    use super::AgentKind;

    #[test]
    fn qwen_launch_spec_uses_interactive_runtime_args() {
        let spec = AgentKind::Qwen.build_launch_spec(Path::new("."), Path::new("./.sp"), &[]);
        assert_eq!(
            spec.args,
            vec!["--approval-mode".to_owned(), "yolo".to_owned()]
        );
        assert_eq!(spec.prompt_delay, Duration::from_millis(14000));
        assert_eq!(
            spec.startup_input,
            Some((Duration::from_millis(8000), "\n".to_owned()))
        );
    }
}
