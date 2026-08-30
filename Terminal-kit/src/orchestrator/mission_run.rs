use super::*;

impl Orchestrator {
    pub(super) async fn run_live_mission(
        &self,
        mission_id: Uuid,
        config: &LaunchConfig,
        supervisor_session: SessionRecord,
        supervisor_task_id: Uuid,
        supervisor_spec: ProcessLaunchSpec,
        supervisor_prompt: String,
        worker_launches: Vec<WorkerLaunch>,
        mission_profile: MissionProfile,
        control_surface: &ControlSurface,
    ) -> Result<WatchdogStats> {
        let tmux_sessions = if !control_surface.tmux_session_names.is_empty() {
            let session_names = self.ensure_tmux_surface(
                &control_surface.tmux_session_names[0],
                mission_id,
                config,
            )?;
            std::thread::sleep(std::time::Duration::from_secs(2));
            #[cfg(target_os = "macos")]
            {
                let tmux = tmux::Tmux::new(None);
                if let Err(e) = tmux.open_ghostty_batch_tabs(&session_names) {
                    tracing::warn!("could not auto-open Ghostty tabs for tmux sessions: {}", e);
                }
            }
            session_names
        } else {
            Vec::new()
        };

        let mut supervisor_runtime = SessionRuntime::new();
        let mut worker_runtimes: Vec<SessionRuntime> = tmux_sessions
            .iter()
            .map(|session_name| {
                SessionRuntime::with_tmux(
                    session_name.clone(),
                    control_surface.transcript_dir.clone(),
                )
            })
            .collect();
        let tick = Duration::from_millis(config.watchdog_tick_millis);
        let stall_after = Duration::from_secs(config.stall_seconds);
        let max_runtime = config.watchdog_max_seconds.map(Duration::from_secs);
        let started_at = Instant::now();
        let primary_supervisor_id = supervisor_session.id;
        let mut active_supervisor_id = primary_supervisor_id;
        let supervisor_count = if mission_profile.enable_supervisor_team {
            supervisor_team::recommended_supervisor_count(worker_launches.len())
        } else {
            1
        }
        .max(1);
        let mut supervisor_ids = Vec::with_capacity(supervisor_count);
        let mut worker_continuity_announced = false;
        let mut stats = WatchdogStats::default();
        let mut mass_death_detector = health::MassDeathDetector::default();
        let mut active_sessions = HashMap::<Uuid, ActiveSession>::new();
        let mut alias_map = HashMap::<String, Uuid>::new();
        let mut leases = HashMap::<String, LeaseOwner>::new();
        let mut pending_mail = HashMap::<Uuid, PendingMail>::new();
        let mut pending_supervisor_decisions = HashMap::<String, PendingSupervisorDecision>::new();
        let mut recent_failures = Vec::<RecentFailure>::new();
        let mut final_synthesis_requested = false;
        let mut final_synthesis_requested_at = None;
        let mut final_synthesis_wakeup_sent = false;
        let mut final_synthesis_escalated = false;
        let mut supervisor_mode = SupervisorMode::Healthy;
        let mut last_state_card_sent = Instant::now();
        let state_card_interval = Duration::from_secs(supervisor::STATE_CARD_INTERVAL_SECS);
        let status_snapshot_interval = Duration::from_secs(1);
        let full_surface_interval = Duration::from_secs(5);
        let tmux_live = !control_surface.tmux_session_names.is_empty();

        let supervisor_assignments = supervisor_team::assign_packets_to_supervisors(
            &worker_launches
                .iter()
                .map(|w| w.packet.clone())
                .collect::<Vec<_>>(),
            supervisor_count,
        );
        let mut supervisor_sessions = Vec::new();
        for index in 0..supervisor_count {
            let branch_name = supervisor_team::supervisor_branch_name(index);
            let is_primary = index == 0;
            let session = if is_primary {
                supervisor_session.clone()
            } else {
                SessionRecord {
                    id: Uuid::new_v4(),
                    mission_id,
                    role: SessionRole::Supervisor,
                    ordinal: supervisor_session.ordinal + index,
                    agent: supervisor_session.agent,
                    terminal_id: branch_name.clone(),
                    name: branch_name.clone(),
                    owned_scope: "active supervision branch".to_owned(),
                    status: SessionState::Booting,
                    launch_command: launch_command(
                        supervisor_spec.program.as_str(),
                        &supervisor_spec.args,
                    ),
                    last_heartbeat_at: Utc::now(),
                    last_summary: Some("Supervisor branch active".to_owned()),
                }
            };
            if !is_primary {
                self.store.persist_session(&session, None)?;
                let task_id = Uuid::new_v4();
                self.store.persist_task(&TaskRecord {
                    id: task_id,
                    mission_id,
                    worker_id: session.id,
                    title: "Supervision branch".to_owned(),
                    description: "Actively supervise assigned workers; enforce evidence and progress; coordinate via mail.".to_owned(),
                    status: "assigned".to_owned(),
                    priority: "high".to_owned(),
                    depends_on_json: "[]".to_owned(),
                    definition_of_done_json: serde_json::to_string(&vec![
                        "Assigned workers resolved".to_owned(),
                        "Evidence enforced".to_owned(),
                        "No passive observe loops".to_owned(),
                    ])?,
                })?;
            }
            supervisor_ids.push(session.id);
            supervisor_sessions.push((session, index));
        }
        let worker_supervisor_map = supervisor_sessions
            .iter()
            .flat_map(|(session, index)| {
                supervisor_assignments
                    .get(*index)
                    .into_iter()
                    .flatten()
                    .map(move |worker_name| (worker_name.clone(), session.id))
            })
            .collect::<HashMap<_, _>>();

        let mut supervisor_runnings = Vec::new();
        for (session, index) in &supervisor_sessions {
            let branch_name = session.name.clone();
            let branch_prompt = supervisor_team::branch_prompt(
                &supervisor_prompt,
                &branch_name,
                supervisor_assignments
                    .get(*index)
                    .map(|v| v.as_slice())
                    .unwrap_or(&[]),
            );
            write_prompt_file(&config.state_dir, &branch_name, &branch_prompt)?;
            let mut spec = supervisor_spec.clone();
            spec.surface_label = branch_name.clone();
            let (spec, prompt_embedded) = if tmux_live {
                (spec, false)
            } else {
                embed_initial_prompt_if_supported(session.agent, spec, &branch_prompt)
            };
            let running = supervisor_runtime.spawn(session.id, spec.clone())?;
            supervisor_runnings.push((
                session.clone(),
                spec,
                branch_prompt,
                prompt_embedded,
                running,
            ));
        }
        if tmux_live {
            std::thread::sleep(std::time::Duration::from_millis(800));
        }
        let mut prompt_queue = Vec::<(Uuid, Duration, String)>::new();
        for (session, spec, prompt, prompt_embedded, running) in supervisor_runnings {
            let mut aliases = vec![session.name.clone()];
            aliases.push(session.name.clone());
            if session.id == primary_supervisor_id {
                aliases.push("supervisor".to_owned());
                aliases.push("supervisor-01".to_owned());
            }
            register_session(
                &mut active_sessions,
                &mut alias_map,
                session.clone(),
                None,
                running,
                None,
                spec,
                prompt.clone(),
                if session.id == primary_supervisor_id {
                    Some(supervisor_task_id)
                } else {
                    None
                },
                aliases,
            );
            let _ = self
                .store
                .update_session_state(session.id, SessionState::Progressing);
            if !prompt_embedded {
                let prompt_delay = active_sessions
                    .get(&session.id)
                    .map(|active| active.runtime.prompt_delay())
                    .unwrap_or_default();
                prompt_queue.push((session.id, prompt_delay, prompt));
            }
        }

        for (worker_index, launch) in worker_launches.into_iter().enumerate() {
            let (launch_spec, prompt_embedded) = if tmux_live {
                (launch.launch_spec, false)
            } else {
                embed_initial_prompt_if_supported(
                    launch.session.agent,
                    launch.launch_spec,
                    &launch.prompt,
                )
            };
            let runtime_slot = if worker_runtimes.is_empty() {
                None
            } else {
                Some(worker_index % worker_runtimes.len())
            };
            let running = if let Some(runtime_slot) = runtime_slot {
                worker_runtimes[runtime_slot].spawn(launch.session.id, launch_spec.clone())?
            } else {
                supervisor_runtime.spawn(launch.session.id, launch_spec.clone())?
            };
            if tmux_live {
                std::thread::sleep(std::time::Duration::from_millis(400));
            }
            let prompt_delay = running.prompt_delay();
            info!(
                "launched {} via {}",
                launch.session.name,
                running.display_name()
            );
            register_session(
                &mut active_sessions,
                &mut alias_map,
                launch.session.clone(),
                Some(launch.packet.clone()),
                running,
                runtime_slot,
                launch_spec.clone(),
                launch.prompt.clone(),
                launch.task_id,
                vec![
                    launch.session.name.clone(),
                    launch.packet.display_name.clone(),
                    launch.packet.worker_id.clone(),
                ],
            );
            self.store
                .update_session_state(launch.session.id, SessionState::NotStarted)?;
            if let Some(session) = active_sessions.get_mut(&launch.session.id) {
                session.state = SessionState::NotStarted;
                session.supervising_supervisor_id = worker_supervisor_map
                    .get(&launch.packet.display_name)
                    .copied();
                session.record.last_summary =
                    Some("Assignment delivered; awaiting first Sapphire status.".to_owned());
            }
            self.store.update_worker_summary(
                launch.session.id,
                "Assignment delivered; awaiting first Sapphire status.",
            )?;
            if !prompt_embedded {
                prompt_queue.push((launch.session.id, prompt_delay, launch.prompt));
            }
        }

        prompt_queue.sort_by_key(|(_, delay, _)| *delay);
        let mut waited = Duration::default();
        for (session_id, delay, prompt) in prompt_queue {
            let additional = delay.saturating_sub(waited);
            if !additional.is_zero() {
                tokio::time::sleep(additional).await;
                waited = delay;
            }
            if let Some(session) = active_sessions.get_mut(&session_id) {
                if session.launch_prompt_sent {
                    info!(session = %session_id, worker = %session.record.name, "launch prompt already sent, skipping");
                    continue;
                }
                send_prompt_immediately(session, &prompt)?;
                session.launch_prompt_sent = true;
                session.launch_prompt_sent_at = Some(Instant::now());
            }
        }

        self.prime_supervisor_backlog(
            mission_id,
            active_supervisor_id,
            &active_sessions,
            &mut pending_supervisor_decisions,
        )?;
        self.write_status_snapshot(
            mission_id,
            active_supervisor_id,
            control_surface,
            &active_sessions,
            &pending_mail,
            &stats,
            true,
        )?;
        let mut last_snapshot_written_at = Instant::now();
        let mut last_full_surface_written_at = last_snapshot_written_at;

        loop {
            if let Some(limit) = max_runtime
                && started_at.elapsed() >= limit
            {
                self.store.append_json_event(
                    mission_id,
                    None,
                    "watchdog_timeout",
                    "watchdog max runtime reached",
                    &json!({ "seconds": limit.as_secs() }),
                )?;
                break;
            }

            if let Some(event) =
                next_runtime_event(&mut supervisor_runtime, &mut worker_runtimes, tick).await
            {
                stats.runtime_events += 1;
                self.handle_runtime_event(
                    mission_id,
                    &config.repo,
                    active_supervisor_id,
                    &event,
                    &mut active_sessions,
                    &alias_map,
                    &mut leases,
                    &mut pending_mail,
                    &mut pending_supervisor_decisions,
                    &mut recent_failures,
                    &mut mass_death_detector,
                    supervisor_mode == SupervisorMode::Degraded,
                    &mut stats,
                    control_surface,
                )?;
            }

            self.read_worker_status_files(
                mission_id,
                &config.repo,
                active_supervisor_id,
                &mut active_sessions,
                &mut pending_supervisor_decisions,
                supervisor_mode == SupervisorMode::Degraded,
                &mut stats,
                control_surface,
            )?;
            self.handle_settled_worker_observations(
                mission_id,
                &config.repo,
                active_supervisor_id,
                &mut active_sessions,
                &mut pending_supervisor_decisions,
                supervisor_mode == SupervisorMode::Degraded,
                &mut stats,
            )?;
            refresh_tmux_health_cache(&mut active_sessions);
            let rebalances = supervisor_team::rebalance_worker_supervision(
                &mut active_sessions,
                &supervisor_ids,
                active_supervisor_id,
            );
            for rebalance in rebalances {
                self.store.append_json_event(
                    mission_id,
                    Some(active_supervisor_id),
                    "supervisor_rebalance",
                    &rebalance,
                    &json!({ "rebalance": rebalance }),
                )?;
            }
            self.handle_supervisor_team_health(
                mission_id,
                &supervisor_ids,
                &mut active_supervisor_id,
                stall_after,
                &mut active_sessions,
                &pending_mail,
                &pending_supervisor_decisions,
                started_at,
                &mut supervisor_mode,
                &mut worker_continuity_announced,
                &mut stats,
            )?;
            if mission_profile.enable_health_probes {
                self.health_probe_sessions(
                    mission_id,
                    stall_after,
                    &mut active_sessions,
                    &mut stats,
                )?;
            }
            self.zombie_debounce_check(mission_id, &mut active_sessions, &mut stats)?;
            self.handle_worker_liveness_incidents(
                mission_id,
                &config.repo,
                active_supervisor_id,
                &mut active_sessions,
                &mut pending_supervisor_decisions,
                supervisor_mode == SupervisorMode::Degraded,
                &mut stats,
                control_surface,
            )?;
            self.handle_stalls(
                mission_id,
                &config.repo,
                active_supervisor_id,
                stall_after,
                &mut active_sessions,
                &mut pending_supervisor_decisions,
                supervisor_mode == SupervisorMode::Degraded,
                &mut stats,
            )?;
            if mission_profile.enable_protocol_reminders {
                self.handle_protocol_reminders(
                    mission_id,
                    active_supervisor_id,
                    &mut active_sessions,
                    &mut pending_supervisor_decisions,
                    &mut stats,
                )?;
            }
            self.handle_pending_mail(
                mission_id,
                active_supervisor_id,
                &mut active_sessions,
                &mut pending_mail,
            )?;
            self.handle_pending_supervisor_decisions(
                mission_id,
                &config.repo,
                active_supervisor_id,
                &mut active_sessions,
                &mut pending_supervisor_decisions,
                &mut stats,
            )?;
            self.handle_pending_restarts(
                mission_id,
                &mut supervisor_runtime,
                &mut worker_runtimes,
                &mut active_sessions,
            )
            .await?;

            drain_prompt_queues(&mut active_sessions);
            let nudge_injections = mail::drain_nudge_queues(&config.state_dir, &active_sessions);
            for (session_id, formatted) in nudge_injections {
                if let Some(session) = active_sessions.get(&session_id) {
                    let _ = session.runtime.send_prompt(&formatted);
                }
            }

            if last_snapshot_written_at.elapsed() >= status_snapshot_interval {
                let full_surface = last_full_surface_written_at.elapsed() >= full_surface_interval;
                self.write_status_snapshot(
                    mission_id,
                    active_supervisor_id,
                    control_surface,
                    &active_sessions,
                    &pending_mail,
                    &stats,
                    full_surface,
                )?;
                last_snapshot_written_at = Instant::now();
                if full_surface {
                    last_full_surface_written_at = last_snapshot_written_at;
                }
            }

            if mission_profile.enable_state_cards
                && last_state_card_sent.elapsed() >= state_card_interval
                && supervisor_mode != SupervisorMode::Degraded
                && !final_synthesis_requested
            {
                for (supervisor_branch_id, card) in supervisor_team::build_supervisor_cards(
                    &active_sessions,
                    &pending_mail,
                    &pending_supervisor_decisions,
                    supervisor_mode,
                    started_at,
                    &supervisor_ids,
                    active_supervisor_id,
                ) {
                    if queue_supervisor_state_card(
                        &mut active_sessions,
                        supervisor_branch_id,
                        SupervisorEventType::Notice,
                        &card,
                    ) {
                        last_state_card_sent = Instant::now();
                    }
                }
            }

            self.drive_final_synthesis(
                mission_id,
                active_supervisor_id,
                &mut active_sessions,
                &mut final_synthesis_requested,
                &mut final_synthesis_requested_at,
                &mut final_synthesis_wakeup_sent,
                &mut final_synthesis_escalated,
            )?;

            if finalization::should_break_run(
                &active_sessions,
                active_supervisor_id,
                final_synthesis_requested,
                supervisor_mode,
            ) {
                break;
            }
        }

        for session in active_sessions.values() {
            let _ = session.runtime.terminate();
        }

        let final_status = if active_sessions
            .values()
            .filter(|session| session.record.role == SessionRole::Worker)
            .any(|session| session.state == SessionState::Failed)
        {
            MissionStatus::Failed
        } else {
            MissionStatus::Completed
        };
        self.store.update_mission_status(mission_id, final_status)?;
        self.write_status_snapshot(
            mission_id,
            active_supervisor_id,
            control_surface,
            &active_sessions,
            &pending_mail,
            &stats,
            true,
        )?;
        if !control_surface.tmux_session_names.is_empty() {
            self.store.append_summary(
                mission_id,
                None,
                "surface",
                format!(
                    "teamwork surface active at {}",
                    control_surface.dashboard_file.display()
                ),
            )?;
        }

        Ok(stats)
    }

    pub(super) fn prime_supervisor_backlog(
        &self,
        mission_id: Uuid,
        supervisor_id: Uuid,
        active_sessions: &HashMap<Uuid, ActiveSession>,
        pending_supervisor_decisions: &mut HashMap<String, PendingSupervisorDecision>,
    ) -> Result<()> {
        let mut seeded = Vec::new();
        for (session_id, session) in active_sessions {
            if session.record.role != SessionRole::Worker {
                continue;
            }
            match session.state {
                SessionState::DoneClaimed | SessionState::NeedsValidation => {
                    if queue_supervisor_decision(
                        pending_supervisor_decisions,
                        SupervisorDecisionKind::Validation,
                        *session_id,
                        &format!(
                            "Worker {} resumed with a pending completion claim. Validate it before acceptance.",
                            session.record.name
                        ),
                    ) {
                        seeded.push(format!("validation:{}", session.record.name));
                    }
                }
                SessionState::Stalled => {
                    if queue_supervisor_decision(
                        pending_supervisor_decisions,
                        SupervisorDecisionKind::StallRecovery,
                        *session_id,
                        &format!(
                            "Worker {} resumed in a stalled state. Decide whether to retry, redirect, or unblock it.",
                            session.record.name
                        ),
                    ) {
                        seeded.push(format!("stall:{}", session.record.name));
                    }
                }
                SessionState::Blocked
                | SessionState::Contradictory
                | SessionState::WrongDirection => {
                    seeded.push(format!(
                        "{}:{}",
                        session.state.as_str(),
                        session.record.name
                    ));
                }
                _ => {}
            }
        }

        if !seeded.is_empty() {
            self.store.append_json_event(
                mission_id,
                Some(supervisor_id),
                "startup_sweep",
                "seeded supervisor backlog from live session state",
                &json!({ "items": seeded }),
            )?;
        }
        Ok(())
    }

    pub(super) async fn handle_pending_restarts(
        &self,
        mission_id: Uuid,
        supervisor_runtime: &mut SessionRuntime,
        worker_runtimes: &mut [SessionRuntime],
        active_sessions: &mut HashMap<Uuid, ActiveSession>,
    ) -> Result<()> {
        let now = Instant::now();
        let restart_ids = active_sessions
            .iter()
            .filter_map(|(session_id, session)| {
                session
                    .restart_at
                    .filter(|when| now >= *when)
                    .map(|_| *session_id)
            })
            .collect::<Vec<_>>();

        for session_id in restart_ids {
            let Some(session) = active_sessions.get_mut(&session_id) else {
                continue;
            };
            session.restart_at = None;
            let (launch_spec, prompt_embedded) = embed_initial_prompt_if_supported(
                session.record.agent,
                session.launch_spec.clone(),
                &session.launch_prompt,
            );
            let running = if session.record.role == SessionRole::Worker {
                if let Some(runtime_slot) = session.runtime_slot {
                    if let Some(runtime) = worker_runtimes.get(runtime_slot) {
                        runtime.spawn(session_id, launch_spec)?
                    } else {
                        supervisor_runtime.spawn(session_id, launch_spec)?
                    }
                } else {
                    supervisor_runtime.spawn(session_id, launch_spec)?
                }
            } else {
                supervisor_runtime.spawn(session_id, launch_spec)?
            };
            let prompt_delay = running.prompt_delay();
            session.runtime = running;
            session.state = SessionState::Booting;
            session.started_at = Instant::now();
            session.last_output_at = Instant::now();
            session.startup_grace_until = Instant::now() + startup_grace(session.record.agent);
            session.output_chunks = 0;
            session.directive_count = 0;
            session.initial_status_received = false;
            session.output_chunks_at_last_status = 0;
            session.reported_overlap = None;
            session.protocol_reminder_sent = false;
            session.low_confidence_count = 0;
            session.last_observation_key = None;
            session.queued_prompts.clear();
            session.queued_prompt_keys.clear();
            session.last_prompt_sent_at = None;
            session.launch_prompt_sent = false;
            session.launch_prompt_sent_at = None;
            session.cleanup_authorized = false;
            session.last_tmux_health = None;
            session.last_tmux_health_checked_at = None;
            session.last_supervisor_notice_key = None;
            session.recent_supervisor_notice_keys.clear();
            session.last_supervisor_state_card_key = None;
            session.record.last_summary = Some(format!(
                "Restarted after exit; attempt {}",
                session.restart_count
            ));
            self.store
                .update_session_state(session_id, SessionState::Booting)?;
            self.store.update_worker_summary(
                session_id,
                session
                    .record
                    .last_summary
                    .as_deref()
                    .unwrap_or("restarted"),
            )?;
            self.store.append_json_event(
                mission_id,
                Some(session_id),
                "session_restarted",
                "session restarted after exit",
                &json!({ "name": session.record.name, "restart_count": session.restart_count }),
            )?;
            if !prompt_embedded {
                tokio::time::sleep(prompt_delay).await;
                if !session.launch_prompt_sent {
                    let launch_prompt = if session.record.role == SessionRole::Worker {
                        match (
                            session.packet.as_ref(),
                            session.launch_spec.env.get("SAPPHIRE_SESSION_ROOT"),
                        ) {
                            (Some(packet), Some(state_root)) => {
                                launch_prompt::worker_resume_prompt(
                                    std::path::Path::new(state_root),
                                    packet,
                                    session.restart_count,
                                )
                            }
                            _ => session.launch_prompt.clone(),
                        }
                    } else {
                        session.launch_prompt.clone()
                    };
                    send_prompt_immediately(session, &launch_prompt)?;
                    session.launch_prompt_sent = true;
                    session.launch_prompt_sent_at = Some(Instant::now());
                }
            }
            session.state = SessionState::NotStarted;
            self.store
                .update_session_state(session_id, SessionState::NotStarted)?;
        }

        Ok(())
    }
}

pub(super) async fn next_runtime_event(
    supervisor_runtime: &mut SessionRuntime,
    worker_runtimes: &mut [SessionRuntime],
    tick: Duration,
) -> Option<RuntimeEvent> {
    if let Some(event) = supervisor_runtime
        .next_event(Duration::from_millis(5))
        .await
    {
        return Some(event);
    }

    for worker_runtime in worker_runtimes.iter_mut() {
        if let Some(event) = worker_runtime.next_event(Duration::from_millis(5)).await {
            return Some(event);
        }
    }

    if worker_runtimes.is_empty() {
        return supervisor_runtime.next_event(tick).await;
    }

    let polling_window = tick.min(Duration::from_millis(25));
    let started = Instant::now();
    loop {
        if let Some(event) = supervisor_runtime
            .next_event(Duration::from_millis(5))
            .await
        {
            return Some(event);
        }
        for worker_runtime in worker_runtimes.iter_mut() {
            if let Some(event) = worker_runtime.next_event(Duration::from_millis(5)).await {
                return Some(event);
            }
        }
        if started.elapsed() >= tick {
            return None;
        }
        tokio::time::sleep(polling_window).await;
    }
}
