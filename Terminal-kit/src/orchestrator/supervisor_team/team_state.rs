use std::collections::HashMap;
use std::time::Instant;

use uuid::Uuid;

use crate::model::SessionRole;

use super::super::{ActiveSession, PendingMail, PendingSupervisorDecision, SupervisorMode};
use super::agent_state::AgentState;
use super::supervisor_state::SupervisorBranchState;

pub fn routed_supervisor_for_worker(
    active_sessions: &HashMap<Uuid, ActiveSession>,
    worker_session_id: Uuid,
    active_supervisor_id: Uuid,
) -> Uuid {
    active_sessions
        .get(&worker_session_id)
        .and_then(|session| session.supervising_supervisor_id)
        .filter(|supervisor_id| {
            active_sessions.get(supervisor_id).is_some_and(|session| {
                session.record.role == SessionRole::Supervisor && !session.state.is_terminal()
            })
        })
        .unwrap_or(active_supervisor_id)
}

pub fn rebalance_worker_supervision(
    active_sessions: &mut HashMap<Uuid, ActiveSession>,
    supervisor_ids: &[Uuid],
    active_supervisor_id: Uuid,
) -> Vec<String> {
    let live_supervisors = supervisor_ids
        .iter()
        .copied()
        .filter(|session_id| {
            active_sessions.get(session_id).is_some_and(|session| {
                session.record.role == SessionRole::Supervisor && !session.state.is_terminal()
            })
        })
        .collect::<Vec<_>>();
    if live_supervisors.is_empty() {
        return Vec::new();
    }

    let mut load = live_supervisors
        .iter()
        .copied()
        .map(|session_id| (session_id, 0usize))
        .collect::<HashMap<_, _>>();

    for session in active_sessions.values() {
        if session.record.role == SessionRole::Worker
            && !session.state.is_terminal()
            && let Some(owner) = session.supervising_supervisor_id
            && let Some(count) = load.get_mut(&owner)
        {
            *count += 1;
        }
    }

    let mut changes = Vec::new();
    let worker_ids = active_sessions
        .iter()
        .filter_map(|(session_id, session)| {
            (session.record.role == SessionRole::Worker && !session.state.is_terminal())
                .then_some(*session_id)
        })
        .collect::<Vec<_>>();

    for worker_id in worker_ids {
        let best_owner =
            least_loaded_supervisor(&load, &live_supervisors).unwrap_or(active_supervisor_id);
        let (current_owner, worker_name) = match active_sessions.get(&worker_id) {
            Some(session) => (
                session.supervising_supervisor_id,
                session.record.name.clone(),
            ),
            None => continue,
        };
        let current_owner_live = current_owner
            .is_some_and(|owner| live_supervisors.iter().any(|candidate| candidate == &owner));
        if current_owner_live {
            continue;
        }
        let previous = current_owner
            .and_then(|owner| active_sessions.get(&owner))
            .map(|owner| owner.record.name.clone())
            .unwrap_or_else(|| "unassigned".to_owned());
        let next_name = active_sessions
            .get(&best_owner)
            .map(|owner| owner.record.name.clone())
            .unwrap_or_else(|| "supervisor".to_owned());
        let Some(session) = active_sessions.get_mut(&worker_id) else {
            continue;
        };
        session.supervising_supervisor_id = Some(best_owner);
        *load.entry(best_owner).or_default() += 1;
        changes.push(format!(
            "{} reassigned from {} to {}",
            worker_name, previous, next_name
        ));
    }

    while let Some((busiest, lightest, gap)) = busiest_and_lightest(&load) {
        if gap < 2 {
            break;
        }
        let candidate = active_sessions
            .values()
            .filter(|session| {
                session.record.role == SessionRole::Worker
                    && !session.state.is_terminal()
                    && session.supervising_supervisor_id == Some(busiest)
                    && !matches!(
                        session.state,
                        crate::model::SessionState::Blocked | crate::model::SessionState::Stalled
                    )
            })
            .max_by_key(|session| {
                session.plan_only_count + session.repeated_status_without_evidence
            })
            .map(|session| session.record.id);
        let Some(worker_id) = candidate else {
            break;
        };
        if busiest == lightest {
            break;
        }
        let from_name = active_sessions
            .get(&busiest)
            .map(|session| session.record.name.clone())
            .unwrap_or_else(|| "supervisor".to_owned());
        let to_name = active_sessions
            .get(&lightest)
            .map(|session| session.record.name.clone())
            .unwrap_or_else(|| "supervisor".to_owned());
        let worker_name = active_sessions
            .get(&worker_id)
            .map(|session| session.record.name.clone())
            .unwrap_or_else(|| "worker".to_owned());
        let Some(worker) = active_sessions.get_mut(&worker_id) else {
            break;
        };
        worker.supervising_supervisor_id = Some(lightest);
        *load.entry(busiest).or_default() =
            load.get(&busiest).copied().unwrap_or(1).saturating_sub(1);
        *load.entry(lightest).or_default() += 1;
        changes.push(format!(
            "{} rebalanced from {} to {}",
            worker_name, from_name, to_name
        ));
    }

    changes
}

pub fn build_supervisor_cards(
    active_sessions: &HashMap<Uuid, ActiveSession>,
    pending_mail: &HashMap<Uuid, PendingMail>,
    pending_decisions: &HashMap<String, PendingSupervisorDecision>,
    supervisor_mode: SupervisorMode,
    started_at: Instant,
    supervisor_ids: &[Uuid],
    active_supervisor_id: Uuid,
) -> Vec<(Uuid, String)> {
    let snapshot = TeamMissionState::build(
        active_sessions,
        pending_mail,
        pending_decisions,
        supervisor_mode,
        started_at,
        supervisor_ids,
        active_supervisor_id,
    );
    snapshot.render_cards()
}

struct TeamMissionState {
    runtime_secs: u64,
    active_supervisor_id: Uuid,
    workers: Vec<AgentState>,
    supervisors: Vec<SupervisorBranchState>,
    pending_mail_count: usize,
    unassigned_workers: Vec<String>,
    first_status_incidents: Vec<String>,
    no_progress_workers: Vec<String>,
    blocked_workers: Vec<String>,
}

impl TeamMissionState {
    fn build(
        active_sessions: &HashMap<Uuid, ActiveSession>,
        pending_mail: &HashMap<Uuid, PendingMail>,
        pending_decisions: &HashMap<String, PendingSupervisorDecision>,
        _supervisor_mode: SupervisorMode,
        started_at: Instant,
        supervisor_ids: &[Uuid],
        active_supervisor_id: Uuid,
    ) -> Self {
        let now = Instant::now();
        let mut workers = active_sessions
            .values()
            .filter_map(|session| AgentState::from_session(session, pending_mail, now))
            .collect::<Vec<_>>();
        workers.sort_by(|left, right| {
            right
                .attention_score
                .cmp(&left.attention_score)
                .then_with(|| left.name.cmp(&right.name))
        });

        let mut supervisors = supervisor_ids
            .iter()
            .filter_map(|session_id| {
                let session = active_sessions.get(session_id)?;
                Some(SupervisorBranchState::new(
                    *session_id,
                    session.record.name.clone(),
                    session.state,
                    *session_id == active_supervisor_id,
                ))
            })
            .collect::<Vec<_>>();
        let supervisor_index = supervisors
            .iter()
            .enumerate()
            .map(|(index, state)| (state.session_id, index))
            .collect::<HashMap<_, _>>();

        for worker in &workers {
            if let Some(owner) = worker.owner_supervisor_id
                && let Some(index) = supervisor_index.get(&owner).copied()
            {
                supervisors[index].owned_workers.push(worker.session_id);
                if worker.attention_score >= 4 {
                    supervisors[index].critical_workers += 1;
                }
            }
        }

        for pending in pending_decisions.values() {
            let owner = routed_supervisor_for_worker(
                active_sessions,
                pending.target_session_id,
                active_supervisor_id,
            );
            if let Some(index) = supervisor_index.get(&owner).copied() {
                supervisors[index].pending_decisions += 1;
            }
        }

        supervisors.sort_by(|left, right| {
            right
                .active
                .cmp(&left.active)
                .then_with(|| right.burden_score().cmp(&left.burden_score()))
                .then_with(|| left.name.cmp(&right.name))
        });

        let unassigned_workers = workers
            .iter()
            .filter(|worker| worker.owner_supervisor_id.is_none())
            .map(|worker| worker.name.clone())
            .collect::<Vec<_>>();
        let first_status_incidents = workers
            .iter()
            .filter(|worker| worker.first_status_overdue)
            .map(|worker| worker.name.clone())
            .collect::<Vec<_>>();
        let no_progress_workers = workers
            .iter()
            .filter(|worker| worker.no_progress_loop)
            .map(|worker| worker.name.clone())
            .collect::<Vec<_>>();
        let blocked_workers = workers
            .iter()
            .filter(|worker| worker.blocked || worker.contradictory)
            .map(|worker| worker.name.clone())
            .collect::<Vec<_>>();

        Self {
            runtime_secs: started_at.elapsed().as_secs(),
            active_supervisor_id,
            workers,
            supervisors,
            pending_mail_count: pending_mail
                .values()
                .filter(|pending| !pending.acked)
                .count(),
            unassigned_workers,
            first_status_incidents,
            no_progress_workers,
            blocked_workers,
        }
    }

    fn render_cards(&self) -> Vec<(Uuid, String)> {
        self.supervisors
            .iter()
            .map(|supervisor_state| {
                let owned_workers = self
                    .workers
                    .iter()
                    .filter(|worker| worker.owner_supervisor_id == Some(supervisor_state.session_id))
                    .collect::<Vec<_>>();
                let mut card = String::new();
                let mins = self.runtime_secs / 60;
                let secs = self.runtime_secs % 60;
                let branch_mode = if supervisor_state.active {
                    "ACTIVE"
                } else {
                    "BRANCH"
                };
                card.push_str(&format!(
                    "SUPERVISOR TEAM STATE CARD [{} | {}m {}s]\n",
                    branch_mode, mins, secs
                ));
                card.push_str(&format!(
                    "Global mission: workers={} first_status_incidents={} blocked_or_contradictory={} no_progress_loops={} pending_mail={}\n",
                    self.workers.len(),
                    self.first_status_incidents.len(),
                    self.blocked_workers.len(),
                    self.no_progress_workers.len(),
                    self.pending_mail_count,
                ));
                if !self.unassigned_workers.is_empty() {
                    card.push_str(&format!(
                        "Ownership gaps: {}\n",
                        self.unassigned_workers.join(", ")
                    ));
                }
                card.push_str(&format!(
                    "Your branch: {} [{}] owned_workers={} critical={} pending_decisions={}\n",
                    supervisor_state.name,
                    supervisor_state.state_label,
                    owned_workers.len(),
                    supervisor_state.critical_workers,
                    supervisor_state.pending_decisions,
                ));
                if owned_workers.is_empty() {
                    card.push_str("Owned workers:\n  (none)\n");
                } else {
                    card.push_str("Owned workers:\n");
                    for worker in owned_workers.iter().take(12) {
                        let restart_flag = if worker.restart_pending {
                            " restart_pending"
                        } else {
                            ""
                        };
                        card.push_str(&format!(
                            "  {} [{}|{}{}] mail={} {}\n",
                            worker.name,
                            worker.state_label,
                            worker.liveness_label,
                            restart_flag,
                            worker.pending_mail_threads,
                            worker.summary.chars().take(110).collect::<String>(),
                        ));
                    }
                }
                let urgent_owned = owned_workers
                    .iter()
                    .filter(|worker| worker.first_status_overdue || worker.blocked || worker.no_progress_loop)
                    .map(|worker| worker.name.clone())
                    .collect::<Vec<_>>();
                if !urgent_owned.is_empty() {
                    card.push_str(&format!(
                        "Immediate branch interventions: {}\n",
                        urgent_owned.join(", ")
                    ));
                }
                if !self.first_status_incidents.is_empty() {
                    card.push_str(&format!(
                        "Global first-status incident set: {}\n",
                        self.first_status_incidents.join(", ")
                    ));
                }
                if !self.no_progress_workers.is_empty() {
                    card.push_str(&format!(
                        "Anti-loop targets: {}\n",
                        self.no_progress_workers.join(", ")
                    ));
                }
                if self.active_supervisor_id == supervisor_state.session_id {
                    card.push_str(
                        "Active-supervisor responsibility: resolve systemic incidents, ownership gaps, and final mission closure.\n",
                    );
                } else {
                    card.push_str(
                        "Branch-supervisor responsibility: act on owned workers directly; escalate only cross-branch or systemic incidents.\n",
                    );
                }
                (supervisor_state.session_id, card)
            })
            .collect()
    }
}

fn least_loaded_supervisor(load: &HashMap<Uuid, usize>, live_supervisors: &[Uuid]) -> Option<Uuid> {
    live_supervisors
        .iter()
        .copied()
        .min_by_key(|session_id| load.get(session_id).copied().unwrap_or(0))
}

fn busiest_and_lightest(load: &HashMap<Uuid, usize>) -> Option<(Uuid, Uuid, usize)> {
    let busiest = load.iter().max_by_key(|(_, count)| **count)?;
    let lightest = load.iter().min_by_key(|(_, count)| **count)?;
    Some((
        *busiest.0,
        *lightest.0,
        busiest.1.saturating_sub(*lightest.1),
    ))
}
