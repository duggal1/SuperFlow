//! JSON-only interpretation for Super Whisper terminal commands.
//!
//! This LLM has one narrow job: identify the requested agent CLI and worker
//! count. It never creates worker prompts. The supervisor receives the
//! original transcript and owns decomposition, prompting, and orchestration.

use super::grammar::{self, AgentKind};
use crate::ai_cleanup;
use crate::settings::AppSettings;
use log::{info, warn};
use serde::Deserialize;

const MAX_WORKERS: usize = super::grammar::MAX_WORKERS;

const INTERPRET_SYSTEM_PROMPT: &str = r#"Classify a spoken terminal-agent launch request.

Allowed terminal values: claude, codex, opencode, cline, kilo.

For a launch request, output exactly this JSON shape:
{"terminal":"codex","terminal_number":4}

If it is not a launch request, output exactly:
{"terminal":null,"terminal_number":0}

Rules:
- Output JSON only. No prose, markdown, or extra keys.
- Normalize spoken names and likely transcription errors to an allowed terminal value.
- terminal_number is the requested number of worker agents; default to 1 and never exceed 16.
- Do not create, rewrite, summarize, or include any prompt or worker assignment."#;

#[derive(Debug, Clone)]
pub struct AiPlan {
    pub agent: AgentKind,
    pub workers: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPlan {
    terminal: Option<String>,
    terminal_number: usize,
}

fn validate_plan(raw: &str) -> Option<AiPlan> {
    let plan: RawPlan = match serde_json::from_str(raw.trim()) {
        Ok(plan) => plan,
        Err(error) => {
            warn!(target: "voice_terminal", "Rejected AI plan (strict JSON required): {error}");
            return None;
        }
    };

    if plan.terminal.is_none() && plan.terminal_number == 0 {
        return None;
    }

    let Some(terminal) = plan.terminal else {
        warn!(target: "voice_terminal", "Rejected AI plan (missing terminal)");
        return None;
    };
    let Some(agent) = AgentKind::from_json_id(terminal.trim().to_lowercase().as_str()) else {
        warn!(target: "voice_terminal", "Rejected AI plan (unsupported agent): {terminal:?}");
        return None;
    };
    if plan.terminal_number == 0 || plan.terminal_number > MAX_WORKERS {
        warn!(
            target: "voice_terminal",
            "Rejected AI plan (invalid count): {}",
            plan.terminal_number
        );
        return None;
    }

    Some(AiPlan {
        agent,
        workers: plan.terminal_number,
    })
}

pub async fn interpret(instruction: &str, settings: &AppSettings) -> Option<AiPlan> {
    if !grammar::mentions_agent(instruction) {
        return None;
    }

    let raw =
        ai_cleanup::generate_with_system_prompt(INTERPRET_SYSTEM_PROMPT, instruction, settings)
            .await
            .inspect_err(|error| {
                warn!(target: "voice_terminal", "AI interpretation failed: {error}");
            })
            .ok()?;
    let plan = validate_plan(&raw)?;
    info!(
        target: "voice_terminal",
        "AI plan: agent={}, workers={}",
        plan.agent.display_name(),
        plan.workers
    );
    Some(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_exact_json_without_prompts() {
        let plan =
            validate_plan(r#"{"terminal":"claude","terminal_number":4}"#).expect("valid plan");
        assert_eq!(plan.agent, AgentKind::Claude);
        assert_eq!(plan.workers, 4);
    }

    #[test]
    fn non_command_json_is_not_a_plan() {
        assert!(validate_plan(r#"{"terminal":null,"terminal_number":0}"#).is_none());
    }

    #[test]
    fn rejects_prompt_fields_and_every_unknown_key() {
        assert!(
            validate_plan(r#"{"terminal":"codex","terminal_number":1,"prompt":"fix it"}"#)
                .is_none()
        );
        assert!(
            validate_plan(r#"{"terminal":"codex","terminal_number":1,"prompts":["fix it"]}"#)
                .is_none()
        );
        assert!(
            validate_plan(r#"{"terminal":"codex","terminal_number":1,"extra":true}"#).is_none()
        );
    }

    #[test]
    fn rejects_non_json_and_markdown_fences() {
        assert!(validate_plan("none").is_none());
        assert!(validate_plan("opening codex").is_none());
        assert!(
            validate_plan("```json\n{\"terminal\":\"codex\",\"terminal_number\":1}\n```").is_none()
        );
    }

    #[test]
    fn rejects_unsupported_agents_and_invalid_counts() {
        assert!(validate_plan(r#"{"terminal":"cursor","terminal_number":1}"#).is_none());
        assert!(validate_plan(r#"{"terminal":"codex","terminal_number":0}"#).is_none());
        let too_many = format!(
            r#"{{"terminal":"codex","terminal_number":{}}}"#,
            MAX_WORKERS + 1
        );
        assert!(validate_plan(&too_many).is_none());
    }
}
