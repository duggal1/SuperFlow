//! Engineering-semantic rendering of mail for PTY delivery.
//! Each message type gets a distinct header format so agents instantly understand intent.

use uuid::Uuid;

use super::types::normalize_message_type;
use crate::protocol::MailDirective;

/// Render mail for PTY injection with engineering-team semantics.
/// Each type gets a distinct header format so agents instantly understand intent.
pub fn render_mail_for_delivery(
    message_id: Uuid,
    thread_id: &str,
    sender_name: &str,
    directive: &MailDirective,
    cc_ids: &[Uuid],
    requires_ack: bool,
    is_urgent: bool,
) -> String {
    let normalized = normalize_message_type(&directive.message_type);

    let header = match normalized {
        "task" => format!(
            "[SAPPHIRE TASK]\n\
             FROM: {sender}\nTO: {to}\nTHREAD: {thread}\nPRIORITY: {priority}\n\
             SUBJECT: {subject}\nCONTEXT: {context}\nASK: {request}\nDONE WHEN: {expected}",
            sender = sender_name,
            to = directive.to,
            thread = thread_id,
            priority = directive.priority.to_uppercase(),
            subject = directive.subject,
            context = directive.context,
            request = directive.request,
            expected = directive.expected_action,
        ),
        "escalation" => format!(
            "[SAPPHIRE ESCALATION]\n\
             FROM: {sender}\nTO: {to}\nTHREAD: {thread}\nPRIORITY: {priority}\n\
             SUBJECT: {subject}\nFAILED COORDINATION: {context}\nBLOCKER: {request}\nRULING NEEDED: {expected}",
            sender = sender_name,
            to = directive.to,
            thread = thread_id,
            priority = directive.priority.to_uppercase(),
            subject = directive.subject,
            context = directive.context,
            request = directive.request,
            expected = directive.expected_action,
        ),
        "scavenge" => format!(
            "[SAPPHIRE SCAVENGE]\n\
             FROM: {sender}\nTO: {to}\nTHREAD: {thread}\n\
             SUBJECT: {subject}\nCONTEXT: {context}\nAVAILABLE WORK: {request}\n\
             CLAIM ONLY IF YOU CAN OWN IT NOW.",
            sender = sender_name,
            to = directive.to,
            thread = thread_id,
            subject = directive.subject,
            context = directive.context,
            request = directive.request,
        ),
        "reply" => format!(
            "[SAPPHIRE REPLY]\n\
             FROM: {sender}\nTHREAD: {thread}\nSUBJECT: Re: {subject}\nANSWER: {body}",
            sender = sender_name,
            thread = thread_id,
            subject = directive.subject,
            body = if directive.request.is_empty() {
                &directive.context
            } else {
                &directive.request
            },
        ),
        _ => format!(
            "[SAPPHIRE NOTICE]\n\
             FROM: {sender}\nTO: {to}\nTHREAD: {thread}\nSUBJECT: {subject}\nBODY: {body}",
            sender = sender_name,
            to = directive.to,
            thread = thread_id,
            subject = directive.subject,
            body = if directive.request.is_empty() {
                &directive.context
            } else {
                &directive.request
            },
        ),
    };

    let cc_line = if cc_ids.is_empty() {
        String::new()
    } else {
        format!("\nCC: {} recipients (visibility only)\n", cc_ids.len())
    };

    let ack_instruction = if requires_ack {
        if is_urgent {
            format!(
                "\nACK NOW:\n\
                 SAPPHIRE_ACK {{\"mail_id\":\"{}\",\"status\":\"acked\",\"summary\":\"acknowledged\"}}\n\
                 Then do one of two things only: finish the ask, or reply with one blocker and keep moving on independent work.",
                message_id
            )
        } else {
            format!(
                "\nACK NOW:\n\
                 SAPPHIRE_ACK {{\"mail_id\":\"{}\",\"status\":\"acked\",\"summary\":\"one short sentence\"}}\n\
                 If you are partially blocked, send one narrow reply and continue independent work. Do not freeze.",
                message_id
            )
        }
    } else {
        "\nNo ack required. Reply only if your coordination state materially changes.".to_owned()
    };

    format!("{header}{cc_line}{ack_instruction}")
}

// ─── CC notice rendering ────────────────────────────────────────────────────

/// Render a CC visibility notice for non-primary recipients.
pub fn render_cc_notice(
    thread_id: &str,
    sender_name: &str,
    recipient_name: &str,
    directive: &MailDirective,
) -> String {
    format!(
        "[SAPPHIRE CC NOTICE]\n\
         THREAD: {thread}\nFROM: {from}\nTO: {to}\nSUBJECT: {subject}\nTYPE: {msg_type}\n\
         No action required. Monitor only.",
        thread = thread_id,
        from = sender_name,
        to = recipient_name,
        subject = directive.subject,
        msg_type = normalize_message_type(&directive.message_type),
    )
}
