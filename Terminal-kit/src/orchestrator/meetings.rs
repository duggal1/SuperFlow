use std::collections::{BTreeSet, HashMap};
use std::fs;

use anyhow::Result;
use blake3::hash;
use chrono::Utc;
use serde::Serialize;
use uuid::Uuid;

use super::{ActiveSession, ControlSurface, PendingMail, coordination, write_string_to_file};

#[derive(Debug, Clone, Serialize)]
pub struct MeetingArtifact {
    pub id: String,
    pub kind: String,
    pub status: String,
    pub reason: String,
    pub participants: Vec<String>,
    pub pod: String,
    pub trigger_thread_id: Option<String>,
    pub agenda: Vec<String>,
    pub created_at: String,
    pub last_updated_at: String,
}

pub fn build_meetings(
    active_sessions: &HashMap<Uuid, ActiveSession>,
    pending_mail: &HashMap<Uuid, PendingMail>,
) -> Vec<MeetingArtifact> {
    let now = Utc::now().to_rfc3339();
    let mut meetings = Vec::new();

    for pending in pending_mail.values() {
        if pending.thread_state != "needs_reroute"
            && pending.thread_state != "coordination_failure"
            && pending.thread_state != "rerouted"
        {
            continue;
        }

        let sender = active_sessions.get(&pending.sender_session_id);
        let recipient = active_sessions.get(&pending.recipient_session_id);
        let mut participants = BTreeSet::new();
        if let Some(sender) = sender {
            participants.insert(sender.record.name.clone());
        }
        if let Some(recipient) = recipient {
            participants.insert(recipient.record.name.clone());
        }
        for cc_id in pending.cc_session_ids.iter().take(2) {
            if let Some(cc) = active_sessions.get(cc_id) {
                participants.insert(cc.record.name.clone());
            }
        }

        let participants = participants.into_iter().collect::<Vec<_>>();
        let reason = format!(
            "Thread '{}' requires reroute or blocker review.",
            pending.subject
        );
        let meeting_id = hash(
            format!(
                "{}:{}:{}",
                pending.thread_id, pending.subject, pending.thread_state
            )
            .as_bytes(),
        )
        .to_hex()
        .to_string();
        meetings.push(MeetingArtifact {
            id: meeting_id,
            kind: if pending.thread_state == "coordination_failure" {
                "blocker_review".to_owned()
            } else {
                "dependency_sync".to_owned()
            },
            status: "active".to_owned(),
            reason,
            participants,
            pod: pending.sender_pod.clone(),
            trigger_thread_id: Some(pending.thread_id.clone()),
            agenda: vec![
                "confirm the blocker or unanswered dependency".to_owned(),
                "assign the next owner and narrower follow-up".to_owned(),
                "close or reroute the thread cleanly".to_owned(),
            ],
            created_at: now.clone(),
            last_updated_at: now.clone(),
        });
    }

    for session in active_sessions.values() {
        if !matches!(
            session.state,
            crate::model::SessionState::Contradictory
                | crate::model::SessionState::Blocked
                | crate::model::SessionState::WrongDirection
        ) {
            continue;
        }
        let id =
            hash(format!("session:{}:{}", session.record.id, session.state.as_str()).as_bytes())
                .to_hex()
                .to_string();
        meetings.push(MeetingArtifact {
            id,
            kind: "worker_recovery".to_owned(),
            status: "active".to_owned(),
            reason: format!(
                "{} is {} and needs a recovery decision.",
                session.record.name,
                session.state.as_str()
            ),
            participants: vec![session.record.name.clone()],
            pod: coordination::pod_for_role(coordination::session_role_type(session)).to_owned(),
            trigger_thread_id: None,
            agenda: vec![
                "confirm the exact blocker or contradiction".to_owned(),
                "decide reroute, retry, or validation path".to_owned(),
            ],
            created_at: now.clone(),
            last_updated_at: now.clone(),
        });
    }

    meetings.sort_by(|left, right| left.id.cmp(&right.id));
    meetings.dedup_by(|left, right| left.id == right.id);
    meetings
}

pub fn write_meeting_artifacts(
    control_surface: &ControlSurface,
    active_sessions: &HashMap<Uuid, ActiveSession>,
    pending_mail: &HashMap<Uuid, PendingMail>,
) -> Result<Vec<MeetingArtifact>> {
    let meetings = build_meetings(active_sessions, pending_mail);
    let visible_dir = control_surface
        .status_file
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join("meetings");

    fs::create_dir_all(&visible_dir)?;

    let current_ids = meetings
        .iter()
        .map(|meeting| format!("{}.json", meeting.id))
        .collect::<BTreeSet<_>>();

    if let Ok(entries) = fs::read_dir(&visible_dir) {
        for entry in entries.flatten() {
            let file_name = entry.file_name().to_string_lossy().into_owned();
            if file_name == "meetings.json" {
                continue;
            }
            if !current_ids.contains(&file_name) {
                let _ = fs::remove_file(entry.path());
            }
        }
    }

    let index = serde_json::to_string_pretty(&meetings)?;
    write_string_to_file(&visible_dir.join("meetings.json"), &index)?;
    for meeting in &meetings {
        let rendered = serde_json::to_string_pretty(meeting)?;
        write_string_to_file(&visible_dir.join(format!("{}.json", meeting.id)), &rendered)?;
    }

    Ok(meetings)
}
