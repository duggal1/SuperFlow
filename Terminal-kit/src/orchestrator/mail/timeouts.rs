//! Mail timeout intervals, stage detection, timeout prompts, and pending mail probing.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use anyhow::Result;
use serde_json::json;
use uuid::Uuid;

use crate::orchestrator::ActiveSession;
use crate::store::Store;

// Re-export PendingMail from the orchestrator
pub use super::PendingMail;

pub fn mail_timeout_interval(priority: &str) -> Duration {
    match priority.to_lowercase().as_str() {
        "urgent" | "critical" => Duration::from_secs(10),
        "high" => Duration::from_secs(15),
        "low" => Duration::from_secs(45),
        _ => Duration::from_secs(20),
    }
}

pub fn mail_timeout_stage_due(pending: &PendingMail, now: Instant) -> bool {
    let since = pending.last_timeout_at.unwrap_or(pending.routed_at);
    now.duration_since(since) >= mail_timeout_interval(&pending.priority)
}

pub fn recipient_timeout_prompt(pending: &PendingMail, sender_name: &str, stage: u8) -> String {
    match stage {
        1 => format!(
            "Outstanding {} mail in thread {} from {}: '{}'. Reply now.\nSAPPHIRE_ACK statuses allowed: acked if you own it, done if finished, cannot_comply with one concrete blocker.\nIf partially blocked, send one narrow teammate reply and keep moving.",
            pending.message_type, pending.thread_id, sender_name, pending.subject
        ),
        2 => format!(
            "Second coordination timeout for thread {}. Stop staying silent. Send SAPPHIRE_ACK with status=acked|done|cannot_comply immediately, then one narrow reply with the answer, blocker, or reroute path.",
            pending.thread_id
        ),
        _ => format!(
            "Final coordination timeout for thread {}. Respond now with SAPPHIRE_ACK status=done or cannot_comply. If cannot_comply, include the blocker and the best reroute path so the team can keep moving.",
            pending.thread_id
        ),
    }
}

pub fn sender_timeout_prompt(pending: &PendingMail, recipient_name: &str, stage: u8) -> String {
    match stage {
        1 => format!(
            "Your mail '{}' to {} in thread {} is still waiting on acknowledgment. Keep moving on independent work. Prepare a narrower follow-up or alternate teammate path if it becomes critical.",
            pending.subject, recipient_name, pending.thread_id
        ),
        2 => format!(
            "Your mail '{}' to {} in thread {} is still unanswered after a second timeout. Narrow the ask or reroute through another teammate. Do not keep sending broad follow-ups.",
            pending.subject, recipient_name, pending.thread_id
        ),
        _ => format!(
            "Coordination failed for '{}' with {} in thread {} after repeated timeouts. Keep moving on independent scope. Try another teammate first, then supervisor if you need a ruling.",
            pending.subject, recipient_name, pending.thread_id
        ),
    }
}

pub fn cc_timeout_prompt(pending: &PendingMail, stage: u8) -> String {
    match stage {
        1 => format!(
            "[SAPPHIRE CC NOTICE] Thread {} is waiting on an acknowledgment for '{}'. Monitor only.",
            pending.thread_id, pending.subject
        ),
        2 => format!(
            "[SAPPHIRE CC NOTICE] Thread {} has hit a second timeout for '{}'. Be ready to help if rerouted.",
            pending.thread_id, pending.subject
        ),
        _ => format!(
            "[SAPPHIRE CC NOTICE] Thread {} has entered coordination failure for '{}'. Supervisor review is expected.",
            pending.thread_id, pending.subject
        ),
    }
}

/// Probe unacked mail and escalate to supervisor if timeout exceeded.
#[allow(dead_code)]
pub fn probe_pending_mail(
    store: &Store,
    mission_id: Uuid,
    supervisor_id: Uuid,
    active_sessions: &mut HashMap<Uuid, ActiveSession>,
    pending_mail: &mut HashMap<Uuid, PendingMail>,
    timeout_secs: u64,
) -> Result<()> {
    let now = Instant::now();
    let to_probe: Vec<Uuid> = pending_mail
        .iter()
        .filter(|(_, m)| !m.acked && mail_timeout_stage_due(m, now))
        .map(|(id, _)| *id)
        .collect();

    for mail_id in &to_probe {
        if let Some(pending) = pending_mail.get_mut(mail_id) {
            pending.timeout_stage = pending.timeout_stage.saturating_add(1);
            pending.last_timeout_at = Some(now);

            // Notify sender that ack is overdue
            if let Some(sender) = active_sessions.get(&pending.sender_session_id) {
                let _ = sender.runtime.send_prompt(&format!(
                    "Mail ack timeout: {} (thread: {}) from {} has not been acked after {}s.",
                    pending.subject, pending.thread_id, pending.recipient_session_id, timeout_secs
                ));
            }

            // Notify recipient that ack is overdue
            if let Some(recipient) = active_sessions.get(&pending.recipient_session_id) {
                let _ = recipient.runtime.send_prompt(&format!(
                    "OVERDUE ACK: Mail '{}' from {} (thread: {}) requires acknowledgment. Respond with SAPPHIRE_ACK.",
                    pending.subject, pending.sender_session_id, pending.thread_id
                ));
            }

            // Escalate to supervisor
            store.append_json_event(
                mission_id,
                Some(supervisor_id),
                "mail_ack_timeout",
                &format!("ack overdue: {}", pending.subject),
                &json!({
                    "mail_id": mail_id.to_string(),
                    "thread_id": pending.thread_id,
                    "sender": pending.sender_session_id.to_string(),
                    "recipient": pending.recipient_session_id.to_string(),
                    "subject": pending.subject,
                    "timeout_secs": timeout_secs,
                    "stage": pending.timeout_stage,
                }),
            )?;
        }
    }
    Ok(())
}
