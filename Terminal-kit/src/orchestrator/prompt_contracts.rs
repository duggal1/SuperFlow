use std::path::Path;

use crate::model::WorkerPacket;

use super::communication_policy;
use super::coordination;

pub fn render_supervisor_bootstrap(
    base_prompt: String,
    agents_path: &Path,
    agents_existed: bool,
    requested_workers: usize,
) -> String {
    format!(
        "{base_prompt}\n\n---\n\n# AGENTS.md PROTOCOL\n\n- AGENTS path: {path}\n- AGENTS status at session start: {status}\n- Total worker terminals requested: {requested_workers}\n- Worker packet count must equal {requested_workers}. Do not invent extra workers, stewards, alternates, or helper terminals.\n- Every newly launched worker must read AGENTS.md on first initialization only, before real work.\n- AGENTS.md is repository guidance, not an implicit reserved worker role.\n- If AGENTS.md must be created or refreshed, use Sapphire's embedded `agents.md-instructions.md` source. Do not dump that source back into the session unless the refresh is actually required.\n\n# SUPERVISOR TEAM RULES\n\n- Git push is operator-only. Workers and supervisors must never run `git push`. The human push path is `sp push`.\n- Workers and supervisors must never run `git restore` or `git reset`.\n- Treat a dirty git tree as normal multi-agent reality. Multiple workers commit in parallel. Do not trigger cleanup theater because `git status` is noisy.\n- If a worker reports a dirty tree, reroute it back to owned scope unless `git status --porcelain` itself fails.\n- Prefer direct worker-to-worker coordination over supervisor chatter when a peer can answer faster.\n- Demand concrete `SAPPHIRE_MAIL` threads for dependencies, reviews, contract questions, handoffs, and unblock requests.\n- Do not classify active streaming workers as stalled based on guesswork. Prefer fresh status files, recent output, and real teammate mail evidence.\n- When workers report overlap, preserve newer teammate work, settle ownership explicitly, and forbid cleanup theater. Resolve the collision; do not let both workers rewrite the same file blind.\n- Approve cleanup only after every worker is resolved and no more reroute, retry, or proof challenge is needed.\n{enforcement_rules}\n",
        path = agents_path.display(),
        status = if agents_existed {
            "already present"
        } else {
            "created before worker launch"
        },
        enforcement_rules = communication_policy::supervisor_enforcement_rules(),
    )
}

pub fn render_worker_bootstrap(
    base_prompt: String,
    agents_path: &Path,
    state_dir: &Path,
    packet: &WorkerPacket,
    _git_remote: Option<&str>,
    memory_block: Option<&str>,
) -> String {
    let display_name = &packet.display_name;
    let prompt_path = state_dir.join("prompts").join(format!("{display_name}.md"));
    let status_path = state_dir
        .join("workers")
        .join(display_name)
        .join("status.json");
    let memory_path = state_dir
        .join("workers")
        .join(display_name)
        .join("memory.json");
    let preferred_counterparts = coordination::preferred_counterparts(&packet.role_type).join(", ");

    // Inject persistent memory block if available — this is what makes agents
    // remember what they did last session, last mission. No amnesia on reboot.
    let memory_section = memory_block.map(|m| format!("{m}\n\n")).unwrap_or_default();
    let boot_contract = format!(
        "Execution boot order:\n\
1. Read the assignment file once.\n\
2. Write the first real status immediately to {status_path} before repo exploration.\n\
3. Then start execution inside owned scope.\n\
\n\
First status requirements:\n\
- It must describe the true current state, not a plan.\n\
- Include exact next action.\n\
- Include touched files only if you already touched them; otherwise use an empty list.\n\
- Include commands only if already run; otherwise use an empty list.\n\
- If the status file write fails, emit one raw SAPPHIRE_STATUS line immediately.\n\
\n\
Transient failure recovery:\n\
- If the CLI hits rate limit, transport disconnect, ECONNRESET, retry UI, or similar provider/runtime failure, do NOT restart the mission and do NOT re-ask for the task.\n\
- Recover in place from the last confirmed work state.\n\
- After recovery, emit one exact status with files, commands, blockers, and next action.\n\
- If a tool call fails because your arguments are invalid, correct the call and continue the same owned task.\n\
\n\
Anti-drift rules:\n\
- Do not restate the assignment file.\n\
- Do not ask for the top-level mission again.\n\
- Do not wander outside owned scope without a real dependency.\n\
- If blocked by another worker, send Sapphire mail instead of broad narration.",
        status_path = status_path.display(),
    );

    format!(
        "File paths for this worker:\n- Role assignment: {prompt_path}\n- Status file: {status_path}\n- Memory file: {memory_path}\n- AGENTS.md: {agents}\n\nPreferred coordination lanes: {preferred_counterparts}.\n\n{boot_contract}\n\n{memory_section}{base_prompt}",
        agents = agents_path.display(),
        prompt_path = prompt_path.display(),
        status_path = status_path.display(),
        memory_path = memory_path.display(),
        preferred_counterparts = preferred_counterparts,
        boot_contract = boot_contract,
    )
}
