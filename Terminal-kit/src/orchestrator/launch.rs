use super::*;

impl Orchestrator {
    pub async fn launch(&self, config: LaunchConfig) -> Result<LaunchSummary> {
        let mission_id = Uuid::new_v4();
        write_planning_status_snapshot(
            &config.state_dir.join("control/status.txt"),
            mission_id,
            "supervisor-01",
            "Planning mission decomposition",
        )?;
        let agents_bootstrap = ensure_agents_bootstrap(&config.repo, &self.prompts)?;
        let mission_profile = MissionProfile::from_launch(&config);

        let mut notes = vec![
            format!(
                "supervisor prompt written to {}",
                config.state_dir.join("prompts").display()
            ),
            if agents_bootstrap.existed {
                format!("AGENTS.md present at {}", agents_bootstrap.path.display())
            } else {
                format!(
                    "AGENTS.md created from instruction source at {}",
                    agents_bootstrap.path.display()
                )
            },
        ];

        let supervisor_name = "supervisor-01".to_owned();
        let mut supervisor_spec = config.supervisor_agent.build_launch_spec(
            &config.repo,
            &config.state_dir,
            &config.supervisor_args,
        );
        supervisor_spec = harden_supervisor_launch_spec(config.supervisor_agent, supervisor_spec);
        supervisor_spec.surface_label = supervisor_name.clone();
        let supervisor_session = SessionRecord {
            id: Uuid::new_v4(),
            mission_id,
            role: SessionRole::Supervisor,
            ordinal: 1,
            agent: config.supervisor_agent,
            terminal_id: supervisor_name.clone(),
            name: supervisor_name.clone(),
            owned_scope: "Mission rewrite, worker supervision, validation, contradiction control, and final synthesis.".to_owned(),
            status: if config.dry_run { SessionState::Planned } else { SessionState::Booting },
            launch_command: launch_command(supervisor_spec.program.as_str(), &supervisor_spec.args),
            last_heartbeat_at: Utc::now(),
            last_summary: Some("Planning mission decomposition".to_owned()),
        };

        if !config.dry_run {
            notes.push("startup: direct live launch without preflight gate".to_owned());
        }

        let plan_outcome = self
            .plan_with_supervisor(mission_id, &config, &supervisor_session)
            .await?;
        let planned_worker_count = plan_outcome.plan.worker_packets.len();
        let effective_plan = plan_outcome.plan.clone();
        let mission_record = MissionRecord {
            id: mission_id,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            repo_path: config.repo.clone(),
            mission: config.mission.clone(),
            mission_rewrite: effective_plan.mission_rewrite.clone(),
            worker_agent: config.worker_agent,
            supervisor_agent: config.supervisor_agent,
            worker_count: config.worker_count,
            status: if config.dry_run {
                MissionStatus::Planned
            } else {
                MissionStatus::Launching
            },
            final_summary: None,
        };
        self.store
            .persist_mission(&mission_record, &effective_plan)?;
        self.store.persist_session(&supervisor_session, None)?;
        let supervisor_task = TaskRecord {
            id: Uuid::new_v4(),
            mission_id,
            worker_id: supervisor_session.id,
            title: "Mission supervision".to_owned(),
            description: "Own decomposition, live supervision, validation, contradiction handling, and final synthesis.".to_owned(),
            status: "assigned".to_owned(),
            priority: "high".to_owned(),
            depends_on_json: "[]".to_owned(),
            definition_of_done_json: serde_json::to_string(&vec![
                "All worker claims challenged".to_owned(),
                "Contradictions ruled on".to_owned(),
                "Final mission state summarized".to_owned(),
            ])?,
        };
        self.store.persist_task(&supervisor_task)?;
        self.store.append_summary(
            mission_id,
            None,
            "mission_rewrite",
            &effective_plan.mission_rewrite,
        )?;
        self.store.append_summary(
            mission_id,
            Some(supervisor_session.id),
            "plan_source",
            format!(
                "plan source: {} ({} requested workers, {} planned workers)",
                plan_outcome.source, config.worker_count, planned_worker_count
            ),
        )?;

        let supervisor_prompt = render_supervisor_prompt_with_agents(
            self.prompts
                .render_supervisor_prompt(&config.mission, &effective_plan),
            &agents_bootstrap,
            config.worker_count,
        );
        write_prompt_file(&config.state_dir, &supervisor_name, &supervisor_prompt)?;
        let supervisor_launch_prompt = launch_prompt::supervisor_loader_prompt(
            config.supervisor_agent,
            &launch_prompt::prompt_file_path(&config.state_dir, &supervisor_name),
            &supervisor_name,
        );

        let worker_adapter = adapter_for(config.worker_agent);
        let mut session_names = Vec::with_capacity(config.worker_count + 1);
        session_names.push(supervisor_name.clone());
        let mut worker_launches = Vec::with_capacity(effective_plan.worker_packets.len());

        for (index, packet) in effective_plan.worker_packets.iter().enumerate() {
            let worker_name = if packet.display_name.trim().is_empty() {
                packet.worker_id.clone()
            } else {
                packet.display_name.clone()
            };
            let base_prompt =
                worker_adapter.build_assignment_prompt(&self.prompts, &config.mission, packet);
            let memory_block = self
                .store
                .agent_memory()
                .format_history_for_injection(&worker_name, &packet.role_type, 3)
                .ok()
                .flatten();

            let full_prompt = render_worker_prompt_with_agents(
                base_prompt,
                &agents_bootstrap,
                &config.state_dir,
                packet,
                config.git_remote.as_deref(),
                memory_block.as_deref(),
            );
            write_prompt_file(&config.state_dir, &worker_name, &full_prompt)?;
            ensure_worker_status_path(
                &config
                    .state_dir
                    .join("workers")
                    .join(&worker_name)
                    .join("status.json"),
            )?;
            let live_prompt = launch_prompt::worker_terminal_prompt(
                &config.state_dir,
                &launch_prompt::prompt_file_path(&config.state_dir, &worker_name),
                packet,
            );
            let mut launch_spec = config.worker_agent.build_launch_spec(
                &config.repo,
                &config.state_dir,
                &config.worker_args,
            );
            launch_spec.surface_label = worker_name.clone();
            let session = SessionRecord {
                id: Uuid::new_v4(),
                mission_id,
                role: SessionRole::Worker,
                ordinal: index + 1,
                agent: config.worker_agent,
                terminal_id: worker_name.clone(),
                name: worker_name.clone(),
                owned_scope: packet.owned_scope.clone(),
                status: if config.dry_run {
                    SessionState::Planned
                } else {
                    SessionState::Booting
                },
                launch_command: launch_command(launch_spec.program.as_str(), &launch_spec.args),
                last_heartbeat_at: Utc::now(),
                last_summary: Some(format!("Assigned {}", packet.role)),
            };
            self.store.persist_session(&session, Some(packet))?;
            let task_id = Uuid::new_v4();
            self.store.persist_task(&TaskRecord {
                id: task_id,
                mission_id,
                worker_id: session.id,
                title: packet.role.clone(),
                description: packet.explicit_task.clone(),
                status: "assigned".to_owned(),
                priority: if packet.role.to_ascii_lowercase().contains("validation") {
                    "high".to_owned()
                } else {
                    "medium".to_owned()
                },
                depends_on_json: "[]".to_owned(),
                definition_of_done_json: serde_json::to_string(&packet.definition_of_done)?,
            })?;
            session_names.push(worker_name.clone());
            worker_launches.push(WorkerLaunch {
                session,
                launch_spec,
                prompt: live_prompt,
                packet: packet.clone(),
                task_id: Some(task_id),
            });
        }
        notes.push(format!(
            "plan source: {} ({} requested workers, {} planned workers)",
            plan_outcome.source, config.worker_count, planned_worker_count
        ));

        if config.dry_run {
            notes.push("dry-run only: no agent processes were spawned".to_owned());
        } else {
            self.store
                .update_mission_status(mission_id, MissionStatus::Running)?;
            let control_surface = ControlSurface {
                state_dir: config.state_dir.clone(),
                status_file: config.state_dir.join("control/status.txt"),
                dashboard_file: config.state_dir.join("control/dashboard.txt"),
                transcript_dir: runtime_capture_dir(mission_id),
                workers_state_dir: config.state_dir.join("workers"),
                persist_transcripts: false,
                tmux_session_names: config
                    .tmux_session_name
                    .clone()
                    .or_else(|| {
                        if config.tmux {
                            Some(format!("sp-{}", &mission_id.to_string()[..8]))
                        } else {
                            None
                        }
                    })
                    .into_iter()
                    .collect(),
            };
            let stats = self
                .run_live_mission(
                    mission_id,
                    &config,
                    supervisor_session.clone(),
                    supervisor_task.id,
                    supervisor_spec,
                    supervisor_launch_prompt,
                    worker_launches,
                    mission_profile,
                    &control_surface,
                )
                .await?;
            notes.push(format!(
                "watchdog: events={} directives={} mail={} validation={} stalls={} lease_conflicts={} protocol_reminders={} supervisor_health={}",
                stats.runtime_events,
                stats.directives,
                stats.mails_routed,
                stats.validation_challenges,
                stats.stall_interventions,
                stats.lease_conflicts,
                stats.protocol_reminders,
                stats.supervisor_health_events,
            ));
            for tmux_name in &control_surface.tmux_session_names {
                notes.push(format!("tmux session: {tmux_name}"));
            }
        }

        Ok(LaunchSummary {
            mission_id,
            repo: config.repo,
            state_dir: config.state_dir.clone(),
            dry_run: config.dry_run,
            worker_agent: config.worker_agent,
            supervisor_agent: config.supervisor_agent,
            worker_count: config.worker_count,
            session_names,
            mission_rewrite: effective_plan.mission_rewrite,
            workstream_names: effective_plan
                .workstreams
                .iter()
                .map(|workstream| workstream.name.clone())
                .collect(),
            notes,
        })
    }

    pub(super) async fn plan_with_supervisor(
        &self,
        mission_id: Uuid,
        config: &LaunchConfig,
        supervisor_session: &SessionRecord,
    ) -> Result<PlanOutcome> {
        let adapter = adapter_for(config.supervisor_agent);
        let planning_prompt =
            adapter.build_supervisor_plan_prompt(&config.mission, config.worker_count);
        let planning_prompt_path =
            launch_prompt::prompt_file_path(&config.state_dir, "__supervisor_plan__");
        write_string_to_file(&planning_prompt_path, &planning_prompt)
            .context("failed to persist supervisor planning brief")?;
        let planning_loader_prompt = launch_prompt::supervisor_planning_loader_prompt(
            config.supervisor_agent,
            &planning_prompt_path,
            &supervisor_session.name,
        );

        let planning_spec = harden_supervisor_launch_spec(
            config.supervisor_agent,
            config.supervisor_agent.build_launch_spec(
                &config.repo,
                &config.state_dir,
                &config.supervisor_args,
            ),
        );
        let (planning_spec, prompt_embedded) = embed_initial_prompt_if_supported(
            config.supervisor_agent,
            planning_spec,
            &planning_prompt,
        );
        let mut runtime = SessionRuntime::new();
        let planning_runtime = runtime.spawn(supervisor_session.id, planning_spec)?;
        self.store
            .update_session_state(supervisor_session.id, SessionState::Progressing)?;
        if !prompt_embedded {
            tokio::time::sleep(planning_runtime.prompt_delay()).await;
            planning_runtime.send_prompt(&planning_loader_prompt)?;
        }

        let started_at = Instant::now();
        let timeout = Duration::from_secs(120);
        let mut raw_buffer = String::new();
        let mut correction_sent = false;
        let mut incomplete_block_sent = false;
        let mut hard_retry_sent = false;
        let mut wrapped_plan_started_at = None;
        let mut compact_retry_sent = false;

        while started_at.elapsed() < timeout {
            let Some(event) = runtime.next_event(Duration::from_millis(750)).await else {
                if should_prompt_plan_continuation(
                    &raw_buffer,
                    wrapped_plan_started_at,
                    incomplete_block_sent,
                ) {
                    planning_runtime.send_prompt(&continue_wrapped_plan_prompt())?;
                    incomplete_block_sent = true;
                } else if !correction_sent && started_at.elapsed() >= Duration::from_secs(12) {
                    planning_runtime.send_prompt(
                        "Your last output was not yet a usable Sapphire plan. Reply only with the JSON plan block between BEGIN_SAPPHIRE_PLAN_JSON and END_SAPPHIRE_PLAN_JSON.",
                    )?;
                    correction_sent = true;
                } else if !hard_retry_sent && started_at.elapsed() >= Duration::from_secs(28) {
                    planning_runtime.send_prompt(
                        "Last attempt. Output only a single valid JSON plan block. No prose. No markdown. BEGIN_SAPPHIRE_PLAN_JSON on its own line, then JSON, then END_SAPPHIRE_PLAN_JSON.",
                    )?;
                    hard_retry_sent = true;
                }
                continue;
            };
            persist_runtime_event(&self.store, mission_id, &event)?;
            match event {
                RuntimeEvent::Output { chunk, .. } => {
                    raw_buffer.push_str(&crate::protocol::sanitize_output(&chunk));
                    trim_recent_utf8(&mut raw_buffer, 64_000, 32_000);
                    if wrapped_plan_started_at.is_none()
                        && raw_buffer.contains("BEGIN_SAPPHIRE_PLAN_JSON")
                    {
                        wrapped_plan_started_at = Some(Instant::now());
                    }
                    if let Some(plan) = adapter.extract_supervisor_plan(&raw_buffer) {
                        let plan =
                            normalize_supervisor_plan(plan, config.worker_count, "supervisor")?;
                        planning::validation::validate_supervisor_plan_packets(
                            &plan,
                            config.worker_count,
                            "supervisor",
                        )?;
                        let _ = planning_runtime.terminate();
                        return Ok(PlanOutcome {
                            plan,
                            source: "supervisor",
                        });
                    }
                    if !correction_sent
                        && raw_buffer.contains("mission_rewrite")
                        && !raw_buffer.contains("BEGIN_SAPPHIRE_PLAN_JSON")
                    {
                        planning_runtime.send_prompt(
                            "You likely produced a plan without the required wrapper. Re-output the same plan only as JSON between BEGIN_SAPPHIRE_PLAN_JSON and END_SAPPHIRE_PLAN_JSON.",
                        )?;
                        correction_sent = true;
                    } else if !compact_retry_sent
                        && raw_buffer.contains("BEGIN_SAPPHIRE_PLAN_JSON")
                        && raw_buffer.contains("END_SAPPHIRE_PLAN_JSON")
                    {
                        planning_runtime.send_prompt(&compact_plan_retry_prompt())?;
                        compact_retry_sent = true;
                    } else if should_prompt_plan_continuation(
                        &raw_buffer,
                        wrapped_plan_started_at,
                        incomplete_block_sent,
                    ) {
                        planning_runtime.send_prompt(&continue_wrapped_plan_prompt())?;
                        incomplete_block_sent = true;
                    }
                }
                RuntimeEvent::Exited { .. } => break,
                RuntimeEvent::Automation { .. } => {}
            }
        }

        let _ = planning_runtime.terminate();
        anyhow::bail!(
            "supervisor planning timed out or produced no valid Sapphire plan. Last output: {}",
            truncate(&raw_buffer, 600)
        )
    }
}

fn should_prompt_plan_continuation(
    raw_buffer: &str,
    wrapped_plan_started_at: Option<Instant>,
    continuation_sent: bool,
) -> bool {
    if continuation_sent {
        return false;
    }
    let Some(started_at) = wrapped_plan_started_at else {
        return false;
    };
    raw_buffer.contains("BEGIN_SAPPHIRE_PLAN_JSON")
        && !raw_buffer.contains("END_SAPPHIRE_PLAN_JSON")
        && started_at.elapsed() >= Duration::from_secs(8)
}

fn continue_wrapped_plan_prompt() -> String {
    "You started BEGIN_SAPPHIRE_PLAN_JSON but did not finish the wrapped plan.\nContinue immediately with the same JSON object and close it with END_SAPPHIRE_PLAN_JSON.\nDo not restart from scratch. Do not explain. Finish the wrapped JSON now.".to_owned()
}

fn compact_plan_retry_prompt() -> String {
    "Your wrapped plan block was still not parseable.\nRe-output a smaller plan now.\nUse only:\n- mission_rewrite\n- worker_packets\nOmit workstreams, risk_map, and supervision_strategy unless absolutely necessary.\nReturn exactly one wrapped JSON block. No prose. No explanation.".to_owned()
}

fn write_planning_status_snapshot(
    status_file: &Path,
    mission_id: Uuid,
    supervisor_name: &str,
    summary: &str,
) -> Result<()> {
    if let Some(parent) = status_file.parent() {
        fs::create_dir_all(parent)?;
    }

    let lines = [
        format!("Session: {}", mission_id),
        format!("Updated: {}", Utc::now().to_rfc3339()),
        "Workers: 0 | Directives: 0 | Mail: 0 | Validation: 0 | Stalls: 0 | Lease Conflicts: 0 | Protocol Reminders: 0 | Supervisor Health: 0 | Critical Failures: 0 | Crash Loops: 0".to_owned(),
        format!("Supervisor: {supervisor_name} [booting] {summary}"),
        String::new(),
        "Blocked: none".to_owned(),
        "Validation Queue: none".to_owned(),
        "Contradictions: none".to_owned(),
        "Mail Pressure: none".to_owned(),
        "Problems: none".to_owned(),
        "Ownership Gaps: none".to_owned(),
        "First-Status Incidents: none".to_owned(),
        "Systemic Incidents: none".to_owned(),
        "Crash Loops: none".to_owned(),
        "Pods: none".to_owned(),
        "Meetings: none".to_owned(),
        String::new(),
        "Supervisors".to_owned(),
        format!(
            "- {supervisor_name} [booting] branch=planning agents=0 blocked=0 validating=0 summary=\"{summary}\""
        ),
        String::new(),
        "Workers".to_owned(),
    ];
    fs::write(status_file, lines.join("\n"))?;
    Ok(())
}

pub(super) fn normalize_supervisor_plan(
    plan: MissionPlan,
    expected_worker_packets: usize,
    planner_label: &str,
) -> Result<MissionPlan> {
    if plan.workstreams.is_empty() {
        anyhow::bail!("{planner_label} returned an invalid plan: no workstreams");
    }
    if plan.worker_packets.is_empty() {
        anyhow::bail!("{planner_label} returned an invalid plan: no worker packets");
    }
    if plan.worker_packets.len() < expected_worker_packets {
        anyhow::bail!(
            "{planner_label} returned an invalid plan: expected at least {} worker packets, got {}",
            expected_worker_packets,
            plan.worker_packets.len()
        );
    }
    if plan.worker_packets.len() > expected_worker_packets {
        anyhow::bail!(
            "{planner_label} returned an invalid plan: expected exactly {} worker packets, got {}",
            expected_worker_packets,
            plan.worker_packets.len()
        );
    }
    Ok(plan)
}
