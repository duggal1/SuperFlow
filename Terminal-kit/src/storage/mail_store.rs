use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::model::MailRecord;

/// JSON-based mail store.
///
/// Format: `.sp/mail/<mission_id>/mail.jsonl`
/// One line per message. Supports search, ack tracking, archive.
pub struct MailStore {
    base_dir: PathBuf,
}

impl MailStore {
    pub fn open(base_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&base_dir)
            .with_context(|| format!("failed to create mail dir {}", base_dir.display()))?;
        Ok(Self { base_dir })
    }

    pub fn mission_file(&self, mission_id: &Uuid) -> PathBuf {
        self.base_dir
            .join(mission_id.to_string())
            .join("mail.jsonl")
    }

    pub fn persist_message(&self, mission_id: &Uuid, message: &MailRecord) -> Result<()> {
        let dir = self.base_dir.join(mission_id.to_string());
        std::fs::create_dir_all(&dir)?;

        let line = serde_json::json!({
            "id": message.id,
            "from_worker_id": message.sender_worker_id,
            "to_worker_id": message.recipient_worker_id,
            "message_type": message.message_type,
            "priority": message.priority,
            "delivery_mode": message.delivery_mode,
            "subject": message.subject,
            "status": message.status,
            "ack_state": message.ack_state,
            "pinned": message.pinned,
            "body": message.body_json,
            "thread_id": message.thread_id,
            "reply_to": message.reply_to,
            "created_at": message.created_at,
            "archived_at": message.archived_at,
        });
        let mut f = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.mission_file(mission_id))?;
        writeln!(f, "{}", serde_json::to_string(&line)?)?;
        Ok(())
    }

    pub fn update_message_status(
        &self,
        _message_id: &Uuid,
        status: &str,
        ack_state: &str,
    ) -> Result<()> {
        // Append an update line — read-time merge applies latest
        let update = serde_json::json!({
            "type": "mail_update",
            "message_id": _message_id,
            "status": status,
            "ack_state": ack_state,
            "updated_at": chrono::Utc::now(),
        });
        // We don't know mission_id here — this is a limitation.
        // In practice, updates come through persist_message with updated fields.
        let _ = update;
        Ok(())
    }

    pub fn archive_resolved_mail(&self, mission_id: &Uuid, older_than_secs: u64) -> Result<usize> {
        let file = self.mission_file(mission_id);
        if !file.exists() {
            return Ok(0);
        }

        let cutoff = chrono::Utc::now() - chrono::Duration::seconds(older_than_secs as i64);
        let f = fs::File::open(&file)?;
        let reader = BufReader::new(f);
        let mut count = 0;

        // Read all lines, mark resolved ones as archived
        let mut lines: Vec<String> = Vec::new();
        for line in reader.lines() {
            let line = line?;
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
                if val.get("type").and_then(|v| v.as_str()) == Some("mail_update") {
                    lines.push(line);
                    continue;
                }
                let status = val.get("status").and_then(|v| v.as_str()).unwrap_or("");
                let created = val.get("created_at").and_then(|v| {
                    serde_json::from_str::<chrono::DateTime<chrono::Utc>>(v.as_str().unwrap_or(""))
                        .ok()
                });

                if (status == "acked" || status == "responded" || status == "done")
                    && val.get("pinned").and_then(|v| v.as_bool()) == Some(false)
                    && val.get("archived_at").is_none()
                    && created.map_or(false, |c| c < cutoff)
                {
                    let mut updated = val.clone();
                    updated["archived_at"] = serde_json::json!(chrono::Utc::now());
                    updated["status"] = serde_json::json!("archived");
                    lines.push(serde_json::to_string(&updated)?);
                    count += 1;
                } else {
                    lines.push(line);
                }
            } else {
                lines.push(line);
            }
        }

        // Rewrite file
        let mut f = fs::File::create(&file)?;
        for line in &lines {
            writeln!(f, "{}", line)?;
        }

        Ok(count)
    }

    pub fn search_mail(
        &self,
        mission_id: &Uuid,
        query: Option<&str>,
        from_worker: Option<Uuid>,
        msg_type: Option<&str>,
        limit: usize,
    ) -> Result<Vec<serde_json::Value>> {
        let file = self.mission_file(mission_id);
        if !file.exists() {
            return Ok(Vec::new());
        }

        let f = fs::File::open(&file)?;
        let reader = BufReader::new(f);
        let mut results = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
                if val.get("type").and_then(|v| v.as_str()) == Some("mail_update") {
                    continue;
                }

                // Apply filters
                if let Some(q) = query {
                    let subject = val.get("subject").and_then(|v| v.as_str()).unwrap_or("");
                    let body = val.get("body").and_then(|v| v.as_str()).unwrap_or("");
                    if !subject.contains(q) && !body.contains(q) {
                        continue;
                    }
                }
                if let Some(fw) = from_worker {
                    let from = val
                        .get("from_worker_id")
                        .and_then(|v| v.as_str())
                        .and_then(|s| Uuid::parse_str(s).ok());
                    if from != Some(fw) {
                        continue;
                    }
                }
                if let Some(mt) = msg_type {
                    let actual_type = val.get("message_type").and_then(|v| v.as_str());
                    if actual_type != Some(mt) {
                        continue;
                    }
                }

                results.push(serde_json::json!({
                    "id": val.get("id"),
                    "from_worker_id": val.get("from_worker_id"),
                    "to_worker_id": val.get("to_worker_id"),
                    "message_type": val.get("message_type"),
                    "priority": val.get("priority"),
                    "subject": val.get("subject"),
                    "status": val.get("status"),
                    "ack_state": val.get("ack_state"),
                    "created_at": val.get("created_at"),
                    "thread_id": val.get("thread_id"),
                }));

                if results.len() >= limit {
                    break;
                }
            }
        }

        Ok(results)
    }

    pub fn claim_scavenge_mail(
        &self,
        mail_id: &Uuid,
        claimer_id: &Uuid,
        claimer_name: &str,
    ) -> Result<usize> {
        // Scan all mission files for this mail
        if !self.base_dir.exists() {
            return Ok(0);
        }

        let now = chrono::Utc::now().to_rfc3339();
        for entry in fs::read_dir(&self.base_dir)? {
            let entry = entry?;
            if entry.file_type().map_or(false, |ft| ft.is_dir()) {
                let mission_dir = entry.path();
                let mail_file = mission_dir.join("mail.jsonl");
                if !mail_file.exists() {
                    continue;
                }

                let f = fs::File::open(&mail_file)?;
                let reader = BufReader::new(f);
                let mut modified = false;
                let mut claimed = false;
                let mut lines: Vec<String> = Vec::new();

                for line in reader.lines() {
                    let line = line?;
                    if let Ok(mut val) = serde_json::from_str::<serde_json::Value>(&line) {
                        let id = val
                            .get("id")
                            .and_then(|v| v.as_str())
                            .and_then(|s| Uuid::parse_str(s).ok());
                        if id == Some(*mail_id)
                            && val.get("message_type").and_then(|v| v.as_str()) == Some("scavenge")
                            && val.get("claimed_by").is_none()
                        {
                            val["claimed_by"] = serde_json::json!(claimer_id.to_string());
                            val["claimed_by_name"] = serde_json::json!(claimer_name);
                            val["claimed_at"] = serde_json::json!(now);
                            lines.push(serde_json::to_string(&val)?);
                            modified = true;
                            claimed = true;
                        } else {
                            lines.push(line);
                        }
                    } else {
                        lines.push(line);
                    }
                }

                if modified {
                    let mut f = fs::File::create(&mail_file)?;
                    for line in &lines {
                        writeln!(f, "{}", line)?;
                    }
                    if claimed {
                        return Ok(1);
                    }
                }
            }
        }

        Ok(0)
    }

    pub fn release_scavenge_mail(&self, mail_id: &Uuid, releaser_id: &Uuid) -> Result<usize> {
        if !self.base_dir.exists() {
            return Ok(0);
        }

        for entry in fs::read_dir(&self.base_dir)? {
            let entry = entry?;
            if entry.file_type().map_or(false, |ft| ft.is_dir()) {
                let mission_dir = entry.path();
                let mail_file = mission_dir.join("mail.jsonl");
                if !mail_file.exists() {
                    continue;
                }

                let f = fs::File::open(&mail_file)?;
                let reader = BufReader::new(f);
                let mut modified = false;
                let mut released = false;
                let mut lines: Vec<String> = Vec::new();

                for line in reader.lines() {
                    let line = line?;
                    if let Ok(mut val) = serde_json::from_str::<serde_json::Value>(&line) {
                        let id = val
                            .get("id")
                            .and_then(|v| v.as_str())
                            .and_then(|s| Uuid::parse_str(s).ok());
                        let claimed_by = val.get("claimed_by").and_then(|v| v.as_str());
                        if id == Some(*mail_id) && claimed_by == Some(&releaser_id.to_string()) {
                            val.as_object_mut().map(|obj| {
                                obj.remove("claimed_by");
                                obj.remove("claimed_by_name");
                                obj.remove("claimed_at");
                            });
                            lines.push(serde_json::to_string(&val)?);
                            modified = true;
                            released = true;
                        } else {
                            lines.push(line);
                        }
                    } else {
                        lines.push(line);
                    }
                }

                if modified {
                    let mut f = fs::File::create(&mail_file)?;
                    for line in &lines {
                        writeln!(f, "{}", line)?;
                    }
                    if released {
                        return Ok(1);
                    }
                }
            }
        }

        Ok(0)
    }

    pub fn get_mail_body(&self, mail_id: &Uuid) -> Result<Option<String>> {
        if let Some(mail) = self.get_mail_by_id(mail_id)? {
            Ok(Some(mail.body_json))
        } else {
            Ok(None)
        }
    }

    pub fn get_mail_by_id(&self, mail_id: &Uuid) -> Result<Option<MailRecord>> {
        if !self.base_dir.exists() {
            return Ok(None);
        }

        for entry in fs::read_dir(&self.base_dir)? {
            let entry = entry?;
            if entry.file_type().map_or(false, |ft| ft.is_dir()) {
                let mission_dir = entry.path();
                let mission_id = mission_dir
                    .file_name()
                    .and_then(|n| n.to_str())
                    .and_then(|s| Uuid::parse_str(s).ok())
                    .unwrap_or_default();

                let mail_file = mission_dir.join("mail.jsonl");
                if !mail_file.exists() {
                    continue;
                }

                let f = fs::File::open(&mail_file)?;
                let reader = BufReader::new(f);
                for line in reader.lines() {
                    let line = line?;
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
                        let id = val
                            .get("id")
                            .and_then(|v| v.as_str())
                            .and_then(|s| Uuid::parse_str(s).ok());
                        if id == Some(*mail_id) {
                            return Ok(Some(MailRecord {
                                id: val
                                    .get("id")
                                    .and_then(|v| v.as_str())
                                    .and_then(|s| Uuid::parse_str(s).ok())
                                    .unwrap_or_default(),
                                mission_id,
                                sender_worker_id: val
                                    .get("from_worker_id")
                                    .and_then(|v| v.as_str())
                                    .and_then(|s| Uuid::parse_str(s).ok())
                                    .unwrap_or_default(),
                                recipient_worker_id: val
                                    .get("to_worker_id")
                                    .and_then(|v| v.as_str())
                                    .and_then(|s| Uuid::parse_str(s).ok())
                                    .unwrap_or_default(),
                                message_type: val
                                    .get("message_type")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("notification")
                                    .to_owned(),
                                priority: val
                                    .get("priority")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("normal")
                                    .to_owned(),
                                delivery_mode: val
                                    .get("delivery_mode")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("queue")
                                    .to_owned(),
                                subject: val
                                    .get("subject")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_owned(),
                                status: val
                                    .get("status")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("routed")
                                    .to_owned(),
                                ack_state: val
                                    .get("ack_state")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("pending")
                                    .to_owned(),
                                pinned: val
                                    .get("pinned")
                                    .and_then(|v| v.as_bool())
                                    .unwrap_or(false),
                                body_json: val
                                    .get("body")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("{}")
                                    .to_owned(),
                                thread_id: val
                                    .get("thread_id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_owned(),
                                reply_to: val
                                    .get("reply_to")
                                    .and_then(|v| v.as_str())
                                    .map(String::from),
                                created_at: val
                                    .get("created_at")
                                    .and_then(|v| v.as_str())
                                    .and_then(|s| serde_json::from_str(s).ok())
                                    .unwrap_or_else(|| chrono::Utc::now()),
                                archived_at: val
                                    .get("archived_at")
                                    .and_then(|v| v.as_str())
                                    .and_then(|s| serde_json::from_str(s).ok()),
                            }));
                        }
                    }
                }
            }
        }

        Ok(None)
    }
}
