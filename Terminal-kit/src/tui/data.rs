//! Data source — reads ALL real backend state, builds RuntimeSnapshot.
//! Sources: control status file, JSONL history, mail store, meetings, memory.

use anyhow::Result;
use chrono::{DateTime, Utc};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

use crate::model::{MissionSnapshot, SessionRole, WorkerSnapshot};
use crate::storage;
use crate::store::Store;

use super::control_status::{ControlStatusSnapshot, parse_control_status};
use super::state::*;

pub struct DashboardDataSource {
    store: Store,
    control_status_path: PathBuf,
    state_dir: PathBuf,
}

impl DashboardDataSource {
    pub fn open(state_dir: PathBuf, control_status_path: PathBuf) -> Result<Self> {
        let store = Store::open(&state_dir)?;
        Ok(Self {
            store,
            control_status_path,
            state_dir,
        })
    }

    pub fn snapshot(&self, mission_id: Option<Uuid>) -> Result<RuntimeSnapshot> {
        let control_content = fs::read_to_string(&self.control_status_path).unwrap_or_default();
        let status = parse_control_status(&control_content);
        let resolved_mission_id = mission_id.or_else(|| {
            status
                .session_id
                .as_deref()
                .and_then(|value| Uuid::parse_str(value).ok())
        });

        let Some(mission_id) = resolved_mission_id else {
            return if status_has_live_state(&status) {
                Ok(self.snapshot_from_live_status(None, &status, &control_content)?)
            } else {
                Ok(RuntimeSnapshot::empty_waiting())
            };
        };

        let mission = self.store.load_mission_snapshot(mission_id)?;
        let Some(mission) = mission else {
            return if status_has_live_state(&status) {
                Ok(self.snapshot_from_live_status(Some(mission_id), &status, &control_content)?)
            } else {
                Ok(RuntimeSnapshot::empty_waiting())
            };
        };

        let workers = self.store.load_workers(mission_id)?;
        let mail_threads = self.parse_mail_threads(&mission_id);
        let meetings = self.parse_meetings();
        let memory_summaries = self.parse_memory_summaries(&control_content);
        let supervisor_logs = self.parse_supervisor_logs(Some(mission_id), &status);

        // Build agent nodes with real data
        let mut all_agents = Vec::new();
        let mut supervisor_nodes = Vec::new();
        let active_supervisor_name = status.supervisor.as_ref().map(|sv| sv.name.clone());
        let standby_supervisor_name = status.standby_supervisor.as_ref().map(|sv| sv.name.clone());

        for w in &workers {
            let is_sv = w.session.role == SessionRole::Supervisor;
            let live = if is_sv {
                status
                    .supervisors
                    .get(&w.session.name)
                    .or_else(|| {
                        status
                            .supervisor
                            .as_ref()
                            .filter(|sv| sv.name == w.session.name)
                    })
                    .or_else(|| {
                        status
                            .standby_supervisor
                            .as_ref()
                            .filter(|sv| sv.name == w.session.name)
                    })
            } else {
                status.workers.get(&w.session.name)
            };
            let is_active_supervisor = is_sv
                && active_supervisor_name
                    .as_deref()
                    .is_some_and(|name| name == w.session.name);
            let is_standby = is_sv
                && standby_supervisor_name
                    .as_deref()
                    .is_some_and(|name| name == w.session.name);

            let node = self.build_agent(w, live, is_sv, is_standby, is_active_supervisor);

            if is_sv {
                supervisor_nodes.push(node);
            } else {
                all_agents.push(node);
            }
        }

        if supervisor_nodes.is_empty() {
            if let Some(ref sv) = status.supervisor {
                supervisor_nodes.push(AgentNode {
                    id: Uuid::nil(),
                    name: sv.name.clone(),
                    role_type: "supervisor".to_owned(),
                    display_role: "Supervisor".to_owned(),
                    status: AgentStatus::from_str(&sv.state),
                    summary: sv.summary.clone(),
                    liveness: sv.liveness.clone(),
                    owner_supervisor: None,
                    branch_label: sv.branch.clone().or_else(|| Some("active".to_owned())),
                    incident_scope: sv.incident_scope.clone(),
                    failure_kind: sv.failure_kind.clone(),
                    owned_agent_count: sv.agent_count,
                    blocked_agent_count: sv.blocked_count,
                    validating_agent_count: sv.validating_count,
                    owned_scope: String::new(),
                    explicit_task: String::new(),
                    files_touched: sv.files_touched.clone(),
                    stall_count: 0,
                    intervention_count: 0,
                    consecutive_stall_failures: sv.consecutive_stall_failures,
                    output_chunks: sv.output_chunks,
                    mail_thread_count: sv.mail_thread_count,
                    started_at: None,
                    last_output_at: None,
                    is_supervisor: true,
                    is_standby: false,
                    is_active_supervisor: true,
                });
            }
        }

        supervisor_nodes.sort_by(|left, right| {
            right
                .is_active_supervisor
                .cmp(&left.is_active_supervisor)
                .then_with(|| left.name.cmp(&right.name))
        });

        let supervisor_node = supervisor_nodes
            .iter()
            .find(|node| node.is_active_supervisor)
            .cloned()
            .or_else(|| supervisor_nodes.first().cloned());
        let standby_node = supervisor_nodes
            .iter()
            .find(|node| node.is_standby)
            .cloned();

        let is_done = mission.status == "completed"
            || mission.status == "failed"
            || (!all_agents.is_empty() && all_agents.iter().all(|a| a.status.is_terminal()));

        let exec_summary = self.build_execution_summary(
            &mission,
            &all_agents,
            &supervisor_node,
            &mail_threads,
            &status,
        );

        // Build watchdog stats from status file (REAL data, no zeros)
        let watchdog = WatchdogStats {
            worker_count: status.watchdog.worker_count,
            directives: status.watchdog.directives,
            mail_routed: status.watchdog.mail_routed,
            validation_challenges: status.watchdog.validation_challenges,
            stall_interventions: status.watchdog.stall_interventions,
            lease_conflicts: status.watchdog.lease_conflicts,
            protocol_reminders: status.watchdog.protocol_reminders,
            supervisor_health_events: status.watchdog.supervisor_health_events,
            critical_failures: status.watchdog.critical_failures,
            crash_loops_detected: status.watchdog.crash_loops_detected,
            blocked: status.blocked.clone(),
            validation_queue: status.validation_queue.clone(),
            contradictions: status.contradictions.clone(),
            mail_pressure: status.mail_pressure.clone(),
            problems: status.problems.clone(),
            ownership_gaps: status.ownership_gaps.clone(),
            first_status_incidents: status.first_status_incidents.clone(),
            systemic_incidents: status.systemic_incidents.clone(),
            crash_loop_sessions: status.crash_loops.clone(),
            pods: status
                .pods
                .iter()
                .map(|p| PodSummary {
                    name: p.name.clone(),
                    members: p.members.clone(),
                    blocked_members: p.blocked.clone(),
                    open_threads: p.open_threads,
                })
                .collect(),
            memory_summaries,
        };

        Ok(RuntimeSnapshot {
            mission_id: Some(mission_id),
            mission_status: mission.status,
            supervisor: supervisor_node,
            standby_supervisor: standby_node,
            supervisors: supervisor_nodes,
            agents: all_agents,
            supervisor_logs,
            mail_threads,
            meetings,
            watchdog,
            execution_summary: exec_summary,
            is_done,
        })
    }

    pub fn snapshot_for_attach(&self, target: &AttachTarget) -> Result<RuntimeSnapshot> {
        let mission_id = self.resolve_mission_id(target)?;
        if mission_id.is_some() {
            return self.snapshot(mission_id);
        }

        let control_content = fs::read_to_string(&self.control_status_path).unwrap_or_default();
        let status = parse_control_status(&control_content);
        if !status_has_live_state(&status) {
            return Ok(RuntimeSnapshot::empty_waiting());
        }

        let fresh_for_target = match target {
            AttachTarget::Mission(_) => true,
            AttachTarget::LatestForRepo { started_after, .. } => status
                .updated_at
                .as_deref()
                .and_then(parse_status_timestamp)
                .is_some_and(|updated_at| updated_at >= *started_after),
        };

        if fresh_for_target {
            self.snapshot_from_live_status(None, &status, &control_content)
        } else {
            Ok(RuntimeSnapshot::empty_waiting())
        }
    }

    fn snapshot_from_live_status(
        &self,
        mission_id: Option<Uuid>,
        status: &ControlStatusSnapshot,
        control_content: &str,
    ) -> Result<RuntimeSnapshot> {
        let workers = mission_id
            .map(|id| self.store.load_workers(id))
            .transpose()?
            .unwrap_or_default();
        let worker_by_name = workers
            .iter()
            .map(|worker| (worker.session.name.clone(), worker))
            .collect::<std::collections::HashMap<_, _>>();

        let active_supervisor_name = status.supervisor.as_ref().map(|sv| sv.name.clone());
        let standby_supervisor_name = status.standby_supervisor.as_ref().map(|sv| sv.name.clone());

        let mut supervisor_names = status.supervisors.keys().cloned().collect::<Vec<_>>();
        if let Some(active) = active_supervisor_name.as_ref() {
            if !supervisor_names.iter().any(|name| name == active) {
                supervisor_names.push(active.clone());
            }
        }
        if let Some(standby) = standby_supervisor_name.as_ref() {
            if !supervisor_names.iter().any(|name| name == standby) {
                supervisor_names.push(standby.clone());
            }
        }
        supervisor_names.sort();

        let mut supervisor_nodes = supervisor_names
            .into_iter()
            .filter_map(|name| {
                let live = status
                    .supervisors
                    .get(&name)
                    .or_else(|| status.supervisor.as_ref().filter(|sv| sv.name == name))
                    .or_else(|| {
                        status
                            .standby_supervisor
                            .as_ref()
                            .filter(|sv| sv.name == name)
                    })?;
                Some(self.build_live_agent(
                    worker_by_name.get(&name).copied(),
                    live,
                    true,
                    standby_supervisor_name.as_deref() == Some(name.as_str()),
                    active_supervisor_name.as_deref() == Some(name.as_str()),
                ))
            })
            .collect::<Vec<_>>();

        supervisor_nodes.sort_by(|left, right| {
            right
                .is_active_supervisor
                .cmp(&left.is_active_supervisor)
                .then_with(|| left.name.cmp(&right.name))
        });

        let mut agents = status
            .workers
            .values()
            .map(|live| {
                self.build_live_agent(
                    worker_by_name.get(&live.name).copied(),
                    live,
                    false,
                    false,
                    false,
                )
            })
            .collect::<Vec<_>>();
        agents.sort_by(|left, right| left.name.cmp(&right.name));

        let mail_threads = mission_id
            .map(|id| self.parse_mail_threads(&id))
            .unwrap_or_default();
        let meetings = self.parse_meetings();
        let supervisor_logs = self.parse_supervisor_logs(mission_id, status);
        let watchdog =
            self.watchdog_from_status(status, self.parse_memory_summaries(control_content));
        let is_done = !agents.is_empty() && agents.iter().all(|agent| agent.status.is_terminal());
        let planning = agents.is_empty()
            && supervisor_nodes
                .iter()
                .any(|node| node.summary.to_ascii_lowercase().contains("planning"));
        let launch_failed = agents.is_empty()
            && supervisor_nodes
                .iter()
                .any(|node| node.status == AgentStatus::Failed);
        let mission_status = if launch_failed {
            "failed".to_owned()
        } else if planning {
            "planning".to_owned()
        } else if is_done {
            "completed".to_owned()
        } else if !agents.is_empty() || !supervisor_nodes.is_empty() {
            "running".to_owned()
        } else {
            "launching".to_owned()
        };

        let execution_summary = ExecutionSummary {
            agents_deployed: agents.len(),
            agents_completed: agents
                .iter()
                .filter(|agent| agent.status == AgentStatus::Validated)
                .count(),
            agents_failed: agents
                .iter()
                .filter(|agent| agent.status == AgentStatus::Failed)
                .count(),
            mail_threads_total: mail_threads.len(),
            mail_threads_resolved: mail_threads
                .iter()
                .filter(|thread| {
                    matches!(
                        thread.state.as_str(),
                        "archived" | "responded" | "done" | "closed"
                    )
                })
                .count(),
            lease_conflicts: watchdog.lease_conflicts,
            stall_interventions: watchdog.stall_interventions,
            protocol_reminders: watchdog.protocol_reminders,
            supervisor_health_events: watchdog.supervisor_health_events,
            critical_failures: watchdog.critical_failures,
            crash_loops_detected: watchdog.crash_loops_detected,
            supervisor_mode: if watchdog.critical_failures > 0 {
                "degraded".to_owned()
            } else {
                "healthy".to_owned()
            },
            ..Default::default()
        };

        Ok(RuntimeSnapshot {
            mission_id,
            mission_status,
            supervisor: supervisor_nodes
                .iter()
                .find(|node| node.is_active_supervisor)
                .cloned()
                .or_else(|| supervisor_nodes.first().cloned()),
            standby_supervisor: supervisor_nodes
                .iter()
                .find(|node| node.is_standby)
                .cloned(),
            supervisors: supervisor_nodes,
            agents,
            supervisor_logs,
            mail_threads,
            meetings,
            watchdog,
            execution_summary,
            is_done,
        })
    }

    fn build_live_agent(
        &self,
        worker: Option<&WorkerSnapshot>,
        live: &super::control_status::LiveSessionStatus,
        is_supervisor: bool,
        is_standby: bool,
        is_active_supervisor: bool,
    ) -> AgentNode {
        let packet = worker.and_then(|worker| worker.packet.as_ref());
        AgentNode {
            id: worker
                .map(|worker| worker.session.id)
                .unwrap_or_else(Uuid::nil),
            name: live.name.clone(),
            role_type: packet
                .map(|packet| packet.role_type.clone())
                .or_else(|| live.role.clone())
                .unwrap_or_else(|| {
                    if is_supervisor {
                        "supervisor".to_owned()
                    } else {
                        "worker".to_owned()
                    }
                }),
            display_role: packet
                .map(|packet| packet.role.clone())
                .or_else(|| live.role.clone())
                .unwrap_or_else(|| {
                    if is_supervisor {
                        "Supervisor".to_owned()
                    } else {
                        "Worker".to_owned()
                    }
                }),
            status: AgentStatus::from_str(&live.state),
            summary: truncate_(&live.summary, 120),
            liveness: live.liveness.clone(),
            owner_supervisor: live.owner.clone(),
            branch_label: live.branch.clone(),
            incident_scope: live.incident_scope.clone(),
            failure_kind: live.failure_kind.clone(),
            owned_agent_count: live.agent_count,
            blocked_agent_count: live.blocked_count,
            validating_agent_count: live.validating_count,
            owned_scope: packet
                .map(|packet| packet.owned_scope.clone())
                .unwrap_or_default(),
            explicit_task: packet
                .map(|packet| packet.explicit_task.clone())
                .filter(|task| !task.trim().is_empty())
                .or_else(|| live.task.clone())
                .unwrap_or_default(),
            files_touched: live.files_touched.clone(),
            stall_count: 0,
            intervention_count: live.total_interventions,
            consecutive_stall_failures: live.consecutive_stall_failures,
            output_chunks: live.output_chunks,
            mail_thread_count: live.mail_thread_count,
            started_at: None,
            last_output_at: None,
            is_supervisor,
            is_standby,
            is_active_supervisor,
        }
    }

    fn build_agent(
        &self,
        worker: &WorkerSnapshot,
        live: Option<&super::control_status::LiveSessionStatus>,
        is_supervisor: bool,
        is_standby: bool,
        is_active_supervisor: bool,
    ) -> AgentNode {
        let packet = worker.packet.as_ref();
        let status = live
            .map(|s| AgentStatus::from_str(&s.state))
            .unwrap_or_else(|| AgentStatus::from_str(worker.session.status.as_str()));
        let summary = live
            .map(|s| s.summary.clone())
            .or_else(|| worker.session.last_summary.clone())
            .unwrap_or_else(|| "—".to_owned());

        // Real per-agent metrics from status file
        let intervention_count = live.map(|s| s.total_interventions).unwrap_or(0);
        let output_chunks = live.map(|s| s.output_chunks).unwrap_or(0);
        let mail_count = live.map(|s| s.mail_thread_count).unwrap_or(0);
        let consecutive_stall_failures = live.map(|s| s.consecutive_stall_failures).unwrap_or(0);

        AgentNode {
            id: worker.session.id,
            name: worker.session.name.clone(),
            role_type: packet
                .map(|p| p.role_type.clone())
                .or_else(|| live.and_then(|s| s.role.clone()))
                .unwrap_or_else(|| {
                    storage::role_name(worker.session.role)
                        .replace(' ', "-")
                        .to_lowercase()
                }),
            display_role: packet
                .map(|p| p.role.clone())
                .or_else(|| live.and_then(|s| s.role.clone()))
                .unwrap_or_else(|| storage::role_name(worker.session.role).to_owned()),
            status,
            summary: truncate_(&summary, 120),
            liveness: live.and_then(|s| s.liveness.clone()),
            owner_supervisor: live.and_then(|s| s.owner.clone()),
            branch_label: live.and_then(|s| s.branch.clone()).or_else(|| {
                is_supervisor.then(|| {
                    if is_active_supervisor {
                        "active".to_owned()
                    } else {
                        "branch".to_owned()
                    }
                })
            }),
            incident_scope: live.and_then(|s| s.incident_scope.clone()),
            failure_kind: live.and_then(|s| s.failure_kind.clone()),
            owned_agent_count: live.map(|s| s.agent_count).unwrap_or(0),
            blocked_agent_count: live.map(|s| s.blocked_count).unwrap_or(0),
            validating_agent_count: live.map(|s| s.validating_count).unwrap_or(0),
            owned_scope: packet.map(|p| p.owned_scope.clone()).unwrap_or_default(),
            explicit_task: packet
                .map(|p| p.explicit_task.clone())
                .filter(|task| !task.trim().is_empty())
                .or_else(|| live.and_then(|s| s.task.clone()))
                .unwrap_or_default(),
            files_touched: live.map(|s| s.files_touched.clone()).unwrap_or_default(),
            stall_count: 0,
            intervention_count,
            consecutive_stall_failures,
            output_chunks,
            mail_thread_count: mail_count,
            started_at: None,
            last_output_at: None,
            is_supervisor,
            is_standby,
            is_active_supervisor,
        }
    }

    fn parse_mail_threads(&self, mission_id: &Uuid) -> Vec<MailThread> {
        let mut threads = Vec::new();
        if let Ok(items) = self.store.search_mail(*mission_id, None, None, None, 50) {
            for item in items {
                threads.push(MailThread {
                    thread_id: item
                        .get("thread_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_owned(),
                    subject: item
                        .get("subject")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_owned(),
                    from: item
                        .get("from_worker_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_owned(),
                    to: item
                        .get("to_worker_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_owned(),
                    message_type: item
                        .get("message_type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_owned(),
                    priority: item
                        .get("priority")
                        .and_then(|v| v.as_str())
                        .unwrap_or("normal")
                        .to_owned(),
                    state: item
                        .get("status")
                        .and_then(|v| v.as_str())
                        .unwrap_or("open")
                        .to_owned(),
                    acked: item.get("ack_state").and_then(|v| v.as_str()) == Some("acked"),
                });
            }
        }
        threads.sort_by(|a, b| b.subject.cmp(&a.subject));
        threads.truncate(20);
        threads
    }

    fn parse_meetings(&self) -> Vec<MeetingArtifact> {
        let meetings_path = self.state_dir.join("control/meetings/meetings.json");
        if let Ok(content) = fs::read_to_string(&meetings_path) {
            if let Ok(arr) = serde_json::from_str::<Vec<serde_json::Value>>(&content) {
                return arr
                    .iter()
                    .filter_map(|v| {
                        Some(MeetingArtifact {
                            id: v.get("id")?.as_str()?.to_owned(),
                            kind: v.get("kind")?.as_str()?.to_owned(),
                            participants: v
                                .get("participants")?
                                .as_array()?
                                .iter()
                                .filter_map(|x| x.as_str().map(String::from))
                                .collect(),
                            reason: v.get("reason")?.as_str()?.to_owned(),
                        })
                    })
                    .collect();
            }
        }
        Vec::new()
    }

    fn parse_memory_summaries(&self, content: &str) -> Vec<AgentMemorySummary> {
        let mut summaries = Vec::new();
        let mut in_memory = false;
        for line in content.lines() {
            if line.starts_with("Memory:") {
                in_memory = true;
                continue;
            }
            if in_memory && line.starts_with("- ") {
                let rest = &line[2..];
                if let Some(paren) = rest.find('(') {
                    let name = rest[..paren].trim().to_owned();
                    let inner = rest[paren + 1..].trim_end_matches(')');
                    let parts: Vec<&str> = inner.split(',').collect();
                    let pod = parts
                        .first()
                        .map(|s| s.trim().to_owned())
                        .unwrap_or_default();
                    let threads = parts
                        .get(1)
                        .and_then(|s| s.trim().strip_suffix(" threads"))
                        .and_then(|s| s.parse::<usize>().ok())
                        .unwrap_or(0);
                    summaries.push(AgentMemorySummary {
                        display_name: name,
                        pod,
                        active_threads: threads,
                    });
                }
            } else if in_memory && !line.starts_with("- ") {
                in_memory = false;
            }
        }
        summaries
    }

    fn parse_supervisor_logs(
        &self,
        mission_id: Option<Uuid>,
        status: &ControlStatusSnapshot,
    ) -> Vec<SupervisorLogEntry> {
        let mut entries = Vec::new();

        // From supervisor summary
        if let Some(mission_id) = mission_id {
            if let Ok(Some(content)) = self.store.latest_supervisor_summary(mission_id) {
                for line in content.lines() {
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }
                    entries.push(SupervisorLogEntry {
                        timestamp: Utc::now(),
                        kind: classify_log_line(line),
                        message: line.to_owned(),
                        target: extract_target_from_line(line),
                    });
                }
            }
        }

        // From live status
        if let Some(ref sv) = status.supervisor {
            if !sv.summary.is_empty() && sv.summary != "—" {
                entries.push(SupervisorLogEntry {
                    timestamp: Utc::now(),
                    kind: SupervisorLogKind::Action,
                    message: sv.summary.clone(),
                    target: None,
                });
            }
        }

        entries.dedup_by(|a, b| a.message == b.message);
        entries.truncate(50);
        if entries.is_empty() {
            if status_has_live_state(status) {
                entries.push(SupervisorLogEntry {
                    timestamp: Utc::now(),
                    kind: SupervisorLogKind::Action,
                    message: format!(
                        "{} supervisor{} · {} agent{} live",
                        supervisor_count(status),
                        if supervisor_count(status) == 1 {
                            ""
                        } else {
                            "s"
                        },
                        status.workers.len(),
                        if status.workers.len() == 1 { "" } else { "s" }
                    ),
                    target: None,
                });
            } else {
                entries.push(SupervisorLogEntry {
                    timestamp: Utc::now(),
                    kind: SupervisorLogKind::Startup,
                    message: "supervisor's still waking up".to_owned(),
                    target: None,
                });
            }
        }
        entries
    }

    fn watchdog_from_status(
        &self,
        status: &ControlStatusSnapshot,
        memory_summaries: Vec<AgentMemorySummary>,
    ) -> WatchdogStats {
        WatchdogStats {
            worker_count: status.watchdog.worker_count,
            directives: status.watchdog.directives,
            mail_routed: status.watchdog.mail_routed,
            validation_challenges: status.watchdog.validation_challenges,
            stall_interventions: status.watchdog.stall_interventions,
            lease_conflicts: status.watchdog.lease_conflicts,
            protocol_reminders: status.watchdog.protocol_reminders,
            supervisor_health_events: status.watchdog.supervisor_health_events,
            critical_failures: status.watchdog.critical_failures,
            crash_loops_detected: status.watchdog.crash_loops_detected,
            blocked: status.blocked.clone(),
            validation_queue: status.validation_queue.clone(),
            contradictions: status.contradictions.clone(),
            mail_pressure: status.mail_pressure.clone(),
            problems: status.problems.clone(),
            ownership_gaps: status.ownership_gaps.clone(),
            first_status_incidents: status.first_status_incidents.clone(),
            systemic_incidents: status.systemic_incidents.clone(),
            crash_loop_sessions: status.crash_loops.clone(),
            pods: status
                .pods
                .iter()
                .map(|p| PodSummary {
                    name: p.name.clone(),
                    members: p.members.clone(),
                    blocked_members: p.blocked.clone(),
                    open_threads: p.open_threads,
                })
                .collect(),
            memory_summaries,
        }
    }

    fn build_execution_summary(
        &self,
        mission: &MissionSnapshot,
        agents: &[AgentNode],
        supervisor: &Option<AgentNode>,
        mail_threads: &[MailThread],
        status: &ControlStatusSnapshot,
    ) -> ExecutionSummary {
        let final_summary = mission.final_summary.clone().or_else(|| {
            supervisor.as_ref().and_then(|s| {
                if !s.summary.is_empty() && s.summary != "—" {
                    Some(s.summary.clone())
                } else {
                    None
                }
            })
        });

        ExecutionSummary {
            mission_rewrite: mission.mission_rewrite.clone(),
            agents_deployed: agents.len(),
            agents_completed: agents
                .iter()
                .filter(|a| a.status == AgentStatus::Validated)
                .count(),
            agents_failed: agents
                .iter()
                .filter(|a| a.status == AgentStatus::Failed)
                .count(),
            mail_threads_total: mail_threads.len(),
            mail_threads_resolved: mail_threads
                .iter()
                .filter(|m| {
                    matches!(
                        m.state.as_str(),
                        "archived" | "responded" | "done" | "closed"
                    )
                })
                .count(),
            lease_conflicts: status.watchdog.lease_conflicts,
            stall_interventions: status.watchdog.stall_interventions,
            protocol_reminders: status.watchdog.protocol_reminders,
            supervisor_health_events: status.watchdog.supervisor_health_events,
            critical_failures: status.watchdog.critical_failures,
            crash_loops_detected: status.watchdog.crash_loops_detected,
            supervisor_mode: if status.watchdog.critical_failures > 0 {
                "degraded".to_owned()
            } else {
                "healthy".to_owned()
            },
            final_summary,
            started_at: Some(mission.created_at),
            ended_at: if mission.status == "completed" || mission.status == "failed" {
                Some(mission.updated_at)
            } else {
                None
            },
        }
    }

    pub fn resolve_mission_id(&self, target: &AttachTarget) -> Result<Option<Uuid>> {
        match target {
            AttachTarget::Mission(id) => Ok(Some(*id)),
            AttachTarget::LatestForRepo {
                repo_path,
                started_after,
            } => {
                if let Some(s) = self
                    .store
                    .latest_session_for_repo(repo_path, *started_after)?
                {
                    Ok(Some(s.id))
                } else {
                    Ok(None)
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum AttachTarget {
    Mission(Uuid),
    LatestForRepo {
        repo_path: PathBuf,
        started_after: DateTime<Utc>,
    },
}

fn classify_log_line(line: &str) -> SupervisorLogKind {
    let l = line.to_ascii_lowercase();
    if l.contains("planning") || l.contains("plan") {
        SupervisorLogKind::Planning
    } else if l.contains("dispatch") || l.contains("assign") || l.contains("deploy") {
        SupervisorLogKind::Dispatch
    } else if l.contains("validat") || l.contains("accept") {
        SupervisorLogKind::Validation
    } else if l.contains("escalat") || l.contains("redirect") || l.contains("retry") {
        SupervisorLogKind::Escalation
    } else if l.contains("mail") || l.contains("route") {
        SupervisorLogKind::Mail
    } else if l.contains("health") || l.contains("degraded") {
        SupervisorLogKind::Health
    } else if l.contains("fail") || l.contains("error") || l.contains("crash") {
        SupervisorLogKind::Error
    } else if l.contains("complet") || l.contains("final") || l.contains("summary") {
        SupervisorLogKind::Completion
    } else if l.contains("restart") {
        SupervisorLogKind::Restart
    } else {
        SupervisorLogKind::Action
    }
}

fn extract_target_from_line(line: &str) -> Option<String> {
    let prefixes = [
        "engineer",
        "designer",
        "reviewer",
        "validator",
        "qa",
        "security",
        "architect",
        "researcher",
        "product",
        "compliance",
        "sales",
        "solutions",
        "customer",
        "revenue",
        "supervisor",
        "steward",
    ];
    let l = line.to_ascii_lowercase();
    for prefix in &prefixes {
        if let Some(pos) = l.find(prefix) {
            let rest = &line[pos..];
            let end = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
                .count();
            if end > prefix.len() {
                return Some(rest.chars().take(end).collect());
            }
        }
    }
    None
}

fn truncate_(value: &str, max: usize) -> String {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= max {
        compact
    } else {
        compact
            .chars()
            .take(max.saturating_sub(1))
            .collect::<String>()
            + ".."
    }
}

fn status_has_live_state(status: &ControlStatusSnapshot) -> bool {
    !status.supervisors.is_empty()
        || !status.workers.is_empty()
        || status.supervisor.is_some()
        || status.standby_supervisor.is_some()
        || status.watchdog.worker_count > 0
}

fn supervisor_count(status: &ControlStatusSnapshot) -> usize {
    let mut count = status.supervisors.len();
    if let Some(supervisor) = status.supervisor.as_ref() {
        if !status.supervisors.contains_key(&supervisor.name) {
            count += 1;
        }
    }
    if let Some(supervisor) = status.standby_supervisor.as_ref() {
        if !status.supervisors.contains_key(&supervisor.name)
            && status
                .supervisor
                .as_ref()
                .is_none_or(|active| active.name != supervisor.name)
        {
            count += 1;
        }
    }
    count
}

fn parse_status_timestamp(value: &str) -> Option<DateTime<Utc>> {
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|timestamp| timestamp.with_timezone(&Utc))
}
