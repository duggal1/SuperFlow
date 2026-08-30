use super::*;

impl Orchestrator {
    pub(super) fn send_worker_supervisor_notice(
        &self,
        mission_id: Uuid,
        active_supervisor_id: Uuid,
        worker_session_id: Uuid,
        active_sessions: &mut HashMap<Uuid, ActiveSession>,
        event_type: SupervisorEventType,
        body: &str,
    ) -> Result<()> {
        let routed_supervisor_id = supervisor_team::routed_supervisor_for_worker(
            active_sessions,
            worker_session_id,
            active_supervisor_id,
        );
        self.send_supervisor_notice(
            mission_id,
            routed_supervisor_id,
            active_sessions,
            event_type,
            body,
        )
    }

    pub(super) fn apply_supervisor_action(
        &self,
        mission_id: Uuid,
        repo_root: &Path,
        supervisor_id: Uuid,
        action: SupervisorAction,
        active_sessions: &mut HashMap<Uuid, ActiveSession>,
        alias_map: &HashMap<String, Uuid>,
        pending_supervisor_decisions: &mut HashMap<String, PendingSupervisorDecision>,
        stats: &mut WatchdogStats,
    ) -> Result<()> {
        let Some(supervisor) = active_sessions.get_mut(&supervisor_id) else {
            return Ok(());
        };
        let signature = format!(
            "{}|{}|{}|{}",
            action.action,
            action.target.as_deref().unwrap_or("NONE"),
            action.summary,
            action.message.as_deref().unwrap_or("NONE")
        );
        if supervisor.last_supervisor_action_key.as_deref() == Some(signature.as_str()) {
            return Ok(());
        }
        supervisor.last_supervisor_action_key = Some(signature);

        let action_name = action.action.trim().to_ascii_lowercase();
        let target_id = action
            .target
            .as_deref()
            .and_then(|target_alias| resolve_alias(alias_map, target_alias));
        if action_name != "observe" && target_id.is_none() {
            return Ok(());
        }
        let Some(target_id) =
            target_id.or_else(|| action_name.eq("observe").then_some(supervisor_id))
        else {
            return Ok(());
        };
        pending_supervisor_decisions.retain(|_, pending| {
            pending.target_session_id != target_id
                || !enforcement::action_resolves_decision(pending.kind, &action_name)
        });

        match action_name.as_str() {
            "observe" => {
                if let Some(target) = active_sessions.get(&target_id) {
                    let actionable = target.record.role == SessionRole::Worker
                        && (!target.initial_status_received
                            || target.validation_pending
                            || target.state == SessionState::Stalled
                            || target.state == SessionState::Blocked
                            || target.plan_only_count >= 2
                            || target.repeated_status_without_evidence >= 2);
                    if actionable {
                        let reason = format!(
                            "Observe rejected for {}. The worker still requires an active supervisory decision. Choose retry_worker, message_worker, validate_worker, redirect_worker, or fail_worker with one concrete instruction.",
                            target.record.name
                        );
                        self.store.append_json_event(
                            mission_id,
                            Some(supervisor_id),
                            "observe_rejected",
                            &reason,
                            &json!({ "target": target.record.name, "summary": action.summary }),
                        )?;
                        self.send_supervisor_notice(
                            mission_id,
                            supervisor_id,
                            active_sessions,
                            SupervisorEventType::WeakOutput,
                            &reason,
                        )?;
                    }
                }
            }
            "validate_worker" => {
                if let Some(target) = active_sessions.get_mut(&target_id) {
                    let adapter = adapter_for(target.record.agent);
                    if let Some(msg) = action.message.as_deref() {
                        let prompt = adapter.build_validation_prompt(msg);
                        let _ = send_or_queue_prompt(target, &prompt);
                        stats.validation_challenges += 1;
                    } else {
                        self.store.append_json_event(
                            mission_id,
                            Some(supervisor_id),
                            "supervisor_action_skipped",
                            "validate_worker without message — skipping because no concrete validation instruction was provided",
                            &json!({ "action": action.action, "target": action.target }),
                        )?;
                    }
                }
            }
            "retry_worker" | "redirect_worker" | "message_worker" => {
                if let Some(target) = active_sessions.get_mut(&target_id) {
                    let adapter = adapter_for(target.record.agent);
                    let message = action.message.as_deref().unwrap_or(&action.summary);
                    let prompt = adapter.build_correction_prompt(message);
                    let _ = send_or_queue_prompt(target, &prompt);
                }
            }
            "accept_worker" => {
                self.handle_status_directive(
                    mission_id,
                    repo_root,
                    supervisor_id,
                    target_id,
                    StatusDirective {
                        state: SessionState::Validated.as_str().to_owned(),
                        summary: action.summary.clone(),
                        files: Vec::new(),
                        commands: Vec::new(),
                        risks: Vec::new(),
                        overlap: None,
                    },
                    true,
                    active_sessions,
                    pending_supervisor_decisions,
                    false,
                    stats,
                )?;
            }
            "fail_worker" => {
                self.handle_status_directive(
                    mission_id,
                    repo_root,
                    supervisor_id,
                    target_id,
                    StatusDirective {
                        state: SessionState::Failed.as_str().to_owned(),
                        summary: action.summary.clone(),
                        files: Vec::new(),
                        commands: Vec::new(),
                        risks: Vec::new(),
                        overlap: None,
                    },
                    true,
                    active_sessions,
                    pending_supervisor_decisions,
                    false,
                    stats,
                )?;
            }
            _ => {}
        }

        self.store.append_json_event(
            mission_id,
            Some(supervisor_id),
            "supervisor_action",
            &action.summary,
            &json!({
                "action": action.action,
                "target": action.target,
                "summary": action.summary,
                "message": action.message,
            }),
        )?;
        let summary =
            supervisor::summarize_action(&action.action, action.target.as_deref(), &action.summary);
        if let Some(supervisor) = active_sessions.get_mut(&supervisor_id) {
            supervisor.record.last_summary = Some(summary.clone());
        }
        self.store.update_worker_summary(supervisor_id, &summary)?;
        Ok(())
    }

    pub(super) fn apply_final_envelope(
        &self,
        mission_id: Uuid,
        supervisor_id: Uuid,
        final_envelope: FinalEnvelope,
        active_sessions: &mut HashMap<Uuid, ActiveSession>,
    ) -> Result<()> {
        let key = format!(
            "{}|{}|{}",
            final_envelope.state.as_str(),
            if final_envelope.ready_for_cleanup {
                "cleanup_yes"
            } else {
                "cleanup_no"
            },
            final_envelope.summary
        );
        if let Some(supervisor) = active_sessions.get_mut(&supervisor_id) {
            if supervisor.last_observation_key.as_deref() == Some(key.as_str()) {
                return Ok(());
            }
            supervisor.last_observation_key = Some(key);
            supervisor.cleanup_authorized = final_envelope.ready_for_cleanup;
            supervisor.record.last_summary = Some(final_envelope.summary.clone());
            if final_envelope.ready_for_cleanup {
                supervisor.state = final_envelope.state;
                self.store
                    .update_session_state(supervisor_id, final_envelope.state)?;
            }
        }
        self.store.append_summary(
            mission_id,
            Some(supervisor_id),
            "final_summary",
            &final_envelope.summary,
        )?;
        if let Some(report_markdown) = final_envelope.report_markdown.as_deref() {
            self.store.append_summary(
                mission_id,
                Some(supervisor_id),
                "final_summary_markdown",
                report_markdown,
            )?;
        }
        self.store.append_json_event(
            mission_id,
            Some(supervisor_id),
            "cleanup_decision",
            &final_envelope.summary,
            &json!({
                "final_state": final_envelope.state.as_str(),
                "ready_for_cleanup": final_envelope.ready_for_cleanup,
            }),
        )?;
        self.store
            .update_worker_summary(supervisor_id, &final_envelope.summary)?;
        if final_envelope.ready_for_cleanup {
            let mission_summary = final_envelope
                .report_markdown
                .as_deref()
                .unwrap_or(&final_envelope.summary);
            self.store
                .update_mission_final_summary(mission_id, mission_summary)?;
        }
        Ok(())
    }

    pub(super) fn handle_pending_supervisor_decisions(
        &self,
        mission_id: Uuid,
        repo_root: &Path,
        supervisor_id: Uuid,
        active_sessions: &mut HashMap<Uuid, ActiveSession>,
        pending_supervisor_decisions: &mut HashMap<String, PendingSupervisorDecision>,
        stats: &mut WatchdogStats,
    ) -> Result<()> {
        let now = Instant::now();
        let mut completed = Vec::new();
        let mut follow_ups = Vec::new();
        enum PendingDecisionAction {
            Prompt {
                key: String,
                target_session_id: Uuid,
                prompt_kind: enforcement::PromptKind,
                message: String,
                intervention_type: &'static str,
                event_type: SupervisorEventType,
            },
            Override {
                key: String,
                target_session_id: Uuid,
                state: SessionState,
                summary: String,
                intervention_type: &'static str,
                event_type: SupervisorEventType,
            },
        }
        let mut autonomous_actions = Vec::new();
        for (key, pending) in pending_supervisor_decisions.iter_mut() {
            let Some(target) = active_sessions.get(&pending.target_session_id) else {
                completed.push(key.clone());
                continue;
            };
            let resolved = target.state.is_terminal()
                || (pending.kind == SupervisorDecisionKind::Validation
                    && !target.validation_pending)
                || (pending.kind == SupervisorDecisionKind::StallRecovery
                    && target.state != SessionState::Stalled)
                || (pending.kind == SupervisorDecisionKind::LowConfidenceRecovery
                    && target.low_confidence_count == 0)
                || (pending.kind == SupervisorDecisionKind::OverlapRecovery
                    && target
                        .reported_overlap
                        .as_deref()
                        .is_none_or(|value| value.trim().is_empty()));
            if resolved {
                completed.push(key.clone());
                continue;
            }
            if enforcement::should_attempt_autonomous_resolution(pending, now)
                && let Some(decision) = enforcement::autonomous_resolution(pending, target, now)
            {
                match decision {
                    enforcement::AutonomousDecision::Prompt {
                        prompt_kind,
                        message,
                        intervention_type,
                        event_type,
                    } => {
                        autonomous_actions.push(PendingDecisionAction::Prompt {
                            key: key.clone(),
                            target_session_id: pending.target_session_id,
                            prompt_kind,
                            message,
                            intervention_type,
                            event_type,
                        });
                    }
                    enforcement::AutonomousDecision::OverrideState {
                        state,
                        summary,
                        intervention_type,
                        event_type,
                    } => {
                        autonomous_actions.push(PendingDecisionAction::Override {
                            key: key.clone(),
                            target_session_id: pending.target_session_id,
                            state,
                            summary,
                            intervention_type,
                            event_type,
                        });
                        completed.push(key.clone());
                    }
                }
                enforcement::mark_autonomous_action(pending, now);
                continue;
            }
            if enforcement::should_follow_up_pending_decision(pending, now) {
                let routed_supervisor_id = supervisor_team::routed_supervisor_for_worker(
                    active_sessions,
                    pending.target_session_id,
                    supervisor_id,
                );
                follow_ups.push((
                    routed_supervisor_id,
                    enforcement::follow_up_event_type(pending.kind),
                    enforcement::follow_up_reason(pending, target, now),
                ));
                enforcement::mark_pending_decision_notified(pending, now);
            }
        }
        for key in completed {
            pending_supervisor_decisions.remove(&key);
        }
        for action in autonomous_actions {
            match action {
                PendingDecisionAction::Prompt {
                    key,
                    target_session_id,
                    prompt_kind,
                    message,
                    intervention_type,
                    event_type,
                } => {
                    if let Some(target) = active_sessions.get_mut(&target_session_id) {
                        let adapter = adapter_for(target.record.agent);
                        let prompt = match prompt_kind {
                            enforcement::PromptKind::Status => {
                                adapter.build_status_prompt(&message)
                            }
                            enforcement::PromptKind::Validation => {
                                adapter.build_validation_prompt(&message)
                            }
                        };
                        let _ = send_or_queue_prompt(target, &prompt);
                        record_intervention(target, intervention_type, now);
                    }
                    self.store.append_json_event(
                        mission_id,
                        Some(supervisor_id),
                        "autonomous_supervision_prompt",
                        &message,
                        &json!({
                            "pending_decision_key": key,
                            "target_session_id": target_session_id,
                            "event_type": event_type.as_str(),
                            "intervention_type": intervention_type,
                        }),
                    )?;
                }
                PendingDecisionAction::Override {
                    key,
                    target_session_id,
                    state,
                    summary,
                    intervention_type,
                    event_type,
                } => {
                    self.handle_status_directive(
                        mission_id,
                        repo_root,
                        supervisor_id,
                        target_session_id,
                        StatusDirective {
                            state: state.as_str().to_owned(),
                            summary: summary.clone(),
                            files: Vec::new(),
                            commands: Vec::new(),
                            risks: vec![format!(
                                "control-plane autonomous escalation via {}",
                                intervention_type
                            )],
                            overlap: None,
                        },
                        true,
                        active_sessions,
                        pending_supervisor_decisions,
                        false,
                        stats,
                    )?;
                    self.store.append_json_event(
                        mission_id,
                        Some(supervisor_id),
                        "autonomous_supervision_override",
                        &summary,
                        &json!({
                            "pending_decision_key": key,
                            "target_session_id": target_session_id,
                            "state": state.as_str(),
                            "event_type": event_type.as_str(),
                            "intervention_type": intervention_type,
                        }),
                    )?;
                }
            }
        }
        for (routed_supervisor_id, event_type, body) in follow_ups {
            self.send_supervisor_notice(
                mission_id,
                routed_supervisor_id,
                active_sessions,
                event_type,
                &body,
            )?;
        }
        Ok(())
    }

    pub(super) fn send_supervisor_notice(
        &self,
        mission_id: Uuid,
        supervisor_id: Uuid,
        active_sessions: &mut HashMap<Uuid, ActiveSession>,
        event_type: SupervisorEventType,
        body: &str,
    ) -> Result<()> {
        if let Some(supervisor) = active_sessions.get_mut(&supervisor_id) {
            if matches!(
                supervisor.state,
                SessionState::Exited | SessionState::Failed
            ) {
                return Ok(());
            }
            let now = Instant::now();
            prune_recent_supervisor_notice_keys(supervisor, now);
            let notice_key = supervisor::notice_key(event_type, body);
            if supervisor.last_supervisor_notice_key.as_deref() == Some(notice_key.as_str())
                || has_recent_supervisor_notice_key(supervisor, &notice_key)
            {
                return Ok(());
            }
            let adapter = adapter_for(supervisor.record.agent);
            let prompt = adapter.build_supervisor_action_prompt(event_type, body);
            remember_supervisor_notice(supervisor, notice_key, now);
            supervisor.record.last_summary = Some(supervisor::summarize_notice(event_type, body));
            let _ = send_or_queue_prompt(supervisor, &prompt);
        }
        self.store.append_json_event(
            mission_id,
            Some(supervisor_id),
            "supervisor_notice",
            truncate(body, 240),
            &json!({ "event_type": event_type.as_str(), "body": body }),
        )?;
        self.store.append_summary(
            mission_id,
            Some(supervisor_id),
            "supervisor_notice",
            body.to_owned(),
        )?;
        self.store.update_worker_summary(
            supervisor_id,
            &supervisor::summarize_notice(event_type, body),
        )?;
        Ok(())
    }
}

pub(crate) fn queue_supervisor_state_card(
    active_sessions: &mut HashMap<Uuid, ActiveSession>,
    supervisor_id: Uuid,
    event_type: SupervisorEventType,
    card: &str,
) -> bool {
    let Some(supervisor_session) = active_sessions.get_mut(&supervisor_id) else {
        return false;
    };
    if matches!(
        supervisor_session.state,
        SessionState::Exited | SessionState::Failed
    ) {
        return false;
    }
    let key = supervisor::state_card_key(card);
    if supervisor_session.last_supervisor_state_card_key.as_deref() == Some(key.as_str()) {
        return false;
    }
    supervisor_session.last_supervisor_state_card_key = Some(key);
    supervisor_session.record.last_summary = Some(supervisor::summarize_state_card(card));
    let adapter = adapter_for(supervisor_session.record.agent);
    let prompt = adapter.build_supervisor_action_prompt(event_type, card);
    send_or_queue_prompt(supervisor_session, &prompt)
}
