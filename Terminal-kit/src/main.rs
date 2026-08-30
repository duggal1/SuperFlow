mod adapter;
mod agent;
mod cli;
mod git;
mod internal;
mod model;
mod no_supervisor;
mod orchestrator;
mod protocol;
mod runtime;
#[allow(dead_code, unused_imports, unused_variables, unused_mut)]
mod storage;
mod store; // Deprecated — backward compat alias, will be removed
mod templates;
mod tmux;
mod tui;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, CliAction};
use orchestrator::Orchestrator;
use std::fs;
use std::path::Path;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let action = cli.into_action()?;
    init_tracing(&action);

    match action {
        CliAction::Run(mut config) => {
            // ─── Git Pre-Flight Check ──────────────────────────────
            // Ultra-fast check: .git exists + git remote -v has output.
            let git_state = git::check_git_state(&config.repo);
            match git_state {
                git::GitState::Ready { .. } => {
                    // Remote exists — agents will handle commits. Skip prompt.
                }
                git::GitState::Unavailable => {
                    let _ = git::prompt_git_init(&config.repo, &git_state)?;
                }
                git::GitState::NoRemote => {
                    // Has git but no remote — prompt user
                    match git::prompt_git_init(&config.repo, &git_state)? {
                        git::GitInitResult::Initialized { remote_url } => {
                            config.git_remote = Some(remote_url);
                        }
                        git::GitInitResult::Declined => {
                            // User declined — continue without git
                            // Agents will be told there's no remote via their prompt
                        }
                    }
                }
                git::GitState::NotARepo => {
                    // No git repo at all — prompt user
                    match git::prompt_git_init(&config.repo, &git_state)? {
                        git::GitInitResult::Initialized { remote_url } => {
                            config.git_remote = Some(remote_url);
                        }
                        git::GitInitResult::Declined => {
                            // User declined — continue without git
                        }
                    }
                }
            }

            let state_path = config.state_dir.clone();
            let control_status = config.state_dir.join("control/status.txt");
            let use_teamwork = config.tmux
                && tui::run_enabled_for_launch(config.dry_run)
                && tmux::Tmux::is_available();
            let use_tui = config.tui || (tui::run_enabled_for_launch(config.dry_run));

            if use_teamwork && !config.dry_run {
                let session_name = config
                    .tmux_session_name
                    .clone()
                    .expect("teamwork surface session name is always set");
                seed_launch_status(&control_status, "Preparing launch")?;
                let config_clone = config.clone();
                let task = tokio::spawn(async move {
                    let orchestrator = Orchestrator::bootstrap(&config_clone)?;
                    orchestrator.launch(config_clone).await
                });
                let attach = tui::attach_for_repo(config.repo.clone(), chrono::Utc::now());
                let tmux_ready = tui::run_startup_dashboard_until_tmux(
                    state_path.clone(),
                    control_status.clone(),
                    attach.clone(),
                    &session_name,
                    &task,
                )
                .await?;
                // External terminal is opened by the orchestrator inside run_live_mission.
                // Do NOT open it here — that would create a duplicate window.
                let _ = tmux_ready;
                let summary =
                    tui::run_launch_dashboard(state_path, control_status, attach, task).await?;
                println!("{}", summary.render());
            } else if use_tui && !config.dry_run {
                seed_launch_status(&control_status, "Preparing launch")?;
                let config_clone = config.clone();
                let task = tokio::spawn(async move {
                    let orchestrator = Orchestrator::bootstrap(&config_clone)?;
                    orchestrator.launch(config_clone).await
                });
                let attach = tui::attach_for_repo(config.repo.clone(), chrono::Utc::now());
                let summary =
                    tui::run_launch_dashboard(state_path, control_status, attach, task).await?;
                println!("{}", summary.render());
            } else {
                let orchestrator = Orchestrator::bootstrap(&config)?;
                let summary = orchestrator.launch(config).await?;
                println!("{}", summary.render());
            }
        }
        CliAction::NoSupervisorLaunch(config) => {
            let summary = no_supervisor::launch(config).await?;
            println!("{summary}");
        }
        CliAction::Status { repo, state_dir } => {
            let state_dir = state_dir.unwrap_or_else(|| repo.join(".sp"));
            let orchestrator = Orchestrator::open(&state_dir)?;
            println!("{}", orchestrator.render_status()?);
        }
        CliAction::Push { repo } => {
            git::push_current_branch(&repo)?;
        }
        CliAction::Sessions { repo, state_dir } => {
            let state_dir = state_dir.unwrap_or_else(|| repo.join(".sp"));
            let orchestrator = Orchestrator::open(&state_dir)?;
            println!("{}", orchestrator.render_sessions()?);
        }
        CliAction::Resume(config) => {
            let state_dir = config
                .state_dir
                .clone()
                .unwrap_or_else(|| std::env::current_dir().expect("cwd").join(".sp"));
            let control_status = state_dir.join("control/status.txt");
            let use_teamwork =
                config.tmux && tmux::Tmux::is_available() && tui::run_enabled_for_launch(false);
            if use_teamwork {
                let session_name = config
                    .tmux_session_name
                    .clone()
                    .expect("teamwork surface session name is always set");
                let config_clone = config.clone();
                let state_dir_clone = state_dir.clone();
                let task = tokio::spawn(async move {
                    let orchestrator = Orchestrator::open(&state_dir_clone)?;
                    orchestrator.resume(config_clone).await
                });
                let attach = tui::attach_for_mission(config.mission_id);
                let tmux_ready = tui::run_startup_dashboard_until_tmux(
                    state_dir.clone(),
                    control_status.clone(),
                    attach.clone(),
                    &session_name,
                    &task,
                )
                .await?;
                // External terminal is opened by the orchestrator inside run_live_mission.
                let _ = tmux_ready;
                let summary =
                    tui::run_launch_dashboard(state_dir, control_status, attach, task).await?;
                println!("{}", summary.render());
            } else {
                let orchestrator = Orchestrator::open(&state_dir)?;
                let summary = orchestrator.resume(config).await?;
                println!("{}", summary.render());
            }
        }
        CliAction::Replay { mission_id, limit } => {
            let state_dir = std::env::current_dir().expect("cwd").join(".sp");
            let orchestrator = Orchestrator::open(&state_dir)?;
            println!("{}", orchestrator.render_replay(mission_id, limit)?);
        }
        CliAction::Summary { mission_id } => {
            let state_dir = std::env::current_dir().expect("cwd").join(".sp");
            let orchestrator = Orchestrator::open(&state_dir)?;
            println!("{}", orchestrator.render_supervisor_summary(mission_id)?);
        }
        CliAction::Watch {
            mission_id,
            worker,
            limit,
        } => {
            let state_dir = std::env::current_dir().expect("cwd").join(".sp");
            let orchestrator = Orchestrator::open(&state_dir)?;
            println!(
                "{}",
                orchestrator.render_worker_replay(mission_id, &worker, limit)?
            );
        }
    }
    Ok(())
}

fn seed_launch_status(path: &Path, summary: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let lines = [
        "Session: launch-pending".to_owned(),
        format!("Updated: {}", chrono::Utc::now().to_rfc3339()),
        "Workers: 0 | Directives: 0 | Mail: 0 | Validation: 0 | Stalls: 0 | Lease Conflicts: 0 | Protocol Reminders: 0 | Supervisor Health: 0 | Critical Failures: 0 | Crash Loops: 0".to_owned(),
        format!("Supervisor: supervisor-01 [booting] {summary}"),
        String::new(),
        "Blocked: none".to_owned(),
        "Validation Queue: none".to_owned(),
        "Contradictions: none".to_owned(),
        "Mail Pressure: none".to_owned(),
        "Problems: none".to_owned(),
        "Ownership Gaps: none".to_owned(),
        "First-Status Incidents: none".to_owned(),
        "Systemic Incidents: none".to_owned(),
        "Crash Loops: none".to_owned(),
        "Pods: none".to_owned(),
        "Meetings: none".to_owned(),
        String::new(),
        "Supervisors".to_owned(),
        format!(
            "- supervisor-01 [booting] branch=planning agents=0 blocked=0 validating=0 summary=\"{summary}\""
        ),
        String::new(),
        "Workers".to_owned(),
    ];
    fs::write(path, lines.join("\n"))?;
    Ok(())
}

fn init_tracing(action: &CliAction) {
    let interactive = match action {
        CliAction::Run(config) => !config.dry_run && (config.tmux || config.tui),
        CliAction::Resume(config) => config.tmux || config.tui,
        CliAction::NoSupervisorLaunch(_) => true,
        _ => false,
    };
    let default_filter = match action {
        CliAction::Run(_) | CliAction::Resume(_) | CliAction::NoSupervisorLaunch(_) => "warn",
        _ => "warn",
    };
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter));
    let builder = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .compact();
    if interactive {
        builder.with_writer(std::io::sink).init();
    } else {
        builder.init();
    }
}
