use std::collections::HashMap;

use uuid::Uuid;

use super::super::{ActiveSession, PendingMail};

/// Build a single concise coordination digest for the supervisor.
///
/// This is meant to be high-signal:
/// - unacked mail threads
/// - who is waiting on whom
pub fn build_coordination_digest(
    active_sessions: &HashMap<Uuid, ActiveSession>,
    pending_mail: &HashMap<Uuid, PendingMail>,
) -> String {
    let mut lines = Vec::new();
    let mut unacked: Vec<&PendingMail> = pending_mail.values().filter(|m| !m.acked).collect();
    unacked.sort_by(|a, b| a.routed_at.cmp(&b.routed_at));

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
            "- thread {} [{}:{}] {} -> {}: {} ({})",
            pending.thread_id,
            pending.message_type,
            pending.priority,
            sender,
            recipient,
            pending.subject,
            pending.thread_state
        ));
    }

    if lines.is_empty() {
        "Coordination digest: no unacked threads.".to_owned()
    } else {
        format!("Coordination digest:\n{}", lines.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_digest_is_stable() {
        let active = HashMap::<Uuid, ActiveSession>::new();
        let pending = HashMap::<Uuid, PendingMail>::new();
        let digest = build_coordination_digest(&active, &pending);
        assert!(digest.to_ascii_lowercase().contains("no unacked"));
    }
}
