use std::collections::HashMap;

use anyhow::Result;
use chrono::Utc;
use serde::Serialize;
use uuid::Uuid;

use super::{ActiveSession, ControlSurface, PendingMail, coordination, write_string_to_file};

#[derive(Debug, Clone)]
pub struct MemorySummary {
    pub display_name: String,
    pub pod: String,
    pub active_threads: usize,
}

#[derive(Serialize)]
struct MemoryThread {
    thread_id: String,
    counterpart: String,
    direction: &'static str,
    intent: String,
    state: String,
    subject: String,
    priority: String,
}

#[derive(Serialize)]
struct AgentMemoryArtifact {
    display_name: String,
    role_type: String,
    pod: String,
    owned_scope: String,
    current_state: String,
    current_summary: String,
    preferred_counterparts: Vec<String>,
    active_threads: Vec<MemoryThread>,
    recent_risks: Vec<String>,
    last_updated: String,
}

pub fn write_agent_memories(
    control_surface: &ControlSurface,
    active_sessions: &HashMap<Uuid, ActiveSession>,
    pending_mail: &HashMap<Uuid, PendingMail>,
) -> Result<Vec<MemorySummary>> {
    let mut summaries = Vec::new();

    for session in active_sessions.values() {
        if session.record.role != crate::model::SessionRole::Worker {
            continue;
        }

        let role_type = coordination::session_role_type(session).to_owned();
        let pod = coordination::pod_for_role(&role_type).to_owned();
        let active_threads = pending_mail
            .values()
            .filter(|pending| {
                pending.thread_state != "closed"
                    && (pending.sender_session_id == session.record.id
                        || pending.recipient_session_id == session.record.id)
            })
            .map(|pending| MemoryThread {
                thread_id: pending.thread_id.clone(),
                counterpart: if pending.sender_session_id == session.record.id {
                    active_sessions
                        .get(&pending.recipient_session_id)
                        .map(|other| other.record.name.clone())
                        .unwrap_or_else(|| pending.recipient_session_id.to_string())
                } else {
                    active_sessions
                        .get(&pending.sender_session_id)
                        .map(|other| other.record.name.clone())
                        .unwrap_or_else(|| pending.sender_session_id.to_string())
                },
                direction: if pending.sender_session_id == session.record.id {
                    "outbound"
                } else {
                    "inbound"
                },
                intent: pending.intent.clone(),
                state: pending.thread_state.clone(),
                subject: pending.subject.clone(),
                priority: pending.priority.clone(),
            })
            .take(8)
            .collect::<Vec<_>>();
        let recent_risks = session
            .last_risks
            .iter()
            .take(8)
            .cloned()
            .collect::<Vec<_>>();
        let artifact = AgentMemoryArtifact {
            display_name: session.record.name.clone(),
            role_type: role_type.clone(),
            pod: pod.clone(),
            owned_scope: session.record.owned_scope.clone(),
            current_state: session.state.as_str().to_owned(),
            current_summary: session
                .record
                .last_summary
                .clone()
                .unwrap_or_else(|| "no summary yet".to_owned()),
            preferred_counterparts: coordination::preferred_counterparts(&role_type)
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
            active_threads,
            recent_risks,
            last_updated: Utc::now().to_rfc3339(),
        };

        let rendered = serde_json::to_string_pretty(&artifact)?;
        let primary = control_surface
            .workers_state_dir
            .join(&session.record.name)
            .join("memory.json");
        write_string_to_file(&primary, &rendered)?;

        summaries.push(MemorySummary {
            display_name: session.record.name.clone(),
            pod,
            active_threads: artifact.active_threads.len(),
        });
    }

    summaries.sort_by(|left, right| left.display_name.cmp(&right.display_name));
    Ok(summaries)
}
