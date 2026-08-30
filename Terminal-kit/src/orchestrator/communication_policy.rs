pub const WATCHDOG_PROTOCOL_REMINDERS: bool = false;
pub const MAIL_TIMEOUT_DIRECT_PROMPTS: bool = false;
pub const MAIL_QUEUE_DIRECT_FALLBACK: bool = false;
pub const SUPERVISOR_HEALTH_PROBES: bool = true;

pub fn supervisor_enforcement_rules() -> &'static str {
    "- Supervisor owns challenge, redirect, retry, and validation prompts. The watchdog should escalate evidence, not spray worker prompts.\n\
- Every worker must report back through Sapphire status updates and teammate mail. Silence is a supervision problem, not a prompt-spam opportunity.\n\
- Demand one initial status after prompt ingestion, then status on material progress, blockers, teammate waits, and completion claims.\n\
- Challenge every completion with files, evidence, tests, or explicit remaining risk. Do not accept narration without proof.\n\
- If a worker drifts, push back once with a narrow instruction. Do not repeat the same prompt. Escalate or reroute decisively.\n\
- If a worker asks whether to continue and the task is still incomplete, answer directly and keep the worker moving.\n\
- Approve cleanup only after every worker is resolved and no further reroute, retry, or proof challenge is needed."
}
