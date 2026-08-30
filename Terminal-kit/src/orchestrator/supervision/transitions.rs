use crate::model::SessionState;
use crate::protocol::StatusDirective;

use super::state::TaskStage;

pub struct TransitionOutcome {
    pub stage: TaskStage,
    pub plan_only_signal: bool,
}

pub fn from_status(
    current: TaskStage,
    reported_state: SessionState,
    directive: &StatusDirective,
) -> TransitionOutcome {
    let stage = match reported_state {
        SessionState::Progressing => TaskStage::Running,
        SessionState::Blocked => TaskStage::Blocked,
        SessionState::Stalled => TaskStage::Waiting,
        SessionState::DoneClaimed | SessionState::NeedsValidation => TaskStage::Validating,
        SessionState::Validated => TaskStage::Done,
        SessionState::WeakOutput => TaskStage::Running,
        SessionState::Failed | SessionState::Exited => TaskStage::Failed,
        SessionState::WrongDirection | SessionState::Contradictory | SessionState::NeedsRetry => {
            TaskStage::Running
        }
        SessionState::Booting | SessionState::NotStarted | SessionState::Planned => current,
    };

    let summary = directive.summary.to_ascii_lowercase();
    let plan_only_signal = matches!(
        reported_state,
        SessionState::Progressing | SessionState::DoneClaimed
    ) && directive.files.is_empty()
        && directive.commands.is_empty()
        && (summary.contains("plan")
            || summary.contains("next i will")
            || summary.contains("i will ")
            || summary.contains("i can implement")
            || summary.contains("would you like me to continue")
            || summary.contains("i started")
            || summary.contains("i'm working on")
            || summary.contains("working on it"));

    TransitionOutcome {
        stage,
        plan_only_signal,
    }
}
