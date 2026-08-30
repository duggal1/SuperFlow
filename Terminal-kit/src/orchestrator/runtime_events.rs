use super::*;

impl Orchestrator {
    pub(super) fn handle_runtime_event(
        &self,
        mission_id: Uuid,
        repo_root: &Path,
        active_supervisor_id: Uuid,
        event: &RuntimeEvent,
        active_sessions: &mut HashMap<Uuid, ActiveSession>,
        alias_map: &HashMap<String, Uuid>,
        leases: &mut HashMap<String, LeaseOwner>,
        pending_mail: &mut HashMap<Uuid, PendingMail>,
        pending_supervisor_decisions: &mut HashMap<String, PendingSupervisorDecision>,
        recent_failures: &mut Vec<RecentFailure>,
        mass_death_detector: &mut health::MassDeathDetector,
        supervisor_degraded: bool,
        stats: &mut WatchdogStats,
        control_surface: &ControlSurface,
    ) -> Result<()> {
        persist_runtime_event(&self.store, mission_id, event)?;
        match event {
            RuntimeEvent::Output { session_id, chunk } => {
                let mut directives = Vec::new();
                let mut supervisor_action = None;
                let mut final_envelope = None;
                let mut runtime_failure_signal = None;
                let mut should_reset_restart_tracker = false;
                let mut should_clear_transient_supervisor_decisions = false;
                if let Some(session) = active_sessions.get_mut(session_id) {
                    let now = Instant::now();
                    session.last_output_at = now;
                    session.last_confirmed_alive = now;
                    session.consecutive_stall_failures = 0;
                    session.output_chunks += 1;
                    session.health_state.record_response();
                    session.zombie_debounce.record_alive();
                    if session.restart_count > 0 || session.restart_at.is_some() {
                        session.restart_count = 0;
                        session.restart_at = None;
                        should_reset_restart_tracker = true;
                    }
                    self.store.update_worker_heartbeat(*session_id)?;
                    record_intervention_response(session, now);
                    if session.record.role == SessionRole::Worker {
                        clear_superseded_prompt_queue(session);
                        should_clear_transient_supervisor_decisions = true;
                    }
                    let sanitized = crate::protocol::sanitize_output(chunk);
                    directives = consume_directives(&mut session.line_buffer, &sanitized);
                    session.raw_buffer.push_str(&sanitized);
                    trim_recent_utf8(&mut session.raw_buffer, 64_000, 32_000);
                    let adapter = adapter_for(session.record.agent);
                    if session.record.role == SessionRole::Supervisor
                        && *session_id == active_supervisor_id
                    {
                        supervisor_action = adapter.extract_supervisor_action(&session.raw_buffer);
                        final_envelope = adapter.extract_final_envelope(&session.raw_buffer);
                    }
                    if directives.is_empty() {
                        runtime_failure_signal = runtime_failures::detect_runtime_failure(
                            session.record.role,
                            session.record.agent,
                            &sanitized,
                        );
                    }
                    if control_surface.persist_transcripts {
                        append_transcript(control_surface, &session.record.name, &sanitized)?;
                    }
                }
                if should_reset_restart_tracker {
                    let _ = self.store.reset_restart_tracker(*session_id);
                }
                if should_clear_transient_supervisor_decisions {
                    clear_transient_supervisor_decisions_on_output(
                        pending_supervisor_decisions,
                        *session_id,
                    );
                }
                for directive in directives {
                    stats.directives += 1;
                    if let Some(session) = active_sessions.get_mut(session_id) {
                        session.directive_count += 1;
                        session.protocol_reminder_sent = false;
                    }
                    self.store.append_json_event(
                        mission_id,
                        Some(*session_id),
                        "directive",
                        directive_kind(&directive),
                        &directive,
                    )?;
                    match directive {
                        SapphireDirective::Status(status) => {
                            self.persist_normalized_status(
                                mission_id,
                                *session_id,
                                &status.state,
                                "sapphire_directive",
                                Confidence::High,
                                &status.summary,
                                &crate::protocol::sanitize_output(chunk),
                            )?;
                            self.handle_status_directive(
                                mission_id,
                                repo_root,
                                active_supervisor_id,
                                *session_id,
                                status,
                                false,
                                active_sessions,
                                pending_supervisor_decisions,
                                supervisor_degraded,
                                stats,
                            )?
                        }
                        SapphireDirective::Mail(mail) => {
                            if let Some(mail_id) = &mail.mail_id {
                                if let Some(session) = active_sessions.get_mut(session_id) {
                                    if session.message_dedup.already_processed(mail_id) {
                                        tracing::debug!(mail_id = %mail_id, "mail directive deduped — already processed");
                                        continue;
                                    }
                                    session.message_dedup.mark_processed(mail_id);
                                }
                            }
                            self.handle_mail_directive(
                                &control_surface.state_dir,
                                mission_id,
                                active_supervisor_id,
                                *session_id,
                                mail,
                                active_sessions,
                                alias_map,
                                pending_mail,
                                stats,
                            )?
                        }
                        SapphireDirective::Ack(ack) => self.handle_ack_directive(
                            mission_id,
                            active_supervisor_id,
                            *session_id,
                            ack,
                            active_sessions,
                            pending_mail,
                        )?,
                        SapphireDirective::Lease(lease) => self.handle_lease_directive(
                            mission_id,
                            active_supervisor_id,
                            *session_id,
                            lease,
                            active_sessions,
                            leases,
                            stats,
                        )?,
                    }
                }
                if let Some(action) = supervisor_action {
                    self.apply_supervisor_action(
                        mission_id,
                        repo_root,
                        active_supervisor_id,
                        action,
                        active_sessions,
                        alias_map,
                        pending_supervisor_decisions,
                        stats,
                    )?;
                }
                if let Some(final_envelope) = final_envelope {
                    self.apply_final_envelope(
                        mission_id,
                        active_supervisor_id,
                        final_envelope,
                        active_sessions,
                    )?;
                }
                if let Some(signal) = runtime_failure_signal {
                    self.handle_runtime_failure_signal(
                        mission_id,
                        active_supervisor_id,
                        *session_id,
                        signal,
                        active_sessions,
                        pending_supervisor_decisions,
                    )?;
                }
            }
            RuntimeEvent::Automation { session_id, .. } => {
                if let Some(session) = active_sessions.get_mut(session_id) {
                    session.last_output_at = Instant::now();
                    session.last_confirmed_alive = Instant::now();
                    session.consecutive_stall_failures = 0;
                    self.store.update_worker_heartbeat(*session_id)?;
                }
            }
            RuntimeEvent::Exited {
                session_id,
                exit_code,
            } => {
                let mut crash_loop_notice = None;
                if let Some(session) = active_sessions.get_mut(session_id) {
                    let previous_state = session.state;
                    let mut should_restart = false;
                    let mut restart_delay = Duration::default();
                    let mut crash_loop = false;
                    if !previous_state.is_terminal() {
                        let restart_record =
                            self.store.upsert_restart_attempt(*session_id, mission_id)?;
                        session.restart_count = restart_record.restart_count;
                        restart_delay = Duration::from_secs_f64(
                            restart_record
                                .backoff_seconds
                                .max(restart_base_secs() as f64),
                        );
                        crash_loop = self.store.is_crash_loop(
                            *session_id,
                            restart_crash_loop_threshold(),
                            restart_crash_loop_window(),
                        )?;
                        if crash_loop {
                            crash_loop_notice = Some(format!(
                                "{} entered a crash loop after {} restart attempts in the last {}s.",
                                session.record.name,
                                restart_record.restart_count,
                                restart_crash_loop_window().as_secs()
                            ));
                        }
                        should_restart = !crash_loop
                            && should_auto_restart(
                                session.record.role,
                                previous_state,
                                session.restart_count,
                            );
                    }
                    if should_restart {
                        session.restart_at = Some(Instant::now() + restart_delay);
                        session.state = SessionState::Booting;
                        session.startup_grace_until =
                            Instant::now() + startup_grace(session.record.agent);
                        session.record.last_summary = Some(format!(
                            "{} exited; restart {} scheduled in {}s",
                            session.record.name,
                            session.restart_count,
                            restart_delay.as_secs()
                        ));
                        self.store
                            .update_session_state(*session_id, SessionState::Booting)?;
                        self.store.update_worker_summary(
                            *session_id,
                            session
                                .record
                                .last_summary
                                .as_deref()
                                .unwrap_or("restart scheduled"),
                        )?;
                    } else if crash_loop {
                        session.state = SessionState::Failed;
                        self.store
                            .update_session_state(*session_id, SessionState::Failed)?;
                        if let Some(task_id) = session.task_id {
                            let _ = self.store.update_task_status(task_id, "failed");
                        }
                    } else {
                        session.state = SessionState::Exited;
                        self.store
                            .update_session_state(*session_id, SessionState::Exited)?;
                        if let Some(task_id) = session.task_id {
                            let _ = self.store.update_task_status(task_id, "exited");
                        }
                    }
                    self.store.append_json_event(
                        mission_id,
                        Some(*session_id),
                        "session_state",
                        "session exited",
                        &json!({ "name": session.record.name, "exit_code": exit_code }),
                    )?;
                    self.store.append_summary(
                        mission_id,
                        Some(*session_id),
                        "exit",
                        if should_restart {
                            format!(
                                "{} exited and was queued for restart {}",
                                session.record.name, session.restart_count
                            )
                        } else if crash_loop_notice.is_some() {
                            format!(
                                "{} entered a crash loop and was marked failed",
                                session.record.name
                            )
                        } else {
                            format!("{} exited", session.record.name)
                        },
                    )?;
                    if session.record.role == SessionRole::Worker && !should_restart {
                        recent_failures.push(RecentFailure {
                            recorded_at: Instant::now(),
                        });
                        if let Some(mass_death_event) =
                            mass_death_detector.record_death(&session.record.name)
                        {
                            stats.mass_deaths_detected += 1;
                            stats.critical_failures += 1;
                            tracing::error!(mass_death_count = mass_death_event.count, dead_sessions = ?mass_death_event.dead_sessions, "MASS DEATH DETECTED: {} sessions died within {:?}", mass_death_event.count, mass_death_event.window);
                        }
                        trim_recent_failures(recent_failures);
                    }
                }
                if let Some(notice) = crash_loop_notice {
                    stats.crash_loops_detected += 1;
                    self.store.append_json_event(
                        mission_id,
                        Some(active_supervisor_id),
                        "crash_loop_detected",
                        &notice,
                        &json!({ "session_id": session_id.to_string(), "exit_code": exit_code }),
                    )?;
                    self.send_worker_supervisor_notice(
                        mission_id,
                        active_supervisor_id,
                        *session_id,
                        active_sessions,
                        SupervisorEventType::Failed,
                        &notice,
                    )?;
                }
            }
        }
        Ok(())
    }
}
