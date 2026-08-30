use std::path::{Path, PathBuf};

use crate::agent::AgentKind;
use crate::model::WorkerPacket;

pub fn prompt_file_path(state_dir: &Path, session_name: &str) -> PathBuf {
    state_dir.join("prompts").join(format!("{session_name}.md"))
}

pub fn worker_terminal_prompt(
    state_dir: &Path,
    prompt_path: &Path,
    packet: &WorkerPacket,
) -> String {
    let status_path = state_dir
        .join("workers")
        .join(&packet.display_name)
        .join("status.json");

    let lines = vec![
        format!("You are {} ({}) for this mission.", packet.display_name, packet.role),
        format!("Assignment source of truth: {}", prompt_path.display()),
        format!("Owned scope: {}", packet.owned_scope),
        format!("Current task: {}", packet.explicit_task),
        "- Read that file once now. It contains your full role description, task, evidence rules, and boundaries.".to_owned(),
        "- Do not restate that file back to me.".to_owned(),
        "- Do not ask for the top-level mission again if the file exists.".to_owned(),
        "- Work inside your owned scope unless a real dependency requires coordination.".to_owned(),
        String::new(),
        "Critical team awareness:".to_owned(),
        "- You are NOT alone. Teammates (Engineers, Designers, Security, Architects) edit the same repo concurrently.".to_owned(),
        "- If you see file changes you didn't make, that is NORMAL and EXPECTED. Do NOT panic, revert, or delete.".to_owned(),
        "- Iterate over teammate changes. Merge useful work. Adapt your implementation on top.".to_owned(),
        "- Never run `git push`, `git restore`, or `git reset`. If using worktrees, stay inside your own tree.".to_owned(),
        String::new(),
        "Mandatory first action:".to_owned(),
        format!("- Write your first real status file at: {}", status_path.display()),
        "- Create the file if it does not exist. Do not wait for Sapphire to prefill it.".to_owned(),
        "- Only if that write fails, print one raw `SAPPHIRE_STATUS` line.".to_owned(),
        "- Do not inspect repo files before this first status write.".to_owned(),
        "- Do not replace the first status with narration about what you plan to do.".to_owned(),
        "- The first status must report your true current state, your exact next action, files=[], commands=[], risks=[] if nothing exists yet, and overlap=null unless a real collision exists.".to_owned(),
        String::new(),
        "If the CLI/runtime misbehaves:".to_owned(),
        "- If you hit rate limit, 404/429 provider failure, ECONNRESET, retry UI, or similar transient runtime error, do not restart the mission and do not re-ask for the assignment.".to_owned(),
        "- Recover in place from the last confirmed work state.".to_owned(),
        "- After recovery, emit one exact status with real files, commands, blockers, and next action.".to_owned(),
        "- If a tool call fails because your arguments were invalid, correct the call and continue the same task.".to_owned(),
        String::new(),
        "Begin now.".to_owned(),
    ];
    lines.join("\n")
}

pub fn worker_resume_prompt(
    state_dir: &Path,
    packet: &WorkerPacket,
    restart_count: usize,
) -> String {
    let prompt_path = state_dir
        .join("prompts")
        .join(format!("{}.md", packet.display_name));
    let status_path = state_dir
        .join("workers")
        .join(&packet.display_name)
        .join("status.json");

    let lines = vec![
        format!(
            "Resume {} ({}) after restart {}.",
            packet.display_name, packet.role, restart_count
        ),
        format!("Assignment source of truth: {}", prompt_path.display()),
        format!("Status file: {}", status_path.display()),
        format!("Owned scope: {}", packet.owned_scope),
        format!("Current task: {}", packet.explicit_task),
        "- Continue the same owned assignment. Do not re-ingest or restate the full mission.".to_owned(),
        "- If your first real status was never written, write it now. Otherwise continue execution and report concrete progress only.".to_owned(),
        "- Do not duplicate prior narration. Continue from the last real execution state.".to_owned(),
        "- If restart followed a transient CLI/runtime failure, recover in place. Do not ask for the top-level mission again.".to_owned(),
        "- After the first successful post-restart action, emit one exact status with real files, commands, blockers, and next action.".to_owned(),
    ];
    lines.join("\n")
}

pub fn supervisor_loader_prompt(
    agent: AgentKind,
    prompt_path: &Path,
    display_name: &str,
) -> String {
    let prompt_ref = prompt_reference(agent, prompt_path);
    format!(
        "You are {display_name}. Read and fully ingest this supervisor brief now:\n\
{prompt_ref}\n\
\n\
Rules:\n\
- That file is the source of truth for mission supervision.\n\
- Ingest it once. Do not paraphrase the brief back to me.\n\
- Do not spend turns narrating the brief.\n\
- After reading it, start supervising immediately with the next concrete worker-facing action."
    )
}

pub fn supervisor_planning_loader_prompt(
    agent: AgentKind,
    prompt_path: &Path,
    display_name: &str,
) -> String {
    let prompt_ref = prompt_reference(agent, prompt_path);
    format!(
        "You are {display_name} in planning-only mode.\n\
Read this planning brief now:\n\
{prompt_ref}\n\
\n\
Rules:\n\
- That file is the only source of truth for planning.\n\
- Read it once.\n\
- Do NOT inspect the repository.\n\
- Do NOT read code files.\n\
- Do NOT run tools.\n\
- Do NOT spawn sub-agents.\n\
- Do NOT explore.\n\
- Return exactly one wrapped plan JSON block and stop."
    )
}

fn prompt_reference(agent: AgentKind, prompt_path: &Path) -> String {
    let absolute = prompt_path.display().to_string();
    match agent {
        AgentKind::Qwen => format!("@{absolute}\n{absolute}"),
        _ => absolute,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn packet() -> WorkerPacket {
        WorkerPacket {
            role_type: "software-engineer".to_owned(),
            display_name: "Engineer-1".to_owned(),
            worker_id: "Engineer-1".to_owned(),
            role: "Software Engineer".to_owned(),
            starting_angle: "Start at src/main.rs".to_owned(),
            owned_scope: "src/main.rs".to_owned(),
            explicit_task: "Implement the feature".to_owned(),
            out_of_scope: "tests/".to_owned(),
            definition_of_done: vec!["feature works".to_owned()],
            required_evidence: vec!["cargo test".to_owned()],
            blocker_protocol: "mail supervisor".to_owned(),
            conflict_warning: "avoid overlap".to_owned(),
            communication_rules: vec!["mail QA-1 on blocker".to_owned()],
            validation_standard: vec!["tests pass".to_owned()],
            expected_output_format: vec!["status".to_owned()],
        }
    }

    #[test]
    fn worker_terminal_prompt_requires_first_status_and_recovery() {
        let rendered = worker_terminal_prompt(
            Path::new(".sp"),
            Path::new(".sp/prompts/Engineer-1.md"),
            &packet(),
        );
        assert!(rendered.contains("Mandatory first action:"));
        assert!(rendered.contains("Do not inspect repo files before this first status write."));
        assert!(rendered.contains("If the CLI/runtime misbehaves:"));
        assert!(rendered.contains("ECONNRESET"));
    }

    #[test]
    fn worker_resume_prompt_keeps_same_assignment_after_restart() {
        let rendered = worker_resume_prompt(Path::new(".sp"), &packet(), 2);
        assert!(rendered.contains("Continue the same owned assignment."));
        assert!(rendered.contains("recover in place"));
    }

    #[test]
    fn supervisor_planning_loader_forbids_tools_and_repo_reads() {
        let rendered = supervisor_planning_loader_prompt(
            AgentKind::Qwen,
            Path::new(".sp/prompts/__supervisor_plan__.md"),
            "supervisor-01",
        );
        assert!(rendered.contains("planning-only mode"));
        assert!(rendered.contains("Do NOT inspect the repository."));
        assert!(rendered.contains("Do NOT run tools."));
        assert!(rendered.contains("Do NOT spawn sub-agents."));
    }
}
