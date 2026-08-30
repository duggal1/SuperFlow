use blake3::hash;

use crate::model::{SessionRole, SessionState, WorkerPacket};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStage {
    Queued,
    Dispatched,
    Running,
    Blocked,
    Waiting,
    Validating,
    Done,
    Failed,
}

pub fn initial_stage(role: SessionRole, state: SessionState) -> TaskStage {
    if role == SessionRole::Supervisor {
        return TaskStage::Running;
    }
    match state {
        SessionState::Failed => TaskStage::Failed,
        SessionState::Validated => TaskStage::Done,
        SessionState::DoneClaimed | SessionState::NeedsValidation => TaskStage::Validating,
        SessionState::WeakOutput => TaskStage::Running,
        SessionState::Blocked => TaskStage::Blocked,
        SessionState::Progressing | SessionState::Booting | SessionState::NotStarted => {
            TaskStage::Dispatched
        }
        SessionState::Stalled => TaskStage::Waiting,
        SessionState::Planned => TaskStage::Queued,
        SessionState::WrongDirection | SessionState::Contradictory | SessionState::NeedsRetry => {
            TaskStage::Running
        }
        SessionState::Exited => TaskStage::Failed,
    }
}

pub fn assignment_fingerprint(packet: Option<&WorkerPacket>) -> Option<String> {
    let packet = packet?;
    let normalized = format!(
        "{}|{}|{}|{}|{}",
        packet.role_type.trim(),
        packet.display_name.trim(),
        packet
            .owned_scope
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" "),
        packet
            .explicit_task
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" "),
        packet
            .definition_of_done
            .iter()
            .map(|v| v.split_whitespace().collect::<Vec<_>>().join(" "))
            .collect::<Vec<_>>()
            .join(";")
    );
    Some(hash(normalized.as_bytes()).to_hex().to_string())
}
