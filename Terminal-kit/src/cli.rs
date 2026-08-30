use std::num::NonZeroUsize;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use uuid::Uuid;

use crate::agent::AgentKind;

#[derive(Debug, Parser)]
#[command(
    name = "sp",
    about = "Sapphire: terminal-first orchestration for coding agents",
    subcommand_negates_reqs = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Agent: qwen, forge, codex, claude
    #[arg(value_enum)]
    pub agent: Option<AgentKind>,

    /// Number of worker terminals
    pub count: Option<NonZeroUsize>,

    /// Mission text — what the agents should do
    pub mission: Option<String>,

    #[command(flatten)]
    pub run: RunOptions,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Show active/running missions at a glance
    Status,
    /// Launch peer terminals without a supervisor or control plane
    Ns(NoSupervisorCommand),
    /// Push the current branch through Sapphire's operator-owned git path
    Push,
    /// List all missions (history)
    Sessions,
    /// Replay events for a mission
    Replay(ReplayCommand),
    /// Supervisor's final summary for a mission
    Summary(MissionArg),
    /// Resume a stalled or partial mission
    Resume(ResumeCommand),
    /// Watch a single worker's journey
    Watch(WatchCommand),
}

#[derive(Debug, Args, Clone)]
pub struct RunOptions {
    #[arg(long, default_value = ".")]
    pub repo: PathBuf,

    #[arg(long, value_enum)]
    pub supervisor_agent: Option<AgentKind>,

    #[arg(long)]
    pub state_dir: Option<PathBuf>,

    #[arg(long)]
    pub dry_run: bool,

    #[arg(long, default_value_t = 45)]
    pub stall_seconds: u64,

    #[arg(long)]
    pub watchdog_max_seconds: Option<u64>,

    #[arg(long, default_value_t = 1000)]
    pub watchdog_tick_millis: u64,

    #[arg(long)]
    pub tmux_session_name: Option<String>,

    #[arg(long)]
    pub persist_transcripts: bool,

    #[arg(long)]
    pub tui: bool,

    #[arg(long)]
    pub worker_args: Vec<String>,

    #[arg(long = "supervisor-arg")]
    pub supervisor_args: Vec<String>,
}

#[derive(Debug, Args, Clone)]
pub struct NoSupervisorCommand {
    #[arg(value_enum)]
    pub agent: AgentKind,

    pub count: NonZeroUsize,

    #[arg(required = true)]
    pub prompts: Vec<String>,

    #[arg(long, default_value = ".")]
    pub repo: PathBuf,

    #[arg(long)]
    pub session_name: Option<String>,

    #[arg(long = "worker-arg")]
    pub worker_args: Vec<String>,
}

#[derive(Debug, Args, Clone)]
pub struct ResumeCommand {
    pub mission_id: Uuid,

    #[command(flatten)]
    pub options: RuntimeOptions,

    #[arg(long)]
    pub state_dir: Option<PathBuf>,
}

#[derive(Debug, Args, Clone)]
pub struct ReplayCommand {
    pub mission_id: Uuid,

    #[arg(long, short = 'n', default_value_t = 40)]
    pub limit: usize,
}

#[derive(Debug, Args, Clone)]
pub struct MissionArg {
    pub mission_id: Uuid,
}

#[derive(Debug, Args, Clone)]
pub struct WatchCommand {
    pub mission_id: Uuid,
    pub worker: String,

    #[arg(long, short = 'n', default_value_t = 20)]
    pub limit: usize,
}

#[derive(Debug, Args, Clone)]
pub struct RuntimeOptions {
    #[arg(long, default_value_t = 45)]
    pub stall_seconds: u64,

    #[arg(long)]
    pub watchdog_max_seconds: Option<u64>,

    #[arg(long, default_value_t = 1000)]
    pub watchdog_tick_millis: u64,

    #[arg(long)]
    pub tmux_session_name: Option<String>,

    #[arg(long)]
    pub persist_transcripts: bool,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct LaunchConfig {
    pub worker_agent: AgentKind,
    pub supervisor_agent: AgentKind,
    pub worker_count: usize,
    pub repo: PathBuf,
    pub mission: String,
    pub state_dir: PathBuf,
    pub dry_run: bool,
    pub stall_seconds: u64,
    pub watchdog_max_seconds: Option<u64>,
    pub watchdog_tick_millis: u64,
    pub tmux: bool,
    pub tmux_session_name: Option<String>,
    pub persist_transcripts: bool,
    pub tui: bool,
    pub worker_args: Vec<String>,
    pub supervisor_args: Vec<String>,
    /// Git remote URL if repo has a remote, None otherwise.
    /// Used to decide whether to inject git commit rules into worker prompts.
    pub git_remote: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ResumeConfig {
    pub mission_id: Uuid,
    pub state_dir: Option<PathBuf>,
    pub stall_seconds: u64,
    pub watchdog_max_seconds: Option<u64>,
    pub watchdog_tick_millis: u64,
    pub tmux: bool,
    pub tmux_session_name: Option<String>,
    pub persist_transcripts: bool,
    pub tui: bool,
}

#[derive(Debug, Clone)]
pub struct NoSupervisorLaunchConfig {
    pub agent: AgentKind,
    pub count: usize,
    pub prompts: Vec<String>,
    pub repo: PathBuf,
    pub session_name: String,
    pub worker_args: Vec<String>,
}

impl Cli {
    pub fn into_action(self) -> Result<CliAction> {
        let Cli {
            command,
            agent,
            count,
            mission,
            run,
        } = self;

        if let Some(command) = command {
            return match command {
                Command::Status => Ok(CliAction::Status {
                    repo: run.repo,
                    state_dir: run.state_dir,
                }),
                Command::Ns(cmd) => Ok(CliAction::NoSupervisorLaunch(
                    Self::no_supervisor_config_from_command(cmd)?,
                )),
                Command::Push => Ok(CliAction::Push {
                    repo: run.repo.canonicalize().with_context(|| {
                        format!("failed to resolve repo path {}", run.repo.display())
                    })?,
                }),
                Command::Sessions => Ok(CliAction::Sessions {
                    repo: run.repo,
                    state_dir: run.state_dir,
                }),
                Command::Replay(cmd) => Ok(CliAction::Replay {
                    mission_id: cmd.mission_id,
                    limit: cmd.limit,
                }),
                Command::Summary(cmd) => Ok(CliAction::Summary {
                    mission_id: cmd.mission_id,
                }),
                Command::Resume(cmd) => Ok(CliAction::Resume(ResumeConfig {
                    mission_id: cmd.mission_id,
                    state_dir: cmd.state_dir,
                    stall_seconds: cmd.options.stall_seconds,
                    watchdog_max_seconds: cmd.options.watchdog_max_seconds,
                    watchdog_tick_millis: cmd.options.watchdog_tick_millis,
                    tmux: true,
                    tmux_session_name: cmd
                        .options
                        .tmux_session_name
                        .or_else(|| Some(default_resume_session_name(cmd.mission_id))),
                    persist_transcripts: true,
                    tui: false,
                })),
                Command::Watch(cmd) => Ok(CliAction::Watch {
                    mission_id: cmd.mission_id,
                    worker: cmd.worker,
                    limit: cmd.limit,
                }),
            };
        }

        Ok(CliAction::Run(Self::launch_config_from_parts(
            agent, count, mission, run,
        )?))
    }

    fn no_supervisor_config_from_command(
        cmd: NoSupervisorCommand,
    ) -> Result<NoSupervisorLaunchConfig> {
        let repo = cmd
            .repo
            .canonicalize()
            .with_context(|| format!("failed to resolve repo path {}", cmd.repo.display()))?;
        let count = cmd.count.get();
        if cmd.prompts.len() != count {
            anyhow::bail!(
                "prompt count mismatch: expected {count} prompt(s) for {count} terminal(s), got {}",
                cmd.prompts.len()
            );
        }
        Ok(NoSupervisorLaunchConfig {
            agent: cmd.agent,
            count,
            prompts: cmd.prompts,
            repo: repo.clone(),
            session_name: cmd
                .session_name
                .unwrap_or_else(|| default_no_supervisor_session_name(&repo, cmd.agent)),
            worker_args: cmd.worker_args,
        })
    }

    fn launch_config_from_parts(
        agent: Option<AgentKind>,
        count: Option<NonZeroUsize>,
        mission: Option<String>,
        run: RunOptions,
    ) -> Result<LaunchConfig> {
        let worker_agent =
            agent.context("missing agent — usage: sp <agent> <count> \"mission\"")?;
        let repo = run
            .repo
            .canonicalize()
            .with_context(|| format!("failed to resolve repo path {}", run.repo.display()))?;

        let state_dir = match run.state_dir {
            Some(path) => path,
            None => repo.join(".sp"),
        };

        let mission =
            mission.context("missing mission — usage: sp <agent> <count> \"mission text\"")?;
        let tmux_session_name = run
            .tmux_session_name
            .or_else(|| Some(default_launch_session_name(&repo, worker_agent)));

        Ok(LaunchConfig {
            worker_agent,
            supervisor_agent: run.supervisor_agent.unwrap_or(worker_agent),
            worker_count: count
                .context("missing count — usage: sp <agent> <count> \"mission\"")?
                .get(),
            repo: repo.clone(),
            mission: mission.trim().to_owned(),
            state_dir,
            dry_run: run.dry_run,
            stall_seconds: run.stall_seconds,
            watchdog_max_seconds: run.watchdog_max_seconds,
            watchdog_tick_millis: run.watchdog_tick_millis,
            tmux: !run.dry_run,
            tmux_session_name,
            persist_transcripts: true,
            tui: run.tui,
            worker_args: run.worker_args,
            supervisor_args: run.supervisor_args,
            git_remote: match crate::git::check_git_state(&repo) {
                crate::git::GitState::Ready { remote_url } => Some(remote_url),
                _ => None,
            },
        })
    }
}

#[derive(Debug)]
pub enum CliAction {
    Run(LaunchConfig),
    NoSupervisorLaunch(NoSupervisorLaunchConfig),
    Status {
        repo: PathBuf,
        state_dir: Option<PathBuf>,
    },
    Push {
        repo: PathBuf,
    },
    Sessions {
        repo: PathBuf,
        state_dir: Option<PathBuf>,
    },
    Replay {
        mission_id: Uuid,
        limit: usize,
    },
    Summary {
        mission_id: Uuid,
    },
    Resume(ResumeConfig),
    Watch {
        mission_id: Uuid,
        worker: String,
        limit: usize,
    },
}

fn default_launch_session_name(repo: &std::path::Path, agent: AgentKind) -> String {
    let stem = repo
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("repo");
    let clean = stem
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' => ch.to_ascii_lowercase(),
            _ => '-',
        })
        .collect::<String>();
    format!("sp-{}-{}-{}", clean, agent.as_str(), std::process::id())
}

fn default_resume_session_name(mission_id: Uuid) -> String {
    format!("sp-resume-{}", mission_id.simple())
}

fn default_no_supervisor_session_name(repo: &std::path::Path, agent: AgentKind) -> String {
    let stem = repo
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("repo");
    let clean = stem
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' => ch.to_ascii_lowercase(),
            _ => '-',
        })
        .collect::<String>();
    format!("sp-ns-{}-{}-{}", clean, agent.as_str(), std::process::id())
}

#[cfg(test)]
mod cli_integration_tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        let mut full_args = vec!["sp"];
        full_args.extend_from_slice(args);
        Cli::try_parse_from(full_args)
    }

    fn parse_action(args: &[&str]) -> Result<CliAction, String> {
        let cli = parse(args).map_err(|e| e.to_string())?;
        cli.into_action().map_err(|e| format!("{:#}", e))
    }

    // ── Launch parsing ──────────────────────────────────────────────

    #[test]
    fn parses_basic_launch() {
        // MISSION is positional (3rd arg), not a flag
        let cli = parse(&["codex", "4", "--repo", ".", "test mission"]).unwrap();
        let action = cli.into_action().unwrap();
        match action {
            CliAction::Run(cfg) => {
                assert_eq!(cfg.worker_agent, AgentKind::Codex);
                assert_eq!(cfg.worker_count, 4);
                assert_eq!(cfg.mission, "test mission");
            }
            other => panic!("expected Run, got {:?}", other),
        }
    }

    #[test]
    fn parses_all_agent_kinds() {
        for agent in [
            AgentKind::Qwen,
            AgentKind::Forge,
            AgentKind::Codex,
            AgentKind::Claude,
        ] {
            let args = &[agent.as_str(), "2", "--repo", ".", "t"];
            let result = parse(args);
            assert!(
                result.is_ok(),
                "agent {} should parse: {:?}",
                agent.as_str(),
                result
            );
        }
    }

    #[test]
    fn rejects_missing_mission() {
        // CLI parsing succeeds (mission is Option), but into_action() fails
        let cli = parse(&["codex", "2", "--repo", "."]).unwrap();
        assert!(
            cli.into_action().is_err(),
            "into_action should fail without mission"
        );
    }

    #[test]
    fn missing_repo_uses_default() {
        let cli = parse(&["codex", "2", "t"]).unwrap();
        assert!(cli.run.repo.to_str().map_or(false, |p| p == "."));
    }

    #[test]
    fn dry_run_disables_tmux() {
        let action = parse_action(&["codex", "2", "--repo", ".", "--dry-run", "t"]).unwrap();
        match action {
            CliAction::Run(cfg) => {
                assert!(cfg.dry_run);
                assert!(!cfg.tmux);
            }
            other => panic!("expected Run, got {:?}", other),
        }
    }

    // ── Subcommands ─────────────────────────────────────────────────

    #[test]
    fn parses_status_subcommand() {
        let action = parse_action(&["status"]).unwrap();
        assert!(matches!(action, CliAction::Status { .. }));
    }

    #[test]
    fn parses_no_supervisor_launch() {
        let action = parse_action(&["ns", "claude", "2", "hi one", "hi two"]).unwrap();
        match action {
            CliAction::NoSupervisorLaunch(cfg) => {
                assert_eq!(cfg.agent, AgentKind::Claude);
                assert_eq!(cfg.count, 2);
                assert_eq!(cfg.prompts, vec!["hi one", "hi two"]);
            }
            other => panic!("expected NoSupervisorLaunch, got {:?}", other),
        }
    }

    #[test]
    fn rejects_no_supervisor_prompt_count_mismatch() {
        let error = parse_action(&["ns", "claude", "3", "hi one", "hi two"]).unwrap_err();
        assert!(error.contains("prompt count mismatch"), "{error}");
    }

    #[test]
    fn parses_sessions_subcommand() {
        let action = parse_action(&["sessions"]).unwrap();
        assert!(matches!(action, CliAction::Sessions { .. }));
    }

    #[test]
    fn parses_replay_subcommand() {
        let id = "00000000-0000-0000-0000-000000000001";
        let action = parse_action(&["replay", id]).unwrap();
        match action {
            CliAction::Replay { mission_id, limit } => {
                assert_eq!(mission_id.to_string(), id);
                assert_eq!(limit, 40); // default
            }
            other => panic!("expected Replay, got {:?}", other),
        }
    }

    #[test]
    fn parses_replay_with_limit() {
        let id = "00000000-0000-0000-0000-000000000001";
        let action = parse_action(&["replay", id, "-n", "100"]).unwrap();
        match action {
            CliAction::Replay { limit, .. } => assert_eq!(limit, 100),
            other => panic!("expected Replay, got {:?}", other),
        }
    }

    #[test]
    fn parses_summary_subcommand() {
        let id = "00000000-0000-0000-0000-000000000002";
        let action = parse_action(&["summary", id]).unwrap();
        match action {
            CliAction::Summary { mission_id } => assert_eq!(mission_id.to_string(), id),
            other => panic!("expected Summary, got {:?}", other),
        }
    }

    #[test]
    fn parses_watch_subcommand() {
        let id = "00000000-0000-0000-0000-000000000003";
        let action = parse_action(&["watch", id, "Engineer-1"]).unwrap();
        match action {
            CliAction::Watch { worker, limit, .. } => {
                assert_eq!(worker, "Engineer-1");
                assert_eq!(limit, 20); // default
            }
            other => panic!("expected Watch, got {:?}", other),
        }
    }

    #[test]
    fn parses_watch_with_limit() {
        let id = "00000000-0000-0000-0000-000000000003";
        let action = parse_action(&["watch", id, "Engineer-1", "-n", "50"]).unwrap();
        match action {
            CliAction::Watch { limit, .. } => assert_eq!(limit, 50),
            other => panic!("expected Watch, got {:?}", other),
        }
    }

    #[test]
    fn parses_resume_subcommand() {
        let id = "00000000-0000-0000-0000-000000000004";
        let action = parse_action(&["resume", id]).unwrap();
        match action {
            CliAction::Resume(cfg) => assert_eq!(cfg.mission_id.to_string(), id),
            other => panic!("expected Resume, got {:?}", other),
        }
    }

    // ── Launch flags ────────────────────────────────────────────────

    #[test]
    fn parses_tmux_session_name() {
        let cli = parse(&[
            "codex",
            "2",
            "--repo",
            ".",
            "--tmux-session-name",
            "sapphire-test",
            "t",
        ])
        .unwrap();
        assert_eq!(cli.run.tmux_session_name.as_deref(), Some("sapphire-test"));
    }

    #[test]
    fn parses_stall_seconds() {
        let cli = parse(&["codex", "2", "--repo", ".", "--stall-seconds", "9000", "t"]).unwrap();
        assert_eq!(cli.run.stall_seconds, 9000);
    }

    #[test]
    fn parses_watchdog_max_seconds() {
        let cli = parse(&[
            "codex",
            "2",
            "--repo",
            ".",
            "--watchdog-max-seconds",
            "3600",
            "t",
        ])
        .unwrap();
        assert_eq!(cli.run.watchdog_max_seconds, Some(3600));
    }

    #[test]
    fn parses_watchdog_tick_millis_default() {
        let cli = parse(&["codex", "2", "--repo", ".", "t"]).unwrap();
        assert_eq!(cli.run.watchdog_tick_millis, 1000);
    }

    #[test]
    fn parses_tui_flag() {
        let cli = parse(&["codex", "2", "--repo", ".", "--tui", "t"]).unwrap();
        assert!(cli.run.tui);
    }

    #[test]
    fn parses_persist_transcripts() {
        let cli = parse(&["codex", "2", "--repo", ".", "--persist-transcripts", "t"]).unwrap();
        assert!(cli.run.persist_transcripts);
    }

    #[test]
    fn parses_worker_args() {
        // --worker-args takes a single string value
        let cli = parse(&[
            "codex",
            "2",
            "--repo",
            ".",
            "--worker-args",
            "extra-arg",
            "t",
        ])
        .unwrap();
        assert_eq!(cli.run.worker_args, vec!["extra-arg"]);
    }

    #[test]
    fn parses_supervisor_agent() {
        let cli = parse(&[
            "codex",
            "2",
            "--repo",
            ".",
            "--supervisor-agent",
            "claude",
            "t",
        ])
        .unwrap();
        assert_eq!(cli.run.supervisor_agent, Some(AgentKind::Claude));
    }

    #[test]
    fn parses_supervisor_args() {
        // --supervisor-arg takes individual string values
        let cli = parse(&[
            "codex",
            "2",
            "--repo",
            ".",
            "--supervisor-arg",
            "custom-val",
            "t",
        ])
        .unwrap();
        assert_eq!(cli.run.supervisor_args, vec!["custom-val"]);
    }

    #[test]
    fn parses_state_dir() {
        let cli = parse(&["codex", "2", "--repo", ".", "--state-dir", "/tmp/sp", "t"]).unwrap();
        assert_eq!(
            cli.run.state_dir.as_deref(),
            Some(std::path::Path::new("/tmp/sp"))
        );
    }

    // ── Config construction ─────────────────────────────────────────

    #[test]
    fn launch_config_sets_supervisor_same_as_worker_when_not_override() {
        let action = parse_action(&["qwen", "3", "--repo", ".", "t"]).unwrap();
        match action {
            CliAction::Run(cfg) => assert_eq!(cfg.supervisor_agent, cfg.worker_agent),
            other => panic!("expected Run, got {:?}", other),
        }
    }

    #[test]
    fn launch_config_canonicalizes_repo_path() {
        let action = parse_action(&["codex", "1", "--repo", ".", "t"]).unwrap();
        match action {
            CliAction::Run(cfg) => {
                assert!(cfg.repo.is_absolute());
            }
            other => panic!("expected Run, got {:?}", other),
        }
    }

    #[test]
    fn launch_config_trims_mission() {
        let action = parse_action(&["codex", "1", "--repo", ".", "  trimmed  "]).unwrap();
        match action {
            CliAction::Run(cfg) => assert_eq!(cfg.mission, "trimmed"),
            other => panic!("expected Run, got {:?}", other),
        }
    }
}
