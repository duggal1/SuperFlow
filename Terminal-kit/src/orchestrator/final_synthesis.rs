use super::*;

const FINAL_SYNTHESIS_WAKE_AFTER: Duration = Duration::from_secs(20);
const FINAL_SYNTHESIS_ESCALATE_AFTER: Duration = Duration::from_secs(45);
const FINAL_SYNTHESIS_FORCE_AFTER: Duration = Duration::from_secs(90);

impl Orchestrator {
    pub(super) fn drive_final_synthesis(
        &self,
        mission_id: Uuid,
        active_supervisor_id: Uuid,
        active_sessions: &mut HashMap<Uuid, ActiveSession>,
        final_synthesis_requested: &mut bool,
        final_synthesis_requested_at: &mut Option<Instant>,
        final_synthesis_wakeup_sent: &mut bool,
        final_synthesis_escalated: &mut bool,
    ) -> Result<()> {
        let workers_terminal = finalization::workers_are_terminal(active_sessions);
        if !workers_terminal {
            return Ok(());
        }

        let now = Instant::now();
        if !*final_synthesis_requested {
            if let Some(supervisor) = active_sessions.get_mut(&active_supervisor_id)
                && !supervisor.state.is_terminal()
            {
                let adapter = adapter_for(supervisor.record.agent);
                let _ = send_or_queue_prompt(supervisor, &adapter.build_final_summary_prompt());
                let summary = "Workers terminal; final synthesis requested.".to_owned();
                supervisor.record.last_summary = Some(summary.clone());
                self.store
                    .update_worker_summary(active_supervisor_id, &summary)?;
            }
            *final_synthesis_requested = true;
            *final_synthesis_requested_at = Some(now);
            *final_synthesis_wakeup_sent = false;
            return Ok(());
        }

        let Some(requested_at) = *final_synthesis_requested_at else {
            *final_synthesis_requested_at = Some(now);
            return Ok(());
        };
        if finalization::cleanup_authorized(active_sessions, active_supervisor_id) {
            return Ok(());
        }

        let elapsed = now.duration_since(requested_at);
        if elapsed >= FINAL_SYNTHESIS_WAKE_AFTER && !*final_synthesis_wakeup_sent {
            if let Some(supervisor) = active_sessions.get_mut(&active_supervisor_id)
                && !supervisor.state.is_terminal()
                && should_wake_supervisor(supervisor, requested_at)
            {
                let prompt = wake_final_synthesis_prompt();
                let _ = send_or_queue_prompt(supervisor, &prompt);
                let summary = "Workers terminal; supervisor wake-up prompt sent to resume closure."
                    .to_owned();
                supervisor.record.last_summary = Some(summary.clone());
                self.store
                    .update_worker_summary(active_supervisor_id, &summary)?;
                self.store.append_json_event(
                    mission_id,
                    Some(active_supervisor_id),
                    "final_synthesis_wakeup",
                    "deterministic supervisor wake-up sent after workers finished",
                    &json!({ "elapsed_secs": elapsed.as_secs() }),
                )?;
            }
            *final_synthesis_wakeup_sent = true;
            return Ok(());
        }

        if elapsed >= FINAL_SYNTHESIS_ESCALATE_AFTER && !*final_synthesis_escalated {
            if let Some(supervisor) = active_sessions.get_mut(&active_supervisor_id)
                && !supervisor.state.is_terminal()
            {
                let prompt = strict_final_synthesis_prompt();
                let _ = send_or_queue_prompt(supervisor, &prompt);
                let summary =
                    "Workers terminal; strict final synthesis enforcement triggered.".to_owned();
                supervisor.record.last_summary = Some(summary.clone());
                self.store
                    .update_worker_summary(active_supervisor_id, &summary)?;
                self.store.append_json_event(
                    mission_id,
                    Some(active_supervisor_id),
                    "final_synthesis_enforced",
                    "strict supervisor final synthesis prompt sent",
                    &json!({ "elapsed_secs": elapsed.as_secs() }),
                )?;
            }
            *final_synthesis_escalated = true;
            return Ok(());
        }

        if elapsed >= FINAL_SYNTHESIS_FORCE_AFTER {
            let envelope = forced_final_envelope(active_sessions);
            self.apply_final_envelope(mission_id, active_supervisor_id, envelope, active_sessions)?;
            self.store.append_json_event(
                mission_id,
                Some(active_supervisor_id),
                "final_synthesis_forced",
                "control plane forced final mission closure from live worker evidence",
                &json!({ "elapsed_secs": elapsed.as_secs() }),
            )?;
        }
        Ok(())
    }
}

fn strict_final_synthesis_prompt() -> String {
    "All workers are already terminal. Stop supervising individual workers now.\nYou must close the mission immediately.\nReply exactly with:\nFINAL_STATE: validated or failed\nREADY_FOR_CLEANUP: yes\nFINAL_SUMMARY: one concise sentence\nBEGIN_FINAL_REPORT_MD\n## Mission Outcome\n- Result: ...\n- Team: ...\n- Risks: ...\nEND_FINAL_REPORT_MD".to_owned()
}

fn wake_final_synthesis_prompt() -> String {
    "Wake up and continue your job now.\nAll workers are already terminal.\nDo not re-plan. Do not observe passively. Read the latest live worker state and finish the mission.\nEither emit the final markdown closure now or report one exact blocking contradiction.\nReply exactly with:\nFINAL_STATE: validated or failed\nREADY_FOR_CLEANUP: yes\nFINAL_SUMMARY: one concise sentence\nBEGIN_FINAL_REPORT_MD\n## Mission Outcome\n- Result: ...\n- Team: ...\n- Risks: ...\nEND_FINAL_REPORT_MD".to_owned()
}

fn should_wake_supervisor(supervisor: &ActiveSession, requested_at: Instant) -> bool {
    let no_output_since_request = supervisor.last_output_at <= requested_at;
    let no_status_since_request = supervisor
        .last_status_update_at
        .is_none_or(|updated_at| updated_at <= requested_at);
    let no_recent_response = supervisor
        .last_response_time
        .is_none_or(|response| response >= Duration::from_secs(20));
    no_output_since_request && no_status_since_request && no_recent_response
}

fn forced_final_envelope(active_sessions: &HashMap<Uuid, ActiveSession>) -> FinalEnvelope {
    let failed = active_sessions
        .values()
        .filter(|session| session.record.role == SessionRole::Worker)
        .any(|session| session.state == SessionState::Failed);
    let final_state = if failed {
        SessionState::Failed
    } else {
        SessionState::Validated
    };
    let worker_lines = active_sessions
        .values()
        .filter(|session| session.record.role == SessionRole::Worker)
        .map(|session| {
            format!(
                "- {}: {}. {}",
                session.record.name,
                session.state.as_str(),
                session
                    .record
                    .last_summary
                    .as_deref()
                    .unwrap_or("no final summary reported")
            )
        })
        .collect::<Vec<_>>();
    let summary = if failed {
        "Mission closed by control plane: at least one worker failed before supervisor synthesis arrived."
    } else {
        "Mission closed by control plane after all workers reached terminal states."
    };
    let report_markdown = format!(
        "## Mission Outcome\n- Result: {}\n- Closure: forced from live worker evidence after supervisor did not finalize in time.\n- Workers:\n{}\n",
        final_state.as_str(),
        worker_lines.join("\n")
    );
    FinalEnvelope {
        state: final_state,
        ready_for_cleanup: true,
        summary: summary.to_owned(),
        report_markdown: Some(report_markdown),
    }
}
