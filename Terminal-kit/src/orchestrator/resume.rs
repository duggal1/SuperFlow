use super::*;

impl Orchestrator {
    pub async fn resume(&self, config: ResumeConfig) -> Result<LaunchSummary> {
        let snapshot = self
            .store
            .load_mission_snapshot(config.mission_id)?
            .context("unknown session id")?;
        let workers = self.store.load_workers(snapshot.id)?;
        let supervisor_snapshot = workers
            .iter()
            .find(|worker| worker.session.role == SessionRole::Supervisor)
            .cloned()
            .context("session is missing supervisor row")?;
        let worker_snapshots = workers
            .into_iter()
            .filter(|worker| {
                worker.session.role == SessionRole::Worker && !worker.session.status.is_terminal()
            })
            .collect::<Vec<_>>();

        let repo = snapshot.repo_path.clone();
        let state_dir = config
            .state_dir
            .unwrap_or_else(|| snapshot.repo_path.join(".sp"));
        let agents_bootstrap = ensure_agents_bootstrap(&repo, &self.prompts)?;
        let git_remote = match crate::git::check_git_state(&repo) {
            crate::git::GitState::Ready { remote_url } => Some(remote_url),
            _ => None,
        };
        let supervisor_prompt = render_supervisor_prompt_with_agents(
            format!(
                "{}\n\nResume context:\nMission rewrite: {}\nRecent replay:\n{}\nContinue supervising from durable state. Use the action envelope when the watchdog asks for intervention.",
                self.prompts
                    .render_supervisor_prompt(&snapshot.user_mission_raw, &snapshot.plan),
                snapshot.mission_rewrite,
                self.render_replay(snapshot.id, 12)?
            ),
            &agents_bootstrap,
            worker_snapshots.len(),
        );
        write_prompt_file(
            &state_dir,
            &supervisor_snapshot.session.name,
            &supervisor_prompt,
        )?;
        let supervisor_launch_prompt = launch_prompt::supervisor_loader_prompt(
            supervisor_snapshot.session.agent,
            &launch_prompt::prompt_file_path(&state_dir, &supervisor_snapshot.session.name),
            &supervisor_snapshot.session.name,
        );

        let mut worker_launches = Vec::new();
        for worker in worker_snapshots {
            let packet = worker
                .packet
                .clone()
                .context("resume requires persisted worker packet")?;
            let adapter = adapter_for(worker.session.agent);
            let memory_context = self
                .store
                .agent_memory()
                .format_for_resume(&snapshot.id, &worker.session.name)
                .ok()
                .flatten()
                .map(|m| format!("\n\n{}\n\nContinue from your previous session. Do not repeat work you already completed.", m))
                .unwrap_or_default();
            let base_prompt = format!(
                "{}\n\nResume context:\n{}\n{}{}",
                adapter.build_assignment_prompt(&self.prompts, &snapshot.user_mission_raw, &packet),
                worker
                    .session
                    .last_summary
                    .as_deref()
                    .unwrap_or("no prior summary"),
                self.render_worker_replay(snapshot.id, &worker.session.id.to_string(), 8)?,
                memory_context,
            );
            let cross_mission_memory = self
                .store
                .agent_memory()
                .format_history_for_injection(&worker.session.name, &packet.role_type, 2)
                .ok()
                .flatten();
            let worker_prompt = render_worker_prompt_with_agents(
                base_prompt,
                &agents_bootstrap,
                &state_dir,
                &packet,
                git_remote.as_deref(),
                cross_mission_memory.as_deref(),
            );
            write_prompt_file(&state_dir, &worker.session.name, &worker_prompt)?;
            ensure_worker_status_path(
                &state_dir
                    .join("workers")
                    .join(&worker.session.name)
                    .join("status.json"),
            )?;
            let live_prompt = launch_prompt::worker_terminal_prompt(
                &state_dir,
                &launch_prompt::prompt_file_path(&state_dir, &worker.session.name),
                &packet,
            );
            worker_launches.push(WorkerLaunch {
                session: worker.session.clone(),
                launch_spec: rebuild_launch_spec(&worker.session, &repo, &state_dir),
                prompt: live_prompt,
                packet,
                task_id: self.store.find_task_id(snapshot.id, worker.session.id)?,
            });
        }

        let live_config = LaunchConfig {
            worker_agent: worker_launches
                .first()
                .map(|worker| worker.session.agent)
                .unwrap_or(supervisor_snapshot.session.agent),
            supervisor_agent: supervisor_snapshot.session.agent,
            worker_count: worker_launches.len(),
            repo: repo.clone(),
            mission: snapshot.user_mission_raw.clone(),
            state_dir: state_dir.clone(),
            dry_run: false,
            stall_seconds: config.stall_seconds,
            watchdog_max_seconds: config.watchdog_max_seconds,
            watchdog_tick_millis: config.watchdog_tick_millis,
            tmux: config.tmux,
            tmux_session_name: config.tmux_session_name.clone(),
            persist_transcripts: config.persist_transcripts,
            tui: config.tui,
            worker_args: Vec::new(),
            supervisor_args: Vec::new(),
            git_remote,
        };
        let control_surface = ControlSurface {
            state_dir: state_dir.clone(),
            status_file: state_dir.join("control/status.txt"),
            dashboard_file: state_dir.join("control/dashboard.txt"),
            transcript_dir: runtime_capture_dir(snapshot.id),
            workers_state_dir: state_dir.join("workers"),
            persist_transcripts: false,
            tmux_session_names: config
                .tmux_session_name
                .clone()
                .or_else(|| {
                    if config.tmux {
                        Some(format!("sp-{}", &snapshot.id.to_string()[..8]))
                    } else {
                        None
                    }
                })
                .into_iter()
                .collect(),
        };
        self.store.append_summary(
            snapshot.id,
            Some(supervisor_snapshot.session.id),
            "resume",
            "mission resumed from durable orchestration state",
        )?;
        let stats = self
            .run_live_mission(
                snapshot.id,
                &live_config,
                supervisor_snapshot.session.clone(),
                self.store
                    .find_task_id(snapshot.id, supervisor_snapshot.session.id)?
                    .unwrap_or(Uuid::new_v4()),
                rebuild_launch_spec(&supervisor_snapshot.session, &repo, &state_dir),
                supervisor_launch_prompt,
                worker_launches,
                MissionProfile::from_launch(&live_config),
                &control_surface,
            )
            .await?;
        Ok(LaunchSummary {
            mission_id: snapshot.id,
            repo,
            state_dir: state_dir.clone(),
            dry_run: false,
            worker_agent: live_config.worker_agent,
            supervisor_agent: live_config.supervisor_agent,
            worker_count: live_config.worker_count,
            session_names: vec![supervisor_snapshot.session.name],
            mission_rewrite: snapshot.mission_rewrite,
            workstream_names: snapshot
                .plan
                .workstreams
                .iter()
                .map(|workstream| workstream.name.clone())
                .collect(),
            notes: vec![
                "resumed from durable orchestration state".to_owned(),
                format!(
                    "watchdog: events={} directives={} mail={} validation={} stalls={} lease_conflicts={} protocol_reminders={} supervisor_health={}",
                    stats.runtime_events,
                    stats.directives,
                    stats.mails_routed,
                    stats.validation_challenges,
                    stats.stall_interventions,
                    stats.lease_conflicts,
                    stats.protocol_reminders,
                    stats.supervisor_health_events,
                ),
            ],
        })
    }
}
