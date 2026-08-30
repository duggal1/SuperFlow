use crate::adapter::SupervisorEventType;
use crate::model::SessionState;
use crate::protocol::StatusDirective;

use super::state::TaskStage;

pub struct InterceptDecision {
    pub event_type: SupervisorEventType,
    pub notice_body: String,
}

/// Detect “plan-only” or weak completion and request supervisor follow-up.
///
/// This is intentionally conservative: it triggers only when a worker is repeatedly
/// reporting planning intent without concrete evidence (files/commands).
pub fn maybe_intercept_plan_only(
    worker_name: &str,
    stage: TaskStage,
    plan_only_count: usize,
    reported_state: SessionState,
    directive: &StatusDirective,
) -> Option<InterceptDecision> {
    if plan_only_count < 1 {
        return None;
    }
    if !matches!(stage, TaskStage::Running | TaskStage::Validating) {
        return None;
    }
    if !matches!(
        reported_state,
        SessionState::Progressing | SessionState::DoneClaimed
    ) {
        return None;
    }
    if !(directive.files.is_empty() && directive.commands.is_empty()) {
        return None;
    }

    Some(InterceptDecision {
        event_type: SupervisorEventType::WeakOutput,
        notice_body: format!(
            "{} is reporting planning intent without evidence (no files/commands). Force execution now: demand exact next command(s), expected output, and the first concrete change or test result. If they cannot, redirect or fail decisively.",
            worker_name
        ),
    })
}
