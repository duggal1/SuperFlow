use uuid::Uuid;

use crate::model::SessionState;

#[derive(Debug, Clone)]
pub struct SupervisorBranchState {
    pub session_id: Uuid,
    pub name: String,
    pub state_label: String,
    pub active: bool,
    pub owned_workers: Vec<Uuid>,
    pub critical_workers: usize,
    pub pending_decisions: usize,
}

impl SupervisorBranchState {
    pub fn new(session_id: Uuid, name: String, state: SessionState, active: bool) -> Self {
        Self {
            session_id,
            name,
            state_label: state.as_str().to_owned(),
            active,
            owned_workers: Vec::new(),
            critical_workers: 0,
            pending_decisions: 0,
        }
    }

    pub fn burden_score(&self) -> usize {
        self.owned_workers.len() + self.critical_workers.saturating_mul(2) + self.pending_decisions
    }
}
