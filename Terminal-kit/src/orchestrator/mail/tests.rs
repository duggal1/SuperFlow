//! Unit tests for the mail subsystem.
//!
//! Verification harness for inter-agent mail exchange and escalation.
//! Covers: all 5 message types, validation, ack lifecycle, escalation paths,
//! lease conflicts, nudge queue, scavenge, rendering, and directive round-trips.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use uuid::Uuid;

use super::PendingMail;
use super::nudge_queue::*;
use super::render::*;
use super::timeouts::*;
use super::types::*;
use crate::protocol::{MailDirective, SapphireDirective, consume_directives};

// ─── Test fixtures ───────────────────────────────────────────────────────────

fn sample_pending(priority: &str) -> PendingMail {
    PendingMail {
        message_id: Uuid::new_v4(),
        thread_id: "thread-1".to_owned(),
        intent: "dependency".to_owned(),
        thread_state: "open".to_owned(),
        duplicate_key: "dup-1".to_owned(),
        sender_session_id: Uuid::new_v4(),
        recipient_session_id: Uuid::new_v4(),
        cc_session_ids: Vec::new(),
        sender_pod: "build".to_owned(),
        recipient_pod: "platform".to_owned(),
        routing_class: "cross_pod".to_owned(),
        subject: "Need dependency answer".to_owned(),
        message_type: "task".to_owned(),
        priority: priority.to_owned(),
        routed_at: Instant::now(),
        acked: false,
        timeout_stage: 0,
        last_timeout_at: None,
        reply_count: 0,
    }
}

fn make_mail_directive(
    to: &str,
    message_type: &str,
    priority: &str,
    subject: &str,
) -> MailDirective {
    MailDirective {
        to: to.to_owned(),
        message_type: message_type.to_owned(),
        priority: priority.to_owned(),
        subject: subject.to_owned(),
        mail_id: Some(Uuid::new_v4().to_string()),
        reply_to: None,
        thread_id: None,
        cc: Vec::new(),
        delivery_mode: String::new(),
        context: "test context".to_owned(),
        request: "test request".to_owned(),
        expected_action: "test action".to_owned(),
        requires_ack: false,
        delivery_state: None,
        pinned: false,
        suppress_notify: false,
    }
}

// ─── Timeout tests (existing) ────────────────────────────────────────────────

#[test]
fn timeout_interval_respects_priority() {
    assert_eq!(mail_timeout_interval("urgent"), Duration::from_secs(10));
    assert_eq!(mail_timeout_interval("high"), Duration::from_secs(15));
    assert_eq!(mail_timeout_interval("normal"), Duration::from_secs(20));
    assert_eq!(mail_timeout_interval("low"), Duration::from_secs(45));
}

#[test]
fn timeout_stage_due_uses_last_timeout_marker() {
    let mut pending = sample_pending("normal");
    pending.routed_at = Instant::now() - Duration::from_secs(25);
    assert!(mail_timeout_stage_due(&pending, Instant::now()));

    pending.last_timeout_at = Some(Instant::now());
    assert!(!mail_timeout_stage_due(&pending, Instant::now()));
}

#[test]
fn recipient_timeout_prompt_demands_explicit_ack_status() {
    let pending = sample_pending("high");
    let prompt = recipient_timeout_prompt(&pending, "Engineer-1", 1);
    assert!(prompt.contains("SAPPHIRE_ACK"));
    assert!(prompt.contains("cannot_comply"));
    assert!(prompt.contains("done"));
}

#[test]
fn sender_timeout_prompt_advises_independent_work() {
    let pending = sample_pending("normal");
    let prompt = sender_timeout_prompt(&pending, "Engineer-2", 1);
    assert!(prompt.contains("independent work"));
}

#[test]
fn cc_timeout_prompt_is_monitor_only() {
    let pending = sample_pending("high");
    let prompt = cc_timeout_prompt(&pending, 1);
    assert!(prompt.contains("CC NOTICE"));
    assert!(prompt.contains("Monitor only"));
}

// ─── Message type normalization (existing) ───────────────────────────────────

#[test]
fn normalize_message_type_passes_clean_types() {
    assert_eq!(normalize_message_type("task"), "task");
    assert_eq!(normalize_message_type("reply"), "reply");
    assert_eq!(normalize_message_type("notification"), "notification");
    assert_eq!(normalize_message_type("escalation"), "escalation");
    assert_eq!(normalize_message_type("scavenge"), "scavenge");
}

#[test]
fn normalize_message_type_maps_legacy_to_task() {
    assert_eq!(normalize_message_type("dependency_request"), "task");
    assert_eq!(normalize_message_type("dependency_response"), "task");
    assert_eq!(normalize_message_type("review_request"), "task");
    assert_eq!(normalize_message_type("review_response"), "task");
    assert_eq!(normalize_message_type("handoff"), "task");
    assert_eq!(normalize_message_type("collision_warning"), "task");
}

#[test]
fn normalize_message_type_maps_legacy_to_notification() {
    assert_eq!(normalize_message_type("completion_notice"), "notification");
}

#[test]
fn normalize_message_type_maps_legacy_to_escalation() {
    assert_eq!(normalize_message_type("blocker"), "escalation");
    assert_eq!(normalize_message_type("architecture_concern"), "escalation");
    assert_eq!(normalize_message_type("supervisor_directive"), "escalation");
}

#[test]
fn normalize_message_type_unknown_defaults_to_notification() {
    assert_eq!(normalize_message_type("random_gibberish"), "notification");
}

// ─── Delivery mode (existing) ────────────────────────────────────────────────

#[test]
fn derive_delivery_mode_respects_explicit() {
    assert_eq!(derive_delivery_mode("low", "interrupt"), "interrupt");
    assert_eq!(derive_delivery_mode("low", "queue"), "queue");
    assert_eq!(derive_delivery_mode("urgent", ""), "interrupt");
}

#[test]
fn derive_delivery_mode_derives_from_priority() {
    assert_eq!(derive_delivery_mode("urgent", ""), "interrupt");
    assert_eq!(derive_delivery_mode("critical", ""), "interrupt");
    assert_eq!(derive_delivery_mode("high", ""), "queue");
    assert_eq!(derive_delivery_mode("normal", ""), "queue");
    assert_eq!(derive_delivery_mode("low", ""), "queue");
}

// ─── Ack requirements (existing) ─────────────────────────────────────────────

#[test]
fn requires_ack_respects_explicit_flag() {
    assert!(requires_ack("reply", "low", true));
}

#[test]
fn requires_ack_for_task_escalation_scavenge() {
    assert!(requires_ack("task", "low", false));
    assert!(requires_ack("escalation", "low", false));
    assert!(requires_ack("scavenge", "low", false));
}

#[test]
fn requires_ack_for_urgent_notification() {
    assert!(requires_ack("notification", "urgent", false));
    assert!(requires_ack("notification", "high", false));
    assert!(!requires_ack("notification", "normal", false));
    assert!(!requires_ack("notification", "low", false));
}

#[test]
fn requires_ack_false_for_reply() {
    assert!(!requires_ack("reply", "urgent", false));
    assert!(!requires_ack("reply", "normal", false));
}

// ─── Mail ID parsing (existing) ──────────────────────────────────────────────

#[test]
fn parse_mail_id_valid_uuid() {
    let id = parse_mail_id("00000000-0000-0000-0000-000000000000");
    assert!(id.is_some());
}

#[test]
fn parse_mail_id_invalid_uuid() {
    let id = parse_mail_id("not-a-uuid");
    assert!(id.is_none());
}

#[test]
fn parse_mail_id_trims_whitespace() {
    let id = parse_mail_id("  00000000-0000-0000-0000-000000000000  ");
    assert!(id.is_some());
}

// ─── Alias resolution (existing) ─────────────────────────────────────────────

#[test]
fn resolve_alias_direct_match() {
    let mut map = HashMap::new();
    let id = Uuid::new_v4();
    map.insert("Engineer-1".to_owned(), id);
    assert_eq!(resolve_alias(&map, "Engineer-1"), Some(id));
}

#[test]
fn resolve_alias_case_insensitive() {
    let mut map = HashMap::new();
    let id = Uuid::new_v4();
    map.insert("Engineer-1".to_owned(), id);
    assert_eq!(resolve_alias(&map, "engineer-1"), Some(id));
}

#[test]
fn resolve_alias_not_found() {
    let map: HashMap<String, Uuid> = HashMap::new();
    assert_eq!(resolve_alias(&map, "Nobody"), None);
}

// ═════════════════════════════════════════════════════════════════════════════
// NEW: Mail validation tests
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn validate_mail_rejects_empty_subject() {
    let directive = make_mail_directive("Engineer-2", "task", "normal", "");
    let sender = Uuid::new_v4();
    let recipient = Uuid::new_v4();
    let err = validate_mail(&directive, sender, recipient);
    assert!(err.is_some());
    assert!(err.unwrap().contains("subject is empty"));
}

#[test]
fn validate_mail_rejects_oversized_subject() {
    let long_subject = "x".repeat(121);
    let directive = make_mail_directive("Engineer-2", "task", "normal", &long_subject);
    let sender = Uuid::new_v4();
    let recipient = Uuid::new_v4();
    let err = validate_mail(&directive, sender, recipient);
    assert!(err.is_some());
    assert!(err.unwrap().contains("subject too long"));
}

#[test]
fn validate_mail_accepts_subject_at_max_length() {
    let max_subject = "x".repeat(120);
    let directive = make_mail_directive("Engineer-2", "task", "normal", &max_subject);
    let sender = Uuid::new_v4();
    let recipient = Uuid::new_v4();
    let err = validate_mail(&directive, sender, recipient);
    assert!(err.is_none());
}

#[test]
fn validate_mail_rejects_oversized_body() {
    let big_body = "x".repeat(8193);
    let mut directive = make_mail_directive("Engineer-2", "task", "normal", "ok");
    directive.context = big_body.clone();
    let sender = Uuid::new_v4();
    let recipient = Uuid::new_v4();
    let err = validate_mail(&directive, sender, recipient);
    assert!(err.is_some());
    assert!(err.unwrap().contains("body too long"));
}

#[test]
fn validate_mail_accepts_body_at_max_size() {
    // Body = context + request + expected_action, so split the 8192 across them
    let context = "x".repeat(4000);
    let request = "y".repeat(4000);
    let mut directive = make_mail_directive("Engineer-2", "task", "normal", "ok");
    directive.context = context;
    directive.request = request;
    directive.expected_action = "e".to_owned(); // 1 char
    // Total: 4000 + 4000 + 1 = 8001 < 8192
    let sender = Uuid::new_v4();
    let recipient = Uuid::new_v4();
    let err = validate_mail(&directive, sender, recipient);
    assert!(err.is_none());
}

#[test]
fn validate_mail_allows_self_mail_for_task_and_notification() {
    let session = Uuid::new_v4();
    for msg_type in &["task", "notification"] {
        let directive = make_mail_directive("self", msg_type, "normal", "self-mail");
        let err = validate_mail(&directive, session, session);
        assert!(
            err.is_none(),
            "self-mail should be allowed for {}",
            msg_type
        );
    }
}

#[test]
fn validate_mail_rejects_self_mail_for_reply() {
    let session = Uuid::new_v4();
    let directive = make_mail_directive("self", "reply", "normal", "self-reply");
    let err = validate_mail(&directive, session, session);
    assert!(err.is_some());
    assert!(err.unwrap().contains("same session"));
}

#[test]
fn validate_mail_rejects_self_mail_for_escalation() {
    let session = Uuid::new_v4();
    let directive = make_mail_directive("self", "escalation", "normal", "self-escalation");
    let err = validate_mail(&directive, session, session);
    assert!(err.is_some());
}

#[test]
fn validate_mail_rejects_self_mail_for_scavenge() {
    let session = Uuid::new_v4();
    let directive = make_mail_directive("self", "scavenge", "normal", "self-scavenge");
    let err = validate_mail(&directive, session, session);
    assert!(err.is_some());
}

// ═════════════════════════════════════════════════════════════════════════════
// NEW: Nudge queue tests
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn nudge_from_mail_creates_urgent_nudge() {
    let mut directive = make_mail_directive("Engineer-2", "task", "urgent", "urgent task");
    directive.request = "do this now".to_owned();
    let nudge = nudge_from_mail(&directive, "Validator-1");
    assert_eq!(nudge.sender, "Validator-1");
    assert_eq!(nudge.priority, "urgent");
    assert!(nudge.message.contains("urgent task"));
    assert!(nudge.message.contains("do this now"));
}

#[test]
fn nudge_from_mail_creates_normal_nudge() {
    let directive = make_mail_directive("Engineer-2", "notification", "normal", "FYI");
    let nudge = nudge_from_mail(&directive, "Engineer-1");
    assert_eq!(nudge.sender, "Engineer-1");
    assert_eq!(nudge.priority, "normal");
}

#[test]
fn nudge_from_mail_sets_correct_ttl() {
    use chrono::Duration;
    let mut urgent = make_mail_directive("E2", "task", "critical", "urgent");
    urgent.mail_id = Some(Uuid::new_v4().to_string());
    let nudge_urgent = nudge_from_mail(&urgent, "V1");
    let ttl_urgent = nudge_urgent.expires_at - nudge_urgent.timestamp;
    assert_eq!(ttl_urgent, Duration::hours(2));

    let normal = make_mail_directive("E2", "task", "normal", "normal");
    let nudge_normal = nudge_from_mail(&normal, "V1");
    let ttl_normal = nudge_normal.expires_at - nudge_normal.timestamp;
    assert_eq!(ttl_normal, Duration::minutes(30));
}

#[test]
fn nudge_format_for_injection_includes_protocol_hint() {
    let nudge = QueuedNudge {
        sender: "Engineer-1".to_owned(),
        message: "[task] Need API contract — share response shape".to_owned(),
        priority: "normal".to_owned(),
        thread_id: Some("t-1".to_owned()),
        timestamp: chrono::Utc::now(),
        expires_at: chrono::Utc::now() + chrono::Duration::minutes(30),
    };
    let formatted = nudge_format_for_injection(&[nudge]);
    assert!(formatted.contains("Engineer-1"));
    assert!(formatted.contains("Need API contract"));
}

// ═════════════════════════════════════════════════════════════════════════════
// NEW: Mail rendering tests
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn render_mail_for_delivery_includes_all_fields() {
    let directive = MailDirective {
        mail_id: Some("mail-123".to_owned()),
        reply_to: None,
        thread_id: Some("thread-42".to_owned()),
        to: "Engineer-2".to_owned(),
        cc: vec!["Supervisor".to_owned()],
        message_type: "task".to_owned(),
        priority: "high".to_owned(),
        delivery_mode: "queue".to_owned(),
        subject: "Confirm API contract".to_owned(),
        context: "Need the response shape for the user endpoint".to_owned(),
        request: "Share the field names and types".to_owned(),
        expected_action: "Reply with the contract".to_owned(),
        requires_ack: true,
        delivery_state: None,
        pinned: false,
        suppress_notify: false,
    };
    let rendered = render_mail_for_delivery(
        Uuid::new_v4(),
        "thread-42",
        "Engineer-1",
        &directive,
        &[],
        true,
        false,
    );
    assert!(rendered.contains("Engineer-1"));
    assert!(rendered.contains("Engineer-2"));
    assert!(rendered.contains("Confirm API contract"));
    assert!(rendered.contains("HIGH"));
    assert!(rendered.contains("TASK"));
}

#[test]
fn render_cc_notice_includes_thread_and_subject() {
    let directive = make_mail_directive("Engineer-2", "task", "high", "Need dependency answer");
    let rendered = render_cc_notice("thread-1", "Engineer-1", "Validator-1", &directive);
    assert!(rendered.contains("CC NOTICE"));
    assert!(rendered.contains("thread-1"));
    assert!(rendered.contains("Need dependency answer"));
}

// ═════════════════════════════════════════════════════════════════════════════
// NEW: SAPPHIRE_MAIL directive parsing round-trip tests
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn parses_task_mail_directive() {
    let mut buffer = String::new();
    let chunk = concat!(
        "working on the API\n",
        "SAPPHIRE_MAIL {\"to\":\"Engineer-2\",\"message_type\":\"task\",\"priority\":\"high\",\"subject\":\"need contract\",\"context\":\"building endpoint\",\"request\":\"share fields\",\"expected_action\":\"reply\",\"requires_ack\":true}\n",
        "continuing work\n"
    );
    let directives = consume_directives(&mut buffer, chunk);
    assert_eq!(directives.len(), 1);
    let mail = match &directives[0] {
        SapphireDirective::Mail(m) => m,
        _ => panic!("expected mail directive"),
    };
    assert_eq!(mail.to, "Engineer-2");
    assert_eq!(mail.message_type, "task");
    assert_eq!(mail.priority, "high");
    assert!(mail.requires_ack);
}

#[test]
fn parses_escalation_mail_with_cc() {
    let mut buffer = String::new();
    let chunk = concat!(
        "blocked on design\n",
        "SAPPHIRE_MAIL {\"to\":\"Supervisor\",\"message_type\":\"escalation\",\"priority\":\"urgent\",\"subject\":\"design blocker\",\"context\":\"waiting on API spec\",\"request\":\"unblock or reroute\",\"expected_action\":\"decide path\",\"requires_ack\":true,\"cc\":[\"Engineer-1\"]}\n"
    );
    let directives = consume_directives(&mut buffer, chunk);
    assert_eq!(directives.len(), 1);
    let mail = match &directives[0] {
        SapphireDirective::Mail(m) => m,
        _ => panic!("expected mail directive"),
    };
    assert_eq!(mail.message_type, "escalation");
    assert_eq!(mail.priority, "urgent");
    assert_eq!(mail.to, "Supervisor");
    assert_eq!(mail.cc, vec!["Engineer-1"]);
}

#[test]
fn parses_scavenge_mail_directive() {
    let mut buffer = String::new();
    let chunk = concat!(
        "idle between tasks\n",
        "SAPPHIRE_MAIL {\"to\":\"team\",\"message_type\":\"scavenge\",\"priority\":\"normal\",\"subject\":\"pick up auth tests\",\"context\":\"auth module needs tests\",\"request\":\"claim if available\",\"expected_action\":\"claim\",\"requires_ack\":true}\n"
    );
    let directives = consume_directives(&mut buffer, chunk);
    assert_eq!(directives.len(), 1);
    let mail = match &directives[0] {
        SapphireDirective::Mail(m) => m,
        _ => panic!("expected mail directive"),
    };
    assert_eq!(mail.message_type, "scavenge");
    assert!(mail.requires_ack);
}

#[test]
fn parses_reply_mail_with_threading() {
    let mut buffer = String::new();
    let chunk = concat!(
        "answering the question\n",
        "SAPPHIRE_MAIL {\"to\":\"Engineer-1\",\"message_type\":\"reply\",\"priority\":\"normal\",\"subject\":\"re: need contract\",\"context\":\"here are the fields\",\"request\":\"use these\",\"expected_action\":\"integrate\",\"reply_to\":\"mail-orig\",\"thread_id\":\"thread-42\"}\n"
    );
    let directives = consume_directives(&mut buffer, chunk);
    assert_eq!(directives.len(), 1);
    let mail = match &directives[0] {
        SapphireDirective::Mail(m) => m,
        _ => panic!("expected mail directive"),
    };
    assert_eq!(mail.message_type, "reply");
    assert_eq!(mail.reply_to, Some("mail-orig".to_owned()));
    assert_eq!(mail.thread_id, Some("thread-42".to_owned()));
}

#[test]
fn parses_notification_mail_without_ack() {
    let mut buffer = String::new();
    let chunk = concat!(
        "FYI\n",
        "SAPPHIRE_MAIL {\"to\":\"Engineer-3\",\"message_type\":\"notification\",\"priority\":\"low\",\"subject\":\"deployed v2\",\"context\":\"auth endpoint live\",\"request\":\"verify on your end\",\"expected_action\":\"none\"}\n"
    );
    let directives = consume_directives(&mut buffer, chunk);
    assert_eq!(directives.len(), 1);
    let mail = match &directives[0] {
        SapphireDirective::Mail(m) => m,
        _ => panic!("expected mail directive"),
    };
    assert_eq!(mail.message_type, "notification");
    assert!(!mail.requires_ack);
}

// ═════════════════════════════════════════════════════════════════════════════
// NEW: SAPPHIRE_ACK directive parsing and lifecycle tests
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn parses_ack_directive_acked() {
    let mut buffer = String::new();
    let chunk =
        "SAPPHIRE_ACK {\"mail_id\":\"mail-123\",\"status\":\"acked\",\"summary\":\"on it\"}\n";
    let directives = consume_directives(&mut buffer, chunk);
    assert_eq!(directives.len(), 1);
    let ack = match &directives[0] {
        SapphireDirective::Ack(a) => a,
        _ => panic!("expected ack directive"),
    };
    assert_eq!(ack.mail_id, "mail-123");
    assert_eq!(ack.status, "acked");
    assert_eq!(ack.summary, "on it");
}

#[test]
fn parses_ack_directive_done() {
    let mut buffer = String::new();
    let chunk = "SAPPHIRE_ACK {\"mail_id\":\"mail-456\",\"status\":\"done\",\"summary\":\"already handled\"}\n";
    let directives = consume_directives(&mut buffer, chunk);
    assert_eq!(directives.len(), 1);
    let ack = match &directives[0] {
        SapphireDirective::Ack(a) => a,
        _ => panic!("expected ack directive"),
    };
    assert_eq!(ack.status, "done");
}

#[test]
fn parses_ack_directive_cannot_comply() {
    let mut buffer = String::new();
    let chunk = "SAPPHIRE_ACK {\"mail_id\":\"mail-789\",\"status\":\"cannot_comply\",\"summary\":\"missing auth token\"}\n";
    let directives = consume_directives(&mut buffer, chunk);
    assert_eq!(directives.len(), 1);
    let ack = match &directives[0] {
        SapphireDirective::Ack(a) => a,
        _ => panic!("expected ack directive"),
    };
    assert_eq!(ack.status, "cannot_comply");
    assert_eq!(ack.summary, "missing auth token");
}

#[test]
fn ack_looks_like_prompt_example_rejects_placeholders() {
    use crate::protocol::{AckDirective, ack_looks_like_prompt_example};
    let placeholder = AckDirective {
        mail_id: "...".to_owned(),
        status: "acked|done|cannot_comply".to_owned(),
        summary: "...".to_owned(),
    };
    assert!(ack_looks_like_prompt_example(&placeholder));
}

#[test]
fn ack_looks_like_prompt_example_accepts_real_values() {
    use crate::protocol::{AckDirective, ack_looks_like_prompt_example};
    let real = AckDirective {
        mail_id: Uuid::new_v4().to_string(),
        status: "acked".to_owned(),
        summary: "will handle this".to_owned(),
    };
    assert!(!ack_looks_like_prompt_example(&real));
}

// ═════════════════════════════════════════════════════════════════════════════
// NEW: SAPPHIRE_LEASE directive parsing and conflict detection tests
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn parses_lease_claim_directive() {
    let mut buffer = String::new();
    let chunk = concat!(
        "starting work\n",
        "SAPPHIRE_LEASE {\"paths\":[\"src/protocol.rs\",\"src/model.rs\"],\"intent\":\"edit\",\"status\":\"claim\"}\n"
    );
    let directives = consume_directives(&mut buffer, chunk);
    assert_eq!(directives.len(), 1);
    let lease = match &directives[0] {
        SapphireDirective::Lease(l) => l,
        _ => panic!("expected lease directive"),
    };
    assert_eq!(lease.paths, vec!["src/protocol.rs", "src/model.rs"]);
    assert_eq!(lease.intent, "edit");
    assert_eq!(lease.status, "claim");
}

#[test]
fn parses_lease_release_directive() {
    let mut buffer = String::new();
    let chunk = "SAPPHIRE_LEASE {\"paths\":[\"src/protocol.rs\"],\"intent\":\"edit\",\"status\":\"release\"}\n";
    let directives = consume_directives(&mut buffer, chunk);
    assert_eq!(directives.len(), 1);
    let lease = match &directives[0] {
        SapphireDirective::Lease(l) => l,
        _ => panic!("expected lease directive"),
    };
    assert_eq!(lease.status, "release");
}

#[test]
fn parses_lease_with_read_intent() {
    let mut buffer = String::new();
    let chunk = "SAPPHIRE_LEASE {\"paths\":[\"docs/architecture.md\"],\"intent\":\"read\",\"status\":\"claim\"}\n";
    let directives = consume_directives(&mut buffer, chunk);
    assert_eq!(directives.len(), 1);
    let lease = match &directives[0] {
        SapphireDirective::Lease(l) => l,
        _ => panic!("expected lease directive"),
    };
    assert_eq!(lease.intent, "read");
}

#[test]
fn lease_looks_like_prompt_example_rejects_placeholders() {
    use crate::protocol::{LeaseDirective, lease_looks_like_prompt_example};
    let placeholder = LeaseDirective {
        paths: vec!["src/path.rs".to_owned()],
        intent: "read|edit|review".to_owned(),
        status: "claim|release".to_owned(),
    };
    assert!(lease_looks_like_prompt_example(&placeholder));
}

#[test]
fn lease_looks_like_prompt_example_accepts_real_values() {
    use crate::protocol::{LeaseDirective, lease_looks_like_prompt_example};
    let real = LeaseDirective {
        paths: vec!["src/protocol.rs".to_owned()],
        intent: "edit".to_owned(),
        status: "claim".to_owned(),
    };
    assert!(!lease_looks_like_prompt_example(&real));
}

// ═════════════════════════════════════════════════════════════════════════════
// NEW: Full mail exchange integration tests (simulated agent interaction)
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn full_mail_exchange_task_to_ack() {
    // Simulate: Engineer-1 sends task mail → Engineer-2 acknowledges
    let mut buffer = String::new();

    // Step 1: Engineer-1 sends task mail
    let task_chunk = concat!(
        "SAPPHIRE_MAIL {\"mail_id\":\"00000000-0000-0000-0000-000000000001\",\"to\":\"Engineer-2\",\"message_type\":\"task\",\"priority\":\"high\",\"subject\":\"confirm endpoint\",\"context\":\"building user API\",\"request\":\"share response shape\",\"expected_action\":\"reply with fields\",\"requires_ack\":true}\n"
    );
    let directives = consume_directives(&mut buffer, task_chunk);
    assert_eq!(directives.len(), 1);
    let task_mail = match &directives[0] {
        SapphireDirective::Mail(m) => m,
        _ => panic!("expected task mail"),
    };
    assert!(requires_ack(
        &task_mail.message_type,
        &task_mail.priority,
        task_mail.requires_ack
    ));

    // Step 2: Engineer-2 acknowledges
    buffer.clear();
    let ack_chunk = "SAPPHIRE_ACK {\"mail_id\":\"00000000-0000-0000-0000-000000000001\",\"status\":\"acked\",\"summary\":\"will reply shortly\"}\n";
    let directives = consume_directives(&mut buffer, ack_chunk);
    assert_eq!(directives.len(), 1);
    let ack = match &directives[0] {
        SapphireDirective::Ack(a) => a,
        _ => panic!("expected ack"),
    };
    assert_eq!(ack.status, "acked");
}

#[test]
fn full_mail_exchange_escalation_with_cc() {
    // Simulate: Engineer-1 escalates blocker to Supervisor, CC Engineer-2
    let mut buffer = String::new();
    let chunk = concat!(
        "SAPPHIRE_MAIL {\"mail_id\":\"00000000-0000-0000-0000-000000000002\",\"to\":\"Supervisor\",\"message_type\":\"escalation\",\"priority\":\"urgent\",\"subject\":\"auth endpoint blocked\",\"context\":\"Engineer-2 changed the contract\",\"request\":\"decide whether to revert or adapt\",\"expected_action\":\"ruling on contract version\",\"requires_ack\":true,\"cc\":[\"Engineer-2\"]}\n"
    );
    let directives = consume_directives(&mut buffer, chunk);
    assert_eq!(directives.len(), 1);
    let mail = match &directives[0] {
        SapphireDirective::Mail(m) => m,
        _ => panic!("expected mail"),
    };
    assert_eq!(mail.message_type, "escalation");
    assert_eq!(mail.priority, "urgent");
    assert_eq!(mail.to, "Supervisor");
    assert_eq!(mail.cc, vec!["Engineer-2"]);
    assert!(mail.requires_ack);
    assert_eq!(normalize_message_type(&mail.message_type), "escalation");
    assert_eq!(
        derive_delivery_mode(&mail.priority, &mail.delivery_mode),
        "interrupt"
    );
}

#[test]
fn full_mail_exchange_scavenge_claim_workflow() {
    // Simulate: Engineer-1 posts scavenge → Engineer-2 claims it
    let mut buffer = String::new();

    // Step 1: Scavenge mail posted
    let scavenge_chunk = concat!(
        "SAPPHIRE_MAIL {\"mail_id\":\"00000000-0000-0000-0000-000000000003\",\"to\":\"team\",\"message_type\":\"scavenge\",\"priority\":\"normal\",\"subject\":\"tests for auth module\",\"context\":\"auth module has no tests\",\"request\":\"first to claim owns it\",\"expected_action\":\"claim via SAPPHIRE_LEASE\",\"requires_ack\":true}\n"
    );
    let directives = consume_directives(&mut buffer, scavenge_chunk);
    assert_eq!(directives.len(), 1);
    let scavenge = match &directives[0] {
        SapphireDirective::Mail(m) => m,
        _ => panic!("expected mail"),
    };
    assert_eq!(scavenge.message_type, "scavenge");
    assert_eq!(normalize_message_type(&scavenge.message_type), "scavenge");
    assert!(requires_ack(
        &scavenge.message_type,
        &scavenge.priority,
        scavenge.requires_ack
    ));
}

#[test]
fn full_mail_exchange_reply_threading() {
    // Simulate: Engineer-1 sends task → Engineer-2 replies in thread
    let mut buffer = String::new();

    // Original task
    let task_chunk = concat!(
        "SAPPHIRE_MAIL {\"mail_id\":\"00000000-0000-0000-0000-000000000004\",\"to\":\"Engineer-2\",\"message_type\":\"task\",\"priority\":\"normal\",\"subject\":\"API contract\",\"context\":\"need fields\",\"request\":\"share\",\"expected_action\":\"reply\",\"requires_ack\":false,\"thread_id\":\"api-thread\"}\n"
    );
    let directives = consume_directives(&mut buffer, task_chunk);
    assert_eq!(directives.len(), 1);

    // Reply
    buffer.clear();
    let reply_chunk = concat!(
        "SAPPHIRE_MAIL {\"mail_id\":\"00000000-0000-0000-0000-000000000005\",\"to\":\"Engineer-1\",\"message_type\":\"reply\",\"priority\":\"normal\",\"subject\":\"re: API contract\",\"context\":\"here are the fields\",\"request\":\"use these\",\"expected_action\":\"integrate\",\"requires_ack\":false,\"reply_to\":\"00000000-0000-0000-0000-000000000004\",\"thread_id\":\"api-thread\"}\n"
    );
    let directives = consume_directives(&mut buffer, reply_chunk);
    assert_eq!(directives.len(), 1);
    let reply = match &directives[0] {
        SapphireDirective::Mail(m) => m,
        _ => panic!("expected mail"),
    };
    assert_eq!(reply.message_type, "reply");
    assert_eq!(
        reply.reply_to,
        Some("00000000-0000-0000-0000-000000000004".to_owned())
    );
    assert_eq!(reply.thread_id, Some("api-thread".to_owned()));
    assert!(!requires_ack(
        &reply.message_type,
        &reply.priority,
        reply.requires_ack
    ));
}

#[test]
fn full_mail_exchange_notification_no_ack() {
    // Simulate: FYI notification — no ack required
    let mut buffer = String::new();
    let chunk = concat!(
        "SAPPHIRE_MAIL {\"to\":\"Engineer-3\",\"message_type\":\"notification\",\"priority\":\"low\",\"subject\":\"deploy complete\",\"context\":\"v2 live\",\"request\":\"verify when ready\",\"expected_action\":\"check endpoint\"}\n"
    );
    let directives = consume_directives(&mut buffer, chunk);
    assert_eq!(directives.len(), 1);
    let mail = match &directives[0] {
        SapphireDirective::Mail(m) => m,
        _ => panic!("expected mail"),
    };
    assert_eq!(mail.message_type, "notification");
    assert!(!requires_ack(
        &mail.message_type,
        &mail.priority,
        mail.requires_ack
    ));
}

// ═════════════════════════════════════════════════════════════════════════════
// NEW: Mail directive + status directive interleaving tests
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn interleaved_status_mail_ack_parse() {
    let mut buffer = String::new();
    let chunk = concat!(
        "working on auth\n",
        "SAPPHIRE_STATUS {\"state\":\"progressing\",\"summary\":\"building endpoint\",\"files\":[\"src/auth.rs\"],\"commands\":[],\"risks\":[]}\n",
        "need API shape\n",
        "SAPPHIRE_MAIL {\"to\":\"Engineer-2\",\"message_type\":\"task\",\"priority\":\"high\",\"subject\":\"need contract\",\"context\":\"building\",\"request\":\"share\",\"expected_action\":\"reply\",\"requires_ack\":true}\n",
        "waiting\n",
        "SAPPHIRE_ACK {\"mail_id\":\"00000000-0000-0000-0000-000000000001\",\"status\":\"acked\",\"summary\":\"replying\"}\n"
    );
    let directives = consume_directives(&mut buffer, chunk);
    assert_eq!(directives.len(), 3);
    assert!(matches!(directives[0], SapphireDirective::Status(_)));
    assert!(matches!(directives[1], SapphireDirective::Mail(_)));
    assert!(matches!(directives[2], SapphireDirective::Ack(_)));
}

// ═════════════════════════════════════════════════════════════════════════════
// NEW: Edge cases and boundary conditions
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn mail_with_escaped_json_in_request() {
    let mut buffer = String::new();
    let chunk = concat!(
        "SAPPHIRE_MAIL {",
        "\"to\":\"Engineer-2\",",
        "\"message_type\":\"task\",",
        "\"priority\":\"normal\",",
        "\"subject\":\"escape test\",",
        "\"context\":\"need {\\\"type\\\": \\\"string\\\"}\",",
        "\"request\":\"r\",",
        "\"expected_action\":\"e\"",
        "}\n"
    );
    let directives = consume_directives(&mut buffer, chunk);
    assert_eq!(directives.len(), 1);
    let mail = match &directives[0] {
        SapphireDirective::Mail(m) => m,
        _ => panic!("expected mail"),
    };
    assert!(mail.context.contains("{\"type\": \"string\"}"));
}

#[test]
fn mail_with_empty_cc_array() {
    let mut buffer = String::new();
    let chunk = "SAPPHIRE_MAIL {\"to\":\"Engineer-2\",\"message_type\":\"notification\",\"priority\":\"low\",\"subject\":\"FYI\",\"context\":\"info\",\"request\":\"check\",\"expected_action\":\"verify\",\"cc\":[]}\n";
    let directives = consume_directives(&mut buffer, chunk);
    assert_eq!(directives.len(), 1);
    let mail = match &directives[0] {
        SapphireDirective::Mail(m) => m,
        _ => panic!("expected mail"),
    };
    assert!(mail.cc.is_empty());
}

#[test]
fn mail_with_multiple_cc_recipients() {
    let mut buffer = String::new();
    let chunk = "SAPPHIRE_MAIL {\"to\":\"Engineer-2\",\"message_type\":\"escalation\",\"priority\":\"urgent\",\"subject\":\"blocker\",\"context\":\"blocked\",\"request\":\"help\",\"expected_action\":\"decide\",\"cc\":[\"Supervisor\",\"Engineer-3\",\"Engineer-4\"]}\n";
    let directives = consume_directives(&mut buffer, chunk);
    assert_eq!(directives.len(), 1);
    let mail = match &directives[0] {
        SapphireDirective::Mail(m) => m,
        _ => panic!("expected mail"),
    };
    assert_eq!(mail.cc.len(), 3);
    assert_eq!(mail.cc[0], "Supervisor");
}

// ═════════════════════════════════════════════════════════════════════════════
// NEW: Timeout escalation path tests
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn timeout_escalation_progresses_through_stages() {
    let mut pending = sample_pending("normal");
    pending.routed_at = Instant::now() - Duration::from_secs(60);

    // Stage 1: recipient timeout
    let recipient_prompt = recipient_timeout_prompt(&pending, "Engineer-1", 1);
    assert!(recipient_prompt.contains("SAPPHIRE_ACK"));

    // Stage 2: sender timeout
    let sender_prompt = sender_timeout_prompt(&pending, "Engineer-2", 2);
    assert!(sender_prompt.contains("still unanswered"));

    // Stage 3: final escalation
    let cc_prompt = cc_timeout_prompt(&pending, 3);
    assert!(cc_prompt.contains("coordination failure"));
}

#[test]
fn urgent_timeout_has_shorter_interval() {
    let mut urgent = sample_pending("urgent");
    urgent.routed_at = Instant::now() - Duration::from_secs(11);
    assert!(mail_timeout_stage_due(&urgent, Instant::now()));

    let mut normal = sample_pending("normal");
    normal.routed_at = Instant::now() - Duration::from_secs(11);
    assert!(!mail_timeout_stage_due(&normal, Instant::now()));
}

// ═════════════════════════════════════════════════════════════════════════════
// NEW: Protocol compliance summary
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn all_five_message_types_parse_correctly() {
    let message_types = vec![
        ("task", true),          // requires ack
        ("reply", false),        // no ack
        ("notification", false), // no ack for normal priority
        ("escalation", true),    // requires ack
        ("scavenge", true),      // requires ack
    ];

    for (msg_type, expects_ack) in message_types {
        let mut buffer = String::new();
        let chunk = format!(
            "SAPPHIRE_MAIL {{\"to\":\"Engineer-2\",\"message_type\":\"{}\",\"priority\":\"normal\",\"subject\":\"test\",\"context\":\"c\",\"request\":\"r\",\"expected_action\":\"e\"}}\n",
            msg_type
        );
        let directives = consume_directives(&mut buffer, &chunk);
        assert_eq!(directives.len(), 1, "failed to parse {}", msg_type);
        let mail = match &directives[0] {
            SapphireDirective::Mail(m) => m,
            _ => panic!("expected mail for {}", msg_type),
        };
        assert_eq!(normalize_message_type(&mail.message_type), msg_type);
        assert_eq!(
            requires_ack(&mail.message_type, &mail.priority, mail.requires_ack),
            expects_ack,
            "ack requirement mismatch for {}",
            msg_type
        );
    }
}

#[test]
fn legacy_message_types_normalize_correctly() {
    let legacy_mappings = vec![
        ("dependency_request", "task"),
        ("review_request", "task"),
        ("blocker", "escalation"),
        ("completion_notice", "notification"),
    ];

    for (legacy, expected) in legacy_mappings {
        assert_eq!(
            normalize_message_type(legacy),
            expected,
            "{} should normalize to {}",
            legacy,
            expected
        );
    }
}
