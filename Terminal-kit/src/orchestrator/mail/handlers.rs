//! Orchestrator mail handlers: handle_mail_directive, handle_ack_directive, handle_lease_directive.
//! Also handles claim/release special message types and auto-archive.

use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

use anyhow::Result;
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

use super::contract::validate_team_mail;
use super::nudge_queue::nudge_enqueue;
use super::render::*;
use super::scavenge::{ClaimResult, attempt_scavenge_claim, release_scavenge};
use super::types::*;
use crate::model::MailRecord;
use crate::orchestrator::communication_policy;
use crate::orchestrator::coordination;
use crate::orchestrator::{ActiveSession, LeaseOwner};
use crate::protocol::{AckDirective, MailDirective};
use crate::store::Store;

// Re-export the orchestrator's PendingMail to avoid duplication
pub use super::PendingMail;

/// Handle claim/release mail directives — modify scavenge mail ownership.
fn handle_claim_or_release(
    store: &Store,
    mission_id: Uuid,
    sender_session_id: Uuid,
    directive: &MailDirective,
    active_sessions: &mut HashMap<Uuid, ActiveSession>,
    _alias_map: &HashMap<String, Uuid>,
) -> Result<MailHandlingResult> {
    // Parse mail_id from subject or body to find the scavenge mail to claim/release
    let scavenge_id = directive.mail_id.as_deref().and_then(parse_mail_id);

    let Some(mail_id) = scavenge_id else {
        if let Some(sender) = active_sessions.get(&sender_session_id) {
            let _ = sender.runtime.send_prompt(
                "SAPPHIRE_MAIL claim/release requires mail_id set to the scavenge message ID.",
            );
        }
        return Ok(MailHandlingResult {
            supervisor_notice: None,
        });
    };

    let sender_name = active_sessions
        .get(&sender_session_id)
        .map(|s| s.record.name.clone())
        .unwrap_or_else(|| "unknown".to_owned());

    let raw_type = directive.message_type.to_lowercase();
    if raw_type == "claim" {
        match attempt_scavenge_claim(store, mail_id, sender_session_id, &sender_name)? {
            ClaimResult::Claimed => {
                store.append_summary(
                    mission_id,
                    Some(sender_session_id),
                    "scavenge_claimed",
                    &format!("{} claimed scavenge: {}", sender_name, directive.subject),
                )?;
                if let Some(sender) = active_sessions.get(&sender_session_id) {
                    let _ = sender.runtime.send_prompt(&format!(
                        "Scavenge claimed: {}. You now own this work. Emit SAPPHIRE_STATUS with state=progressing when you start.",
                        directive.subject
                    ));
                }
            }
            ClaimResult::AlreadyClaimed {
                claimed_by,
                claimed_at,
            } => {
                if let Some(sender) = active_sessions.get(&sender_session_id) {
                    let _ = sender.runtime.send_prompt(&format!(
                        "Scavenge already claimed by {} at {}. Pick another task or wait.",
                        claimed_by, claimed_at
                    ));
                }
            }
            ClaimResult::NotFound => {
                if let Some(sender) = active_sessions.get(&sender_session_id) {
                    let _ = sender.runtime.send_prompt(
                        "Scavenge not found. Check the mail_id field points to a valid scavenge message.",
                    );
                }
            }
        }
    } else {
        // release
        let released = release_scavenge(store, mail_id, sender_session_id)?;
        if released {
            store.append_summary(
                mission_id,
                Some(sender_session_id),
                "scavenge_released",
                &format!("{} released scavenge: {}", sender_name, directive.subject),
            )?;
            if let Some(sender) = active_sessions.get(&sender_session_id) {
                let _ = sender.runtime.send_prompt(&format!(
                    "Scavenge released: {}. Back in the pool for others to claim.",
                    directive.subject
                ));
            }
        } else {
            if let Some(sender) = active_sessions.get(&sender_session_id) {
                let _ = sender.runtime.send_prompt(
                    "Release failed — you don't own that scavenge or it doesn't exist.",
                );
            }
        }
    }

    Ok(MailHandlingResult {
        supervisor_notice: None,
    })
}

/// Handle SAPPHIRE_MAIL directive — route, persist, inject, track ack.
pub fn handle_mail_directive(
    store: &Store,
    state_dir: &Path,
    mission_id: Uuid,
    supervisor_id: Uuid,
    sender_session_id: Uuid,
    directive: MailDirective,
    active_sessions: &mut HashMap<Uuid, ActiveSession>,
    alias_map: &HashMap<String, Uuid>,
    pending_mail: &mut HashMap<Uuid, PendingMail>,
    _stats: &mut MailStats,
) -> Result<MailHandlingResult> {
    use crate::adapter::SupervisorEventType;

    let Some(recipient_session_id) = resolve_alias(alias_map, &directive.to) else {
        if let Some(sender) = active_sessions.get(&sender_session_id) {
            let _ = sender.runtime.send_prompt(
                "Your SAPPHIRE_MAIL target did not resolve to a known session alias. Use the display name such as Engineer-2 or Supervisor.",
            );
        }
        return Ok(MailHandlingResult {
            supervisor_notice: None,
        });
    };

    // Claim/release handling — special message types that modify scavenge mail
    let raw_type = directive.message_type.to_lowercase();
    if raw_type == "claim" || raw_type == "release" {
        return handle_claim_or_release(
            store,
            mission_id,
            sender_session_id,
            &directive,
            active_sessions,
            alias_map,
        );
    }

    // Validation
    if let Some(error) = validate_mail(&directive, sender_session_id, recipient_session_id) {
        if let Some(sender) = active_sessions.get(&sender_session_id) {
            let _ = sender.runtime.send_prompt(&error);
        }
        return Ok(MailHandlingResult {
            supervisor_notice: None,
        });
    }

    // Normalize message type and delivery mode
    let normalized_type = normalize_message_type(&directive.message_type);
    let delivery_mode = derive_delivery_mode(&directive.priority, &directive.delivery_mode);
    let is_urgent = matches!(
        directive.priority.to_lowercase().as_str(),
        "urgent" | "critical"
    );
    let ack_required = requires_ack(normalized_type, &directive.priority, directive.requires_ack);

    // Escalation auto-CCs supervisor
    let mut cc_session_ids: Vec<Uuid> = directive
        .cc
        .iter()
        .filter_map(|addr| resolve_alias(alias_map, addr))
        .filter(|id| *id != sender_session_id && *id != recipient_session_id)
        .collect();

    if normalized_type == "escalation"
        && !cc_session_ids.contains(&supervisor_id)
        && sender_session_id != supervisor_id
    {
        cc_session_ids.push(supervisor_id);
    }

    // Thread tracking
    let message_id = directive
        .mail_id
        .as_deref()
        .and_then(parse_mail_id)
        .unwrap_or_else(Uuid::new_v4);

    let thread_id = directive
        .thread_id
        .clone()
        .or_else(|| {
            directive
                .reply_to
                .as_deref()
                .and_then(parse_mail_id)
                .and_then(|reply_to_id| pending_mail.get(&reply_to_id).map(|m| m.thread_id.clone()))
        })
        .unwrap_or_else(|| message_id.to_string());

    let sender_name = active_sessions
        .get(&sender_session_id)
        .map(|s| s.record.name.clone())
        .unwrap_or_else(|| "unknown".to_owned());
    let recipient_name = active_sessions
        .get(&recipient_session_id)
        .map(|s| s.record.name.clone())
        .unwrap_or_else(|| directive.to.clone());
    let Some(sender_session) = active_sessions.get(&sender_session_id) else {
        return Ok(MailHandlingResult {
            supervisor_notice: None,
        });
    };
    let Some(recipient_session) = active_sessions.get(&recipient_session_id) else {
        return Ok(MailHandlingResult {
            supervisor_notice: None,
        });
    };
    if let Some(error) = validate_team_mail(sender_session, recipient_session, &directive) {
        if let Some(sender) = active_sessions.get(&sender_session_id) {
            let _ = sender.runtime.send_prompt(&error);
        }
        return Ok(MailHandlingResult {
            supervisor_notice: None,
        });
    }
    let governance =
        coordination::govern_mail(sender_session, recipient_session, &directive, pending_mail);
    if let Some(reason) = governance.block_reason.as_deref() {
        if let Some(sender) = active_sessions.get(&sender_session_id) {
            let _ = sender.runtime.send_prompt(reason);
        }
        return Ok(MailHandlingResult {
            supervisor_notice: None,
        });
    }

    // Persist to SQLite
    let mail_record = MailRecord {
        id: message_id,
        mission_id,
        sender_worker_id: sender_session_id,
        recipient_worker_id: recipient_session_id,
        message_type: normalized_type.to_owned(),
        priority: directive.priority.clone(),
        delivery_mode: delivery_mode.to_owned(),
        subject: directive.subject.clone(),
        status: if ack_required {
            "awaiting_ack".to_owned()
        } else {
            "routed".to_owned()
        },
        ack_state: if ack_required {
            "pending".to_owned()
        } else {
            "not_required".to_owned()
        },
        pinned: directive.pinned,
        body_json: serde_json::to_string(&json!({
            "mail_id": message_id,
            "thread_id": thread_id,
            "reply_to": directive.reply_to,
            "to": directive.to,
            "cc": cc_session_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>(),
            "message_type": normalized_type,
            "intent": governance.intent.clone(),
            "thread_state": governance.thread_state.clone(),
            "duplicate_key": governance.duplicate_key.clone(),
            "sender_pod": governance.sender_pod.clone(),
            "recipient_pod": governance.recipient_pod.clone(),
            "routing_class": governance.routing_class.clone(),
            "priority": directive.priority,
            "delivery_mode": delivery_mode,
            "subject": directive.subject,
            "context": directive.context,
            "request": directive.request,
            "expected_action": directive.expected_action,
            "requires_ack": ack_required,
        }))?,
        thread_id: thread_id.clone(),
        reply_to: directive.reply_to.clone(),
        created_at: Utc::now(),
        archived_at: None,
    };
    store.persist_message(&mail_record)?;
    store.append_summary(
        mission_id,
        Some(sender_session_id),
        "mail_sent",
        format!(
            "to={} type={} priority={} mode={} subject={} thread={}",
            recipient_name,
            normalized_type,
            directive.priority,
            delivery_mode,
            directive.subject,
            thread_id
        ),
    )?;

    // Track pending mail for ack timeout — insert BEFORE delivery to ensure timeout invariant
    if ack_required {
        pending_mail.insert(
            message_id,
            PendingMail {
                message_id,
                thread_id: thread_id.clone(),
                intent: governance.intent.clone(),
                thread_state: governance.thread_state.clone(),
                duplicate_key: governance.duplicate_key.clone(),
                sender_session_id,
                recipient_session_id,
                cc_session_ids: cc_session_ids.clone(),
                sender_pod: governance.sender_pod.clone(),
                recipient_pod: governance.recipient_pod.clone(),
                routing_class: governance.routing_class.clone(),
                subject: directive.subject.clone(),
                message_type: normalized_type.to_owned(),
                priority: directive.priority.clone(),
                routed_at: Instant::now(),
                acked: false,
                timeout_stage: 0,
                last_timeout_at: None,
                reply_count: 0,
            },
        );
    }

    // Deliver to primary recipient — use nudge queue for non-destructive delivery
    if let Some(recipient) = active_sessions.get(&recipient_session_id) {
        let delivery_mode = derive_delivery_mode(&directive.priority, &directive.delivery_mode);
        if delivery_mode == "interrupt" {
            // Direct PTY injection — only for urgent/critical priority
            let mail_prompt = render_mail_for_delivery(
                message_id,
                &thread_id,
                &sender_name,
                &directive,
                &cc_session_ids,
                ack_required,
                is_urgent,
            );
            let _ = recipient.runtime.send_prompt(&mail_prompt);
        } else {
            // Queue-based delivery — write to filesystem, drain at next turn boundary
            let nudge = nudge_from_mail(&directive, &sender_name);
            if let Err(e) = nudge_enqueue(state_dir, &recipient.record.id.to_string(), nudge) {
                if communication_policy::MAIL_QUEUE_DIRECT_FALLBACK {
                    tracing::error!("nudge enqueue failed, falling back to direct injection: {e}");
                    let mail_prompt = render_mail_for_delivery(
                        message_id,
                        &thread_id,
                        &sender_name,
                        &directive,
                        &cc_session_ids,
                        ack_required,
                        is_urgent,
                    );
                    let _ = recipient.runtime.send_prompt(&mail_prompt);
                } else {
                    tracing::error!(
                        "nudge enqueue failed; direct injection disabled by policy: {e}"
                    );
                }
            }
        }
    }

    // Notify CC recipients
    for cc_id in &cc_session_ids {
        if let Some(cc_session) = active_sessions.get(cc_id) {
            let cc_notice = render_cc_notice(&thread_id, &sender_name, &recipient_name, &directive);
            let now = chrono::Utc::now();
            let _ = nudge_enqueue(
                state_dir,
                &cc_session.record.id.to_string(),
                QueuedNudge {
                    sender: "Sapphire".to_owned(),
                    message: cc_notice,
                    priority: "normal".to_owned(),
                    thread_id: Some(thread_id.clone()),
                    timestamp: now,
                    expires_at: now + chrono::Duration::minutes(30),
                },
            );
        }
    }

    let _ = store.update_worker_summary(
        recipient_session_id,
        &format!(
            "mail inbox: {} [{}] ({})",
            directive.subject, normalized_type, directive.priority
        ),
    );

    // Reply chain: mark original as responded
    if let Some(reply_to) = directive.reply_to.as_deref().and_then(parse_mail_id) {
        if let Some(original) = pending_mail.get_mut(&reply_to) {
            original.acked = true;
            original.reply_count += 1;
            original.thread_state = "answered".to_owned();
            let _ = store.update_message_status(reply_to, "responded", "acked");
        }
    }

    // Supervisor notice for inter-worker mail
    let supervisor_notice = if sender_session_id != supervisor_id
        && recipient_session_id != supervisor_id
        && !directive.suppress_notify
    {
        let event_type = if is_urgent {
            SupervisorEventType::Blocked
        } else {
            SupervisorEventType::Notice
        };
        Some((
            event_type,
            format!(
                "{}Mail routed from {} to {} [{}]: {} (thread: {})",
                if is_urgent { "⚡URGENT⚡ " } else { "" },
                sender_name,
                recipient_name,
                normalized_type,
                directive.subject,
                thread_id
            ),
        ))
    } else {
        None
    };

    Ok(MailHandlingResult { supervisor_notice })
}

/// Handle SAPPHIRE_ACK directive — idempotent processing.
pub fn handle_ack_directive(
    store: &Store,
    mission_id: Uuid,
    supervisor_id: Uuid,
    sender_session_id: Uuid,
    directive: AckDirective,
    pending_mail: &mut HashMap<Uuid, PendingMail>,
    active_sessions: &mut HashMap<Uuid, ActiveSession>,
) -> Result<()> {
    use crate::adapter::{SupervisorEventType, adapter_for};

    let message_id = parse_mail_id(&directive.mail_id);
    let Some(mail_id) = message_id else {
        return Ok(());
    };

    let next_thread_state = coordination::thread_state_for_ack(&directive.status).to_owned();

    // Idempotent ack: ignore exact duplicates, but allow acked -> done/cannot_comply upgrades.
    if let Some(pending) = pending_mail.get_mut(&mail_id) {
        if pending.thread_state == "closed" {
            return Ok(());
        }
        if pending.acked && pending.thread_state == next_thread_state {
            return Ok(());
        }
        pending.acked = true;
        pending.thread_state = next_thread_state;
    }

    let ack_status = directive.status.trim().to_ascii_lowercase();
    let persisted_status = match ack_status.as_str() {
        "done" => "completed",
        "cannot_comply" => "cannot_comply",
        _ => "acked",
    };
    let _ = store.update_message_status(mail_id, persisted_status, &directive.summary);

    // Notify original sender that mail was acked
    if let Some(pending) = pending_mail.get(&mail_id) {
        if let Some(sender) = active_sessions.get(&pending.sender_session_id) {
            let sender_prompt = match ack_status.as_str() {
                "done" => format!(
                    "Thread {} resolved: {} reports the requested work is done for '{}'.",
                    pending.thread_id,
                    active_sessions
                        .get(&sender_session_id)
                        .map(|session| session.record.name.as_str())
                        .unwrap_or("teammate"),
                    pending.subject,
                ),
                "cannot_comply" => format!(
                    "Thread {} cannot proceed as requested: {} reported cannot_comply for '{}'. Summary: {}. Reroute the dependency, narrow the ask, or escalate with concrete context.",
                    pending.thread_id,
                    active_sessions
                        .get(&sender_session_id)
                        .map(|session| session.record.name.as_str())
                        .unwrap_or("teammate"),
                    pending.subject,
                    directive.summary,
                ),
                _ => format!(
                    "SAPPHIRE_ACK for mail {mail_id} (thread: {thread}): {status} — {summary}",
                    mail_id = mail_id,
                    thread = pending.thread_id,
                    status = directive.status,
                    summary = directive.summary,
                ),
            };
            let _ = sender.runtime.send_prompt(&sender_prompt);
        }
    }

    // Notify supervisor of ack
    let sender_name = active_sessions
        .get(&sender_session_id)
        .map(|s| s.record.name.as_str())
        .unwrap_or("unknown");

    if let Some(pending) = pending_mail.get(&mail_id) {
        store.append_json_event(
            mission_id,
            Some(sender_session_id),
            "mail_ack",
            &format!("ack from {sender_name} for mail {mail_id}"),
            &json!({
                "mail_id": mail_id,
                "thread_id": pending.thread_id,
                "ack_status": directive.status,
                "summary": directive.summary,
            }),
        )?;

        if ack_status == "cannot_comply" {
            let supervisor_notice = format!(
                "Mail thread {} hit cannot_comply. {} could not complete '{}' for {}. Summary: {}",
                pending.thread_id,
                active_sessions
                    .get(&sender_session_id)
                    .map(|session| session.record.name.as_str())
                    .unwrap_or("unknown"),
                pending.subject,
                active_sessions
                    .get(&pending.sender_session_id)
                    .map(|session| session.record.name.as_str())
                    .unwrap_or("unknown"),
                directive.summary,
            );
            if let Some(supervisor) = active_sessions.get(&supervisor_id) {
                let adapter = adapter_for(supervisor.record.agent);
                let prompt = adapter.build_supervisor_action_prompt(
                    SupervisorEventType::Blocked,
                    &supervisor_notice,
                );
                let _ = supervisor.runtime.send_prompt(&prompt);
            }
            store.append_summary(
                mission_id,
                Some(supervisor_id),
                "mail_cannot_comply",
                supervisor_notice,
            )?;
        }
    }

    Ok(())
}

/// Handle SAPPHIRE_LEASE directive — file ownership claims with conflict detection.
pub fn handle_lease_directive(
    store: &Store,
    mission_id: Uuid,
    supervisor_id: Uuid,
    session_id: Uuid,
    directive: crate::protocol::LeaseDirective,
    active_sessions: &mut HashMap<Uuid, ActiveSession>,
    leases: &mut HashMap<String, LeaseOwner>,
    stats: &mut MailStats,
) -> Result<()> {
    use crate::model::SessionState;

    for path in directive.paths {
        let normalized = path.trim().to_owned();
        let lease_record = crate::model::LeaseRecord {
            mission_id,
            path: normalized.clone(),
            owner_session_id: session_id,
            intent: directive.intent.clone(),
            status: directive.status.clone(),
            updated_at: Utc::now(),
        };
        store.upsert_lease(&lease_record)?;

        if directive.status.eq_ignore_ascii_case("release") {
            if leases
                .get(&normalized)
                .map(|o| o.session_id == session_id)
                .unwrap_or(false)
            {
                leases.remove(&normalized);
            }
            continue;
        }

        if let Some(existing) = leases.get(&normalized) {
            if existing.session_id != session_id {
                stats.lease_conflicts += 1;
                if let Some(session) = active_sessions.get_mut(&session_id) {
                    session.state = SessionState::Contradictory;
                    store.update_session_state(session_id, SessionState::Contradictory)?;
                }

                let owner_name = active_sessions
                    .get(&existing.session_id)
                    .map(|s| s.record.name.clone())
                    .unwrap_or_else(|| existing.session_id.to_string());
                let challenger_name = active_sessions
                    .get(&session_id)
                    .map(|s| s.record.name.clone())
                    .unwrap_or_else(|| session_id.to_string());

                store.append_json_event(
                    mission_id,
                    Some(session_id),
                    "lease_conflict",
                    &format!("conflict on {normalized}"),
                    &json!({
                        "path": normalized,
                        "owner_session_id": existing.session_id,
                        "challenger_session_id": session_id,
                        "owner_intent": existing.intent,
                        "challenger_intent": directive.intent,
                    }),
                )?;

                if let Some(challenger) = active_sessions.get(&session_id) {
                    let _ = challenger.runtime.send_prompt(&format!(
                        "Lease conflict detected on {path}. Another worker currently owns that path. Stop and wait for supervisor ruling. Emit SAPPHIRE_STATUS {{\"state\":\"blocked\",\"summary\":\"lease conflict on {path}\"}}",
                        path = normalized
                    ));
                }

                if let Some(owner) = active_sessions.get(&existing.session_id) {
                    let _ = owner.runtime.send_prompt(&format!(
                        "Another worker attempted to claim {path} while you own it. Emit a fresh SAPPHIRE_STATUS line if your scope or readiness changed.",
                        path = normalized
                    ));
                }

                // Supervisor notice
                store.append_json_event(
                    mission_id,
                    Some(supervisor_id),
                    "supervisor_notice",
                    &format!("Lease conflict on {normalized}"),
                    &json!({
                        "event": "lease_conflict_escalation",
                        "path": normalized,
                        "owner": owner_name,
                        "challenger": challenger_name,
                        "owner_intent": existing.intent,
                        "challenger_intent": directive.intent,
                    }),
                )?;

                continue;
            }
        }

        // No conflict — claim the lease
        leases.insert(
            normalized.clone(),
            LeaseOwner {
                session_id,
                intent: directive.intent.clone(),
            },
        );
    }
    Ok(())
}

/// Auto-archive resolved mail older than the given duration.
/// Returns count of archived messages.
#[allow(dead_code)]
pub fn auto_archive_resolved_mail(
    store: &Store,
    mission_id: Uuid,
    older_than_secs: u64,
) -> Result<usize> {
    store.archive_resolved_mail(mission_id, older_than_secs)
}
