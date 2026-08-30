use super::*;

impl Orchestrator {
    pub(super) fn health_probe_sessions(
        &self,
        _mission_id: Uuid,
        stall_after: Duration,
        active_sessions: &mut HashMap<Uuid, ActiveSession>,
        stats: &mut WatchdogStats,
    ) -> Result<()> {
        let now = Instant::now();
        let probe_threshold = stall_after.mul_f64(0.75);
        let mut probed_ids = Vec::new();
        for (session_id, session) in active_sessions.iter() {
            if session.record.role == SessionRole::Supervisor || session.state.is_terminal() {
                continue;
            }
            if has_recent_status_activity(session, now) || session_has_live_terminal(session) {
                continue;
            }
            let idle_duration = now.duration_since(session.last_confirmed_alive);
            let already_probed_this_idle_window = session
                .health_state
                .last_probe_at
                .is_some_and(|last_probe| last_probe >= session.last_confirmed_alive);
            if idle_duration >= probe_threshold
                && idle_duration < stall_after
                && session.state != SessionState::Stalled
                && !already_probed_this_idle_window
            {
                probed_ids.push(*session_id);
            }
        }
        for session_id in probed_ids {
            if let Some(session) = active_sessions.get_mut(&session_id) {
                session.health_state.record_probe();
                session.health_state.record_failure();
                stats.supervisor_health_events += 1;
            }
        }
        Ok(())
    }

    pub(super) fn handle_supervisor_team_health(
        &self,
        mission_id: Uuid,
        supervisor_ids: &[Uuid],
        active_supervisor_id: &mut Uuid,
        stall_after: Duration,
        active_sessions: &mut HashMap<Uuid, ActiveSession>,
        pending_mail: &HashMap<Uuid, PendingMail>,
        pending_decisions: &HashMap<String, PendingSupervisorDecision>,
        started_at: Instant,
        supervisor_mode: &mut SupervisorMode,
        worker_continuity_announced: &mut bool,
        stats: &mut WatchdogStats,
    ) -> Result<()> {
        let workers_active = active_sessions.values().any(|session| {
            session.record.role == SessionRole::Worker && !session.state.is_terminal()
        });
        if !workers_active {
            return Ok(());
        }
        let describe_supervisor = |session_id: Uuid| {
            active_sessions.get(&session_id).map(|session| {
                let elapsed = session.last_output_at.elapsed();
                let tmux_health = session
                    .runtime
                    .terminal_target()
                    .map(|target| {
                        tmux::Tmux::new(None)
                            .check_session_health(target, zombie_check_max_inactivity())
                    })
                    .unwrap_or_else(|| {
                        if elapsed >= zombie_check_max_inactivity() {
                            tmux::SessionHealth::Hung
                        } else {
                            tmux::SessionHealth::Healthy
                        }
                    });
                let condition = supervisor::classify_supervisor(
                    session.state,
                    elapsed,
                    stall_after,
                    tmux_health,
                );
                (
                    condition,
                    session.state,
                    elapsed,
                    tmux_health,
                    session.record.name.clone(),
                )
            })
        };
        let candidates = supervisor_ids
            .iter()
            .filter_map(|session_id| {
                describe_supervisor(*session_id).map(
                    |(condition, state, elapsed, tmux_health, name)| {
                        (*session_id, condition, state, elapsed, tmux_health, name)
                    },
                )
            })
            .collect::<Vec<_>>();
        if candidates.is_empty() {
            return Ok(());
        }
        let active_condition = candidates
            .iter()
            .find(|(session_id, _, _, _, _, _)| *session_id == *active_supervisor_id)
            .map(|(_, condition, _, _, _, _)| *condition)
            .unwrap_or(supervisor::SupervisorCondition::Unavailable);

        if active_condition == supervisor::SupervisorCondition::Unavailable {
            let replacement = candidates
                .iter()
                .filter(|(session_id, condition, _, _, _, _)| {
                    *session_id != *active_supervisor_id
                        && *condition != supervisor::SupervisorCondition::Unavailable
                })
                .max_by_key(|(_, condition, _, _, _, _)| match condition {
                    supervisor::SupervisorCondition::Healthy => 2,
                    supervisor::SupervisorCondition::ProbeNeeded => 1,
                    supervisor::SupervisorCondition::Unavailable => 0,
                })
                .map(|(session_id, _, _, _, _, name)| (*session_id, name.clone()));
            if let Some((next_supervisor_id, next_name)) = replacement {
                let previous_name = active_sessions
                    .get(active_supervisor_id)
                    .map(|session| session.record.name.clone())
                    .unwrap_or_else(|| "supervisor".to_owned());
                let takeover_card = supervisor_team::build_supervisor_cards(
                    active_sessions,
                    pending_mail,
                    pending_decisions,
                    *supervisor_mode,
                    started_at,
                    supervisor_ids,
                    next_supervisor_id,
                )
                .into_iter()
                .find_map(|(session_id, card)| (session_id == next_supervisor_id).then_some(card))
                .unwrap_or_else(|| "SUPERVISOR STATE CARD unavailable".to_owned());
                *active_supervisor_id = next_supervisor_id;
                *supervisor_mode = SupervisorMode::Recovering;
                if let Some(replacement) = active_sessions.get_mut(&next_supervisor_id) {
                    let prompt = supervisor::build_takeover_prompt(&takeover_card, &previous_name);
                    let _ = send_or_queue_prompt(replacement, &prompt);
                }
                Self::notify_workers_of_supervisor_continuity(
                    active_sessions,
                    &next_name,
                    worker_continuity_announced,
                );
                self.store.append_summary(
                    mission_id,
                    Some(next_supervisor_id),
                    "supervisor_takeover",
                    format!("{next_name} took over active supervision because the previous supervisor became unavailable."),
                )?;
                stats.supervisor_health_events += 1;
                return Ok(());
            }
            if *supervisor_mode != SupervisorMode::Degraded {
                *supervisor_mode = SupervisorMode::Degraded;
                Self::notify_workers_of_supervisor_continuity(
                    active_sessions,
                    "supervision incident mode",
                    worker_continuity_announced,
                );
                stats.supervisor_health_events += 1;
            }
            return Ok(());
        }

        if active_condition == supervisor::SupervisorCondition::ProbeNeeded {
            if *supervisor_mode == SupervisorMode::Healthy {
                *supervisor_mode = SupervisorMode::Recovering;
                if communication_policy::SUPERVISOR_HEALTH_PROBES
                    && let Some(active_supervisor) = active_sessions.get_mut(active_supervisor_id)
                {
                    let _ = send_or_queue_prompt(
                        active_supervisor,
                        "Supervisor health probe. Workers are still active, but your control loop has gone quiet.\nDiagnose overdue workers, issue at least one decisive action, and report a concise supervisory status immediately. Do not observe only.",
                    );
                }
                stats.supervisor_health_events += 1;
            }
            return Ok(());
        }

        if *supervisor_mode != SupervisorMode::Healthy {
            *supervisor_mode = SupervisorMode::Healthy;
            *worker_continuity_announced = false;
            if let Some(active_supervisor) = active_sessions.get(active_supervisor_id) {
                self.store.append_summary(
                    mission_id,
                    Some(*active_supervisor_id),
                    "supervisor_health",
                    format!(
                        "{} is healthy and supervising normally.",
                        active_supervisor.record.name
                    ),
                )?;
            }
            stats.supervisor_health_events += 1;
        }
        Ok(())
    }

    pub(super) fn notify_workers_of_supervisor_continuity(
        active_sessions: &mut HashMap<Uuid, ActiveSession>,
        acting_supervisor_name: &str,
        worker_continuity_announced: &mut bool,
    ) {
        for session in active_sessions.values_mut() {
            if session.record.role == SessionRole::Worker && !session.state.is_terminal() {
                session.record.last_summary = Some(format!(
                    "Supervision control currently owned by {acting_supervisor_name}"
                ));
            }
        }
        *worker_continuity_announced = true;
    }

    pub(super) fn zombie_debounce_check(
        &self,
        _mission_id: Uuid,
        active_sessions: &mut HashMap<Uuid, ActiveSession>,
        stats: &mut WatchdogStats,
    ) -> Result<()> {
        let now = Instant::now();
        let mut zombie_session_ids = Vec::new();
        for (session_id, session) in active_sessions.iter() {
            if session.record.role == SessionRole::Supervisor || session.state.is_terminal() {
                continue;
            }
            if has_recent_status_activity(session, now) || !session.queued_prompts.is_empty() {
                continue;
            }
            let idle_duration = now.duration_since(session.last_confirmed_alive);
            if idle_duration <= Duration::from_secs(60) {
                continue;
            }
            match session_tmux_health(session) {
                Some(tmux::SessionHealth::Zombie | tmux::SessionHealth::Dead) => {
                    zombie_session_ids.push(*session_id)
                }
                _ => {}
            }
        }
        for session_id in zombie_session_ids {
            if let Some(session) = active_sessions.get_mut(&session_id) {
                let should_restart = session.zombie_debounce.record_zombie();
                if should_restart {
                    stats.critical_failures += 1;
                    session.zombie_debounce.record_alive();
                }
            }
        }
        Ok(())
    }

    pub(super) fn handle_worker_liveness_incidents(
        &self,
        mission_id: Uuid,
        repo_root: &Path,
        supervisor_id: Uuid,
        active_sessions: &mut HashMap<Uuid, ActiveSession>,
        pending_supervisor_decisions: &mut HashMap<String, PendingSupervisorDecision>,
        supervisor_degraded: bool,
        stats: &mut WatchdogStats,
        control_surface: &ControlSurface,
    ) -> Result<()> {
        let now = Instant::now();
        let worker_ids = active_sessions
            .iter()
            .filter_map(|(session_id, session)| {
                (session.record.role == SessionRole::Worker && !session.state.is_terminal())
                    .then_some(*session_id)
            })
            .collect::<Vec<_>>();
        if worker_ids.is_empty() {
            return Ok(());
        }
        let mut overdue = Vec::<(Uuid, WorkerLivenessAssessment)>::new();
        for session_id in &worker_ids {
            let Some(session) = active_sessions.get(session_id) else {
                continue;
            };
            let assessment = assess_worker_liveness(session, control_surface, now);
            if assessment.first_status_overdue {
                overdue.push((*session_id, assessment));
            }
        }
        let systemic_incident =
            health::is_systemic_first_status_incident(worker_ids.len(), overdue.len());
        if systemic_incident && !supervisor_degraded {
            let names = overdue
                .iter()
                .filter_map(|(session_id, _)| active_sessions.get(session_id))
                .map(|s| s.record.name.clone())
                .collect::<Vec<_>>();
            let notice = format!(
                "Systemic first-status incident: {} of {} workers missed first-status SLA together ({}). Treat this as a control-plane failure until disproven. Verify prompt delivery, runtime spawn, transcript flow, and status-path writes immediately.",
                overdue.len(),
                worker_ids.len(),
                names.join(", ")
            );
            self.send_supervisor_notice(
                mission_id,
                supervisor_id,
                active_sessions,
                SupervisorEventType::Failed,
                &notice,
            )?;
        }
        let mut forced_failures = Vec::<(Uuid, String)>::new();
        for (session_id, mut assessment) in overdue {
            if systemic_incident {
                assessment.incident_scope = health::IncidentScope::Systemic;
            }
            let Some(session) = active_sessions.get_mut(&session_id) else {
                continue;
            };
            let stage = session.first_status_incident_stage;
            if session
                .last_first_status_escalation_at
                .is_some_and(|at| now.duration_since(at) < first_status_escalation_interval(stage))
            {
                continue;
            }
            let (prompt, summary, event_type) = match stage {
                0 => (
                    first_status_probe_prompt(session, &assessment),
                    format!(
                        "{} missed first-status SLA. Diagnosis: {}.",
                        session.record.name, assessment.diagnosis
                    ),
                    SupervisorEventType::WeakOutput,
                ),
                1 => (
                    first_status_recovery_prompt(session, &assessment, control_surface),
                    format!(
                        "{} still owes first status after probe. Recovery action: {}.",
                        session.record.name, assessment.diagnosis
                    ),
                    SupervisorEventType::Stall,
                ),
                _ => {
                    if assessment.state == health::WorkerLivenessState::Nonresponsive {
                        forced_failures.push((
                            session_id,
                            format!("Worker {} never produced a first Sapphire status after dispatch recovery attempts. Final diagnosis: {}.", session.record.name, assessment.diagnosis),
                        ));
                        session.first_status_incident_stage =
                            session.first_status_incident_stage.saturating_add(1);
                        session.last_first_status_escalation_at = Some(now);
                        continue;
                    }
                    (
                        first_status_final_probe_prompt(&assessment),
                        format!(
                            "{} remains alive but unreported after repeated first-status recovery attempts. Demand exact status artifact now. Diagnosis: {}.",
                            session.record.name, assessment.diagnosis
                        ),
                        SupervisorEventType::WeakOutput,
                    )
                }
            };
            let delivered = send_or_queue_prompt(session, &prompt);
            record_intervention(session, "first_status_incident", now);
            session.first_status_incident_stage =
                session.first_status_incident_stage.saturating_add(1);
            session.last_first_status_escalation_at = Some(now);
            session.record.last_summary = Some(summary.clone());
            let _ = self.store.update_worker_summary(session_id, &summary);
            self.store.append_json_event(
                mission_id,
                Some(session_id),
                "first_status_incident",
                &summary,
                &json!({
                    "liveness_state": assessment.state.as_str(),
                    "incident_scope": assessment.incident_scope.as_str(),
                    "failure_kind": assessment.failure_kind.map(first_status_failure_kind_name),
                    "diagnosis": assessment.diagnosis,
                    "prompt_path_ready": assessment.prompt_path_ready,
                    "status_path_ready": assessment.status_path_ready,
                    "transcript_path_ready": assessment.transcript_path_ready,
                    "transcript_has_output": assessment.transcript_has_output,
                    "runtime_live": assessment.runtime_live,
                    "delivered": delivered,
                    "stage": stage + 1,
                }),
            )?;
            let reason = format!(
                "{} Choose retry_worker, message_worker, redirect_worker, or fail_worker. Do not observe. Worker diagnosis: {}.",
                summary, assessment.diagnosis
            );
            if queue_supervisor_decision(
                pending_supervisor_decisions,
                SupervisorDecisionKind::LowConfidenceRecovery,
                session_id,
                &reason,
            ) && !supervisor_degraded
            {
                self.send_worker_supervisor_notice(
                    mission_id,
                    supervisor_id,
                    session_id,
                    active_sessions,
                    event_type,
                    &reason,
                )?;
            }
        }
        for (session_id, summary) in forced_failures {
            stats.critical_failures += 1;
            self.handle_status_directive(
                mission_id,
                repo_root,
                supervisor_id,
                session_id,
                StatusDirective {
                    state: SessionState::Failed.as_str().to_owned(),
                    summary,
                    files: Vec::new(),
                    commands: Vec::new(),
                    risks: vec!["first-status incident".to_owned()],
                    overlap: None,
                },
                false,
                active_sessions,
                pending_supervisor_decisions,
                supervisor_degraded,
                stats,
            )?;
        }
        Ok(())
    }

    pub(super) fn handle_stalls(
        &self,
        mission_id: Uuid,
        repo_root: &Path,
        supervisor_id: Uuid,
        stall_after: Duration,
        active_sessions: &mut HashMap<Uuid, ActiveSession>,
        pending_supervisor_decisions: &mut HashMap<String, PendingSupervisorDecision>,
        supervisor_degraded: bool,
        stats: &mut WatchdogStats,
    ) -> Result<()> {
        let now = Instant::now();
        let mut stalled_ids = Vec::new();
        for (session_id, session) in active_sessions.iter() {
            if session.record.role == SessionRole::Supervisor || session.state.is_terminal() {
                continue;
            }
            if now < session.startup_grace_until {
                continue;
            }
            if has_recent_status_activity(session, now)
                || session_has_live_terminal(session)
                || !session.queued_prompts.is_empty()
                || recently_prompted(session, now)
            {
                continue;
            }
            if now.duration_since(session.last_confirmed_alive)
                >= effective_stall_threshold(session, stall_after)
                && session.state != SessionState::Stalled
            {
                stalled_ids.push(*session_id);
            }
        }
        for session_id in stalled_ids {
            let (consecutive, worker_name, in_cooldown) = {
                let session = active_sessions.get(&session_id).unwrap();
                (
                    session.consecutive_stall_failures + 1,
                    session.record.name.clone(),
                    is_in_cooldown(session, now),
                )
            };
            if let Some(session) = active_sessions.get_mut(&session_id) {
                session.state = SessionState::Stalled;
                session.stall_count += 1;
                session.consecutive_stall_failures = consecutive;
            }
            self.store
                .update_session_state(session_id, SessionState::Stalled)?;
            if in_cooldown {
                continue;
            }
            if consecutive >= 3 {
                self.handle_status_directive(
                    mission_id,
                    repo_root,
                    supervisor_id,
                    session_id,
                    StatusDirective {
                        state: SessionState::Failed.as_str().to_owned(),
                        summary: format!(
                            "Worker {} has stalled {} consecutive times. Corrective prompts had no effect. Supervisor must decide: respawn or reassign.",
                            worker_name, consecutive
                        ),
                        files: Vec::new(),
                        commands: Vec::new(),
                        risks: Vec::new(),
                        overlap: None,
                    },
                    false,
                    active_sessions,
                    pending_supervisor_decisions,
                    supervisor_degraded,
                    stats,
                )?;
                stats.stall_interventions += 1;
                if let Some(session) = active_sessions.get_mut(&session_id) {
                    record_intervention(session, "stall_fail", now);
                }
            } else {
                let reason = if supervisor_degraded {
                    format!(
                        "Worker {} appears stalled, but the supervisor is degraded.",
                        worker_name
                    )
                } else {
                    format!(
                        "Worker {} appears stalled. Decide whether to message_worker, retry_worker, redirect_worker, or validate_worker.",
                        worker_name
                    )
                };
                if queue_supervisor_decision(
                    pending_supervisor_decisions,
                    SupervisorDecisionKind::StallRecovery,
                    session_id,
                    &reason,
                ) {
                    self.send_worker_supervisor_notice(
                        mission_id,
                        supervisor_id,
                        session_id,
                        active_sessions,
                        SupervisorEventType::Stall,
                        &reason,
                    )?;
                }
            }
        }
        Ok(())
    }
}

pub(super) fn assess_worker_liveness(
    session: &ActiveSession,
    control_surface: &ControlSurface,
    now: Instant,
) -> WorkerLivenessAssessment {
    let prompt_path = worker_prompt_file_path(control_surface, &session.record.name);
    let status_path = worker_status_file_path(control_surface, &session.record.name);
    let transcript_path = worker_transcript_path(control_surface, &session.record.name);
    let prompt_path_ready = prompt_path.exists();
    let status_path_ready = status_path.parent().is_some_and(Path::exists);
    let transcript_path_ready = transcript_path.exists();
    let transcript_has_output = transcript_path
        .metadata()
        .map(|metadata| metadata.len() > 0)
        .unwrap_or(false);
    let terminal_live = session_tmux_health(session).is_some_and(|health| {
        matches!(
            health,
            tmux::SessionHealth::Healthy
                | tmux::SessionHealth::Hung
                | tmux::SessionHealth::Starting
        )
    });
    let runtime_live = terminal_live
        || session.output_chunks > 0
        || session
            .last_status_update_at
            .is_some_and(|at| now.duration_since(at) <= Duration::from_secs(45));
    let first_status_overdue = !session.initial_status_received
        && session
            .launch_prompt_sent_at
            .is_some_and(|at| now.duration_since(at) >= first_status_deadline())
        && now >= session.startup_grace_until;

    let (state, failure_kind, diagnosis) = if session.state == SessionState::Failed {
        (
            health::WorkerLivenessState::Failed,
            None,
            "worker already failed".to_owned(),
        )
    } else if session.state == SessionState::Validated {
        (
            health::WorkerLivenessState::Done,
            None,
            "worker already completed".to_owned(),
        )
    } else if session.state == SessionState::Blocked {
        (
            health::WorkerLivenessState::Blocked,
            None,
            "worker explicitly reported blocked".to_owned(),
        )
    } else if session.state == SessionState::Stalled {
        (
            health::WorkerLivenessState::Stalled,
            Some(FirstStatusFailureKind::Runtime),
            "worker hit stall handling".to_owned(),
        )
    } else if !session.launch_prompt_sent {
        (
            health::WorkerLivenessState::Assigned,
            Some(FirstStatusFailureKind::Dispatch),
            "assignment has not been dispatched into the worker terminal yet".to_owned(),
        )
    } else if now < session.startup_grace_until {
        (
            health::WorkerLivenessState::Booting,
            None,
            "worker is still within startup grace".to_owned(),
        )
    } else if session.initial_status_received {
        let state = if session.last_status_update_at.is_some() {
            health::WorkerLivenessState::Reporting
        } else if session.output_chunks > 0 {
            health::WorkerLivenessState::Executing
        } else {
            health::WorkerLivenessState::AliveConfirmed
        };
        (
            state,
            None,
            "worker emitted a real Sapphire status".to_owned(),
        )
    } else if session.output_chunks > 0 || transcript_has_output {
        let diagnosis = if session.directive_count == 0 && session.last_status_update_at.is_none() {
            "worker produced terminal output but no first Sapphire status; reporting path is failing or the worker is not obeying the reporting contract"
        } else {
            "worker produced activity without a confirmed first status"
        };
        (
            health::WorkerLivenessState::AliveUnconfirmed,
            Some(FirstStatusFailureKind::StatusPipeline),
            diagnosis.to_owned(),
        )
    } else if runtime_live {
        (
            health::WorkerLivenessState::PromptDelivered,
            Some(FirstStatusFailureKind::Reporting),
            "dispatch landed but Sapphire has not seen first-status evidence yet".to_owned(),
        )
    } else {
        let failure_kind = if prompt_path_ready && status_path_ready {
            FirstStatusFailureKind::Runtime
        } else {
            FirstStatusFailureKind::Dispatch
        };
        (
            health::WorkerLivenessState::Nonresponsive,
            Some(failure_kind),
            format!(
                "worker is silent past first-status SLA with no confirmed runtime liveness (prompt_path_ready={} status_path_ready={} runtime_capture_ready={})",
                prompt_path_ready, status_path_ready, transcript_path_ready
            ),
        )
    };

    WorkerLivenessAssessment {
        state,
        incident_scope: if first_status_overdue {
            health::IncidentScope::Local
        } else {
            health::IncidentScope::None
        },
        first_status_overdue,
        failure_kind,
        prompt_path_ready,
        status_path_ready,
        transcript_path_ready,
        transcript_has_output,
        runtime_live,
        diagnosis,
    }
}

pub(super) fn first_status_probe_prompt(
    _session: &ActiveSession,
    assessment: &WorkerLivenessAssessment,
) -> String {
    match assessment.failure_kind {
        Some(FirstStatusFailureKind::StatusPipeline | FirstStatusFailureKind::Reporting) => {
            "First-status SLA breached. Sapphire has activity but no authoritative first status from you.\nWrite the real status file now or emit one raw SAPPHIRE_STATUS line immediately.\nState the exact current state, exact next action, touched files, and blockers. No narration. No plan-only response.".to_owned()
        }
        _ => "Liveness probe. Sapphire has not received your first status.\nIf your assignment is visible, write your first status right now.\nIf your terminal lost the assignment, say so in one raw SAPPHIRE_STATUS line and state whether you can still access the prompt file.\nDo not explain the plan. Report the current execution state now.".to_owned(),
    }
}

pub(super) fn first_status_recovery_prompt(
    session: &ActiveSession,
    assessment: &WorkerLivenessAssessment,
    control_surface: &ControlSurface,
) -> String {
    let prompt_path = worker_prompt_file_path(control_surface, &session.record.name);
    let status_path = worker_status_file_path(control_surface, &session.record.name);
    match assessment.failure_kind {
        Some(FirstStatusFailureKind::Dispatch | FirstStatusFailureKind::Runtime) => format!(
            "Dispatch recovery.\nRe-read your assignment at:\n- {}\nThen create your first status at:\n- {}\nIf that write fails, emit one raw SAPPHIRE_STATUS line immediately.\nDo not paraphrase the assignment. Start executing and report real state.",
            prompt_path.display(),
            status_path.display(),
        ),
        _ => format!(
            "Reporting recovery.\nEmit one raw SAPPHIRE_STATUS line immediately, then write the same status to:\n- {}\nRequired fields: state, summary, files, commands, risks, overlap.\nNo broad narration.",
            status_path.display(),
        ),
    }
}

pub(super) fn first_status_final_probe_prompt(assessment: &WorkerLivenessAssessment) -> String {
    format!(
        "Final first-status recovery. Sapphire still has no authoritative first status from you.\nDiagnosis: {}\nEmit one exact SAPPHIRE_STATUS line now with your true state and concrete evidence, or the control plane will treat this terminal as nonresponsive.",
        assessment.diagnosis
    )
}

impl Orchestrator {
    pub(super) fn handle_protocol_reminders(
        &self,
        mission_id: Uuid,
        supervisor_id: Uuid,
        active_sessions: &mut HashMap<Uuid, ActiveSession>,
        pending_supervisor_decisions: &mut HashMap<String, PendingSupervisorDecision>,
        stats: &mut WatchdogStats,
    ) -> Result<()> {
        if !communication_policy::WATCHDOG_PROTOCOL_REMINDERS {
            let mut low_observability = Vec::new();
            let now = Instant::now();
            for (session_id, session) in active_sessions.iter() {
                if session.record.role != SessionRole::Worker
                    || session.state.is_terminal()
                    || session.protocol_reminder_sent
                    || now < session.startup_grace_until
                    || !session.queued_prompts.is_empty()
                    || recently_prompted(session, now)
                    || is_in_cooldown(session, now)
                {
                    continue;
                }
                let Some(report_kind) = enforcement::pending_report_back(session, now) else {
                    continue;
                };
                low_observability.push((*session_id, report_kind));
            }
            for (session_id, report_kind) in low_observability {
                let Some(session) = active_sessions.get_mut(&session_id) else {
                    continue;
                };
                session.protocol_reminder_sent = true;
                let summary = enforcement::status_summary(report_kind, &session.record.name);
                session.record.last_summary = Some(summary.clone());
                record_intervention(session, "status_enforcement_escalated", now);
                self.store.update_worker_summary(session_id, &summary)?;
                stats.protocol_reminders += 1;
                let reason = enforcement::status_reason(report_kind, &session.record.name);
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
            return Ok(());
        }
        Ok(())
    }
}
