use super::*;

impl Orchestrator {
    pub(super) fn handle_mail_directive(
        &self,
        state_dir: &Path,
        mission_id: Uuid,
        supervisor_id: Uuid,
        sender_session_id: Uuid,
        directive: MailDirective,
        active_sessions: &mut HashMap<Uuid, ActiveSession>,
        alias_map: &HashMap<String, Uuid>,
        pending_mail: &mut HashMap<Uuid, PendingMail>,
        stats: &mut WatchdogStats,
    ) -> Result<()> {
        let result = mail::handle_mail_directive(
            &self.store,
            state_dir,
            mission_id,
            supervisor_id,
            sender_session_id,
            directive,
            active_sessions,
            alias_map,
            pending_mail,
            &mut mail::MailStats {
                mails_routed: 0,
                lease_conflicts: 0,
            },
        )?;
        stats.mails_routed += 1;
        if let Some((event_type, notice)) = result.supervisor_notice {
            self.send_worker_supervisor_notice(
                mission_id,
                supervisor_id,
                sender_session_id,
                active_sessions,
                event_type,
                &notice,
            )?;
        }
        Ok(())
    }

    pub(super) fn handle_ack_directive(
        &self,
        mission_id: Uuid,
        supervisor_id: Uuid,
        sender_session_id: Uuid,
        directive: AckDirective,
        active_sessions: &mut HashMap<Uuid, ActiveSession>,
        pending_mail: &mut HashMap<Uuid, PendingMail>,
    ) -> Result<()> {
        mail::handle_ack_directive(
            &self.store,
            mission_id,
            supervisor_id,
            sender_session_id,
            directive,
            pending_mail,
            active_sessions,
        )
    }

    pub(super) fn handle_lease_directive(
        &self,
        mission_id: Uuid,
        supervisor_id: Uuid,
        session_id: Uuid,
        directive: LeaseDirective,
        active_sessions: &mut HashMap<Uuid, ActiveSession>,
        leases: &mut HashMap<String, LeaseOwner>,
        stats: &mut WatchdogStats,
    ) -> Result<()> {
        let mut mail_stats = mail::MailStats {
            mails_routed: 0,
            lease_conflicts: 0,
        };
        mail::handle_lease_directive(
            &self.store,
            mission_id,
            supervisor_id,
            session_id,
            directive,
            active_sessions,
            leases,
            &mut mail_stats,
        )?;
        stats.lease_conflicts = mail_stats.mails_routed;
        Ok(())
    }

    pub(super) fn handle_pending_mail(
        &self,
        mission_id: Uuid,
        supervisor_id: Uuid,
        active_sessions: &mut HashMap<Uuid, ActiveSession>,
        pending_mail: &mut HashMap<Uuid, PendingMail>,
    ) -> Result<()> {
        let now = Instant::now();
        let mut expired = Vec::new();
        for (message_id, pending) in pending_mail.iter_mut() {
            if pending.acked && pending.thread_state == "closed" {
                expired.push(*message_id);
                continue;
            }
            if pending.acked || !mail::mail_timeout_stage_due(pending, now) {
                continue;
            }
            let recipient_is_live = active_sessions
                .get(&pending.recipient_session_id)
                .is_some_and(|session| live_state::should_pause_mail_timeout(session, now));
            let sender_is_live = active_sessions
                .get(&pending.sender_session_id)
                .is_some_and(|session| live_state::should_pause_mail_timeout(session, now));
            if recipient_is_live || sender_is_live {
                continue;
            }
            pending.timeout_stage = pending.timeout_stage.saturating_add(1);
            pending.last_timeout_at = Some(now);
            pending.thread_state =
                coordination::thread_state_for_timeout_stage(pending.timeout_stage).to_owned();
            let stage = pending.timeout_stage;
            let timeout = mail::mail_timeout_interval(&pending.priority);
            let sender_name = active_sessions
                .get(&pending.sender_session_id)
                .map(|session| session.record.name.clone())
                .unwrap_or_else(|| "unknown".to_owned());
            let recipient_name = active_sessions
                .get(&pending.recipient_session_id)
                .map(|session| session.record.name.clone())
                .unwrap_or_else(|| "unknown".to_owned());
            if communication_policy::MAIL_TIMEOUT_DIRECT_PROMPTS {
                if let Some(recipient) = active_sessions.get_mut(&pending.recipient_session_id) {
                    let adapter = adapter_for(recipient.record.agent);
                    let prompt = mail::recipient_timeout_prompt(pending, &sender_name, stage);
                    let _ = send_or_queue_prompt(recipient, &adapter.build_status_prompt(&prompt));
                }
                if let Some(sender) = active_sessions.get_mut(&pending.sender_session_id) {
                    let prompt = mail::sender_timeout_prompt(pending, &recipient_name, stage);
                    let _ = send_or_queue_prompt(sender, &prompt);
                }
                for cc_id in &pending.cc_session_ids {
                    if let Some(cc_session) = active_sessions.get_mut(cc_id) {
                        let _ = send_or_queue_prompt(
                            cc_session,
                            &mail::cc_timeout_prompt(pending, stage),
                        );
                    }
                }
            }
            let urgency_note = match pending.priority.to_lowercase().as_str() {
                "urgent" | "critical" => " [URGENT]",
                "high" => " [HIGH]",
                _ => "",
            };
            let should_escalate_supervisor = stage >= 2
                || matches!(
                    pending.priority.to_lowercase().as_str(),
                    "urgent" | "critical" | "high"
                );
            let mail_event_type = if stage >= 3 || urgency_note.contains("URGENT") {
                SupervisorEventType::Blocked
            } else {
                SupervisorEventType::Notice
            };
            if should_escalate_supervisor {
                self.send_supervisor_notice(
                    mission_id,
                    supervisor_id,
                    active_sessions,
                    mail_event_type,
                    &format!(
                        "Mail {} in thread {} from {} to {} is still unresolved after timeout stage {} ({}s each).{} Subject: {}. The team may need reroute, override, or direct intervention.",
                        pending.message_id,
                        pending.thread_id,
                        sender_name,
                        recipient_name,
                        stage,
                        timeout.as_secs(),
                        urgency_note,
                        pending.subject
                    ),
                )?;
            }
            let _ = self.store.update_message_status(
                *message_id,
                &format!("timeout_stage_{stage}"),
                "pending",
            );
        }
        for message_id in expired {
            pending_mail.remove(&message_id);
        }
        Ok(())
    }
}

#[allow(dead_code)]
pub(super) fn render_routed_mail_enhanced(
    message_id: Uuid,
    thread_id: &str,
    sender: &str,
    directive: &MailDirective,
    cc_recipients: &[Uuid],
    requires_ack: bool,
    is_urgent: bool,
) -> String {
    let urgency_banner = if is_urgent {
        "═══════════════════════════════════════════\n⚡  URGENT MAIL — IMMEDIATE ATTENTION REQUIRED  ⚡\n═══════════════════════════════════════════\n\n"
    } else {
        ""
    };
    let thread_info = if directive.reply_to.is_some() {
        format!("REPLY in thread {}\n", thread_id)
    } else {
        format!("NEW THREAD {}\n", thread_id)
    };
    let cc_line = if cc_recipients.is_empty() {
        String::new()
    } else {
        format!(
            "CC: {} recipients (visibility only, no action required)\n\n",
            cc_recipients.len()
        )
    };
    let ack_instruction = if requires_ack {
        if is_urgent {
            format!(
                "⚡ URGENT: Acknowledge IMMEDIATELY with:\nSAPPHIRE_ACK {{\"mail_id\":\"{}\",\"status\":\"acked\",\"summary\":\"acknowledged urgent\"}}\nThen respond with your action plan.",
                message_id
            )
        } else {
            format!(
                "Acknowledge within the timeout window with:\nSAPPHIRE_ACK {{\"mail_id\":\"{}\",\"status\":\"acked\",\"summary\":\"one short sentence\"}}\nThen respond with SAPPHIRE_MAIL or SAPPHIRE_STATUS when your coordination state changes.",
                message_id
            )
        }
    } else {
        "No ack required. Process when ready and update SAPPHIRE_STATUS when state changes."
            .to_owned()
    };
    format!(
        "{urgency_banner}[SAPPHIRE ROUTER — Enhanced]\n{thread_info}MAIL_ID: {message_id}\nFROM: {sender}\nTO: {to}\nTYPE: {message_type}\nPRIORITY: {priority}\nSUBJECT: {subject}\n{cc_line}CONTEXT:\n{context}\n\nREQUEST:\n{request}\n\nEXPECTED ACTION:\n{expected_action}\n\n{ack_instruction}",
        to = directive.to,
        message_type = directive.message_type,
        priority = directive.priority,
        subject = directive.subject,
        context = directive.context,
        request = directive.request,
        expected_action = directive.expected_action,
    )
}
