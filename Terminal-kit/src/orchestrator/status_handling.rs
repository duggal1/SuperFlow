use super::*;

impl Orchestrator {
    pub(super) fn persist_normalized_status(
        &self,
        mission_id: Uuid,
        session_id: Uuid,
        normalized_state: &str,
        source: &str,
        confidence: Confidence,
        summary: &str,
        raw_excerpt: &str,
    ) -> Result<()> {
        self.store
            .persist_normalized_update(&NormalizedUpdateRecord {
                id: Uuid::new_v4(),
                mission_id,
                worker_id: session_id,
                source: source.to_owned(),
                raw_excerpt: truncate(raw_excerpt, 400),
                normalized_state: normalized_state.to_owned(),
                confidence: confidence.as_str().to_owned(),
                summary: summary.to_owned(),
                adapter: source.to_owned(),
                created_at: Utc::now(),
            })
    }

    pub(super) fn handle_status_directive(
        &self,
        mission_id: Uuid,
        repo_root: &Path,
        supervisor_id: Uuid,
        session_id: Uuid,
        directive: StatusDirective,
        supervisor_override: bool,
        active_sessions: &mut HashMap<Uuid, ActiveSession>,
        pending_supervisor_decisions: &mut HashMap<String, PendingSupervisorDecision>,
        supervisor_degraded: bool,
        _stats: &mut WatchdogStats,
    ) -> Result<()> {
        let Some(reported_state) = directive.session_state() else {
            return Ok(());
        };
        let Some(session_role) = active_sessions
            .get(&session_id)
            .map(|session| session.record.role)
        else {
            return Ok(());
        };
        let mut new_state = enforcement::canonicalize_worker_state(
            session_role,
            reported_state,
            supervisor_override,
        );

        let mut supervision_notices: Vec<(SupervisorEventType, String)> = Vec::new();
        if let Some(session) = active_sessions.get_mut(&session_id) {
            if session.record.role == SessionRole::Worker {
                let worker_name = session.record.name.clone();
                let outcome = supervision::transitions::from_status(
                    session.task_stage,
                    new_state,
                    &directive,
                );
                session.task_stage = outcome.stage;
                if outcome.plan_only_signal {
                    session.plan_only_count = session.plan_only_count.saturating_add(1);
                } else if !directive.files.is_empty() || !directive.commands.is_empty() {
                    session.plan_only_count = 0;
                }
                let sig = enforcement::compute_status_signature(new_state, &directive);
                let has_evidence = !directive.files.is_empty() || !directive.commands.is_empty();
                let loop_outcome = enforcement::note_status_signature(
                    &mut session.last_status_signature,
                    &mut session.repeated_status_without_evidence,
                    sig,
                    has_evidence,
                );
                if loop_outcome.repeated_without_evidence {
                    supervision_notices.push((
                        SupervisorEventType::WeakOutput,
                        format!(
                            "{} is repeating the same status without evidence. Force execution or reroute now. Demand exact command(s), expected output, and first concrete artifact.",
                            worker_name
                        ),
                    ));
                }
                if let Some(intercept) = supervision::intercept::maybe_intercept_plan_only(
                    &worker_name,
                    session.task_stage,
                    session.plan_only_count,
                    new_state,
                    &directive,
                ) {
                    supervision_notices.push((intercept.event_type, intercept.notice_body));
                }
            }
        }
        if !supervisor_degraded {
            for (event_type, body) in supervision_notices {
                self.send_worker_supervisor_notice(
                    mission_id,
                    supervisor_id,
                    session_id,
                    active_sessions,
                    event_type,
                    &body,
                )?;
            }
        }

        let mut notify_supervisor = false;
        let mut validation_escalation = None::<String>;
        let mut overlap_escalation = None::<String>;
        let mut state_summary = directive.summary.clone();
        let mut session_name = String::new();
        let mut validation_result: Option<(Option<Uuid>, String)> = None;
        let files = directive.files.clone();
        let commands = directive.commands.clone();
        let mut risks = directive.risks.clone();
        let overlap = directive.overlap.clone();
        let overlap_detail = enforcement::meaningful_overlap_detail(overlap.as_deref());
        let mut completion_gate_correction = None::<String>;

        if !supervisor_override
            && session_role == SessionRole::Worker
            && reported_state == SessionState::Validated
        {
            state_summary = format!(
                "worker reported validated; awaiting supervisor acceptance: {}",
                directive.summary
            );
        }

        if matches!(
            new_state,
            SessionState::DoneClaimed | SessionState::NeedsValidation | SessionState::Validated
        ) && let Some(session) = active_sessions.get(&session_id)
            && session.record.role == SessionRole::Worker
            && let Some(packet) = session.packet.as_ref()
        {
            if enforcement::evidence_missing_for_done_claim(&directive) {
                new_state = SessionState::NeedsRetry;
                state_summary =
                    "completion rejected: missing explicit files or commands".to_owned();
                risks.push(
                    "completion claim omitted explicit file evidence and command/test evidence"
                        .to_owned(),
                );
                completion_gate_correction = Some(
                    "Your completion claim was rejected because it did not include explicit evidence. Report the exact changed files and at least one concrete command, test, validation step, or handoff artifact.".to_owned(),
                );
            } else if let Some(rejection) =
                reject_completion_without_artifacts(repo_root, packet, &files)
            {
                new_state = SessionState::NeedsRetry;
                state_summary = format!("completion rejected: missing {}", rejection.join(", "));
                risks.push(format!(
                    "missing required on-disk artifacts: {}",
                    rejection.join(", ")
                ));
                completion_gate_correction = Some(format!(
                    "Your completion claim was rejected because Sapphire could not find the required artifacts on disk: {}. Do the work, then report status again with exact files.",
                    rejection.join(", ")
                ));
            }
        }

        if let Some(session) = active_sessions.get_mut(&session_id) {
            session_name = session.record.name.clone();
            let now = Instant::now();
            session.last_output_at = now;
            session.last_confirmed_alive = now;
            enforcement::note_status_report(session, now);
            session.first_status_incident_stage = 0;
            session.last_first_status_escalation_at = None;
            session.reported_overlap = overlap_detail.clone();
            session.state = new_state;
            session.record.last_summary = Some(state_summary.clone());
            if new_state != SessionState::Stalled {
                session.stall_count = 0;
                session.consecutive_stall_failures = 0;
            }
            if !matches!(
                new_state,
                SessionState::WeakOutput | SessionState::WrongDirection
            ) {
                session.low_confidence_count = 0;
            }
            self.store.update_session_state(session_id, new_state)?;
            self.store
                .update_worker_summary(session_id, &state_summary)?;
            if let Some(task_id) = session.task_id {
                let task_status = match new_state {
                    SessionState::Planned | SessionState::NotStarted => "queued",
                    SessionState::Booting => "dispatched",
                    SessionState::Progressing => {
                        if session.initial_status_received {
                            "running"
                        } else {
                            "dispatched"
                        }
                    }
                    SessionState::Failed => "failed",
                    SessionState::DoneClaimed | SessionState::NeedsValidation => "validating",
                    SessionState::Blocked => "blocked",
                    SessionState::WeakOutput
                    | SessionState::WrongDirection
                    | SessionState::Contradictory => "waiting",
                    SessionState::NeedsRetry => "needs_retry",
                    SessionState::Stalled => "stalled",
                    SessionState::Validated => "done",
                    SessionState::Exited => "failed",
                };
                let _ = self.store.update_task_status(task_id, task_status);
                if matches!(
                    new_state,
                    SessionState::Validated | SessionState::NeedsRetry | SessionState::Failed
                ) {
                    let outcome = match new_state {
                        SessionState::Validated => "pass",
                        SessionState::NeedsRetry => "needs_retry",
                        SessionState::Failed => "fail",
                        _ => unreachable!(),
                    };
                    validation_result = Some((Some(task_id), outcome.to_owned()));
                }
            }
            notify_supervisor = !supervisor_override
                && session.record.role == SessionRole::Worker
                && matches!(
                    new_state,
                    SessionState::Blocked
                        | SessionState::Stalled
                        | SessionState::DoneClaimed
                        | SessionState::NeedsValidation
                        | SessionState::WeakOutput
                        | SessionState::WrongDirection
                        | SessionState::Contradictory
                        | SessionState::NeedsRetry
                        | SessionState::Validated
                        | SessionState::Failed
                )
                && session.escalation_sent_for_state != Some(new_state);
            session.validation_pending = session.record.role == SessionRole::Worker
                && matches!(
                    new_state,
                    SessionState::DoneClaimed | SessionState::NeedsValidation
                );
            session.escalation_sent_for_state = if notify_supervisor {
                Some(new_state)
            } else {
                session.escalation_sent_for_state
            };
            if session.record.role == SessionRole::Worker
                && matches!(
                    new_state,
                    SessionState::DoneClaimed | SessionState::NeedsValidation
                )
                && !supervisor_degraded
            {
                validation_escalation = Some(format!(
                    "Worker {} is requesting validation with state {}. Decide whether to validate_worker, accept_worker, retry_worker, fail_worker, or message_worker. Summary: {}",
                    session.record.name,
                    new_state.as_str(),
                    state_summary
                ));
            }
            if !supervisor_override
                && session.record.role == SessionRole::Worker
                && let Some(detail) = overlap_detail.as_deref()
                && !supervisor_degraded
            {
                overlap_escalation = Some(format!(
                    "Worker {} reported live overlap risk: {}. Enforce ownership, keep newer teammate work intact, and decide whether to redirect_worker, message_worker, or fail_worker.",
                    session.record.name, detail,
                ));
            }
        }

        self.store.append_json_event(
            mission_id,
            Some(session_id),
            "session_state",
            &state_summary,
            &json!({
                "state": new_state.as_str(),
                "summary": state_summary,
                "files": files,
                "commands": commands,
                "risks": risks,
                "overlap": overlap,
            }),
        )?;
        self.store.append_summary(
            mission_id,
            Some(session_id),
            format!("state:{}", new_state.as_str()),
            &state_summary,
        )?;

        if notify_supervisor && session_id != supervisor_id {
            let event_type = event_type_for_state(&new_state);
            self.send_worker_supervisor_notice(
                mission_id,
                supervisor_id,
                session_id,
                active_sessions,
                event_type,
                &format!(
                    "Worker {} reported state {}. Summary: {}",
                    session_name,
                    new_state.as_str(),
                    state_summary,
                ),
            )?;
        }
        if let Some(reason) = validation_escalation
            && queue_supervisor_decision(
                pending_supervisor_decisions,
                SupervisorDecisionKind::Validation,
                session_id,
                &reason,
            )
        {
            self.send_worker_supervisor_notice(
                mission_id,
                supervisor_id,
                session_id,
                active_sessions,
                SupervisorEventType::DoneClaimed,
                &reason,
            )?;
        }
        if let Some(reason) = overlap_escalation
            && queue_supervisor_decision(
                pending_supervisor_decisions,
                SupervisorDecisionKind::OverlapRecovery,
                session_id,
                &reason,
            )
        {
            self.send_worker_supervisor_notice(
                mission_id,
                supervisor_id,
                session_id,
                active_sessions,
                SupervisorEventType::Contradiction,
                &reason,
            )?;
        }
        clear_resolved_supervisor_decisions(
            pending_supervisor_decisions,
            active_sessions,
            session_id,
        );
        if let Some((task_id, outcome)) = validation_result {
            self.store
                .persist_validation_result(&ValidationResultRecord {
                    id: Uuid::new_v4(),
                    mission_id,
                    worker_id: session_id,
                    task_id,
                    outcome,
                    summary: state_summary.clone(),
                    evidence_json: json!({
                        "state": new_state.as_str(),
                        "files": files,
                        "commands": commands,
                        "risks": risks,
                        "overlap": overlap,
                    })
                    .to_string(),
                    created_at: Utc::now(),
                })?;
        }
        if let Some(correction) = completion_gate_correction
            && queue_supervisor_decision(
                pending_supervisor_decisions,
                SupervisorDecisionKind::Validation,
                session_id,
                &format!(
                    "Worker {} needs a strict completion correction: {}",
                    session_name, correction
                ),
            )
        {
            self.send_worker_supervisor_notice(
                mission_id,
                supervisor_id,
                session_id,
                active_sessions,
                SupervisorEventType::WeakOutput,
                &format!(
                    "Completion claim from {} failed the artifact gate. Challenge the worker directly with proof requirements. {}",
                    session_name, correction,
                ),
            )?;
        }
        if session_id == supervisor_id && new_state == SessionState::Validated {
            let _ = self
                .store
                .update_mission_final_summary(mission_id, &state_summary);
        }
        Ok(())
    }

    pub(super) fn handle_normalized_observation(
        &self,
        mission_id: Uuid,
        repo_root: &Path,
        supervisor_id: Uuid,
        session_id: Uuid,
        mut observation: NormalizedObservation,
        active_sessions: &mut HashMap<Uuid, ActiveSession>,
        pending_supervisor_decisions: &mut HashMap<String, PendingSupervisorDecision>,
        supervisor_degraded: bool,
        stats: &mut WatchdogStats,
    ) -> Result<()> {
        if observation.source != "status_envelope"
            && matches!(observation.state, SessionState::Validated)
        {
            observation.state = SessionState::NeedsValidation;
            observation.summary = format!(
                "inferred completion signal needs explicit validation: {}",
                observation.summary
            );
            observation.files.clear();
            observation.blocker = None;
        }
        let Some(session) = active_sessions.get_mut(&session_id) else {
            return Ok(());
        };
        if Instant::now() < session.startup_grace_until
            && observation.source != "status_envelope"
            && observation.confidence == Confidence::Low
        {
            return Ok(());
        }
        let key = format!(
            "{}|{}|{}",
            observation.state.as_str(),
            observation.source,
            observation.summary
        );
        if session.last_observation_key.as_deref() == Some(key.as_str()) {
            return Ok(());
        }
        session.last_observation_key = Some(key);
        match observation.confidence {
            Confidence::High | Confidence::Medium => session.low_confidence_count = 0,
            Confidence::Low => session.low_confidence_count += 1,
        }
        self.store
            .persist_normalized_update(&NormalizedUpdateRecord {
                id: Uuid::new_v4(),
                mission_id,
                worker_id: session_id,
                source: observation.source.to_owned(),
                raw_excerpt: truncate(&observation.raw_excerpt, 400),
                normalized_state: observation.state.as_str().to_owned(),
                confidence: observation.confidence.as_str().to_owned(),
                summary: observation.summary.clone(),
                adapter: session.record.agent.as_str().to_owned(),
                created_at: Utc::now(),
            })?;
        self.handle_status_directive(
            mission_id,
            repo_root,
            supervisor_id,
            session_id,
            StatusDirective {
                state: observation.state.as_str().to_owned(),
                summary: observation.summary.clone(),
                files: observation.files.clone(),
                commands: Vec::new(),
                risks: observation.blocker.clone().into_iter().collect::<Vec<_>>(),
                overlap: None,
            },
            false,
            active_sessions,
            pending_supervisor_decisions,
            supervisor_degraded,
            stats,
        )?;
        let low_confidence_count = active_sessions
            .get(&session_id)
            .map(|worker| worker.low_confidence_count)
            .unwrap_or(0);
        if low_confidence_count >= 2 {
            let reason = format!(
                "Worker {} remains low-confidence after repeated updates. Decide whether to retry_worker, redirect_worker, validate_worker, fail_worker, or message_worker.",
                active_sessions
                    .get(&session_id)
                    .map(|worker| worker.record.name.as_str())
                    .unwrap_or("unknown")
            );
            if queue_supervisor_decision(
                pending_supervisor_decisions,
                SupervisorDecisionKind::LowConfidenceRecovery,
                session_id,
                &reason,
            ) {
                self.send_worker_supervisor_notice(
                    mission_id,
                    supervisor_id,
                    session_id,
                    active_sessions,
                    SupervisorEventType::WeakOutput,
                    &reason,
                )?;
            }
        }
        Ok(())
    }

    pub(super) fn handle_settled_worker_observations(
        &self,
        mission_id: Uuid,
        repo_root: &Path,
        supervisor_id: Uuid,
        active_sessions: &mut HashMap<Uuid, ActiveSession>,
        pending_supervisor_decisions: &mut HashMap<String, PendingSupervisorDecision>,
        supervisor_degraded: bool,
        stats: &mut WatchdogStats,
    ) -> Result<()> {
        let now = Instant::now();
        let mut observations = Vec::new();
        for (session_id, session) in active_sessions.iter() {
            if session.record.role != SessionRole::Worker
                || session.state.is_terminal()
                || session.raw_buffer.is_empty()
                || session.output_chunks == 0
                || now < session.startup_grace_until
                || !worker_output_has_settled(session, now)
                || has_recent_status_activity(session, now)
                || session_has_live_terminal(session)
                || !session.queued_prompts.is_empty()
                || recently_prompted(session, now)
                || is_in_cooldown(session, now)
            {
                continue;
            }
            let adapter = adapter_for(session.record.agent);
            if let Some(observation) = adapter.detect_state(&session.raw_buffer) {
                observations.push((*session_id, observation));
            }
        }
        for (session_id, observation) in observations {
            self.handle_normalized_observation(
                mission_id,
                repo_root,
                supervisor_id,
                session_id,
                observation,
                active_sessions,
                pending_supervisor_decisions,
                supervisor_degraded,
                stats,
            )?;
        }
        Ok(())
    }
}
