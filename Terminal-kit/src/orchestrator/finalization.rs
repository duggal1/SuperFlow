use std::collections::HashMap;

use uuid::Uuid;

use crate::model::{SessionRole, SessionState};

use super::{ActiveSession, SupervisorMode};

pub fn workers_are_terminal(active_sessions: &HashMap<Uuid, ActiveSession>) -> bool {
    active_sessions
        .values()
        .filter(|session| session.record.role == SessionRole::Worker)
        .all(|session| session.state.is_terminal())
}

pub fn everyone_exited(active_sessions: &HashMap<Uuid, ActiveSession>) -> bool {
    active_sessions
        .values()
        .all(|session| session.state == SessionState::Exited)
}

pub fn cleanup_authorized(
    active_sessions: &HashMap<Uuid, ActiveSession>,
    active_supervisor_id: Uuid,
) -> bool {
    active_sessions
        .get(&active_supervisor_id)
        .map(|session| session.cleanup_authorized)
        .unwrap_or(false)
}

pub fn should_break_run(
    active_sessions: &HashMap<Uuid, ActiveSession>,
    active_supervisor_id: Uuid,
    final_synthesis_requested: bool,
    _supervisor_mode: SupervisorMode,
) -> bool {
    let workers_terminal = workers_are_terminal(active_sessions);
    let everyone_exited = everyone_exited(active_sessions);
    if everyone_exited {
        return true;
    }

    workers_terminal
        && final_synthesis_requested
        && cleanup_authorized(active_sessions, active_supervisor_id)
}
