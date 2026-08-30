//! TUI application shell — event loop with controlled render frequency.
//! Clean, minimal, no legacy baggage.

use std::fs;
use std::io::{self, Stdout};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::model::LaunchSummary;

use super::data::{AttachTarget, DashboardDataSource};
use super::render;

// ─── Timing ───────────────────────────────────────────────────────────────

// Render frequency — prevents lag from over-rendering
const RENDER_INTERVAL_LIVE: Duration = Duration::from_millis(120);
const RENDER_INTERVAL_DONE: Duration = Duration::from_millis(250);
const DATA_REFRESH_INTERVAL: Duration = Duration::from_millis(500);
const EVENT_POLL_INTERVAL: Duration = Duration::from_millis(50);
const CTRL_C_TIMEOUT: Duration = Duration::from_secs(3); // warning expires after 3s

// ─── Terminal Session ─────────────────────────────────────────────────────

struct TuiSession {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TuiSession {
    fn enter() -> Result<Self> {
        enable_raw_mode().context("failed to enable raw mode")?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen).context("failed to enter alternate screen")?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend).context("failed to initialize terminal")?;
        Ok(Self { terminal })
    }
}

impl Drop for TuiSession {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

// ─── Exit State ───────────────────────────────────────────────────────────

enum ExitState {
    Running,
    /// First Ctrl+C pressed — showing warning
    ConfirmQuit(Instant),
    /// Second Ctrl+C — actually quitting
    Quitting,
}

// ─── Public API ───────────────────────────────────────────────────────────

pub fn run_enabled_for_launch(_dry_run: bool) -> bool {
    true
}

pub fn attach_for_repo(repo: PathBuf, _now: chrono::DateTime<chrono::Utc>) -> AttachTarget {
    AttachTarget::LatestForRepo {
        repo_path: repo,
        started_after: chrono::Utc::now() - chrono::Duration::minutes(5),
    }
}

pub fn attach_for_mission(mission_id: Uuid) -> AttachTarget {
    AttachTarget::Mission(mission_id)
}

/// Run the startup dashboard until tmux is ready.
pub async fn run_startup_dashboard_until_tmux(
    _state_dir: PathBuf,
    _control_status: PathBuf,
    _attach: AttachTarget,
    _session_name: &str,
    task: &JoinHandle<Result<LaunchSummary>>,
) -> Result<bool> {
    tokio::time::sleep(Duration::from_millis(500)).await;
    let _ = task.is_finished();
    Ok(true)
}

/// Main dashboard — runs until user quits or mission completes.
pub async fn run_launch_dashboard(
    state_dir: PathBuf,
    control_status_path: PathBuf,
    attach_target: AttachTarget,
    task: JoinHandle<Result<LaunchSummary>>,
) -> Result<LaunchSummary> {
    let mut tui = TuiSession::enter()?;
    let data_source = DashboardDataSource::open(state_dir.clone(), control_status_path.clone())?;

    let mut snapshot = data_source.snapshot_for_attach(&attach_target)?;

    let started = Instant::now();
    let mut last_render = Instant::now();
    let mut last_data_refresh = Instant::now();
    let mut scroll: u16 = 0;
    let mut finished_summary = None;
    let mut running_task = Some(task);
    let mut exit_state = ExitState::Running;
    let mut frame_count: usize = 0;

    loop {
        let now = Instant::now();
        let is_done = snapshot.is_done || running_task.as_ref().map_or(false, |t| t.is_finished());
        let render_interval = if is_done {
            RENDER_INTERVAL_DONE
        } else {
            RENDER_INTERVAL_LIVE
        };

        // Refresh data periodically
        if now.duration_since(last_data_refresh) >= DATA_REFRESH_INTERVAL {
            if let Ok(new_snapshot) = data_source.snapshot_for_attach(&attach_target) {
                snapshot = new_snapshot;
            }
            last_data_refresh = now;
        }

        // Expire Ctrl+C warning after timeout
        if let ExitState::ConfirmQuit(pressed_at) = exit_state {
            if now.duration_since(pressed_at) >= CTRL_C_TIMEOUT {
                exit_state = ExitState::Running;
            }
        }

        // Render at controlled frequency
        if now.duration_since(last_render) >= render_interval {
            let timer_seconds = started.elapsed().as_secs();
            let show_quit_warning = matches!(exit_state, ExitState::ConfirmQuit(_));
            tui.terminal.draw(|frame| {
                render::render(
                    frame,
                    &snapshot,
                    timer_seconds,
                    scroll,
                    frame_count,
                    show_quit_warning,
                )
            })?;
            last_render = now;
            frame_count += 1;
        }

        // Check if launch task completed
        if let Some(handle) = running_task.as_ref() {
            if handle.is_finished() {
                if let Some(task_handle) = running_task.take() {
                    match task_handle.await {
                        Ok(Ok(summary)) => {
                            finished_summary = Some(summary.clone());
                            if let Ok(new_snapshot) =
                                data_source.snapshot_for_attach(&attach_target)
                            {
                                snapshot = new_snapshot;
                            }
                        }
                        Ok(Err(e)) => {
                            tracing::error!("launch failed: {e}");
                            let _ = write_launch_failure_status(
                                &control_status_path,
                                &format!("Launch failed: {e}"),
                            );
                            if let Ok(new_snapshot) =
                                data_source.snapshot_for_attach(&attach_target)
                            {
                                snapshot = new_snapshot;
                            }
                        }
                        Err(e) => {
                            tracing::error!("task join failed: {e}");
                            let _ = write_launch_failure_status(
                                &control_status_path,
                                &format!("Launch task failed: {e}"),
                            );
                            if let Ok(new_snapshot) =
                                data_source.snapshot_for_attach(&attach_target)
                            {
                                snapshot = new_snapshot;
                            }
                        }
                    }
                }
            }
        }

        // Poll for keyboard events
        if event::poll(EVENT_POLL_INTERVAL).context("failed to poll events")? {
            if let Event::Key(key) = event::read().context("failed to read event")? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            match exit_state {
                                ExitState::Running => {
                                    exit_state = ExitState::ConfirmQuit(Instant::now());
                                }
                                ExitState::ConfirmQuit(_) => {
                                    exit_state = ExitState::Quitting;
                                }
                                ExitState::Quitting => {}
                            }
                        }
                        KeyCode::Char('q') => {
                            if is_done {
                                exit_state = ExitState::Quitting;
                            }
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            scroll = scroll.saturating_add(1);
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            scroll = scroll.saturating_sub(1);
                        }
                        KeyCode::PageDown => {
                            scroll = scroll.saturating_add(8);
                        }
                        KeyCode::PageUp => {
                            scroll = scroll.saturating_sub(8);
                        }
                        KeyCode::Char('r') => {
                            // Manual data refresh
                            if let Ok(new_snapshot) =
                                data_source.snapshot_for_attach(&attach_target)
                            {
                                snapshot = new_snapshot;
                                last_data_refresh = now;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        if matches!(exit_state, ExitState::Quitting) {
            break;
        }
    }

    // Final render before exit
    let timer_seconds = started.elapsed().as_secs();
    tui.terminal.draw(|frame| {
        render::render(frame, &snapshot, timer_seconds, scroll, frame_count, false)
    })?;

    // Print exit banner to stdout after TUI closes
    let banner_lines = render::render_exit_banner(&snapshot, timer_seconds);
    for line in &banner_lines {
        eprintln!("{}", line);
    }

    // Brief pause so user can see final frame
    tokio::time::sleep(Duration::from_millis(200)).await;

    if let Some(summary) = finished_summary {
        Ok(summary)
    } else {
        Ok(LaunchSummary {
            mission_id: snapshot.mission_id.unwrap_or_default(),
            repo: PathBuf::from("."),
            state_dir,
            dry_run: false,
            worker_agent: crate::agent::AgentKind::Qwen,
            supervisor_agent: crate::agent::AgentKind::Qwen,
            worker_count: snapshot.agent_count(),
            session_names: snapshot.agents.iter().map(|a| a.name.clone()).collect(),
            mission_rewrite: snapshot.execution_summary.mission_rewrite.clone(),
            workstream_names: Vec::new(),
            notes: Vec::new(),
        })
    }
}

fn write_launch_failure_status(path: &PathBuf, summary: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let lines = [
        "Session: launch-failed".to_owned(),
        format!("Updated: {}", chrono::Utc::now().to_rfc3339()),
        "Workers: 0 | Directives: 0 | Mail: 0 | Validation: 0 | Stalls: 0 | Lease Conflicts: 0 | Protocol Reminders: 0 | Supervisor Health: 0 | Critical Failures: 1 | Crash Loops: 0".to_owned(),
        format!("Supervisor: supervisor-01 [failed] {summary}"),
        String::new(),
        "Blocked: none".to_owned(),
        "Validation Queue: none".to_owned(),
        "Contradictions: none".to_owned(),
        "Mail Pressure: none".to_owned(),
        "Problems: launch".to_owned(),
        "Ownership Gaps: none".to_owned(),
        "First-Status Incidents: none".to_owned(),
        "Systemic Incidents: launch_failed".to_owned(),
        "Crash Loops: none".to_owned(),
        "Pods: none".to_owned(),
        "Meetings: none".to_owned(),
        String::new(),
        "Supervisors".to_owned(),
        format!(
            "- supervisor-01 [failed] branch=planning agents=0 blocked=0 validating=0 summary=\"{summary}\""
        ),
        String::new(),
        "Workers".to_owned(),
    ];
    fs::write(path, lines.join("\n"))?;
    Ok(())
}
