use std::collections::HashMap;

use uuid::Uuid;

use crate::orchestrator::{ActiveSession, PendingMail};

pub fn build_mail_digest(
    active_sessions: &HashMap<Uuid, ActiveSession>,
    pending_mail: &HashMap<Uuid, PendingMail>,
) -> String {
    let mut unacked: Vec<&PendingMail> = pending_mail.values().filter(|m| !m.acked).collect();
    unacked.sort_by(|a, b| a.routed_at.cmp(&b.routed_at));

    let mut lines = Vec::new();
    for pending in unacked.into_iter().take(6) {
        let sender = active_sessions
            .get(&pending.sender_session_id)
            .map(|s| s.record.name.as_str())
            .unwrap_or("unknown");
        let recipient = active_sessions
            .get(&pending.recipient_session_id)
            .map(|s| s.record.name.as_str())
            .unwrap_or("unknown");
        lines.push(format!(
            "- {} -> {} [{}:{}] {}",
            sender, recipient, pending.message_type, pending.priority, pending.subject
        ));
    }

    if lines.is_empty() {
        "Mail digest: none pending.".to_owned()
    } else {
        format!("Mail digest (unacked):\n{}", lines.join("\n"))
    }
}
