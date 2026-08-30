use std::collections::HashMap;

use blake3::hash;
use uuid::Uuid;

use crate::model::SessionRole;
use crate::protocol::MailDirective;

use super::{ActiveSession, PendingMail, live_state};

pub const MAX_CC_RECIPIENTS: usize = 3;
pub const MAX_ACTIVE_THREADS_PER_WORKER: usize = 8;
pub const MAX_OPEN_THREADS_PER_PAIR: usize = 2;

#[derive(Debug, Clone)]
pub struct MailGovernance {
    pub intent: String,
    pub duplicate_key: String,
    pub thread_state: String,
    pub sender_pod: String,
    pub recipient_pod: String,
    pub routing_class: String,
    pub block_reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PodSummary {
    pub name: String,
    pub members: Vec<String>,
    pub blocked_members: Vec<String>,
    pub open_threads: usize,
}

pub fn session_role_type(session: &ActiveSession) -> &str {
    if session.record.role == SessionRole::Supervisor {
        "supervisor"
    } else {
        session
            .packet
            .as_ref()
            .map(|packet| packet.role_type.as_str())
            .unwrap_or("software-engineer")
    }
}

pub fn pod_for_role(role_type: &str) -> &'static str {
    match role_type {
        "software-engineer" | "debug-and-review-engineer" | "testing-and-automation-engineer" => {
            "build"
        }
        "architecture-engineer" | "security-engineer" | "compliance-engineer" => "platform",
        "designer-engineer" | "product-engineer" | "product-manager" => "product",
        "research-engineer" | "validation-engineer" => "research",
        "sales-engineer"
        | "solutions-engineer"
        | "customer-success-engineer"
        | "revenue-engineer" => "revenue",
        "supervisor" => "executive",
        _ => "general",
    }
}

pub fn normalize_mail_intent(message_type: &str, expected_action: &str) -> &'static str {
    let lowered = message_type.trim().to_ascii_lowercase();
    let expected = expected_action.trim().to_ascii_lowercase();
    if lowered.contains("review") || expected.contains("review") {
        "review"
    } else if lowered.contains("handoff") || expected.contains("handoff") {
        "handoff"
    } else if lowered.contains("decision") || expected.contains("decide") {
        "decision_request"
    } else if lowered.contains("status") || expected.contains("status") {
        "status_request"
    } else if lowered.contains("proof")
        || expected.contains("proof")
        || expected.contains("evidence")
    {
        "proof_request"
    } else if lowered.contains("blocker")
        || lowered.contains("escalation")
        || expected.contains("block")
    {
        "blocker"
    } else if lowered.contains("reply") {
        "reply"
    } else {
        "dependency"
    }
}

pub fn routing_class(sender_role: &str, recipient_role: &str, intent: &str) -> &'static str {
    let sender_pod = pod_for_role(sender_role);
    let recipient_pod = pod_for_role(recipient_role);
    if recipient_role == "supervisor" {
        "executive"
    } else if sender_role == recipient_role {
        "peer"
    } else if sender_pod == recipient_pod {
        "same_pod"
    } else if matches!(intent, "review" | "proof_request" | "decision_request") {
        "cross_pod_specialist"
    } else {
        "cross_pod"
    }
}

pub fn duplicate_thread_key(
    sender_name: &str,
    recipient_name: &str,
    subject: &str,
    intent: &str,
) -> String {
    let normalized = format!(
        "{}:{}:{}:{}",
        sender_name.to_ascii_lowercase(),
        recipient_name.to_ascii_lowercase(),
        subject.trim().to_ascii_lowercase(),
        intent,
    );
    hash(normalized.as_bytes()).to_hex().to_string()
}

pub fn govern_mail(
    sender: &ActiveSession,
    recipient: &ActiveSession,
    directive: &MailDirective,
    pending_mail: &HashMap<Uuid, PendingMail>,
) -> MailGovernance {
    let sender_role = session_role_type(sender);
    let recipient_role = session_role_type(recipient);
    let sender_pod = pod_for_role(sender_role).to_owned();
    let recipient_pod = pod_for_role(recipient_role).to_owned();
    let intent =
        normalize_mail_intent(&directive.message_type, &directive.expected_action).to_owned();
    let routing_class = routing_class(sender_role, recipient_role, &intent).to_owned();
    let duplicate_key = duplicate_thread_key(
        &sender.record.name,
        &recipient.record.name,
        &directive.subject,
        &intent,
    );

    let sender_open_threads = pending_mail
        .values()
        .filter(|pending| {
            !pending.acked
                && pending.sender_session_id == sender.record.id
                && pending.thread_state != "closed"
        })
        .count();
    let pair_open_threads = pending_mail
        .values()
        .filter(|pending| {
            !pending.acked
                && pending.sender_session_id == sender.record.id
                && pending.recipient_session_id == recipient.record.id
                && pending.duplicate_key == duplicate_key
                && pending.thread_state != "closed"
        })
        .count();

    let block_reason = if directive.cc.len() > MAX_CC_RECIPIENTS {
        Some(format!(
            "Too many CC recipients ({}). Keep coordination tight and direct.",
            directive.cc.len()
        ))
    } else if sender_open_threads >= MAX_ACTIVE_THREADS_PER_WORKER
        && !matches!(
            directive.priority.to_ascii_lowercase().as_str(),
            "urgent" | "critical"
        )
    {
        Some(format!(
            "{} already has {} open coordination threads. Close or reroute before opening more.",
            sender.record.name, sender_open_threads
        ))
    } else if pair_open_threads >= MAX_OPEN_THREADS_PER_PAIR
        && !matches!(
            directive.priority.to_ascii_lowercase().as_str(),
            "urgent" | "critical"
        )
    {
        Some(format!(
            "{} already has {} unresolved thread(s) with {} on this same ask. Do not spam. Wait, narrow the ask, or escalate with concrete blocker context.",
            sender.record.name, pair_open_threads, recipient.record.name
        ))
    } else {
        None
    };

    MailGovernance {
        intent,
        duplicate_key,
        thread_state: "open".to_owned(),
        sender_pod,
        recipient_pod,
        routing_class,
        block_reason,
    }
}

pub fn thread_state_for_ack(status: &str) -> &'static str {
    match status.trim().to_ascii_lowercase().as_str() {
        "done" => "closed",
        "cannot_comply" => "rerouted",
        "acked" => "in_progress",
        _ => "answered",
    }
}

pub fn thread_state_for_timeout_stage(stage: u8) -> &'static str {
    match stage {
        0 => "open",
        1 => "waiting_ack",
        2 => "needs_reroute",
        _ => "coordination_failure",
    }
}

pub fn preferred_counterparts(role_type: &str) -> &'static [&'static str] {
    match role_type {
        "software-engineer" => &[
            "software-engineer",
            "architecture-engineer",
            "testing-and-automation-engineer",
            "designer-engineer",
            "validation-engineer",
        ],
        "designer-engineer" => &["product-engineer", "software-engineer", "product-manager"],
        "architecture-engineer" => &[
            "software-engineer",
            "security-engineer",
            "debug-and-review-engineer",
        ],
        "validation-engineer" => &["software-engineer", "testing-and-automation-engineer"],
        "research-engineer" => &["product-manager", "designer-engineer", "software-engineer"],
        "product-manager" => &["product-engineer", "designer-engineer", "revenue-engineer"],
        "revenue-engineer" => &["sales-engineer", "solutions-engineer", "product-manager"],
        _ => &[
            "software-engineer",
            "architecture-engineer",
            "validation-engineer",
        ],
    }
}

pub fn summarize_pods(
    active_sessions: &HashMap<Uuid, ActiveSession>,
    pending_mail: &HashMap<Uuid, PendingMail>,
) -> Vec<PodSummary> {
    let mut pods: HashMap<String, PodSummary> = HashMap::new();
    let now = std::time::Instant::now();
    let snapshot = live_state::Snapshot::build(active_sessions, now);

    for session in active_sessions.values() {
        if session.record.role != SessionRole::Worker {
            continue;
        }
        let pod_name = pod_for_role(session_role_type(session)).to_owned();
        let entry = pods.entry(pod_name.clone()).or_insert_with(|| PodSummary {
            name: pod_name,
            members: Vec::new(),
            blocked_members: Vec::new(),
            open_threads: 0,
        });
        entry.members.push(session.record.name.clone());
        if snapshot.counts_as_problem(session) {
            entry.blocked_members.push(session.record.name.clone());
        }
    }

    for pending in pending_mail.values() {
        if pending.thread_state == "closed" {
            continue;
        }
        if let Some(sender) = active_sessions.get(&pending.sender_session_id) {
            let pod_name = pod_for_role(session_role_type(sender)).to_owned();
            let entry = pods.entry(pod_name.clone()).or_insert_with(|| PodSummary {
                name: pod_name,
                members: Vec::new(),
                blocked_members: Vec::new(),
                open_threads: 0,
            });
            entry.open_threads += 1;
        }
    }

    let mut summaries = pods.into_values().collect::<Vec<_>>();
    summaries.sort_by(|left, right| left.name.cmp(&right.name));
    for summary in &mut summaries {
        summary.members.sort();
        summary.blocked_members.sort();
    }
    summaries
}
