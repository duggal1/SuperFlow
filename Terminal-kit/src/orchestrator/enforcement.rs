use std::time::{Duration, Instant};

use crate::adapter::SupervisorEventType;
use crate::model::{SessionRole, SessionState};

use super::{ActiveSession, PendingSupervisorDecision, SupervisorDecisionKind};

mod antiloop;
mod autonomy;
mod evidence;

pub use antiloop::{StatusSignature, compute_status_signature, note_status_signature};
pub use autonomy::{
    AutonomousDecision, PromptKind, autonomous_resolution, mark_autonomous_action,
    should_attempt_autonomous_resolution,
};
#[allow(unused_imports)]
pub use evidence::evidence_missing_for_done_claim;

const INITIAL_REPORT_OUTPUT_CHUNKS: usize = 4;
const FOLLOW_UP_REPORT_OUTPUT_CHUNKS: usize = 12;
const FOLLOW_UP_REPORT_SILENCE: Duration = Duration::from_secs(75);
const SUPERVISOR_FOLLOW_UP_INTERVAL: Duration = Duration::from_secs(90);
const MAX_SUPERVISOR_FOLLOW_UPS: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportBackKind {
    Initial,
    Progress,
}

pub fn canonicalize_worker_state(
    session_role: SessionRole,
    reported_state: SessionState,
    supervisor_override: bool,
) -> SessionState {
    if supervisor_override || session_role != SessionRole::Worker {
        return reported_state;
    }

    match reported_state {
        SessionState::Validated => SessionState::NeedsValidation,
        other => other,
    }
}

pub fn status_summary(report_kind: ReportBackKind, worker_name: &str) -> String {
    match report_kind {
        ReportBackKind::Initial => {
            format!("{worker_name} owes the first Sapphire status update.")
        }
        ReportBackKind::Progress => {
            format!("{worker_name} is active but owes a fresh Sapphire status update.")
        }
    }
}

pub fn status_reason(report_kind: ReportBackKind, worker_name: &str) -> String {
    match report_kind {
        ReportBackKind::Initial => format!(
            "Worker {worker_name} has started output but still has not sent the mandatory first Sapphire status. Require one concise status update now with exact scope, current action, and blocker or next step."
        ),
        ReportBackKind::Progress => format!(
            "Worker {worker_name} is still active but has not reported a fresh Sapphire status after material work. Require one concise status update now with exact progress, touched files, and any teammate dependency."
        ),
    }
}

pub fn pending_report_back(session: &ActiveSession, now: Instant) -> Option<ReportBackKind> {
    if session.record.role != SessionRole::Worker || session.state.is_terminal() {
        return None;
    }

    if !session.initial_status_received
        && session.output_chunks >= INITIAL_REPORT_OUTPUT_CHUNKS
        && now >= session.startup_grace_until
    {
        return Some(ReportBackKind::Initial);
    }

    if session.validation_pending {
        return None;
    }

    let last_status = session.last_status_update_at?;
    let chunks_since_last_status = session
        .output_chunks
        .saturating_sub(session.output_chunks_at_last_status);

    if chunks_since_last_status >= FOLLOW_UP_REPORT_OUTPUT_CHUNKS
        && now.duration_since(last_status) >= FOLLOW_UP_REPORT_SILENCE
    {
        return Some(ReportBackKind::Progress);
    }

    None
}

pub fn note_status_report(session: &mut ActiveSession, now: Instant) {
    session.initial_status_received = true;
    session.output_chunks_at_last_status = session.output_chunks;
    session.last_status_update_at = Some(now);
    session.protocol_reminder_sent = false;
}

pub fn meaningful_overlap_detail(overlap: Option<&str>) -> Option<String> {
    let normalized = overlap.map(str::trim).filter(|value| !value.is_empty())?;
    let lowered = normalized.to_ascii_lowercase();
    let benign_prefixes = [
        "none",
        "none detected",
        "none detected yet",
        "no overlap",
        "no overlap yet",
        "n/a",
        "na",
    ];
    if benign_prefixes
        .iter()
        .any(|prefix| lowered == *prefix || lowered.starts_with(&format!("{prefix} ")))
    {
        None
    } else {
        Some(normalized.to_owned())
    }
}

pub fn action_resolves_decision(kind: SupervisorDecisionKind, action_name: &str) -> bool {
    let action = action_name.trim().to_ascii_lowercase();
    match kind {
        SupervisorDecisionKind::Validation => {
            matches!(action.as_str(), "accept_worker" | "fail_worker")
        }
        SupervisorDecisionKind::StallRecovery => matches!(
            action.as_str(),
            "retry_worker" | "redirect_worker" | "message_worker" | "accept_worker" | "fail_worker"
        ),
        SupervisorDecisionKind::LowConfidenceRecovery => {
            matches!(
                action.as_str(),
                "message_worker"
                    | "retry_worker"
                    | "redirect_worker"
                    | "validate_worker"
                    | "accept_worker"
                    | "fail_worker"
            )
        }
        SupervisorDecisionKind::OverlapRecovery => matches!(
            action.as_str(),
            "retry_worker" | "redirect_worker" | "message_worker" | "accept_worker" | "fail_worker"
        ),
    }
}

pub fn follow_up_event_type(kind: SupervisorDecisionKind) -> SupervisorEventType {
    match kind {
        SupervisorDecisionKind::Validation => SupervisorEventType::DoneClaimed,
        SupervisorDecisionKind::StallRecovery => SupervisorEventType::Stall,
        SupervisorDecisionKind::LowConfidenceRecovery => SupervisorEventType::WeakOutput,
        SupervisorDecisionKind::OverlapRecovery => SupervisorEventType::Contradiction,
    }
}

pub fn should_follow_up_pending_decision(
    pending: &PendingSupervisorDecision,
    now: Instant,
) -> bool {
    pending.notice_count < MAX_SUPERVISOR_FOLLOW_UPS
        && now.duration_since(pending.last_notified_at) >= SUPERVISOR_FOLLOW_UP_INTERVAL
}

pub fn mark_pending_decision_notified(pending: &mut PendingSupervisorDecision, now: Instant) {
    pending.last_notified_at = now;
    pending.notice_count = pending.notice_count.saturating_add(1);
}

pub fn follow_up_reason(
    pending: &PendingSupervisorDecision,
    target: &ActiveSession,
    now: Instant,
) -> String {
    let pending_secs = now.duration_since(pending.queued_at).as_secs();
    let summary = target
        .record
        .last_summary
        .as_deref()
        .unwrap_or("no worker summary yet");
    match pending.kind {
        SupervisorDecisionKind::Validation => format!(
            "Validation still unresolved for {} after {}s. Trigger: {}. Current state: {}. Latest summary: {}. Accept, retry, or fail with one decisive supervisor action.",
            target.record.name,
            pending_secs,
            pending.reason,
            target.state.as_str(),
            summary
        ),
        SupervisorDecisionKind::StallRecovery => format!(
            "Stall recovery still unresolved for {} after {}s. Trigger: {}. Current state: {}. Latest summary: {}. Decide one narrow next step now.",
            target.record.name,
            pending_secs,
            pending.reason,
            target.state.as_str(),
            summary
        ),
        SupervisorDecisionKind::LowConfidenceRecovery => format!(
            "Report-back enforcement is still unresolved for {} after {}s. Trigger: {}. Current state: {}. Latest summary: {}. Require one concrete status update with proof or reroute the worker.",
            target.record.name,
            pending_secs,
            pending.reason,
            target.state.as_str(),
            summary
        ),
        SupervisorDecisionKind::OverlapRecovery => format!(
            "Overlap recovery is still unresolved for {} after {}s. Trigger: {}. Current state: {}. Latest summary: {}. Preserve teammate work, settle ownership, and issue one decisive reroute or contradiction ruling.",
            target.record.name,
            pending_secs,
            pending.reason,
            target.state.as_str(),
            summary
        ),
    }
}
