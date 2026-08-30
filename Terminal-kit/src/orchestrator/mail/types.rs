//! Message type normalization, delivery mode derivation, ack requirements, and validation.
//! Also includes orchestrator helpers: MailStats, resolve_alias, parse_mail_id, MailHandlingResult.
//! Also includes QueuedNudge type and nudge_from_mail constructor.

use std::collections::HashMap;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::protocol::MailDirective;

/// Normalize legacy + new message types to the 5 clean engineering types.
/// Gas Town pattern: task, reply, notification, escalation, scavenge.
/// Legacy types map forward; unknown defaults to notification.
pub fn normalize_message_type(raw: &str) -> &'static str {
    match raw.trim().to_lowercase().as_str() {
        // New clean types pass through
        "task" => "task",
        "reply" => "reply",
        "notification" => "notification",
        "escalation" => "escalation",
        "scavenge" => "scavenge",
        // Legacy → task (requires action from recipient)
        "dependency_request"
        | "dependency_response"
        | "review_request"
        | "review_response"
        | "handoff"
        | "collision_warning" => "task",
        // Legacy → notification (FYI, no action)
        "completion_notice" => "notification",
        // Legacy → escalation (blocker requiring supervisor attention)
        "blocker" | "architecture_concern" | "supervisor_directive" => "escalation",
        _ => "notification",
    }
}

/// Derive delivery mode from priority if not explicitly set.
pub fn derive_delivery_mode(priority: &str, explicit: &str) -> &'static str {
    if !explicit.is_empty() {
        return match explicit.to_lowercase().as_str() {
            "interrupt" => "interrupt",
            "queue" => "queue",
            _ => "queue",
        };
    }
    match priority.to_lowercase().as_str() {
        "urgent" | "critical" => "interrupt",
        _ => "queue",
    }
}

/// Determine whether ack is required based on normalized type and priority.
pub fn requires_ack(msg_type: &str, priority: &str, explicit_ack: bool) -> bool {
    if explicit_ack {
        return true;
    }
    match msg_type {
        "task" | "escalation" | "scavenge" => true,
        "notification" => matches!(priority.to_lowercase().as_str(), "urgent" | "high"),
        "reply" => false,
        _ => false,
    }
}

// ─── Validation ──────────────────────────────────────────────────────────────

/// Validate mail directive before routing. Returns error message on failure.
pub fn validate_mail(
    directive: &MailDirective,
    sender_session_id: uuid::Uuid,
    recipient_session_id: uuid::Uuid,
) -> Option<String> {
    if directive.subject.is_empty() {
        return Some("SAPPHIRE_MAIL rejected: subject is empty.".to_owned());
    }
    if directive.subject.len() > 120 {
        return Some(format!(
            "SAPPHIRE_MAIL rejected: subject too long ({} chars, max 120).",
            directive.subject.len()
        ));
    }
    let body_len =
        directive.context.len() + directive.request.len() + directive.expected_action.len();
    if body_len > 8192 {
        return Some(format!(
            "SAPPHIRE_MAIL rejected: body too long ({} chars, max 8KB).",
            body_len
        ));
    }
    if sender_session_id == recipient_session_id
        && !matches!(
            normalize_message_type(&directive.message_type),
            "task" | "notification"
        )
    {
        return Some("SAPPHIRE_MAIL rejected: sender and recipient are the same session. Use internal state instead.".to_owned());
    }
    if directive
        .cc
        .iter()
        .any(|addr| addr.eq_ignore_ascii_case("supervisor") || addr.eq_ignore_ascii_case("sup"))
        && normalize_message_type(&directive.message_type) != "escalation"
    {
        // Warning only, not rejection
    }
    None
}

// ─── Orchestrator helpers ────────────────────────────────────────────────────

/// Orchestrator stats (subset we need for mail)
pub struct MailStats {
    pub mails_routed: usize,
    pub lease_conflicts: usize,
}

/// Resolve a display name alias to a session ID.
pub fn resolve_alias(alias_map: &HashMap<String, uuid::Uuid>, name: &str) -> Option<uuid::Uuid> {
    // Direct match
    if let Some(id) = alias_map.get(name) {
        return Some(*id);
    }
    // Case-insensitive match
    let lower = name.to_lowercase();
    alias_map
        .iter()
        .find(|(k, _)| k.to_lowercase() == lower)
        .map(|(_, v)| *v)
}

/// Parse a mail ID string into a Uuid.
pub fn parse_mail_id(value: &str) -> Option<uuid::Uuid> {
    uuid::Uuid::parse_str(value.trim()).ok()
}

/// Result of mail handling that may require follow-up by the orchestrator.
pub struct MailHandlingResult {
    pub supervisor_notice: Option<(crate::adapter::SupervisorEventType, String)>,
}

// ─── Nudge types ─────────────────────────────────────────────────────────────

/// Nudge queue constants
const NORMAL_TTL_SECS: i64 = 30 * 60; // 30 min
const URGENT_TTL_SECS: i64 = 2 * 3600; // 2 hr

/// A queued nudge waiting for the agent to reach a natural turn boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedNudge {
    pub sender: String,
    pub message: String,
    pub priority: String,
    pub thread_id: Option<String>,
    pub timestamp: chrono::DateTime<Utc>,
    pub expires_at: chrono::DateTime<Utc>,
}

/// Create a nudge from a mail directive.
pub fn nudge_from_mail(directive: &MailDirective, sender_name: &str) -> QueuedNudge {
    let is_urgent = matches!(
        directive.priority.to_lowercase().as_str(),
        "urgent" | "critical"
    );
    let now = chrono::Utc::now();
    QueuedNudge {
        sender: sender_name.to_owned(),
        message: format!(
            "[{}] {} — {}",
            directive.message_type, directive.subject, directive.request
        ),
        priority: if is_urgent {
            "urgent".to_owned()
        } else {
            "normal".to_owned()
        },
        thread_id: directive.thread_id.clone(),
        timestamp: now,
        expires_at: now
            + chrono::Duration::seconds(if is_urgent {
                URGENT_TTL_SECS
            } else {
                NORMAL_TTL_SECS
            }),
    }
}
