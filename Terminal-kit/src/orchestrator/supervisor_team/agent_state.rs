use std::collections::HashMap;
use std::time::{Duration, Instant};

use uuid::Uuid;

use crate::model::{SessionRole, SessionState};

use super::super::{ActiveSession, PendingMail};

const RECENT_ACTIVITY_GRACE: Duration = Duration::from_secs(20);

#[derive(Debug, Clone)]
pub struct AgentState {
    pub session_id: Uuid,
    pub name: String,
    pub owner_supervisor_id: Option<Uuid>,
    pub state_label: String,
    pub liveness_label: &'static str,
    pub summary: String,
    pub blocked: bool,
    pub contradictory: bool,
    pub restart_pending: bool,
    pub first_status_overdue: bool,
    pub no_progress_loop: bool,
    pub pending_mail_threads: usize,
    pub attention_score: usize,
}

impl AgentState {
    pub fn from_session(
        session: &ActiveSession,
        pending_mail: &HashMap<Uuid, PendingMail>,
        now: Instant,
    ) -> Option<Self> {
        if session.record.role != SessionRole::Worker {
            return None;
        }

        let pending_mail_threads = pending_mail
            .values()
            .filter(|pending| {
                pending.thread_state != "closed"
                    && (pending.sender_session_id == session.record.id
                        || pending.recipient_session_id == session.record.id)
            })
            .count();
        let first_status_overdue = !session.initial_status_received
            && session
                .launch_prompt_sent_at
                .is_some_and(|at| now.duration_since(at) >= Duration::from_secs(25))
            && now >= session.startup_grace_until;
        let no_progress_loop =
            session.plan_only_count >= 2 || session.repeated_status_without_evidence >= 2;
        let blocked = matches!(session.state, SessionState::Blocked | SessionState::Stalled);
        let contradictory = matches!(
            session.state,
            SessionState::Contradictory | SessionState::WrongDirection | SessionState::NeedsRetry
        );
        let liveness_label = classify_liveness(session, now, first_status_overdue);

        let mut attention_score = 0;
        if first_status_overdue {
            attention_score += 5;
        }
        if blocked {
            attention_score += 4;
        }
        if contradictory {
            attention_score += 4;
        }
        if no_progress_loop {
            attention_score += 3;
        }
        if pending_mail_threads > 0 {
            attention_score += 1;
        }

        Some(Self {
            session_id: session.record.id,
            name: session.record.name.clone(),
            owner_supervisor_id: session.supervising_supervisor_id,
            state_label: session.state.as_str().to_owned(),
            liveness_label,
            summary: session
                .record
                .last_summary
                .clone()
                .unwrap_or_else(|| "no summary".to_owned()),
            blocked,
            contradictory,
            restart_pending: session.restart_at.is_some(),
            first_status_overdue,
            no_progress_loop,
            pending_mail_threads,
            attention_score,
        })
    }
}

fn classify_liveness(
    session: &ActiveSession,
    now: Instant,
    first_status_overdue: bool,
) -> &'static str {
    if matches!(session.state, SessionState::Failed | SessionState::Exited) {
        "failed"
    } else if session.state == SessionState::Validated {
        "done"
    } else if !session.launch_prompt_sent {
        "assigned"
    } else if now < session.startup_grace_until {
        "booting"
    } else if first_status_overdue && session.output_chunks == 0 {
        "silent"
    } else if first_status_overdue {
        "reporting_gap"
    } else if session.last_status_update_at.is_some() {
        "reporting"
    } else if session.output_chunks > 0
        && now.duration_since(session.last_output_at) <= RECENT_ACTIVITY_GRACE
    {
        "executing"
    } else if session.output_chunks > 0 {
        "alive_unconfirmed"
    } else {
        "prompt_delivered"
    }
}
