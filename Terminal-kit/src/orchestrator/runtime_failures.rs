use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RuntimeFailureKind {
    RateLimit,
    Transport,
    ToolContract,
}

pub(super) struct RuntimeFailureSignal {
    pub(super) kind: RuntimeFailureKind,
    pub(super) summary: String,
    pub(super) prompt: String,
    pub(super) supervisor_notice: String,
    pub(super) event_type: SupervisorEventType,
    pub(super) intervention_type: &'static str,
}

impl Orchestrator {
    pub(super) fn handle_runtime_failure_signal(
        &self,
        mission_id: Uuid,
        active_supervisor_id: Uuid,
        session_id: Uuid,
        signal: RuntimeFailureSignal,
        active_sessions: &mut HashMap<Uuid, ActiveSession>,
        pending_supervisor_decisions: &mut HashMap<String, PendingSupervisorDecision>,
    ) -> Result<()> {
        let role;
        {
            let Some(session) = active_sessions.get_mut(&session_id) else {
                return Ok(());
            };
            if session.state.is_terminal() {
                return Ok(());
            }

            let now = Instant::now();
            role = session.record.role;
            session.record.last_summary = Some(signal.summary.clone());
            record_intervention(session, signal.intervention_type, now);
            let _ = send_or_queue_prompt(session, &signal.prompt);
            self.store
                .update_worker_summary(session_id, &signal.summary)?;
            self.store.append_json_event(
                mission_id,
                Some(session_id),
                "runtime_failure_recovery",
                &signal.summary,
                &json!({
                    "kind": runtime_failure_kind_name(signal.kind),
                    "event_type": signal.event_type.as_str(),
                    "intervention_type": signal.intervention_type,
                }),
            )?;
        }
        let supervisor_notice = signal.supervisor_notice.clone();

        if role == SessionRole::Worker {
            let reason = format!(
                "{} Recovery prompt already sent. If the worker does not recover with a real status, force retry_worker, redirect_worker, or fail_worker.",
                supervisor_notice
            );
            let _ = queue_supervisor_decision(
                pending_supervisor_decisions,
                SupervisorDecisionKind::LowConfidenceRecovery,
                session_id,
                &reason,
            );
            self.send_worker_supervisor_notice(
                mission_id,
                active_supervisor_id,
                session_id,
                active_sessions,
                signal.event_type,
                &reason,
            )?;
        } else {
            self.store.append_summary(
                mission_id,
                Some(session_id),
                "supervisor_runtime_recovery",
                &supervisor_notice,
            )?;
        }
        Ok(())
    }
}

pub(super) fn detect_runtime_failure(
    role: SessionRole,
    agent: crate::agent::AgentKind,
    chunk: &str,
) -> Option<RuntimeFailureSignal> {
    let sanitized = crate::protocol::sanitize_output(chunk);
    let lowered = sanitized.to_ascii_lowercase();
    if looks_like_runtime_instruction_echo(&lowered) {
        return None;
    }
    if is_rate_limit_failure(&lowered) {
        return Some(build_runtime_failure_signal(
            role,
            RuntimeFailureKind::RateLimit,
            "provider rate limit hit; recovering in place",
            "A provider rate limit interrupted the session. Do not restart or restate the mission. Resume the exact owned task from the last confirmed work state as soon as the CLI is responsive. After the next successful step, emit one exact Sapphire status with files, commands, and blockers.",
            "provider rate limit hit on the worker runtime.",
        ));
    }
    if is_transport_failure(&lowered, agent) {
        return Some(build_runtime_failure_signal(
            role,
            RuntimeFailureKind::Transport,
            "runtime transport failed; resume from the last confirmed work state",
            "The CLI transport failed unexpectedly. Recover in place. Do not re-read the top-level mission. Continue the current owned task from the last confirmed step and emit one exact Sapphire status after the next successful action.",
            "transport or API disconnect hit the session runtime.",
        ));
    }
    if is_tool_contract_failure(&lowered) {
        return Some(build_runtime_failure_signal(
            role,
            RuntimeFailureKind::ToolContract,
            "invalid tool invocation detected; correct the call and continue",
            "Your last tool invocation failed because the CLI/tool contract was invalid. Correct the arguments, continue the same owned task, and emit one exact Sapphire status after the next successful step. No planning language.",
            "tool invocation contract failure detected in worker output.",
        ));
    }
    None
}

fn build_runtime_failure_signal(
    role: SessionRole,
    kind: RuntimeFailureKind,
    worker_summary: &str,
    worker_prompt: &str,
    supervisor_notice: &str,
) -> RuntimeFailureSignal {
    if role == SessionRole::Supervisor {
        return RuntimeFailureSignal {
            kind,
            summary: format!("Supervisor recovery: {worker_summary}"),
            prompt: "A runtime/API failure interrupted supervision. Resume from the latest live worker state. Do not re-plan. If workers are terminal, produce final synthesis now. Otherwise continue decisive supervision and resolve pending incidents.".to_owned(),
            supervisor_notice: "supervisor runtime failure detected; recovery prompt sent.".to_owned(),
            event_type: SupervisorEventType::Failed,
            intervention_type: "supervisor_runtime_recovery",
        };
    }
    RuntimeFailureSignal {
        kind,
        summary: format!("Worker recovery: {worker_summary}"),
        prompt: worker_prompt.to_owned(),
        supervisor_notice: supervisor_notice.to_owned(),
        event_type: SupervisorEventType::Failed,
        intervention_type: "worker_runtime_recovery",
    }
}

fn is_rate_limit_failure(lowered: &str) -> bool {
    lowered.contains("rate limit exceeded")
        || lowered.contains("rate limit reached")
        || lowered.contains("rate limit was exceeded")
        || lowered.contains("you have exceeded your rate limit")
        || lowered.contains("too many requests")
        || lowered.contains("429 too many requests")
        || lowered.contains("status code: 429")
        || lowered.contains("status=429")
        || lowered.contains("error 429")
        || lowered.contains("[api error: 429")
        || lowered.contains("rate_limited")
        || lowered.contains("quota exceeded")
}

fn is_transport_failure(lowered: &str, agent: crate::agent::AgentKind) -> bool {
    lowered.contains("econnreset")
        || lowered.contains("connection reset")
        || lowered.contains("terminated (cause:")
        || lowered.contains("read econ")
        || lowered.contains("press ctrl+y to retry")
        || lowered.contains("network error")
        || (agent == crate::agent::AgentKind::Qwen && lowered.contains("[api error:"))
}

fn is_tool_contract_failure(lowered: &str) -> bool {
    lowered.contains("must be object")
        || lowered.contains("invalid params")
        || lowered.contains("invalid arguments")
        || lowered.contains("schema validation")
        || lowered.contains("failed to parse tool")
}

fn runtime_failure_kind_name(kind: RuntimeFailureKind) -> &'static str {
    match kind {
        RuntimeFailureKind::RateLimit => "rate_limit",
        RuntimeFailureKind::Transport => "transport",
        RuntimeFailureKind::ToolContract => "tool_contract",
    }
}

fn looks_like_runtime_instruction_echo(lowered: &str) -> bool {
    lowered.contains("if the cli/runtime misbehaves")
        || lowered.contains("if you hit rate limit")
        || lowered.contains("do not restart the mission")
        || lowered.contains("recover in place from the last confirmed work state")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::AgentKind;

    #[test]
    fn ignores_runtime_instruction_echoes() {
        let chunk = "If the CLI/runtime misbehaves:\n- If you hit rate limit, 404/429 provider failure, ECONNRESET, retry UI, or similar transient runtime error, do not restart the mission.";
        assert!(detect_runtime_failure(SessionRole::Worker, AgentKind::Qwen, chunk).is_none());
    }

    #[test]
    fn detects_real_rate_limit_errors() {
        let chunk = "API Error: 429 Too Many Requests";
        let signal = detect_runtime_failure(SessionRole::Worker, AgentKind::Qwen, chunk)
            .expect("expected a rate limit recovery signal");
        assert_eq!(signal.kind, RuntimeFailureKind::RateLimit);
    }
}
