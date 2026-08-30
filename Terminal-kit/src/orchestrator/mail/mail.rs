
// use std::collections::HashMap;
// use std::fs;
// use std::path::{Path, PathBuf};
// use std::time::{Duration, Instant};

// use anyhow::Result;
// use chrono::Utc;
// use serde::{Deserialize, Serialize};
// use serde_json::json;
// use uuid::Uuid;

// use crate::model::MailRecord;
// use crate::protocol::{AckDirective, MailDirective};
// use crate::store::Store;
// use super::coordination;

// // Re-export the orchestrator's PendingMail to avoid duplication
// pub use super::PendingMail;

// // ─── Message type normalization ──────────────────────────────────────────────

// /// Normalize legacy + new message types to the 5 clean engineering types.
// /// Gas Town pattern: task, reply, notification, escalation, scavenge.
// /// Legacy types map forward; unknown defaults to notification.
// pub fn normalize_message_type(raw: &str) -> &'static str {
//     match raw.trim().to_lowercase().as_str() {
//         // New clean types pass through
//         "task" => "task",
//         "reply" => "reply",
//         "notification" => "notification",
//         "escalation" => "escalation",
//         "scavenge" => "scavenge",
//         // Legacy → task (requires action from recipient)
//         "dependency_request" | "dependency_response"
//         | "review_request" | "review_response"
//         | "handoff" | "collision_warning" => "task",
//         // Legacy → notification (FYI, no action)
//         "completion_notice" => "notification",
//         // Legacy → escalation (blocker requiring supervisor attention)
//         "blocker" | "architecture_concern" | "supervisor_directive" => "escalation",
//         _ => "notification",
//     }
// }

// /// Derive delivery mode from priority if not explicitly set.
// pub fn derive_delivery_mode(priority: &str, explicit: &str) -> &'static str {
//     if !explicit.is_empty() {
//         return match explicit.to_lowercase().as_str() {
//             "interrupt" => "interrupt",
//             "queue" => "queue",
//             _ => "queue",
//         };
//     }
//     match priority.to_lowercase().as_str() {
//         "urgent" | "critical" => "interrupt",
//         _ => "queue",
//     }
// }

// /// Determine whether ack is required based on normalized type and priority.
// pub fn requires_ack(msg_type: &str, priority: &str, explicit_ack: bool) -> bool {
//     if explicit_ack {
//         return true;
//     }
//     match msg_type {
//         "task" | "escalation" | "scavenge" => true,
//         "notification" => matches!(priority.to_lowercase().as_str(), "urgent" | "high"),
//         "reply" => false,
//         _ => false,
//     }
// }

// // ─── Validation ──────────────────────────────────────────────────────────────

// /// Validate mail directive before routing. Returns error message on failure.
// pub fn validate_mail(directive: &MailDirective, sender_session_id: Uuid, recipient_session_id: Uuid) -> Option<String> {
//     if directive.subject.is_empty() {
//         return Some("SAPPHIRE_MAIL rejected: subject is empty.".to_owned());
//     }
//     if directive.subject.len() > 120 {
//         return Some(format!(
//             "SAPPHIRE_MAIL rejected: subject too long ({} chars, max 120).",
//             directive.subject.len()
//         ));
//     }
//     let body_len = directive.context.len() + directive.request.len() + directive.expected_action.len();
//     if body_len > 8192 {
//         return Some(format!(
//             "SAPPHIRE_MAIL rejected: body too long ({} chars, max 8KB).",
//             body_len
//         ));
//     }
//     if sender_session_id == recipient_session_id
//         && !matches!(normalize_message_type(&directive.message_type), "task" | "notification")
//     {
//         return Some("SAPPHIRE_MAIL rejected: sender and recipient are the same session. Use internal state instead.".to_owned());
//     }
//     if directive.cc.iter().any(|addr| {
//         addr.eq_ignore_ascii_case("supervisor") || addr.eq_ignore_ascii_case("sup")
//     }) && normalize_message_type(&directive.message_type) != "escalation"
//     {
//         // Warning only, not rejection
//     }
//     None
// }

// // ─── Engineering-semantic rendering ─────────────────────────────────────────

// /// Render mail for PTY injection with engineering-team semantics.
// /// Each type gets a distinct header format so agents instantly understand intent.
// pub fn render_mail_for_delivery(
//     message_id: Uuid,
//     thread_id: &str,
//     sender_name: &str,
//     directive: &MailDirective,
//     cc_ids: &[Uuid],
//     requires_ack: bool,
//     is_urgent: bool,
// ) -> String {
//     let normalized = normalize_message_type(&directive.message_type);

//     let header = match normalized {
//         "task" => format!(
//             "[SAPPHIRE TASK — ACTION REQUIRED]\n\
//              FROM: {sender}\nTO: {to}\n\
//              THREAD: {thread}\nPRIORITY: {priority}\n\n\
//              SUBJECT: {subject}\n\n\
//              CONTEXT:\n{context}\n\n\
//              REQUEST:\n{request}\n\n\
//              EXPECTED ACTION:\n{expected}",
//             sender = sender_name,
//             to = directive.to,
//             thread = thread_id,
//             priority = directive.priority.to_uppercase(),
//             subject = directive.subject,
//             context = directive.context,
//             request = directive.request,
//             expected = directive.expected_action,
//         ),
//         "escalation" => format!(
//             "⚠ [SAPPHIRE ESCALATION — BLOCKER]\n\
//              FROM: {sender}\nTO: {to}\n\
//              THREAD: {thread}\nPRIORITY: {priority}\n\n\
//              SUBJECT: {subject}\n\n\
//              CONTEXT:\n{context}\n\n\
//              BLOCKER:\n{request}\n\n\
//              ESCALATION REQUEST:\n{expected}",
//             sender = sender_name,
//             to = directive.to,
//             thread = thread_id,
//             priority = directive.priority.to_uppercase(),
//             subject = directive.subject,
//             context = directive.context,
//             request = directive.request,
//             expected = directive.expected_action,
//         ),
//         "scavenge" => format!(
//             "[SAPPHIRE WORK AVAILABLE — FIRST TO CLAIM]\n\
//              FROM: {sender}\nTO: {to}\n\
//              THREAD: {thread}\n\n\
//              SUBJECT: {subject}\n\n\
//              CONTEXT:\n{context}\n\n\
//              AVAILABLE WORK:\n{request}\n\n\
//              To claim this work, acknowledge and take ownership.",
//             sender = sender_name,
//             to = directive.to,
//             thread = thread_id,
//             subject = directive.subject,
//             context = directive.context,
//             request = directive.request,
//         ),
//         "reply" => format!(
//             "[SAPPHIRE REPLY]\n\
//              FROM: {sender}\nTHREAD: {thread}\n\n\
//              SUBJECT: Re: {subject}\n\n\
//              {context}",
//             sender = sender_name,
//             thread = thread_id,
//             subject = directive.subject,
//             context = if directive.request.is_empty() { &directive.context } else { &directive.request },
//         ),
//         _ => format!(
//             "[SAPPHIRE NOTICE]\n\
//              FROM: {sender}\nTO: {to}\n\
//              THREAD: {thread}\n\n\
//              SUBJECT: {subject}\n\n\
//              {body}",
//             sender = sender_name,
//             to = directive.to,
//             thread = thread_id,
//             subject = directive.subject,
//             body = if directive.request.is_empty() { &directive.context } else { &directive.request },
//         ),
//     };

//     let cc_line = if cc_ids.is_empty() {
//         String::new()
//     } else {
//         format!("\nCC: {} recipients (visibility only)\n", cc_ids.len())
//     };

//     let ack_instruction = if requires_ack {
//         if is_urgent {
//             format!(
//                 "\n⚡ URGENT: Acknowledge IMMEDIATELY with:\n\
//                  SAPPHIRE_ACK {{\"mail_id\":\"{}\",\"status\":\"acked\",\"summary\":\"acknowledged\"}}",
//                 message_id
//             )
//         } else {
//             format!(
//                 "\nAcknowledge with:\n\
//                  SAPPHIRE_ACK {{\"mail_id\":\"{}\",\"status\":\"acked\",\"summary\":\"one short sentence\"}}",
//                 message_id
//             )
//         }
//     } else {
//         "\nNo ack required. Respond via SAPPHIRE_MAIL or SAPPHIRE_STATUS when your state changes.".to_owned()
//     };

//     format!("{header}{cc_line}{ack_instruction}")
// }

// // ─── CC notice rendering ────────────────────────────────────────────────────

// /// Render a CC visibility notice for non-primary recipients.
// pub fn render_cc_notice(
//     thread_id: &str,
//     sender_name: &str,
//     recipient_name: &str,
//     directive: &MailDirective,
// ) -> String {
//     format!(
//         "[SAPPHIRE CC NOTICE]\n\
//          You are CC'd on mail thread: {thread}\n\
//          FROM: {from}\nTO: {to}\n\
//          SUBJECT: {subject}\n\
//          TYPE: {msg_type}\n\n\
//          No action required. Monitor thread for context.",
//         thread = thread_id,
//         from = sender_name,
//         to = recipient_name,
//         subject = directive.subject,
//         msg_type = normalize_message_type(&directive.message_type),
//     )
// }

// // ─── Orchestrator mail handlers ──────────────────────────────────────────────

// /// Orchestrator stats (subset we need for mail)
// pub struct MailStats {
//     pub mails_routed: usize,
//     pub lease_conflicts: usize,
// }

// /// Resolve a display name alias to a session ID.
// pub fn resolve_alias(alias_map: &HashMap<String, Uuid>, name: &str) -> Option<Uuid> {
//     // Direct match
//     if let Some(id) = alias_map.get(name) {
//         return Some(*id);
//     }
//     // Case-insensitive match
//     let lower = name.to_lowercase();
//     alias_map
//         .iter()
//         .find(|(k, _)| k.to_lowercase() == lower)
//         .map(|(_, v)| *v)
// }

// /// Parse a mail ID string into a Uuid.
// pub fn parse_mail_id(value: &str) -> Option<Uuid> {
//     Uuid::parse_str(value.trim()).ok()
// }

// /// Handle claim/release mail directives — modify scavenge mail ownership.
// fn handle_claim_or_release(
//     store: &Store,
//     mission_id: Uuid,
//     sender_session_id: Uuid,
//     directive: &MailDirective,
//     active_sessions: &mut HashMap<Uuid, super::ActiveSession>,
//     _alias_map: &HashMap<String, Uuid>,
// ) -> Result<MailHandlingResult> {
//     // Parse mail_id from subject or body to find the scavenge mail to claim/release
//     let scavenge_id = directive
//         .mail_id
//         .as_deref()
//         .and_then(parse_mail_id);

//     let Some(mail_id) = scavenge_id else {
//         if let Some(sender) = active_sessions.get(&sender_session_id) {
//             let _ = sender.runtime.send_prompt(
//                 "SAPPHIRE_MAIL claim/release requires mail_id set to the scavenge message ID.",
//             );
//         }
//         return Ok(MailHandlingResult { supervisor_notice: None });
//     };

//     let sender_name = active_sessions
//         .get(&sender_session_id)
//         .map(|s| s.record.name.clone())
//         .unwrap_or_else(|| "unknown".to_owned());

//     let raw_type = directive.message_type.to_lowercase();
//     if raw_type == "claim" {
//         match attempt_scavenge_claim(store, mail_id, sender_session_id, &sender_name)? {
//             ClaimResult::Claimed => {
//                 store.append_summary(
//                     mission_id, Some(sender_session_id), "scavenge_claimed",
//                     &format!("{} claimed scavenge: {}", sender_name, directive.subject),
//                 )?;
//                 if let Some(sender) = active_sessions.get(&sender_session_id) {
//                     let _ = sender.runtime.send_prompt(&format!(
//                         "Scavenge claimed: {}. You now own this work. Emit SAPPHIRE_STATUS with state=progressing when you start.",
//                         directive.subject
//                     ));
//                 }
//             }
//             ClaimResult::AlreadyClaimed { claimed_by, claimed_at } => {
//                 if let Some(sender) = active_sessions.get(&sender_session_id) {
//                     let _ = sender.runtime.send_prompt(&format!(
//                         "Scavenge already claimed by {} at {}. Pick another task or wait.",
//                         claimed_by, claimed_at
//                     ));
//                 }
//             }
//             ClaimResult::NotFound => {
//                 if let Some(sender) = active_sessions.get(&sender_session_id) {
//                     let _ = sender.runtime.send_prompt(
//                         "Scavenge not found. Check the mail_id field points to a valid scavenge message.",
//                     );
//                 }
//             }
//         }
//     } else {
//         // release
//         let released = release_scavenge(store, mail_id, sender_session_id)?;
//         if released {
//             store.append_summary(
//                 mission_id, Some(sender_session_id), "scavenge_released",
//                 &format!("{} released scavenge: {}", sender_name, directive.subject),
//             )?;
//             if let Some(sender) = active_sessions.get(&sender_session_id) {
//                 let _ = sender.runtime.send_prompt(&format!(
//                     "Scavenge released: {}. Back in the pool for others to claim.",
//                     directive.subject
//                 ));
//             }
//         } else {
//             if let Some(sender) = active_sessions.get(&sender_session_id) {
//                 let _ = sender.runtime.send_prompt(
//                     "Release failed — you don't own that scavenge or it doesn't exist.",
//                 );
//             }
//         }
//     }

//     Ok(MailHandlingResult { supervisor_notice: None })
// }

// /// Result of mail handling that may require follow-up by the orchestrator.
// pub struct MailHandlingResult {
//     pub supervisor_notice: Option<(crate::adapter::SupervisorEventType, String)>,
// }

// /// Handle SAPPHIRE_MAIL directive — route, persist, inject, track ack.
// pub fn handle_mail_directive(
//     store: &Store,
//     state_dir: &Path,
//     mission_id: Uuid,
//     supervisor_id: Uuid,
//     sender_session_id: Uuid,
//     directive: MailDirective,
//     active_sessions: &mut HashMap<Uuid, super::ActiveSession>,
//     alias_map: &HashMap<String, Uuid>,
//     pending_mail: &mut HashMap<Uuid, PendingMail>,
//     _stats: &mut MailStats,
// ) -> Result<MailHandlingResult> {
//     use crate::adapter::SupervisorEventType;

//     let Some(recipient_session_id) = resolve_alias(alias_map, &directive.to) else {
//         if let Some(sender) = active_sessions.get(&sender_session_id) {
//             let _ = sender.runtime.send_prompt(
//                 "Your SAPPHIRE_MAIL target did not resolve to a known session alias. Use the display name such as Engineer-2 or Supervisor.",
//             );
//         }
//         return Ok(MailHandlingResult { supervisor_notice: None });
//     };

//     // Claim/release handling — special message types that modify scavenge mail
//     let raw_type = directive.message_type.to_lowercase();
//     if raw_type == "claim" || raw_type == "release" {
//         return handle_claim_or_release(
//             store, mission_id, sender_session_id, &directive,
//             active_sessions, alias_map,
//         );
//     }

//     // Validation
//     if let Some(error) = validate_mail(&directive, sender_session_id, recipient_session_id) {
//         if let Some(sender) = active_sessions.get(&sender_session_id) {
//             let _ = sender.runtime.send_prompt(&error);
//         }
//         return Ok(MailHandlingResult { supervisor_notice: None });
//     }

//     // Normalize message type and delivery mode
//     let normalized_type = normalize_message_type(&directive.message_type);
//     let delivery_mode = derive_delivery_mode(&directive.priority, &directive.delivery_mode);
//     let is_urgent = matches!(directive.priority.to_lowercase().as_str(), "urgent" | "critical");
//     let ack_required = requires_ack(normalized_type, &directive.priority, directive.requires_ack);

//     // Escalation auto-CCs supervisor
//     let mut cc_session_ids: Vec<Uuid> = directive
//         .cc
//         .iter()
//         .filter_map(|addr| resolve_alias(alias_map, addr))
//         .filter(|id| *id != sender_session_id && *id != recipient_session_id)
//         .collect();

//     if normalized_type == "escalation" && !cc_session_ids.contains(&supervisor_id) && sender_session_id != supervisor_id {
//         cc_session_ids.push(supervisor_id);
//     }

//     // Thread tracking
//     let message_id = directive
//         .mail_id
//         .as_deref()
//         .and_then(parse_mail_id)
//         .unwrap_or_else(Uuid::new_v4);

//     let thread_id = directive
//         .thread_id
//         .clone()
//         .or_else(|| {
//             directive
//                 .reply_to
//                 .as_deref()
//                 .and_then(parse_mail_id)
//                 .and_then(|reply_to_id| pending_mail.get(&reply_to_id).map(|m| m.thread_id.clone()))
//         })
//         .unwrap_or_else(|| message_id.to_string());

//     let sender_name = active_sessions
//         .get(&sender_session_id)
//         .map(|s| s.record.name.clone())
//         .unwrap_or_else(|| "unknown".to_owned());
//     let recipient_name = active_sessions
//         .get(&recipient_session_id)
//         .map(|s| s.record.name.clone())
//         .unwrap_or_else(|| directive.to.clone());
//     let Some(sender_session) = active_sessions.get(&sender_session_id) else {
//         return Ok(MailHandlingResult { supervisor_notice: None });
//     };
//     let Some(recipient_session) = active_sessions.get(&recipient_session_id) else {
//         return Ok(MailHandlingResult { supervisor_notice: None });
//     };
//     let governance = coordination::govern_mail(
//         sender_session,
//         recipient_session,
//         &directive,
//         pending_mail,
//     );
//     if let Some(reason) = governance.block_reason.as_deref() {
//         if let Some(sender) = active_sessions.get(&sender_session_id) {
//             let _ = sender.runtime.send_prompt(reason);
//         }
//         return Ok(MailHandlingResult { supervisor_notice: None });
//     }

//     // Persist to SQLite
//     let mail_record = MailRecord {
//         id: message_id,
//         mission_id,
//         sender_worker_id: sender_session_id,
//         recipient_worker_id: recipient_session_id,
//         message_type: normalized_type.to_owned(),
//         priority: directive.priority.clone(),
//         delivery_mode: delivery_mode.to_owned(),
//         subject: directive.subject.clone(),
//         status: if ack_required { "awaiting_ack".to_owned() } else { "routed".to_owned() },
//         ack_state: if ack_required { "pending".to_owned() } else { "not_required".to_owned() },
//         pinned: directive.pinned,
//         body_json: serde_json::to_string(&json!({
//             "mail_id": message_id,
//             "thread_id": thread_id,
//             "reply_to": directive.reply_to,
//             "to": directive.to,
//             "cc": cc_session_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>(),
//             "message_type": normalized_type,
//             "intent": governance.intent.clone(),
//             "thread_state": governance.thread_state.clone(),
//             "duplicate_key": governance.duplicate_key.clone(),
//             "sender_pod": governance.sender_pod.clone(),
//             "recipient_pod": governance.recipient_pod.clone(),
//             "routing_class": governance.routing_class.clone(),
//             "priority": directive.priority,
//             "delivery_mode": delivery_mode,
//             "subject": directive.subject,
//             "context": directive.context,
//             "request": directive.request,
//             "expected_action": directive.expected_action,
//             "requires_ack": ack_required,
//         }))?,
//         thread_id: thread_id.clone(),
//         reply_to: directive.reply_to.clone(),
//         created_at: Utc::now(),
//         archived_at: None,
//     };
//     store.persist_message(&mail_record)?;
//     store.append_summary(
//         mission_id,
//         Some(sender_session_id),
//         "mail_sent",
//         format!(
//             "to={} type={} priority={} mode={} subject={} thread={}",
//             recipient_name, normalized_type, directive.priority, delivery_mode, directive.subject, thread_id
//         ),
//     )?;

//     // Deliver to primary recipient — use nudge queue for non-destructive delivery
//     if let Some(recipient) = active_sessions.get(&recipient_session_id) {
//         let delivery_mode = derive_delivery_mode(&directive.priority, &directive.delivery_mode);
//         if delivery_mode == "interrupt" {
//             // Direct PTY injection — only for urgent/critical priority
//             let mail_prompt = render_mail_for_delivery(
//                 message_id, &thread_id, &sender_name, &directive,
//                 &cc_session_ids, ack_required, is_urgent,
//             );
//             let _ = recipient.runtime.send_prompt(&mail_prompt);
//         } else {
//             // Queue-based delivery — write to filesystem, drain at next turn boundary
//             let nudge = nudge_from_mail(&directive, &sender_name);
//             if let Err(e) = nudge_enqueue(state_dir, &recipient.record.id.to_string(), nudge) {
//                 // Fallback to direct injection if queue fails
//                 tracing::warn!("nudge enqueue failed, falling back to direct injection: {e}");
//                 let mail_prompt = render_mail_for_delivery(
//                     message_id, &thread_id, &sender_name, &directive,
//                     &cc_session_ids, ack_required, is_urgent,
//                 );
//                 let _ = recipient.runtime.send_prompt(&mail_prompt);
//             }
//         }
//     }

//     // Notify CC recipients
//     for cc_id in &cc_session_ids {
//         if let Some(cc_session) = active_sessions.get(cc_id) {
//             let cc_notice = render_cc_notice(&thread_id, &sender_name, &recipient_name, &directive);
//             let _ = cc_session.runtime.send_prompt(&cc_notice);
//         }
//     }

//     let _ = store.update_worker_summary(
//         recipient_session_id,
//         &format!(
//             "mail inbox: {} [{}] ({})",
//             directive.subject, normalized_type, directive.priority
//         ),
//     );

//     // Track pending mail for ack timeout
//     if ack_required {
//         pending_mail.insert(
//             message_id,
//             PendingMail {
//                 message_id,
//                 thread_id: thread_id.clone(),
//                 intent: governance.intent.clone(),
//                 thread_state: governance.thread_state.clone(),
//                 duplicate_key: governance.duplicate_key.clone(),
//                 sender_session_id,
//                 recipient_session_id,
//                 cc_session_ids: cc_session_ids.clone(),
//                 sender_pod: governance.sender_pod.clone(),
//                 recipient_pod: governance.recipient_pod.clone(),
//                 routing_class: governance.routing_class.clone(),
//                 subject: directive.subject.clone(),
//                 message_type: normalized_type.to_owned(),
//                 priority: directive.priority.clone(),
//                 routed_at: Instant::now(),
//                 acked: false,
//                 timeout_stage: 0,
//                 last_timeout_at: None,
//                 reply_count: 0,
//             },
//         );
//     }

//     // Reply chain: mark original as responded
//     if let Some(reply_to) = directive.reply_to.as_deref().and_then(parse_mail_id) {
//         if let Some(original) = pending_mail.get_mut(&reply_to) {
//             original.acked = true;
//             original.reply_count += 1;
//             original.thread_state = "answered".to_owned();
//             let _ = store.update_message_status(reply_to, "responded", "acked");
//         }
//     }

//     // Supervisor notice for inter-worker mail
//     let supervisor_notice = if sender_session_id != supervisor_id && recipient_session_id != supervisor_id && !directive.suppress_notify {
//         let event_type = if is_urgent { SupervisorEventType::Blocked } else { SupervisorEventType::Notice };
//         Some((event_type, format!(
//             "{}Mail routed from {} to {} [{}]: {} (thread: {})",
//             if is_urgent { "⚡URGENT⚡ " } else { "" },
//             sender_name, recipient_name, normalized_type, directive.subject, thread_id
//         )))
//     } else {
//         None
//     };

//     Ok(MailHandlingResult { supervisor_notice })
// }

// /// Handle SAPPHIRE_ACK directive — idempotent processing.
// pub fn handle_ack_directive(
//     store: &Store,
//     mission_id: Uuid,
//     supervisor_id: Uuid,
//     sender_session_id: Uuid,
//     directive: AckDirective,
//     pending_mail: &mut HashMap<Uuid, PendingMail>,
//     active_sessions: &mut HashMap<Uuid, super::ActiveSession>,
// ) -> Result<()> {
//     use crate::adapter::{SupervisorEventType, adapter_for};

//     let message_id = parse_mail_id(&directive.mail_id);
//     let Some(mail_id) = message_id else {
//         return Ok(());
//     };

//     let next_thread_state = coordination::thread_state_for_ack(&directive.status).to_owned();

//     // Idempotent ack: ignore exact duplicates, but allow acked -> done/cannot_comply upgrades.
//     if let Some(pending) = pending_mail.get_mut(&mail_id) {
//         if pending.thread_state == "closed" {
//             return Ok(());
//         }
//         if pending.acked && pending.thread_state == next_thread_state {
//             return Ok(());
//         }
//         pending.acked = true;
//         pending.thread_state = next_thread_state;
//     }

//     let ack_status = directive.status.trim().to_ascii_lowercase();
//     let persisted_status = match ack_status.as_str() {
//         "done" => "completed",
//         "cannot_comply" => "cannot_comply",
//         _ => "acked",
//     };
//     let _ = store.update_message_status(mail_id, persisted_status, &directive.summary);

//     // Notify original sender that mail was acked
//     if let Some(pending) = pending_mail.get(&mail_id) {
//         if let Some(sender) = active_sessions.get(&pending.sender_session_id) {
//             let sender_prompt = match ack_status.as_str() {
//                 "done" => format!(
//                     "Thread {} resolved: {} reports the requested work is done for '{}'.",
//                     pending.thread_id,
//                     active_sessions
//                         .get(&sender_session_id)
//                         .map(|session| session.record.name.as_str())
//                         .unwrap_or("teammate"),
//                     pending.subject,
//                 ),
//                 "cannot_comply" => format!(
//                     "Thread {} cannot proceed as requested: {} reported cannot_comply for '{}'. Summary: {}. Reroute the dependency, narrow the ask, or escalate with concrete context.",
//                     pending.thread_id,
//                     active_sessions
//                         .get(&sender_session_id)
//                         .map(|session| session.record.name.as_str())
//                         .unwrap_or("teammate"),
//                     pending.subject,
//                     directive.summary,
//                 ),
//                 _ => format!(
//                     "SAPPHIRE_ACK for mail {mail_id} (thread: {thread}): {status} — {summary}",
//                     mail_id = mail_id,
//                     thread = pending.thread_id,
//                     status = directive.status,
//                     summary = directive.summary,
//                 ),
//             };
//             let _ = sender.runtime.send_prompt(&sender_prompt);
//         }
//     }

//     // Notify supervisor of ack
//     let sender_name = active_sessions
//         .get(&sender_session_id)
//         .map(|s| s.record.name.as_str())
//         .unwrap_or("unknown");

//     if let Some(pending) = pending_mail.get(&mail_id) {
//         store.append_json_event(
//             mission_id,
//             Some(sender_session_id),
//             "mail_ack",
//             &format!("ack from {sender_name} for mail {mail_id}"),
//             &json!({
//                 "mail_id": mail_id,
//                 "thread_id": pending.thread_id,
//                 "ack_status": directive.status,
//                 "summary": directive.summary,
//             }),
//         )?;

//         if ack_status == "cannot_comply" {
//             let supervisor_notice = format!(
//                 "Mail thread {} hit cannot_comply. {} could not complete '{}' for {}. Summary: {}",
//                 pending.thread_id,
//                 active_sessions
//                     .get(&sender_session_id)
//                     .map(|session| session.record.name.as_str())
//                     .unwrap_or("unknown"),
//                 pending.subject,
//                 active_sessions
//                     .get(&pending.sender_session_id)
//                     .map(|session| session.record.name.as_str())
//                     .unwrap_or("unknown"),
//                 directive.summary,
//             );
//             if let Some(supervisor) = active_sessions.get(&supervisor_id) {
//                 let adapter = adapter_for(supervisor.record.agent);
//                 let prompt = adapter.build_supervisor_action_prompt(
//                     SupervisorEventType::Blocked,
//                     &supervisor_notice,
//                 );
//                 let _ = supervisor.runtime.send_prompt(&prompt);
//             }
//             store.append_summary(
//                 mission_id,
//                 Some(supervisor_id),
//                 "mail_cannot_comply",
//                 supervisor_notice,
//             )?;
//         }
//     }

//     Ok(())
// }

// /// Handle SAPPHIRE_LEASE directive — file ownership claims with conflict detection.
// pub fn handle_lease_directive(
//     store: &Store,
//     mission_id: Uuid,
//     supervisor_id: Uuid,
//     session_id: Uuid,
//     directive: crate::protocol::LeaseDirective,
//     active_sessions: &mut HashMap<Uuid, super::ActiveSession>,
//     leases: &mut HashMap<String, super::LeaseOwner>,
//     stats: &mut MailStats,
// ) -> Result<()> {
//     use crate::model::SessionState;

//     for path in directive.paths {
//         let normalized = path.trim().to_owned();
//         let lease_record = crate::model::LeaseRecord {
//             mission_id,
//             path: normalized.clone(),
//             owner_session_id: session_id,
//             intent: directive.intent.clone(),
//             status: directive.status.clone(),
//             updated_at: Utc::now(),
//         };
//         store.upsert_lease(&lease_record)?;

//         if directive.status.eq_ignore_ascii_case("release") {
//             if leases.get(&normalized).map(|o| o.session_id == session_id).unwrap_or(false) {
//                 leases.remove(&normalized);
//             }
//             continue;
//         }

//         if let Some(existing) = leases.get(&normalized) {
//             if existing.session_id != session_id {
//                 stats.lease_conflicts += 1;
//                 if let Some(session) = active_sessions.get_mut(&session_id) {
//                     session.state = SessionState::Contradictory;
//                     store.update_session_state(session_id, SessionState::Contradictory)?;
//                 }

//                 let owner_name = active_sessions
//                     .get(&existing.session_id)
//                     .map(|s| s.record.name.clone())
//                     .unwrap_or_else(|| existing.session_id.to_string());
//                 let challenger_name = active_sessions
//                     .get(&session_id)
//                     .map(|s| s.record.name.clone())
//                     .unwrap_or_else(|| session_id.to_string());

//                 store.append_json_event(
//                     mission_id,
//                     Some(session_id),
//                     "lease_conflict",
//                     &format!("conflict on {normalized}"),
//                     &json!({
//                         "path": normalized,
//                         "owner_session_id": existing.session_id,
//                         "challenger_session_id": session_id,
//                         "owner_intent": existing.intent,
//                         "challenger_intent": directive.intent,
//                     }),
//                 )?;

//                 if let Some(challenger) = active_sessions.get(&session_id) {
//                     let _ = challenger.runtime.send_prompt(&format!(
//                         "Lease conflict detected on {path}. Another worker currently owns that path. Stop and wait for supervisor ruling. Emit SAPPHIRE_STATUS {{\"state\":\"blocked\",\"summary\":\"lease conflict on {path}\"}}",
//                         path = normalized
//                     ));
//                 }

//                 if let Some(owner) = active_sessions.get(&existing.session_id) {
//                     let _ = owner.runtime.send_prompt(&format!(
//                         "Another worker attempted to claim {path} while you own it. Emit a fresh SAPPHIRE_STATUS line if your scope or readiness changed.",
//                         path = normalized
//                     ));
//                 }

//                 // Supervisor notice
//                 store.append_json_event(
//                     mission_id,
//                     Some(supervisor_id),
//                     "supervisor_notice",
//                     &format!("Lease conflict on {normalized}"),
//                     &json!({
//                         "event": "lease_conflict_escalation",
//                         "path": normalized,
//                         "owner": owner_name,
//                         "challenger": challenger_name,
//                         "owner_intent": existing.intent,
//                         "challenger_intent": directive.intent,
//                     }),
//                 )?;

//                 continue;
//             }
//         }

//         // No conflict — claim the lease
//         leases.insert(
//             normalized.clone(),
//             super::LeaseOwner {
//                 session_id,
//                 intent: directive.intent.clone(),
//             },
//         );
//     }
//     Ok(())
// }

// /// Auto-archive resolved mail older than the given duration.
// /// Returns count of archived messages.
// #[allow(dead_code)]
// pub fn auto_archive_resolved_mail(
//     store: &Store,
//     mission_id: Uuid,
//     older_than_secs: u64,
// ) -> Result<usize> {
//     store.archive_resolved_mail(mission_id, older_than_secs)
// }

// pub fn mail_timeout_interval(priority: &str) -> Duration {
//     match priority.to_lowercase().as_str() {
//         "urgent" | "critical" => Duration::from_secs(10),
//         "high" => Duration::from_secs(15),
//         "low" => Duration::from_secs(45),
//         _ => Duration::from_secs(20),
//     }
// }

// pub fn mail_timeout_stage_due(pending: &PendingMail, now: Instant) -> bool {
//     let since = pending.last_timeout_at.unwrap_or(pending.routed_at);
//     now.duration_since(since) >= mail_timeout_interval(&pending.priority)
// }

// pub fn recipient_timeout_prompt(
//     pending: &PendingMail,
//     sender_name: &str,
//     stage: u8,
// ) -> String {
//     match stage {
//         1 => format!(
//             "You have an outstanding {} mail in thread {} from {}: '{}'. Reply now. Choose exactly one: 1. SAPPHIRE_ACK status=acked if you will handle it. 2. SAPPHIRE_ACK status=done if already completed. 3. SAPPHIRE_ACK status=cannot_comply with one concrete blocker.",
//             pending.message_type, pending.thread_id, sender_name, pending.subject
//         ),
//         2 => format!(
//             "Second coordination timeout for thread {}. Do not stay silent. Send SAPPHIRE_ACK with status=acked|done|cannot_comply immediately, then send SAPPHIRE_MAIL if the sender needs a concrete handoff, dependency answer, or blocker detail.",
//             pending.thread_id
//         ),
//         _ => format!(
//             "Final coordination timeout for thread {}. Respond now with SAPPHIRE_ACK status=done or cannot_comply. If you cannot comply, include the blocker in the summary so the team can reroute without waiting.",
//             pending.thread_id
//         ),
//     }
// }

// pub fn sender_timeout_prompt(
//     pending: &PendingMail,
//     recipient_name: &str,
//     stage: u8,
// ) -> String {
//     match stage {
//         1 => format!(
//             "Your mail '{}' to {} in thread {} is still waiting on acknowledgment. Continue independent work. If the dependency becomes critical, prepare a narrower follow-up or alternate path.",
//             pending.subject, recipient_name, pending.thread_id
//         ),
//         2 => format!(
//             "Your mail '{}' to {} in thread {} is still unanswered after a second timeout. Send a narrower follow-up only if needed. Otherwise keep moving on independent work and be ready to reroute the dependency.",
//             pending.subject, recipient_name, pending.thread_id
//         ),
//         _ => format!(
//             "Coordination failed for '{}' with {} in thread {} after repeated timeouts. Keep moving on independent scope. Expect supervisor review or reroute the dependency through another teammate if possible.",
//             pending.subject, recipient_name, pending.thread_id
//         ),
//     }
// }

// pub fn cc_timeout_prompt(pending: &PendingMail, stage: u8) -> String {
//     match stage {
//         1 => format!(
//             "[SAPPHIRE CC NOTICE] Thread {} is waiting on an acknowledgment for '{}'. Monitor only.",
//             pending.thread_id, pending.subject
//         ),
//         2 => format!(
//             "[SAPPHIRE CC NOTICE] Thread {} has hit a second timeout for '{}'. Be ready to help if rerouted.",
//             pending.thread_id, pending.subject
//         ),
//         _ => format!(
//             "[SAPPHIRE CC NOTICE] Thread {} has entered coordination failure for '{}'. Supervisor review is expected.",
//             pending.thread_id, pending.subject
//         ),
//     }
// }

// /// Probe unacked mail and escalate to supervisor if timeout exceeded.
// #[allow(dead_code)]
// pub fn probe_pending_mail(
//     store: &Store,
//     mission_id: Uuid,
//     supervisor_id: Uuid,
//     active_sessions: &mut HashMap<Uuid, super::ActiveSession>,
//     pending_mail: &mut HashMap<Uuid, PendingMail>,
//     timeout_secs: u64,
// ) -> Result<()> {
//     let now = Instant::now();
//     let to_probe: Vec<Uuid> = pending_mail
//         .iter()
//         .filter(|(_, m)| !m.acked && mail_timeout_stage_due(m, now))
//         .map(|(id, _)| *id)
//         .collect();

//     for mail_id in &to_probe {
//         if let Some(pending) = pending_mail.get_mut(mail_id) {
//             pending.timeout_stage = pending.timeout_stage.saturating_add(1);
//             pending.last_timeout_at = Some(now);

//             // Notify sender that ack is overdue
//             if let Some(sender) = active_sessions.get(&pending.sender_session_id) {
//                 let _ = sender.runtime.send_prompt(&format!(
//                     "Mail ack timeout: {} (thread: {}) from {} has not been acked after {}s.",
//                     pending.subject, pending.thread_id, pending.recipient_session_id, timeout_secs
//                 ));
//             }

//             // Notify recipient that ack is overdue
//             if let Some(recipient) = active_sessions.get(&pending.recipient_session_id) {
//                 let _ = recipient.runtime.send_prompt(&format!(
//                     "OVERDUE ACK: Mail '{}' from {} (thread: {}) requires acknowledgment. Respond with SAPPHIRE_ACK.",
//                     pending.subject, pending.sender_session_id, pending.thread_id
//                 ));
//             }

//             // Escalate to supervisor
//             store.append_json_event(
//                 mission_id,
//                 Some(supervisor_id),
//                 "mail_ack_timeout",
//                 &format!("ack overdue: {}", pending.subject),
//                 &json!({
//                     "mail_id": mail_id.to_string(),
//                     "thread_id": pending.thread_id,
//                     "sender": pending.sender_session_id.to_string(),
//                     "recipient": pending.recipient_session_id.to_string(),
//                     "subject": pending.subject,
//                     "timeout_secs": timeout_secs,
//                     "stage": pending.timeout_stage,
//                 }),
//             )?;
//         }
//     }
//     Ok(())
// }

// // ─── Nudge queue (non-destructive delivery) ──────────────────────────────────

// /// Non-destructive nudge delivery via filesystem queue.
// /// Instead of injecting text directly into the agent's PTY (which cancels
// /// in-flight tool calls), nudges are written as JSON files and picked up
// /// at the next natural turn boundary.
// const MAX_QUEUE_DEPTH: usize = 50;
// const NORMAL_TTL_SECS: i64 = 30 * 60;  // 30 min
// const URGENT_TTL_SECS: i64 = 2 * 3600;  // 2 hr
// const STALE_CLAIM_SECS: u64 = 5 * 60;   // 5 min

// /// A queued nudge waiting for the agent to reach a natural turn boundary.
// #[derive(Debug, Clone, Serialize, Deserialize)]
// pub struct QueuedNudge {
//     pub sender: String,
//     pub message: String,
//     pub priority: String,
//     pub thread_id: Option<String>,
//     pub timestamp: chrono::DateTime<Utc>,
//     pub expires_at: chrono::DateTime<Utc>,
// }

// fn queue_dir(state_dir: &Path, session_id: &str) -> PathBuf {
//     let safe = session_id.replace(['/', '\\'], "_");
//     state_dir.join("nudge_queue").join(safe)
// }

// /// Write a nudge to the filesystem queue. Returns error if queue is full.
// pub fn nudge_enqueue(state_dir: &Path, session_id: &str, nudge: QueuedNudge) -> Result<()> {
//     let dir = queue_dir(state_dir, session_id);
//     fs::create_dir_all(&dir)?;
//     let pending = nudge_pending_count(state_dir, session_id);
//     if pending >= MAX_QUEUE_DEPTH {
//         anyhow::bail!("nudge queue full ({}/{})", pending, MAX_QUEUE_DEPTH);
//     }
//     let safe_sender = nudge.sender.replace(['/', '\\'], "_");
//     let filename = format!("{}-{}.json",
//         nudge.timestamp.timestamp_nanos_opt().unwrap_or(0), safe_sender);
//     fs::write(dir.join(&filename), serde_json::to_string_pretty(&nudge)?)?;
//     Ok(())
// }

// /// Drain all queued nudges in FIFO order. Atomic rename prevents double-delivery.
// /// Expired nudges are discarded. Orphaned .claimed files are requeued.
// pub fn nudge_drain(state_dir: &Path, session_id: &str) -> Result<Vec<QueuedNudge>> {
//     let dir = queue_dir(state_dir, session_id);
//     if !dir.exists() { return Ok(Vec::new()); }
//     let now = chrono::Utc::now();

//     // Sweep orphaned .claimed files
//     if let Ok(entries) = fs::read_dir(&dir) {
//         for entry in entries.flatten() {
//             let name = entry.file_name().to_string_lossy().into_owned();
//             if name.contains(".claimed") {
//                 if let Ok(meta) = entry.metadata() {
//                     if let Ok(modified_ts) = meta.modified() {
//                         if modified_ts.elapsed().unwrap_or_default().as_secs() > STALE_CLAIM_SECS {
//                             if let Some(base) = name.split(".claimed").next() {
//                                 let _ = fs::rename(entry.path(), dir.join(format!("{}.json", base)));
//                             }
//                         } else {
//                             let _ = fs::remove_file(entry.path());
//                         }
//                     }
//                 }
//             }
//         }
//     }

//     let mut nudges = Vec::new();
//     if let Ok(entries) = fs::read_dir(&dir) {
//         let mut files: Vec<_> = entries.filter_map(|e| e.ok())
//             .filter(|e| e.file_name().to_string_lossy().ends_with(".json"))
//             .collect();
//         files.sort_by_key(|e| e.file_name());
//         for entry in files {
//             let path = entry.path();
//             let claim = PathBuf::from(format!("{}.claimed", path.display()));
//             if fs::rename(&path, &claim).is_err() { continue; }
//             let data = match fs::read_to_string(&claim) {
//                 Ok(d) => d,
//                 Err(_) => { let _ = fs::rename(&claim, &path); continue; }
//             };
//             let nudge: QueuedNudge = match serde_json::from_str(&data) {
//                 Ok(n) => n,
//                 Err(_) => { let _ = fs::remove_file(&claim); continue; }
//             };
//             if now > nudge.expires_at { let _ = fs::remove_file(&claim); continue; }
//             nudges.push(nudge);
//             let _ = fs::remove_file(&claim);
//         }
//     }
//     Ok(nudges)
// }

// /// Count pending nudges without draining.
// pub fn nudge_pending_count(state_dir: &Path, session_id: &str) -> usize {
//     let dir = queue_dir(state_dir, session_id);
//     if !dir.exists() { return 0; }
//     fs::read_dir(&dir).map(|e| e.filter_map(|x| x.ok())
//         .filter(|x| x.file_name().to_string_lossy().ends_with(".json")).count())
//         .unwrap_or(0)
// }

// /// Format nudges as <system-reminder> block for PTY injection.
// pub fn nudge_format_for_injection(nudges: &[QueuedNudge]) -> String {
//     if nudges.is_empty() { return String::new(); }
//     let (urgent, normal): (Vec<_>, Vec<_>) = nudges.iter()
//         .partition(|n| n.priority.eq_ignore_ascii_case("urgent") || n.priority.eq_ignore_ascii_case("critical"));
//     let mut lines = vec!["<system-reminder>".to_owned()];
//     if !urgent.is_empty() {
//         lines.push(format!("QUEUED NUDGE ({} urgent):\n", urgent.len()));
//         for n in &urgent { lines.push(format!("  [URGENT from {}] {}", n.sender, n.message)); }
//         if !normal.is_empty() {
//             lines.push(format!("\nPlus {} non-urgent nudge(s):", normal.len()));
//             for n in &normal { lines.push(format!("  [from {}] {}", n.sender, n.message)); }
//         }
//         lines.push("\nHandle urgent nudges before continuing current work.".to_owned());
//     } else {
//         lines.push(format!("QUEUED NUDGE ({} message(s)):\n", normal.len()));
//         for n in &normal { lines.push(format!("  [from {}] {}", n.sender, n.message)); }
//         lines.push("\nBackground notification. Continue work unless nudge is higher priority.".to_owned());
//     }
//     lines.push("</system-reminder>".to_owned());
//     lines.join("\n")
// }

// /// Create a nudge from a mail directive.
// pub fn nudge_from_mail(directive: &MailDirective, sender_name: &str) -> QueuedNudge {
//     let is_urgent = matches!(directive.priority.to_lowercase().as_str(), "urgent" | "critical");
//     let now = chrono::Utc::now();
//     QueuedNudge {
//         sender: sender_name.to_owned(),
//         message: format!("[{}] {} — {}", directive.message_type, directive.subject, directive.request),
//         priority: if is_urgent { "urgent".to_owned() } else { "normal".to_owned() },
//         thread_id: directive.thread_id.clone(),
//         timestamp: now,
//         expires_at: now + chrono::Duration::seconds(if is_urgent { URGENT_TTL_SECS } else { NORMAL_TTL_SECS }),
//     }
// }

// // ─── Scavenge claim/release ──────────────────────────────────────────────────

// /// Result of attempting to claim a scavenge message.
// pub enum ClaimResult {
//     Claimed,
//     AlreadyClaimed { claimed_by: String, claimed_at: String },
//     NotFound,
// }

// /// Attempt to claim a scavenge message. Only one worker can claim — first wins.
// /// Updates the mail body_json in SQLite with claimed_by and claimed_at.
// pub fn attempt_scavenge_claim(
//     store: &Store,
//     scavenge_mail_id: Uuid,
//     claimer_session_id: Uuid,
//     claimer_name: &str,
// ) -> Result<ClaimResult> {
//     let rows = store.claim_scavenge_mail(scavenge_mail_id, claimer_session_id, claimer_name)?;
//     if rows == 0 {
//         // Check if already claimed or doesn't exist
//         if let Some(body) = store.get_mail_body(scavenge_mail_id)? {
//             let parsed: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
//             if let Some(cb) = parsed.get("claimed_by").and_then(|v| v.as_str()) {
//                 let ca = parsed.get("claimed_at").and_then(|v| v.as_str()).unwrap_or("unknown");
//                 return Ok(ClaimResult::AlreadyClaimed {
//                     claimed_by: cb.to_owned(),
//                     claimed_at: ca.to_owned(),
//                 });
//             }
//             // Not a scavenge or no claim field
//             if parsed.get("message_type").and_then(|v| v.as_str()) != Some("scavenge") {
//                 return Ok(ClaimResult::NotFound);
//             }
//         }
//         return Ok(ClaimResult::NotFound);
//     }
//     Ok(ClaimResult::Claimed)
// }

// /// Release a claimed scavenge message back to the pool.
// pub fn release_scavenge(store: &Store, scavenge_mail_id: Uuid, releaser_id: Uuid) -> Result<bool> {
//     let rows = store.release_scavenge_mail(scavenge_mail_id, releaser_id)?;
//     Ok(rows > 0)
// }

// /// Probe the nudge queue for each active session and drain deliverable nudges.
// /// Returns nudges to inject per session.
// pub fn drain_nudge_queues(
//     state_dir: &Path,
//     active_sessions: &HashMap<Uuid, super::ActiveSession>,
// ) -> HashMap<Uuid, String> {
//     let mut injections = HashMap::new();
//     for (session_id, session) in active_sessions {
//         if session.state.is_terminal() { continue; }
//         let nudges = nudge_drain(state_dir, &session.record.id.to_string()).unwrap_or_default();
//         if nudges.is_empty() { continue; }
//         let formatted = nudge_format_for_injection(&nudges);
//         injections.insert(*session_id, formatted);
//     }
//     injections
// }

// #[cfg(test)]
// mod tests {
//     use super::*;

//     fn sample_pending(priority: &str) -> PendingMail {
//         PendingMail {
//             message_id: Uuid::new_v4(),
//             thread_id: "thread-1".to_owned(),
//             intent: "dependency".to_owned(),
//             thread_state: "open".to_owned(),
//             duplicate_key: "dup-1".to_owned(),
//             sender_session_id: Uuid::new_v4(),
//             recipient_session_id: Uuid::new_v4(),
//             cc_session_ids: Vec::new(),
//             sender_pod: "build".to_owned(),
//             recipient_pod: "platform".to_owned(),
//             routing_class: "cross_pod".to_owned(),
//             subject: "Need dependency answer".to_owned(),
//             message_type: "task".to_owned(),
//             priority: priority.to_owned(),
//             routed_at: Instant::now(),
//             acked: false,
//             timeout_stage: 0,
//             last_timeout_at: None,
//             reply_count: 0,
//         }
//     }

//     #[test]
//     fn timeout_interval_respects_priority() {
//         assert_eq!(mail_timeout_interval("urgent"), Duration::from_secs(10));
//         assert_eq!(mail_timeout_interval("high"), Duration::from_secs(15));
//         assert_eq!(mail_timeout_interval("normal"), Duration::from_secs(20));
//         assert_eq!(mail_timeout_interval("low"), Duration::from_secs(45));
//     }

//     #[test]
//     fn timeout_stage_due_uses_last_timeout_marker() {
//         let mut pending = sample_pending("normal");
//         pending.routed_at = Instant::now() - Duration::from_secs(25);
//         assert!(mail_timeout_stage_due(&pending, Instant::now()));

//         pending.last_timeout_at = Some(Instant::now());
//         assert!(!mail_timeout_stage_due(&pending, Instant::now()));
//     }

//     #[test]
//     fn recipient_timeout_prompt_demands_explicit_ack_status() {
//         let pending = sample_pending("high");
//         let prompt = recipient_timeout_prompt(&pending, "Engineer-1", 1);
//         assert!(prompt.contains("SAPPHIRE_ACK"));
//         assert!(prompt.contains("cannot_comply"));
//         assert!(prompt.contains("done"));
//     }
// }
