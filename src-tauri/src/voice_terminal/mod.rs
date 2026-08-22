//! Voice-driven terminal orchestration.
//!
//! A finalized transcript like "please open four claude code terminals to fix
//! the auth bug" launches a local tmux-backed agent team:
//!
//! - N worker panes (batches of up to 8 per visible page/grid),
//! - one extra BRAIN supervisor pane when a count was spoken (the last one),
//! - each pane boots the chosen agent CLI (`claude` | `codex` | `opencode`),
//! - Apple Intelligence (on-device) splits the spoken mission into one brief
//!   per worker; the brain receives the full mission + roster,
//! - sessions attach to Ghostty tabs (Terminal.app fallback).
//!
//! Mechanism ported from the proven Terminal-kit (`sp`) tmux surface: real
//! tmux splits instead of synthetic Cmd+D keystrokes, and buffer-based
//! prompt pastes instead of character typing.

mod ghostty;
mod grammar;
mod prompts;
mod tmux;

use grammar::{AgentKind, ParsedCommand};
use log::{error, info};
use tmux::Tmux;

/// Panes per visible page/grid before a new tab is used.
const PANES_PER_PAGE: usize = 8;
/// Settle time after a tmux session exists before typing into its panes.
const SHELL_SETTLE: std::time::Duration = std::time::Duration::from_millis(400);
/// Stagger between typing launch lines into consecutive panes.
const TYPE_STAGGER: std::time::Duration = std::time::Duration::from_millis(150);
/// Wait for the agent CLI TUI to boot before nudging/pasting.
const AGENT_BOOT_WAIT: std::time::Duration = std::time::Duration::from_millis(2200);
/// Gap between the Enter nudge and the prompt paste.
const NUDGE_GAP: std::time::Duration = std::time::Duration::from_millis(500);
/// Stagger between pasting prompts into consecutive panes.
const PASTE_STAGGER: std::time::Duration = std::time::Duration::from_millis(200);

/// The shell line typed into each pane: extend PATH for GUI-installed CLIs,
/// then exec the agent through a login shell so the user's environment
/// (homebrew, nvm, etc.) resolves exactly like their own terminal.
fn launch_line(agent: AgentKind) -> String {
    format!(
        "export PATH=\"$HOME/.local/bin:$HOME/.opencode/bin:$HOME/.bun/bin:$PATH\"; exec {}",
        agent.executable()
    )
}

struct TeamLayout {
    /// (session name, pane ids) per worker page, in worker order.
    pages: Vec<(String, Vec<String>)>,
    brain: Option<(String, String)>,
}

/// Entry point called with every finalized transcript. Returns `true` when
/// the transcript was a terminal command and the launch was dispatched —
/// the caller must then skip dictation paste/history for this utterance.
pub fn try_handle_voice_command(transcription: &str) -> bool {
    let Some(cmd) = grammar::parse(transcription) else {
        return false;
    };

    info!(
        target: "voice_terminal",
        "Voice command: agent={}, workers={}, brain={}, mission={:?}",
        cmd.agent.display_name(),
        cmd.workers,
        cmd.brain,
        cmd.mission
    );

    tauri::async_runtime::spawn(async move {
        let worker_prompts = prompts::split_worker_prompts(&cmd.mission, cmd.workers).await;
        let _ = tauri::async_runtime::spawn_blocking(move || launch(cmd, worker_prompts)).await;
    });

    true
}

/// Blocking orchestration — runs on a dedicated thread.
fn launch(cmd: ParsedCommand, worker_prompts: Vec<String>) {
    let Some(tmux) = Tmux::discover() else {
        error!(target: "voice_terminal", "tmux not found; install it (brew install tmux) to use voice terminal commands");
        return;
    };
    let Ok(work_dir) = std::env::var("HOME") else {
        error!(target: "voice_terminal", "HOME is not set; cannot pick a working directory");
        return;
    };

    let Some(layout) = build_layout(&tmux, &cmd, &work_dir) else {
        return;
    };

    // Boot the agent CLIs in every pane (workers first, brain last).
    let mut all_panes: Vec<String> = Vec::new();
    for (_session, panes) in &layout.pages {
        all_panes.extend(panes.iter().cloned());
    }
    let brain_pane = layout.brain.as_ref().map(|(_, pane)| pane.clone());
    type_launch_lines(&tmux, &all_panes, cmd.agent, brain_pane.as_deref());

    // Make the team visible: worker pages first, brain tab last.
    let mut sessions: Vec<String> = layout.pages.iter().map(|(s, _)| s.clone()).collect();
    if let Some((brain_session, _)) = &layout.brain {
        sessions.push(brain_session.clone());
    }
    if let Err(e) = ghostty::open_sessions(tmux.binary(), &sessions) {
        error!(target: "voice_terminal", "Could not attach terminal tabs (sessions still exist detached): {e}");
    }

    // The agents have booted during the tab dance. Nudge each readline,
    // then paste the briefs.
    std::thread::sleep(AGENT_BOOT_WAIT);
    for pane in all_panes.iter().chain(brain_pane.iter()) {
        let _ = tmux.send_enter(pane);
        std::thread::sleep(std::time::Duration::from_millis(60));
    }
    std::thread::sleep(NUDGE_GAP);

    let mut worker_index = 0;
    for (_, panes) in &layout.pages {
        for pane in panes {
            if let Some(prompt) = worker_prompts.get(worker_index) {
                if !prompt.trim().is_empty() {
                    if let Err(e) = tmux.paste_prompt(pane, prompt) {
                        error!(target: "voice_terminal", "Failed to paste prompt into {pane}: {e}");
                    }
                    std::thread::sleep(PASTE_STAGGER);
                }
            }
            worker_index += 1;
        }
    }

    if let Some((_, brain_pane)) = &layout.brain {
        // Roster labels reflect the global worker index across pages.
        let mut roster = Vec::new();
        let mut n = 0;
        for (_, panes) in &layout.pages {
            for pane in panes {
                n += 1;
                roster.push(format!("Worker-{n} at tmux pane {pane}"));
            }
        }
        let brain_text = prompts::brain_prompt(&cmd.mission, &worker_prompts, &roster);
        if let Err(e) = tmux.paste_prompt(brain_pane, &brain_text) {
            error!(target: "voice_terminal", "Failed to paste brain prompt: {e}");
        }
    }

    info!(target: "voice_terminal", "Launch complete: {} worker(s){}, agent={}",
        cmd.workers,
        if cmd.brain { " + brain" } else { "" },
        cmd.agent.display_name()
    );
}

/// Create the tmux sessions, split panes into grids, and return the pane map.
fn build_layout(tmux: &Tmux, cmd: &ParsedCommand, work_dir: &str) -> Option<TeamLayout> {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();

    let mut pages = Vec::new();
    let mut remaining = cmd.workers;
    let mut page_index = 0;
    while remaining > 0 {
        let batch = remaining.min(PANES_PER_PAGE);
        remaining -= batch;
        let session = format!("sf-{suffix}-w{page_index}");
        if let Err(e) = tmux.create_session(&session, work_dir) {
            error!(target: "voice_terminal", "Failed to create tmux session {session}: {e}");
            return None;
        }
        // Split all panes first, then fix the geometry once (sp pattern).
        let first_pane = tmux.pane_ids(&session).into_iter().next();
        let Some(first_pane) = first_pane else {
            error!(target: "voice_terminal", "Session {session} has no panes");
            return None;
        };
        let mut panes = vec![first_pane.clone()];
        for _ in 1..batch {
            match tmux.split_horizontal(&panes.last().unwrap()) {
                Ok(id) => panes.push(id),
                Err(e) => error!(target: "voice_terminal", "Split failed in {session}: {e}"),
            }
        }
        tmux.apply_grid_layout(&session, panes.len());
        pages.push((session, panes));
        page_index += 1;
    }

    let brain = if cmd.brain {
        let session = format!("sf-{suffix}-brain");
        match tmux.create_session(&session, work_dir) {
            Ok(()) => tmux
                .pane_ids(&session)
                .into_iter()
                .next()
                .map(|pane| (session, pane)),
            Err(e) => {
                error!(target: "voice_terminal", "Failed to create brain session: {e}");
                None
            }
        }
    } else {
        None
    };

    std::thread::sleep(SHELL_SETTLE);
    Some(TeamLayout { pages, brain })
}

/// Type the agent launch command into every pane (workers, then brain).
fn type_launch_lines(tmux: &Tmux, panes: &[String], agent: AgentKind, brain_pane: Option<&str>) {
    let line = launch_line(agent);
    for pane in panes
        .iter()
        .map(String::as_str)
        .chain(brain_pane.into_iter())
    {
        if let Err(e) = tmux.send_keys_literal(pane, &line) {
            error!(target: "voice_terminal", "Failed to type launch line into {pane}: {e}");
            continue;
        }
        let _ = tmux.send_enter(pane);
        std::thread::sleep(TYPE_STAGGER);
    }
}
