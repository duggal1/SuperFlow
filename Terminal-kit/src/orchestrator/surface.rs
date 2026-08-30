use super::*;

impl Orchestrator {
    pub(super) fn write_status_snapshot(
        &self,
        mission_id: Uuid,
        active_supervisor_id: Uuid,
        control_surface: &ControlSurface,
        active_sessions: &HashMap<Uuid, ActiveSession>,
        pending_mail: &HashMap<Uuid, PendingMail>,
        stats: &WatchdogStats,
        full_surface: bool,
    ) -> Result<()> {
        let pod_summaries = coordination::summarize_pods(active_sessions, pending_mail);
        let memory_summaries = if full_surface {
            memory::write_agent_memories(control_surface, active_sessions, pending_mail)?
        } else {
            Vec::new()
        };
        let meetings = if full_surface {
            meetings::write_meeting_artifacts(control_surface, active_sessions, pending_mail)?
        } else {
            Vec::new()
        };
        let now = Instant::now();
        let live_snapshot = live_state::Snapshot::build(active_sessions, now);
        let mut supervisors = active_sessions
            .values()
            .filter(|session| session.record.role == SessionRole::Supervisor)
            .collect::<Vec<_>>();
        supervisors.sort_by(|left, right| left.record.name.cmp(&right.record.name));
        let mut workers = active_sessions
            .values()
            .filter(|session| session.record.role == SessionRole::Worker)
            .collect::<Vec<_>>();
        workers.sort_by(|left, right| left.record.name.cmp(&right.record.name));
        let worker_liveness = workers
            .iter()
            .map(|session| {
                (
                    session.record.id,
                    assess_worker_liveness(session, control_surface, now),
                )
            })
            .collect::<HashMap<_, _>>();

        let blocked = workers
            .iter()
            .filter(|session| live_snapshot.counts_as_blocked(session))
            .map(|session| session.record.name.clone())
            .collect::<Vec<_>>();
        let validation_queue = workers
            .iter()
            .filter(|session| session.validation_pending)
            .map(|session| session.record.name.clone())
            .collect::<Vec<_>>();
        let contradictions = workers
            .iter()
            .filter(|session| session.state == SessionState::Contradictory)
            .map(|session| session.record.name.clone())
            .collect::<Vec<_>>();
        let waiting_mail = workers
            .iter()
            .filter_map(|session| {
                let open_threads = pending_mail
                    .values()
                    .filter(|pending| {
                        pending.thread_state != "closed"
                            && (pending.sender_session_id == session.record.id
                                || pending.recipient_session_id == session.record.id)
                    })
                    .count();
                if open_threads == 0 {
                    None
                } else {
                    Some(format!(
                        "{} ({open_threads} thread(s))",
                        session.record.name
                    ))
                }
            })
            .collect::<Vec<_>>();
        let problems = workers
            .iter()
            .filter(|session| live_snapshot.counts_as_problem(session))
            .map(|session| {
                let liveness = worker_liveness
                    .get(&session.record.id)
                    .map(|a| a.state.as_str())
                    .unwrap_or("unknown");
                format!(
                    "{} [{}|{}]",
                    session.record.name,
                    live_snapshot.effective_state_label(session),
                    liveness
                )
            })
            .collect::<Vec<_>>();
        let first_status_incidents = workers
            .iter()
            .filter_map(|session| {
                let assessment = worker_liveness.get(&session.record.id)?;
                assessment.first_status_overdue.then(|| {
                    format!(
                        "{} [{}:{}]",
                        session.record.name,
                        assessment.state.as_str(),
                        assessment.incident_scope.as_str()
                    )
                })
            })
            .collect::<Vec<_>>();
        let crash_loops = self
            .store
            .get_crash_loop_sessions(
                mission_id,
                restart_crash_loop_threshold(),
                restart_crash_loop_window(),
            )
            .unwrap_or_default()
            .into_iter()
            .map(|(session_id, restart_count)| {
                let name = active_sessions
                    .get(&session_id)
                    .map(|session| session.record.name.clone())
                    .unwrap_or_else(|| truncate_id(&session_id));
                format!("{} ({} restarts)", name, restart_count)
            })
            .collect::<Vec<_>>();
        let ownership_gaps = workers
            .iter()
            .filter(|worker| worker.supervising_supervisor_id.is_none())
            .map(|worker| worker.record.name.clone())
            .collect::<Vec<_>>();
        let systemic_incidents = if health::is_systemic_first_status_incident(
            workers.len(),
            first_status_incidents.len(),
        ) {
            vec![format!(
                "first_status_outage:{}_of_{}",
                first_status_incidents.len(),
                workers.len()
            )]
        } else {
            Vec::new()
        };

        let mut lines = Vec::new();
        let supervisor_line = active_sessions
            .get(&active_supervisor_id)
            .map(|session| {
                format!(
                    "{} [{}] {}",
                    session.record.name,
                    live_snapshot.effective_state_label(session),
                    session
                        .record
                        .last_summary
                        .as_deref()
                        .unwrap_or("no summary yet")
                )
            })
            .unwrap_or_else(|| "supervisor unavailable".to_owned());
        lines.push(format!("Session: {}", mission_id));
        lines.push(format!("Updated: {}", Utc::now().to_rfc3339()));
        lines.push(format!(
            "Workers: {} | Directives: {} | Mail: {} | Validation: {} | Stalls: {} | Lease Conflicts: {} | Protocol Reminders: {} | Supervisor Health: {} | Critical Failures: {} | Crash Loops: {}",
            workers.len(), stats.directives, stats.mails_routed, stats.validation_challenges, stats.stall_interventions,
            stats.lease_conflicts, stats.protocol_reminders, stats.supervisor_health_events, stats.critical_failures, stats.crash_loops_detected,
        ));
        lines.push(format!("Supervisor: {}", supervisor_line));
        lines.push(String::new());
        lines.push(format!(
            "Blocked: {}",
            if blocked.is_empty() {
                "none".to_owned()
            } else {
                blocked.join(", ")
            }
        ));
        lines.push(format!(
            "Validation Queue: {}",
            if validation_queue.is_empty() {
                "none".to_owned()
            } else {
                validation_queue.join(", ")
            }
        ));
        lines.push(format!(
            "Contradictions: {}",
            if contradictions.is_empty() {
                "none".to_owned()
            } else {
                contradictions.join(", ")
            }
        ));
        lines.push(format!(
            "Mail Pressure: {}",
            if waiting_mail.is_empty() {
                "none".to_owned()
            } else {
                waiting_mail.join(", ")
            }
        ));
        lines.push(format!(
            "Problems: {}",
            if problems.is_empty() {
                "none".to_owned()
            } else {
                problems.join(", ")
            }
        ));
        lines.push(format!(
            "Ownership Gaps: {}",
            if ownership_gaps.is_empty() {
                "none".to_owned()
            } else {
                ownership_gaps.join(", ")
            }
        ));
        lines.push(format!(
            "First-Status Incidents: {}",
            if first_status_incidents.is_empty() {
                "none".to_owned()
            } else {
                first_status_incidents.join(", ")
            }
        ));
        lines.push(format!(
            "Systemic Incidents: {}",
            if systemic_incidents.is_empty() {
                "none".to_owned()
            } else {
                systemic_incidents.join(", ")
            }
        ));
        lines.push(format!(
            "Crash Loops: {}",
            if crash_loops.is_empty() {
                "none".to_owned()
            } else {
                crash_loops.join(", ")
            }
        ));
        lines.push(format!(
            "Pods: {}",
            if pod_summaries.is_empty() {
                "none".to_owned()
            } else {
                pod_summaries
                    .iter()
                    .map(|pod| {
                        format!(
                            "{} members={} blocked={} threads={}",
                            pod.name,
                            pod.members.len(),
                            pod.blocked_members.len(),
                            pod.open_threads
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" | ")
            }
        ));
        lines.push(format!(
            "Meetings: {}",
            if meetings.is_empty() {
                "none".to_owned()
            } else {
                meetings
                    .iter()
                    .take(4)
                    .map(|meeting| format!("{} [{}]", meeting.kind, meeting.reason))
                    .collect::<Vec<_>>()
                    .join(" | ")
            }
        ));
        lines.push(String::new());
        lines.push("Supervisors".to_owned());
        for supervisor in &supervisors {
            let branch = if supervisor.record.id == active_supervisor_id {
                "active"
            } else {
                "branch"
            };
            let owned_workers = workers
                .iter()
                .filter(|worker| worker.supervising_supervisor_id == Some(supervisor.record.id))
                .collect::<Vec<_>>();
            let blocked_count = owned_workers
                .iter()
                .filter(|worker| live_snapshot.counts_as_blocked(worker))
                .count();
            let validating_count = owned_workers
                .iter()
                .filter(|worker| worker.validation_pending)
                .count();
            lines.push(format!(
                "- {} [{}] branch={} agents={} blocked={} validating={} summary={}",
                supervisor.record.name,
                live_snapshot.effective_state_label(supervisor),
                branch,
                owned_workers.len(),
                blocked_count,
                validating_count,
                quote_status_value(
                    supervisor
                        .record
                        .last_summary
                        .as_deref()
                        .unwrap_or("no summary yet"),
                ),
            ));
        }
        lines.push(String::new());
        lines.push("Workers".to_owned());
        for worker in workers {
            let liveness = worker_liveness.get(&worker.record.id);
            let owner = worker
                .supervising_supervisor_id
                .and_then(|session_id| active_sessions.get(&session_id))
                .map(|session| session.record.name.clone())
                .unwrap_or_else(|| "unassigned".to_owned());
            let role = worker
                .packet
                .as_ref()
                .map(|packet| packet.role_type.clone())
                .unwrap_or_else(|| "worker".to_owned());
            let task = worker
                .packet
                .as_ref()
                .map(|packet| packet.explicit_task.as_str())
                .filter(|task| !task.trim().is_empty())
                .or_else(|| {
                    worker
                        .packet
                        .as_ref()
                        .map(|packet| packet.owned_scope.as_str())
                        .filter(|scope| !scope.trim().is_empty())
                })
                .unwrap_or("no task yet");
            lines.push(format!(
                "- {} [{}] liveness={} owner={} role={} task={} incident={} failure={} outputs={} interventions={} mail={} summary={}",
                worker.record.name,
                live_snapshot.effective_state_label(worker),
                liveness
                    .map(|assessment| assessment.state.as_str())
                    .unwrap_or("unknown"),
                owner,
                role,
                quote_status_value(task),
                liveness
                    .map(|assessment| assessment.incident_scope.as_str())
                    .unwrap_or("none"),
                liveness
                    .and_then(|assessment| assessment.failure_kind.map(first_status_failure_kind_name))
                    .unwrap_or("none"),
                worker.output_chunks,
                worker.total_interventions,
                pending_mail
                    .values()
                    .filter(|pending| {
                        pending.thread_state != "closed"
                            && (pending.sender_session_id == worker.record.id
                                || pending.recipient_session_id == worker.record.id)
                    })
                    .count(),
                quote_status_value(
                    worker
                        .record
                        .last_summary
                        .as_deref()
                        .unwrap_or("no summary yet"),
                ),
            ));
        }
        write_string_to_file(&control_surface.status_file, &lines.join("\n"))?;
        if !full_surface {
            return Ok(());
        }
        let mission = self.store.load_mission_snapshot(mission_id)?;
        let supervisor_summary = self.store.latest_supervisor_summary(mission_id)?;
        let mut dashboard = Vec::new();
        dashboard.push("# Sapphire".to_owned());
        if let Some(mission) = mission {
            dashboard.push(String::new());
            dashboard.push(format!("Mission: {}", mission.mission_rewrite));
            dashboard.push(format!("Status: {}", mission.status));
        }
        dashboard.push(String::new());
        dashboard.push("## Supervisor".to_owned());
        dashboard.push(format!("- {}", supervisor_line));
        if let Some(summary) = supervisor_summary {
            dashboard.push(format!("- Summary: {}", truncate(&summary, 180)));
        }
        dashboard.push(String::new());
        dashboard.push("## Watchdog".to_owned());
        dashboard.push(format!(
            "- events={} directives={} mail={} validation={} stalls={} conflicts={} reminders={} critical_failures={} crash_loops={}",
            stats.runtime_events, stats.directives, stats.mails_routed, stats.validation_challenges,
            stats.stall_interventions, stats.lease_conflicts, stats.protocol_reminders, stats.critical_failures,
            stats.crash_loops_detected,
        ));
        if !crash_loops.is_empty() {
            dashboard.push(format!("- crash loops: {}", crash_loops.join(", ")));
        }
        dashboard.push(String::new());
        dashboard.push("## Pods".to_owned());
        if pod_summaries.is_empty() {
            dashboard.push("- no active pods".to_owned());
        } else {
            for pod in &pod_summaries {
                dashboard.push(format!(
                    "- {} | members={} | blocked={} | threads={}",
                    pod.name,
                    pod.members.len(),
                    pod.blocked_members.len(),
                    pod.open_threads
                ));
            }
        }
        dashboard.push(String::new());
        dashboard.push("## Meetings".to_owned());
        if meetings.is_empty() {
            dashboard.push("- no active coordination meetings".to_owned());
        } else {
            for meeting in meetings.iter().take(6) {
                dashboard.push(format!(
                    "- {} [{}] {}",
                    meeting.kind,
                    meeting.participants.join(", "),
                    truncate(&meeting.reason, 96)
                ));
            }
        }
        dashboard.push(String::new());
        dashboard.push("## Memory".to_owned());
        if memory_summaries.is_empty() {
            dashboard.push("- worker memories not initialized yet".to_owned());
        } else {
            for summary in &memory_summaries {
                dashboard.push(format!(
                    "- {} | pod={} | active_threads={}",
                    summary.display_name, summary.pod, summary.active_threads
                ));
            }
        }
        dashboard.push(String::new());
        dashboard.push("## Sessions".to_owned());
        for session in active_sessions.values() {
            let state_label = live_snapshot.effective_state_label(session);
            dashboard.push(format!(
                "- {} [{}] {}",
                session.record.name,
                state_label,
                truncate(
                    session
                        .record
                        .last_summary
                        .as_deref()
                        .unwrap_or("no summary yet"),
                    96
                )
            ));
        }
        dashboard.push(String::new());
        dashboard.push("Detach: Ctrl-b d".to_owned());
        write_string_to_file(&control_surface.dashboard_file, &dashboard.join("\n"))?;
        Ok(())
    }

    pub(super) fn ensure_tmux_surface(
        &self,
        session_name: &str,
        mission_id: Uuid,
        config: &LaunchConfig,
    ) -> Result<Vec<String>> {
        let tmux = tmux::Tmux::new(None);
        let base_name = session_name.to_string();
        let per_tab = 10;
        let total_workers = config.worker_count.max(1);
        let session_names = tmux
            .create_batch_sessions(
                &base_name,
                &config.repo.to_string_lossy(),
                total_workers,
                per_tab,
            )
            .map_err(anyhow::Error::msg)
            .context("failed to create batch tmux sessions")?;
        self.store.append_summary(
            mission_id,
            None,
            "surface",
            format!(
                "teamwork surface prepared: {} sessions ({} workers, {} per session)",
                session_names.len(),
                total_workers,
                per_tab
            ),
        )?;
        Ok(session_names)
    }

    pub(super) fn read_worker_status_files(
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
        let workers_dir = &control_surface.workers_state_dir;
        if !workers_dir.exists() {
            return Ok(());
        }
        let name_to_id: HashMap<String, Uuid> = active_sessions
            .iter()
            .filter(|(_, s)| s.record.role == SessionRole::Worker)
            .map(|(id, s)| (s.record.name.clone(), *id))
            .collect();
        let mut updates = Vec::new();
        for (display_name, session_id) in &name_to_id {
            let status_file = workers_dir.join(display_name).join("status.json");
            let previous_modified = active_sessions
                .get(session_id)
                .and_then(|session| session.last_status_file_modified);
            let Some(update) =
                status_files::load_status_file_update(&status_file, previous_modified)
            else {
                continue;
            };
            let Some(session) = active_sessions.get_mut(session_id) else {
                continue;
            };
            if session.state.is_terminal() {
                continue;
            }
            let now = Instant::now();
            session.last_status_file_modified = Some(update.modified_at);
            if !update.bootstrap {
                session.last_status_update_at = Some(now);
            }
            session.last_confirmed_alive = now;
            if !update.bootstrap {
                session.protocol_reminder_sent = false;
                record_intervention_response(session, now);
            }
            updates.push((*session_id, update.directive, update.bootstrap));
        }
        for (session_id, directive, bootstrap) in updates {
            if bootstrap {
                continue;
            }
            self.persist_normalized_status(
                mission_id,
                session_id,
                &directive.state,
                "status_file",
                Confidence::High,
                &directive.summary,
                &directive.summary,
            )?;
            self.handle_status_directive(
                mission_id,
                repo_root,
                supervisor_id,
                session_id,
                directive,
                false,
                active_sessions,
                pending_supervisor_decisions,
                supervisor_degraded,
                stats,
            )?;
        }
        Ok(())
    }
}

fn quote_status_value(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_owned())
}
