use std::collections::HashMap;
use std::time::Duration;

use anyhow::Result;
use serde_json::json;
use uuid::Uuid;

use crate::adapter::SupervisorEventType;
use crate::model::{SessionRole, SessionState};

use super::super::{
    ActiveSession, PendingSupervisorDecision, SupervisorDecisionKind, queue_supervisor_decision,
};

pub struct ReportingConfig {
    pub initial_status_deadline: Duration,
}

impl Default for ReportingConfig {
    fn default() -> Self {
        Self {
            initial_status_deadline: Duration::from_secs(25),
        }
    }
}

/// Enforce mandatory reporting.
///
/// - If a worker hasn't emitted an initial status by deadline, escalate to supervisor.
/// - If a worker is low-observability (output but no directives), rely on existing enforcement paths.
pub fn enforce_reporting(
    active_sessions: &mut HashMap<Uuid, ActiveSession>,
    pending_supervisor_decisions: &mut HashMap<String, PendingSupervisorDecision>,
    config: ReportingConfig,
    now: std::time::Instant,
) -> Result<Vec<(SupervisorEventType, String, Uuid)>> {
    let mut notices = Vec::new();
    for (session_id, session) in active_sessions.iter_mut() {
        if session.record.role != SessionRole::Worker || session.state.is_terminal() {
            continue;
        }

        if !session.initial_status_received
            && session.started_at.elapsed() >= config.initial_status_deadline
        {
            let reason = format!(
                "Worker {} has not reported an initial SAPPHIRE_STATUS within {}s of dispatch. Force a status update immediately or redirect if unresponsive.",
                session.record.name,
                config.initial_status_deadline.as_secs()
            );

            if queue_supervisor_decision(
                pending_supervisor_decisions,
                SupervisorDecisionKind::LowConfidenceRecovery,
                *session_id,
                &reason,
            ) {
                let _ = json!({"name": session.record.name}); // keep shape stable for callers
                notices.push((SupervisorEventType::WeakOutput, reason, *session_id));
            }
        }

        // If a worker declares done/validated, treat that as having reported status.
        if matches!(
            session.state,
            SessionState::DoneClaimed | SessionState::NeedsValidation | SessionState::Validated
        ) {
            session.initial_status_received = true;
        }
    }

    let _ = now; // reserved for future enforcement (periodic reporting windows)
    Ok(notices)
}
