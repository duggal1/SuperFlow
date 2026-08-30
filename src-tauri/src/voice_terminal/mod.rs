//! Voice-driven terminal orchestration.
//!
//! Both command paths launch exactly one terminal: the supervisor. Standard
//! transcription uses the deterministic grammar. Super Whisper uses an LLM
//! only to return strict `{terminal, terminal_number}` JSON. The supervisor
//! receives the original mission and owns all worker prompts, tmux panes,
//! follow-ups, validation, and finalization.

mod ai;
mod ghostty;
mod grammar;
mod prompts;
mod tmux;

use crate::settings::AppSettings;
use grammar::AgentKind;
use log::{error, info};
use tmux::Tmux;

const SHELL_SETTLE: std::time::Duration = std::time::Duration::from_millis(400);
const AGENT_BOOT_WAIT: std::time::Duration = std::time::Duration::from_millis(3_200);
const NUDGE_GAP: std::time::Duration = std::time::Duration::from_millis(500);

fn launch_line(agent: AgentKind) -> String {
    format!(
        "export PATH=\"$HOME/.local/bin:$HOME/.opencode/bin:$HOME/.bun/bin:$PATH\"; exec {}",
        agent.executable()
    )
}

#[derive(Debug, Clone)]
struct SupervisorRequest {
    agent: AgentKind,
    workers: usize,
    mission: String,
}

/// Standard transcription path. The parser behavior is unchanged; only the
/// launch architecture is different. Rust dispatches one supervisor instead
/// of pre-creating and pre-prompting the worker team.
pub fn try_handle_voice_command(transcription: &str) -> bool {
    let Some(command) = grammar::parse(transcription) else {
        return false;
    };

    info!(
        target: "voice_terminal",
        "Voice supervisor request: agent={}, workers={}, mission={:?}",
        command.agent.display_name(),
        command.workers,
        command.mission
    );

    let request = SupervisorRequest {
        agent: command.agent,
        workers: command.workers,
        mission: command.mission,
    };
    let _ = tauri::async_runtime::spawn_blocking(move || launch_supervisor(request));
    true
}

/// Super Whisper path. The interpretation LLM returns agent/count JSON only;
/// the original instruction goes untouched to the supervisor as its mission.
pub async fn try_handle_ai_command(instruction: &str, settings: &AppSettings) -> bool {
    let Some(plan) = ai::interpret(instruction, settings).await else {
        return false;
    };

    info!(
        target: "voice_terminal",
        "AI supervisor request: agent={}, workers={}",
        plan.agent.display_name(),
        plan.workers
    );

    let request = SupervisorRequest {
        agent: plan.agent,
        workers: plan.workers,
        mission: prompts::single_line(instruction),
    };
    let _ = tauri::async_runtime::spawn_blocking(move || launch_supervisor(request)).await;
    true
}

/// The only Rust-side launch. One tmux session, one pane, one CLI, one prompt.
/// The supervisor creates and owns every worker surface from inside tmux.
fn launch_supervisor(request: SupervisorRequest) {
    let Some(tmux) = Tmux::discover() else {
        error!(target: "voice_terminal", "tmux not found; install it (brew install tmux) to use voice terminal commands");
        return;
    };
    let Ok(work_dir) = std::env::var("HOME") else {
        error!(target: "voice_terminal", "HOME is not set; cannot pick a working directory");
        return;
    };

    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let session = format!("sf-{suffix}-supervisor");
    if let Err(error) = tmux.create_session(&session, &work_dir) {
        error!(target: "voice_terminal", "Failed to create supervisor session {session}: {error}");
        return;
    }
    let Some(pane) = tmux.pane_ids(&session).into_iter().next() else {
        error!(target: "voice_terminal", "Supervisor session {session} has no pane");
        return;
    };

    std::thread::sleep(SHELL_SETTLE);
    if let Err(error) = tmux.send_keys_literal(&pane, &launch_line(request.agent)) {
        error!(target: "voice_terminal", "Failed to launch supervisor CLI in {pane}: {error}");
        return;
    }
    if let Err(error) = tmux.send_enter(&pane) {
        error!(target: "voice_terminal", "Failed to submit supervisor CLI in {pane}: {error}");
        return;
    }

    if let Err(error) = ghostty::open_sessions(tmux.binary(), std::slice::from_ref(&session)) {
        error!(target: "voice_terminal", "Could not attach supervisor terminal (session remains detached): {error}");
    }

    std::thread::sleep(AGENT_BOOT_WAIT);
    let _ = tmux.send_enter(&pane);
    std::thread::sleep(NUDGE_GAP);

    let prompt =
        prompts::supervisor_prompt(&request.mission, request.agent, request.workers, &session);
    if let Err(error) = tmux.paste_prompt(&pane, &prompt) {
        error!(target: "voice_terminal", "Failed to paste supervisor prompt into {pane}: {error}");
        return;
    }

    info!(
        target: "voice_terminal",
        "Supervisor launch complete: session={}, agent={}, requested_workers={}",
        session,
        request.agent.display_name(),
        request.workers
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_command_preserves_agent_count_and_mission_for_supervisor() {
        let command = grammar::parse("open 4 codex terminals fix auth and run tests")
            .expect("terminal command");
        let request = SupervisorRequest {
            agent: command.agent,
            workers: command.workers,
            mission: command.mission,
        };
        assert_eq!(request.agent, AgentKind::Codex);
        assert_eq!(request.workers, 4);
        assert_eq!(request.mission, "fix auth and run tests");
    }

    #[test]
    fn launch_line_starts_only_the_selected_supervisor_cli() {
        let line = launch_line(AgentKind::Claude);
        assert!(line.ends_with("exec claude"));
        assert!(!line.contains("tmux split-window"));
        assert!(!line.contains("prompt"));
    }
}
