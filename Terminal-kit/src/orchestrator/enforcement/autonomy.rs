use std::time::{Duration, Instant};

use crate::adapter::SupervisorEventType;
use crate::model::SessionState;

use super::super::{ActiveSession, PendingSupervisorDecision, SupervisorDecisionKind};

const AUTONOMOUS_PROMPT_AFTER: Duration = Duration::from_secs(150);
const AUTONOMOUS_TERMINAL_AFTER: Duration = Duration::from_secs(300);
const AUTONOMOUS_RETRY_AFTER: Duration = Duration::from_secs(240);
const AUTONOMOUS_COOLDOWN: Duration = Duration::from_secs(90);

pub enum AutonomousDecision {
    Prompt {
        prompt_kind: PromptKind,
        message: String,
        intervention_type: &'static str,
        event_type: SupervisorEventType,
    },
    OverrideState {
        state: SessionState,
        summary: String,
        intervention_type: &'static str,
        event_type: SupervisorEventType,
    },
}

pub enum PromptKind {
    Status,
    Validation,
}

pub fn should_attempt_autonomous_resolution(
    pending: &PendingSupervisorDecision,
    now: Instant,
) -> bool {
    if pending
        .last_autonomous_action_at
        .is_some_and(|at| now.duration_since(at) < AUTONOMOUS_COOLDOWN)
    {
        return false;
    }
    pending.notice_count >= 1 && now.duration_since(pending.queued_at) >= AUTONOMOUS_PROMPT_AFTER
}

pub fn mark_autonomous_action(pending: &mut PendingSupervisorDecision, now: Instant) {
    pending.autonomous_action_count = pending.autonomous_action_count.saturating_add(1);
    pending.last_autonomous_action_at = Some(now);
}

pub fn autonomous_resolution(
    pending: &PendingSupervisorDecision,
    target: &ActiveSession,
    now: Instant,
) -> Option<AutonomousDecision> {
    let age = now.duration_since(pending.queued_at);
    match pending.kind {
        SupervisorDecisionKind::Validation => validation_resolution(pending, target, age),
        SupervisorDecisionKind::StallRecovery => stall_resolution(pending, target, now, age),
        SupervisorDecisionKind::LowConfidenceRecovery => {
            low_confidence_resolution(pending, target, age)
        }
        SupervisorDecisionKind::OverlapRecovery => overlap_resolution(pending, target, age),
    }
}

fn validation_resolution(
    pending: &PendingSupervisorDecision,
    target: &ActiveSession,
    age: Duration,
) -> Option<AutonomousDecision> {
    if target.state.is_terminal() || !target.validation_pending {
        return None;
    }
    if pending.autonomous_action_count == 0 {
        return Some(AutonomousDecision::Prompt {
            prompt_kind: PromptKind::Validation,
            message: format!(
                "Validation is overdue. Report only concrete proof now: exact changed files, exact command/test results, and any remaining blocker or risk. No planning language. {}",
                pending.reason
            ),
            intervention_type: "autonomous_validation_prompt",
            event_type: SupervisorEventType::DoneClaimed,
        });
    }
    if age >= AUTONOMOUS_RETRY_AFTER
        && (target.plan_only_count >= 2 || target.repeated_status_without_evidence >= 2)
    {
        return Some(AutonomousDecision::OverrideState {
            state: SessionState::NeedsRetry,
            summary:
                "validation unresolved: concrete proof never arrived after autonomous enforcement"
                    .to_owned(),
            intervention_type: "autonomous_validation_retry",
            event_type: SupervisorEventType::WeakOutput,
        });
    }
    None
}

fn stall_resolution(
    pending: &PendingSupervisorDecision,
    target: &ActiveSession,
    now: Instant,
    age: Duration,
) -> Option<AutonomousDecision> {
    if target.state != SessionState::Stalled {
        return None;
    }
    if pending.autonomous_action_count == 0 {
        return Some(AutonomousDecision::Prompt {
            prompt_kind: PromptKind::Status,
            message: format!(
                "Stall recovery is now under direct control-plane enforcement. Report one exact Sapphire status now with your true state, current blocker, next command, and whether you need reroute or restart. {}",
                pending.reason
            ),
            intervention_type: "autonomous_stall_prompt",
            event_type: SupervisorEventType::Stall,
        });
    }
    if age >= AUTONOMOUS_TERMINAL_AFTER && !super::super::live_state::session_is_live(target, now) {
        return Some(AutonomousDecision::OverrideState {
            state: SessionState::Failed,
            summary: "worker failed: nonresponsive after repeated stall recovery and autonomous enforcement".to_owned(),
            intervention_type: "autonomous_stall_fail",
            event_type: SupervisorEventType::Failed,
        });
    }
    None
}

fn low_confidence_resolution(
    pending: &PendingSupervisorDecision,
    target: &ActiveSession,
    age: Duration,
) -> Option<AutonomousDecision> {
    if pending.autonomous_action_count == 0 {
        return Some(AutonomousDecision::Prompt {
            prompt_kind: PromptKind::Status,
            message: format!(
                "Weak execution detected. Stop narrating and emit one exact Sapphire status now with exact progress, touched files, commands run, next command, and blockers. If no implementation happened, say so explicitly. {}",
                pending.reason
            ),
            intervention_type: "autonomous_low_confidence_prompt",
            event_type: SupervisorEventType::WeakOutput,
        });
    }
    if age >= AUTONOMOUS_RETRY_AFTER
        && (target.plan_only_count >= 3 || target.repeated_status_without_evidence >= 3)
    {
        return Some(AutonomousDecision::OverrideState {
            state: SessionState::NeedsRetry,
            summary: "worker reroute required: repeated plan-only or no-evidence loop survived autonomous enforcement".to_owned(),
            intervention_type: "autonomous_low_confidence_retry",
            event_type: SupervisorEventType::WeakOutput,
        });
    }
    None
}

fn overlap_resolution(
    pending: &PendingSupervisorDecision,
    target: &ActiveSession,
    age: Duration,
) -> Option<AutonomousDecision> {
    let overlap_active = target
        .reported_overlap
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty());
    if !overlap_active {
        return None;
    }
    if pending.autonomous_action_count == 0 {
        return Some(AutonomousDecision::Prompt {
            prompt_kind: PromptKind::Status,
            message: format!(
                "Ownership conflict is unresolved. Report one exact Sapphire status now with the overlapping files, current owner assumption, teammate dependency, and the safe next step that preserves existing work. {}",
                pending.reason
            ),
            intervention_type: "autonomous_overlap_prompt",
            event_type: SupervisorEventType::Contradiction,
        });
    }
    if age >= AUTONOMOUS_RETRY_AFTER {
        return Some(AutonomousDecision::OverrideState {
            state: SessionState::NeedsRetry,
            summary: "worker reroute required: overlap remained unresolved after autonomous ownership enforcement".to_owned(),
            intervention_type: "autonomous_overlap_retry",
            event_type: SupervisorEventType::Contradiction,
        });
    }
    None
}
