use crate::orchestrator::{ActiveSession, coordination};
use crate::protocol::MailDirective;

use super::types::normalize_message_type;

const MAX_CONTEXT_CHARS: usize = 1200;
const MAX_REQUEST_CHARS: usize = 320;
const MAX_EXPECTED_ACTION_CHARS: usize = 220;

pub fn validate_team_mail(
    sender: &ActiveSession,
    recipient: &ActiveSession,
    directive: &MailDirective,
) -> Option<String> {
    let normalized_type = normalize_message_type(&directive.message_type);
    let sender_role = coordination::session_role_type(sender);
    let recipient_role = coordination::session_role_type(recipient);
    let intent =
        coordination::normalize_mail_intent(&directive.message_type, &directive.expected_action);

    if directive.context.len() > MAX_CONTEXT_CHARS {
        return Some(format!(
            "SAPPHIRE_MAIL rejected: context too long ({} chars, max {}). Keep it concise and factual.",
            directive.context.len(),
            MAX_CONTEXT_CHARS,
        ));
    }
    if directive.request.len() > MAX_REQUEST_CHARS {
        return Some(format!(
            "SAPPHIRE_MAIL rejected: request too long ({} chars, max {}). Ask for one narrow action.",
            directive.request.len(),
            MAX_REQUEST_CHARS,
        ));
    }
    if directive.expected_action.len() > MAX_EXPECTED_ACTION_CHARS {
        return Some(format!(
            "SAPPHIRE_MAIL rejected: expected_action too long ({} chars, max {}). State one clear deliverable.",
            directive.expected_action.len(),
            MAX_EXPECTED_ACTION_CHARS,
        ));
    }

    if matches!(normalized_type, "task" | "escalation") {
        if directive.request.split_whitespace().count() < 3 {
            return Some(
                "SAPPHIRE_MAIL rejected: request is too vague. Ask for one concrete dependency, review, handoff, or ruling.".to_owned(),
            );
        }
        if directive.expected_action.split_whitespace().count() < 2 {
            return Some(
                "SAPPHIRE_MAIL rejected: expected_action is too vague. State the exact reply or artifact you need back.".to_owned(),
            );
        }
    }

    if recipient_role == "supervisor" && sender_role != "supervisor" {
        if normalized_type != "escalation"
            && matches!(
                intent,
                "dependency" | "review" | "handoff" | "proof_request" | "status_request"
            )
        {
            return Some(
                "Peer-first rule: route this through the responsible teammate first. Use the supervisor only for rulings, failed coordination, or contradictions.".to_owned(),
            );
        }

        if normalized_type == "escalation" && !has_failed_coordination_evidence(directive) {
            return Some(
                "Escalation rejected: include the failed teammate path, the concrete blocker, and the exact ruling you need from the supervisor.".to_owned(),
            );
        }
    }

    if recipient_role != "supervisor"
        && normalized_type == "notification"
        && directive.context.split_whitespace().count() < 4
        && directive.request.split_whitespace().count() < 3
    {
        return Some(
            "SAPPHIRE_MAIL rejected: notification is too vague. Either send a narrow actionable task/reply or keep working silently.".to_owned(),
        );
    }

    None
}

fn has_failed_coordination_evidence(directive: &MailDirective) -> bool {
    if directive.reply_to.is_some() || directive.thread_id.is_some() {
        return true;
    }

    let haystack = format!(
        "{} {} {}",
        directive.context.to_ascii_lowercase(),
        directive.request.to_ascii_lowercase(),
        directive.expected_action.to_ascii_lowercase(),
    );
    [
        "blocked",
        "cannot_comply",
        "no reply",
        "no response",
        "timeout",
        "conflict",
        "contradiction",
        "teammate",
        "worker",
        "failed coordination",
        "need ruling",
    ]
    .iter()
    .any(|needle| haystack.contains(needle))
}
