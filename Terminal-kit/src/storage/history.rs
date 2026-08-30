use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::agent::AgentKind;
use crate::model::{
    MissionPlan, MissionRecord, MissionSnapshot, MissionStatus, ReplayEntry, SessionListItem,
    SessionRecord, SessionRole, SessionState, SummaryRecord, TaskRecord, ValidationResultRecord,
    WorkerPacket, WorkerSnapshot,
};

/// JSONL session history writer/reader.
///
/// Format: `.sp/history/<mission_id>/session.jsonl`
/// Each line is a JSON object with a `type` field:
///   - `mission`: mission record + plan
///   - `worker`: session/worker record
///   - `task`: task record
///   - `summary`: summary record
///   - `validation`: validation result
///   - `event`: significant event (not every output chunk)
pub struct SessionHistory {
    base_dir: PathBuf,
}

impl SessionHistory {
    pub fn open(base_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&base_dir)
            .with_context(|| format!("failed to create history dir {}", base_dir.display()))?;
        Ok(Self { base_dir })
    }

    pub fn mission_dir(&self, mission_id: &Uuid) -> PathBuf {
        self.base_dir.join(mission_id.to_string())
    }

    pub fn mission_file(&self, mission_id: &Uuid) -> PathBuf {
        self.mission_dir(mission_id).join("session.jsonl")
    }

    // ─── Write Operations ──────────────────────────────────────────────

    pub fn write_mission(
        &self,
        mission_id: &Uuid,
        mission: &MissionRecord,
        plan: &MissionPlan,
    ) -> Result<()> {
        let dir = self.mission_dir(mission_id);
        std::fs::create_dir_all(&dir)?;

        let line = serde_json::json!({
            "type": "mission",
            "id": mission.id,
            "created_at": mission.created_at,
            "updated_at": mission.updated_at,
            "repo_path": mission.repo_path.to_string_lossy(),
            "mission": mission.mission,
            "mission_rewrite": mission.mission_rewrite,
            "status": mission.status.as_str(),
            "final_summary": mission.final_summary,
            "plan": plan,
        });
        self.append_line(mission_id, &line)
    }

    pub fn update_mission_status(&self, mission_id: &Uuid, status: &MissionStatus) -> Result<()> {
        let line = serde_json::json!({
            "type": "mission_update",
            "field": "status",
            "value": status.as_str(),
            "created_at": chrono::Utc::now(),
        });
        self.append_line(mission_id, &line)
    }

    pub fn update_final_summary(&self, mission_id: &Uuid, summary: &str) -> Result<()> {
        let line = serde_json::json!({
            "type": "mission_update",
            "field": "final_summary",
            "value": summary,
            "created_at": chrono::Utc::now(),
        });
        self.append_line(mission_id, &line)
    }

    pub fn update_plan(&self, mission_id: &Uuid, plan: &MissionPlan) -> Result<()> {
        let line = serde_json::json!({
            "type": "mission_update",
            "field": "plan",
            "value": plan,
            "created_at": chrono::Utc::now(),
        });
        self.append_line(mission_id, &line)
    }

    pub fn write_worker(
        &self,
        mission_id: &Uuid,
        session: &SessionRecord,
        packet: Option<&WorkerPacket>,
    ) -> Result<()> {
        let line = serde_json::json!({
            "type": "worker",
            "id": session.id,
            "mission_id": session.mission_id,
            "role": crate::storage::role_name(session.role),
            "terminal_id": session.terminal_id,
            "name": session.name,
            "owned_scope": session.owned_scope,
            "status": session.status.as_str(),
            "agent": session.agent.as_str(),
            "launch_command": session.launch_command,
            "last_heartbeat_at": session.last_heartbeat_at,
            "last_summary": session.last_summary,
            "packet": packet,
            "created_at": chrono::Utc::now(),
        });
        self.append_line(mission_id, &line)
    }

    pub fn update_worker_state(&self, worker_id: &Uuid, status: &SessionState) -> Result<()> {
        // We need mission_id to find the right file — scan index or use worker tracking
        // For now, this is a no-op since state is tracked in live_state
        let _ = (worker_id, status);
        Ok(())
    }

    pub fn update_worker_summary(&self, worker_id: &Uuid, summary: &str) -> Result<()> {
        let line = serde_json::json!({
            "type": "worker_update",
            "worker_id": worker_id,
            "field": "last_summary",
            "value": summary,
            "created_at": chrono::Utc::now(),
        });
        // We don't know mission_id here — this is a limitation of the API.
        // In practice, summaries are also tracked via write_summary() which has mission_id.
        // Just log it silently — the summary is also in the worker record itself.
        let _ = (line,);
        Ok(())
    }

    pub fn write_task(&self, mission_id: &Uuid, task: &TaskRecord) -> Result<()> {
        let line = serde_json::json!({
            "type": "task",
            "id": task.id,
            "worker_id": task.worker_id,
            "title": task.title,
            "description": task.description,
            "status": task.status,
            "priority": task.priority,
            "depends_on": task.depends_on_json,
            "definition_of_done": task.definition_of_done_json,
            "created_at": chrono::Utc::now(),
        });
        self.append_line(mission_id, &line)
    }

    pub fn update_task_status(&self, task_id: &Uuid, status: &str) -> Result<()> {
        let line = serde_json::json!({
            "type": "task_update",
            "task_id": task_id,
            "field": "status",
            "value": status,
            "created_at": chrono::Utc::now(),
        });
        self.append_to_all_missions(&line)
    }

    pub fn find_task_id(&self, mission_id: &Uuid, worker_id: &Uuid) -> Result<Option<Uuid>> {
        // Read tasks from mission file, find last task for worker
        let file = self.mission_file(mission_id);
        if !file.exists() {
            return Ok(None);
        }
        let f = File::open(&file)?;
        let reader = BufReader::new(f);
        let mut last_task_id = None;
        for line in reader.lines() {
            let line = line?;
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
                if val.get("type").and_then(|v| v.as_str()) == Some("task") {
                    if val.get("worker_id").and_then(|v| v.as_str()) == Some(&worker_id.to_string())
                    {
                        if let Some(id_str) = val.get("id").and_then(|v| v.as_str()) {
                            if let Ok(id) = Uuid::parse_str(id_str) {
                                last_task_id = Some(id);
                            }
                        }
                    }
                }
            }
        }
        Ok(last_task_id)
    }

    pub fn write_summary(&self, mission_id: &Uuid, summary: &SummaryRecord) -> Result<()> {
        let line = serde_json::json!({
            "type": "summary",
            "id": summary.id,
            "worker_id": summary.worker_id,
            "summary_type": summary.summary_type,
            "content": summary.content,
            "created_at": summary.created_at,
        });
        self.append_line(mission_id, &line)
    }

    pub fn write_validation_result(
        &self,
        mission_id: &Uuid,
        result: &ValidationResultRecord,
    ) -> Result<()> {
        let line = serde_json::json!({
            "type": "validation",
            "id": result.id,
            "worker_id": result.worker_id,
            "task_id": result.task_id,
            "outcome": result.outcome,
            "summary": result.summary,
            "evidence": result.evidence_json,
            "created_at": result.created_at,
        });
        self.append_line(mission_id, &line)
    }

    pub fn append_event(&self, mission_id: &Uuid, event: &serde_json::Value) -> Result<()> {
        // Only append significant events, not every output chunk
        self.append_line(mission_id, event)
    }

    // ─── Read Operations ───────────────────────────────────────────────

    pub fn load_mission_snapshot(&self, mission_id: &Uuid) -> Result<Option<MissionSnapshot>> {
        let file = self.mission_file(mission_id);
        if !file.exists() {
            return Ok(None);
        }

        let f = File::open(&file)?;
        let reader = BufReader::new(f);

        let mut mission_record: Option<MissionRecord> = None;
        let mut plan: Option<MissionPlan> = None;

        for line in reader.lines() {
            let line = line?;
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
                match val.get("type").and_then(|v| v.as_str()) {
                    Some("mission") => {
                        if let Ok(m) = serde_json::from_value::<MissionRecordFromJson>(val.clone())
                        {
                            mission_record = Some(m.into_record());
                        }
                        if let Some(p) = val.get("plan") {
                            if let Ok(pl) = serde_json::from_value::<MissionPlan>(p.clone()) {
                                plan = Some(pl);
                            }
                        }
                    }
                    Some("mission_update") => match val.get("field").and_then(|v| v.as_str()) {
                        Some("status") => {
                            if let Some(rec) = &mut mission_record {
                                if let Some(s) = val.get("value").and_then(|v| v.as_str()) {
                                    if let Ok(st) = MissionStatus::from_str(s) {
                                        rec.status = st;
                                    }
                                }
                            }
                        }
                        Some("final_summary") => {
                            if let Some(rec) = &mut mission_record {
                                rec.final_summary =
                                    val.get("value").and_then(|v| v.as_str()).map(String::from);
                            }
                        }
                        Some("plan") => {
                            if let Some(p) = val.get("value") {
                                if let Ok(pl) = serde_json::from_value::<MissionPlan>(p.clone()) {
                                    plan = Some(pl);
                                }
                            }
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
        }

        if let (Some(mut mission), Some(plan)) = (mission_record, plan) {
            // Apply final summary from last mission_update if present
            Ok(Some(MissionSnapshot {
                id: mission.id,
                created_at: mission.created_at,
                updated_at: mission.updated_at,
                repo_path: mission.repo_path.clone(),
                user_mission_raw: mission.mission.clone(),
                mission_rewrite: mission.mission_rewrite.clone(),
                status: mission.status.as_str().to_owned(),
                final_summary: mission.final_summary.clone(),
                plan,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn load_workers(&self, mission_id: &Uuid) -> Result<Vec<WorkerSnapshot>> {
        let file = self.mission_file(mission_id);
        if !file.exists() {
            return Ok(Vec::new());
        }

        let f = File::open(&file)?;
        let reader = BufReader::new(f);
        let mut workers = Vec::new();
        let mut summaries: HashMap<Uuid, String> = HashMap::new();

        for line in reader.lines() {
            let line = line?;
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
                match val.get("type").and_then(|v| v.as_str()) {
                    Some("worker") => {
                        if let Ok(w) = parse_worker_from_json(val.clone()) {
                            workers.push(w);
                        }
                    }
                    Some("worker_update") => {
                        if val.get("field").and_then(|v| v.as_str()) == Some("last_summary") {
                            if let Some(wid_str) = val.get("worker_id").and_then(|v| v.as_str()) {
                                if let Ok(wid) = Uuid::parse_str(wid_str) {
                                    if let Some(s) = val.get("value").and_then(|v| v.as_str()) {
                                        summaries.insert(wid, s.to_owned());
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        // Apply latest summaries to workers
        for w in &mut workers {
            if let Some(summary) = summaries.get(&w.session.id) {
                w.session.last_summary = Some(summary.clone());
            }
        }

        // Sort by role then name
        workers.sort_by(|a, b| {
            let role_a = crate::storage::role_name(a.session.role);
            let role_b = crate::storage::role_name(b.session.role);
            role_a
                .cmp(role_b)
                .then_with(|| a.session.name.cmp(&b.session.name))
        });

        Ok(workers)
    }

    pub fn recent_replay_entries(
        &self,
        mission_id: &Uuid,
        limit: usize,
    ) -> Result<Vec<ReplayEntry>> {
        let file = self.mission_file(mission_id);
        if !file.exists() {
            return Ok(Vec::new());
        }

        let f = File::open(&file)?;
        let reader = BufReader::new(f);
        let mut entries = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
                let entry = match val.get("type").and_then(|v| v.as_str()) {
                    Some("event") => {
                        let worker_id = val
                            .get("worker_id")
                            .and_then(|v| v.as_str())
                            .and_then(|s| Uuid::parse_str(s).ok());
                        Some(ReplayEntry {
                            created_at: val
                                .get("created_at")
                                .and_then(|v| serde_json::from_value(v.clone()).ok())
                                .unwrap_or_else(|| chrono::Utc::now()),
                            lane: format!(
                                "event:{}",
                                worker_id
                                    .map(|id| id.to_string())
                                    .unwrap_or_else(|| "mission".to_owned())
                            ),
                            kind: val
                                .get("event_type")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown")
                                .to_owned(),
                            body: val
                                .get("body")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_owned(),
                        })
                    }
                    Some("summary") => {
                        let worker_id = val
                            .get("worker_id")
                            .and_then(|v| v.as_str())
                            .and_then(|s| Uuid::parse_str(s).ok());
                        Some(ReplayEntry {
                            created_at: val
                                .get("created_at")
                                .and_then(|v| serde_json::from_value(v.clone()).ok())
                                .unwrap_or_else(|| chrono::Utc::now()),
                            lane: format!(
                                "summary:{}",
                                worker_id
                                    .map(|id| id.to_string())
                                    .unwrap_or_else(|| "mission".to_owned())
                            ),
                            kind: val
                                .get("summary_type")
                                .and_then(|v| v.as_str())
                                .unwrap_or("summary")
                                .to_owned(),
                            body: val
                                .get("content")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_owned(),
                        })
                    }
                    _ => None,
                };
                if let Some(e) = entry {
                    entries.push(e);
                }
            }
        }

        // Reverse chronological, limit
        entries.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        entries.truncate(limit);
        Ok(entries)
    }

    pub fn recent_worker_replay(
        &self,
        mission_id: &Uuid,
        worker_id: &Uuid,
        limit: usize,
    ) -> Result<Vec<ReplayEntry>> {
        let file = self.mission_file(mission_id);
        if !file.exists() {
            return Ok(Vec::new());
        }

        let f = File::open(&file)?;
        let reader = BufReader::new(f);
        let mut entries = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
                let matches_worker = val
                    .get("worker_id")
                    .and_then(|v| v.as_str())
                    .map_or(false, |s| s == &worker_id.to_string())
                    || val
                        .get("id")
                        .and_then(|v| v.as_str())
                        .map_or(false, |s| s == &worker_id.to_string());

                if !matches_worker {
                    continue;
                }

                let entry = match val.get("type").and_then(|v| v.as_str()) {
                    Some("event") => Some(ReplayEntry {
                        created_at: val
                            .get("created_at")
                            .and_then(|v| serde_json::from_value(v.clone()).ok())
                            .unwrap_or_else(|| chrono::Utc::now()),
                        lane: "event".to_owned(),
                        kind: val
                            .get("event_type")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown")
                            .to_owned(),
                        body: val
                            .get("body")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_owned(),
                    }),
                    Some("summary") => Some(ReplayEntry {
                        created_at: val
                            .get("created_at")
                            .and_then(|v| serde_json::from_value(v.clone()).ok())
                            .unwrap_or_else(|| chrono::Utc::now()),
                        lane: "summary".to_owned(),
                        kind: val
                            .get("summary_type")
                            .and_then(|v| v.as_str())
                            .unwrap_or("summary")
                            .to_owned(),
                        body: val
                            .get("content")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_owned(),
                    }),
                    _ => None,
                };
                if let Some(e) = entry {
                    entries.push(e);
                }
            }
        }

        entries.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        entries.truncate(limit);
        Ok(entries)
    }

    pub fn latest_supervisor_summary(&self, mission_id: &Uuid) -> Result<Option<String>> {
        let file = self.mission_file(mission_id);
        if !file.exists() {
            return Ok(None);
        }

        let f = File::open(&file)?;
        let reader = BufReader::new(f);
        let mut best: Option<(usize, String)> = None;

        for (idx, line) in reader.lines().enumerate() {
            let line = line?;
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
                if val.get("type").and_then(|v| v.as_str()) == Some("summary") {
                    // Check if it's a supervisor summary (worker_id is null or supervisor-related)
                    let is_supervisor = val.get("worker_id").is_none()
                        || val
                            .get("summary_type")
                            .and_then(|v| v.as_str())
                            .map_or(false, |t| {
                                matches!(
                                    t,
                                    "final_synthesis"
                                        | "mission_complete"
                                        | "supervisor_notice"
                                        | "supervisor_action"
                                )
                            });

                    if is_supervisor {
                        if let Some(content) = val.get("content").and_then(|v| v.as_str()) {
                            let priority = match val.get("summary_type").and_then(|v| v.as_str()) {
                                Some("final_synthesis") => 0,
                                Some("mission_complete") => 1,
                                Some("supervisor_notice") => 2,
                                _ => 9,
                            };
                            if best.is_none() || priority < best.as_ref().unwrap().0 {
                                best = Some((priority, content.to_owned()));
                            }
                        }
                    }
                }
            }
        }

        Ok(best.map(|(_, content)| content))
    }

    pub fn recent_worker_summary(&self, worker_id: &Uuid) -> Result<Option<String>> {
        // Scan all mission files for this worker's last summary
        if !self.base_dir.exists() {
            return Ok(None);
        }

        let mut latest: Option<(chrono::DateTime<chrono::Utc>, String)> = None;

        for entry in std::fs::read_dir(&self.base_dir)? {
            let entry = entry?;
            if !entry.file_type().map_or(false, |ft| ft.is_dir()) {
                continue;
            }
            let mission_id_str = entry.file_name().to_string_lossy().to_string();
            if let Ok(mission_id) = Uuid::parse_str(&mission_id_str) {
                let file = self.mission_file(&mission_id);
                if !file.exists() {
                    continue;
                }
                let f = File::open(&file)?;
                let reader = BufReader::new(f);
                for line in reader.lines() {
                    let line = line?;
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
                        if val.get("type").and_then(|v| v.as_str()) == Some("summary") {
                            let wid = val
                                .get("worker_id")
                                .and_then(|v| v.as_str())
                                .and_then(|s| Uuid::parse_str(s).ok());
                            if wid == Some(*worker_id) {
                                let created = val
                                    .get("created_at")
                                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                                    .unwrap_or_else(|| chrono::DateTime::<chrono::Utc>::MIN_UTC);
                                if let Some(content) = val.get("content").and_then(|v| v.as_str()) {
                                    if latest.is_none() || created > latest.as_ref().unwrap().0 {
                                        latest = Some((created, content.to_owned()));
                                    }
                                }
                            }
                        }
                        // Also check worker updates
                        if val.get("type").and_then(|v| v.as_str()) == Some("worker_update") {
                            let wid = val
                                .get("worker_id")
                                .and_then(|v| v.as_str())
                                .and_then(|s| Uuid::parse_str(s).ok());
                            if wid == Some(*worker_id)
                                && val.get("field").and_then(|v| v.as_str()) == Some("last_summary")
                            {
                                if let Some(content) = val.get("value").and_then(|v| v.as_str()) {
                                    let created = val
                                        .get("created_at")
                                        .and_then(|v| serde_json::from_value(v.clone()).ok())
                                        .unwrap_or_else(|| {
                                            chrono::DateTime::<chrono::Utc>::MIN_UTC
                                        });
                                    if latest.is_none() || created > latest.as_ref().unwrap().0 {
                                        latest = Some((created, content.to_owned()));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(latest.map(|(_, content)| content))
    }

    // ─── Internal Helpers ──────────────────────────────────────────────

    fn append_line(&self, mission_id: &Uuid, line: &serde_json::Value) -> Result<()> {
        let dir = self.mission_dir(mission_id);
        std::fs::create_dir_all(&dir)?;
        let file = self.mission_file(mission_id);
        let mut f = OpenOptions::new().create(true).append(true).open(&file)?;
        writeln!(f, "{}", serde_json::to_string(line)?)?;
        Ok(())
    }

    fn append_to_all_missions(&self, line: &serde_json::Value) -> Result<()> {
        if !self.base_dir.exists() {
            return Ok(());
        }
        for entry in std::fs::read_dir(&self.base_dir)? {
            let entry = entry?;
            if entry.file_type().map_or(false, |ft| ft.is_dir()) {
                let mission_id_str = entry.file_name().to_string_lossy().to_string();
                if let Ok(mission_id) = Uuid::parse_str(&mission_id_str) {
                    let _ = self.append_line(&mission_id, line);
                }
            }
        }
        Ok(())
    }
}

// ─── Mission Index ─────────────────────────────────────────────────────────

/// Lightweight index for listing sessions without reading full history files.
/// Format: `.sp/history/index.jsonl` — one line per mission.
pub struct HistoryIndex {
    file: PathBuf,
}

#[derive(Debug, Serialize, Deserialize)]
struct IndexEntry {
    id: String,
    created_at: chrono::DateTime<chrono::Utc>,
    repo_path: String,
    mission: String,
    status: String,
}

impl HistoryIndex {
    pub fn open(file: PathBuf) -> Result<Self> {
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(Self { file })
    }

    pub fn upsert(
        &self,
        mission_id: &Uuid,
        repo_path: &Path,
        mission: &str,
        status: &MissionStatus,
    ) -> Result<()> {
        let entry = IndexEntry {
            id: mission_id.to_string(),
            created_at: chrono::Utc::now(),
            repo_path: repo_path.to_string_lossy().to_string(),
            mission: mission.to_owned(),
            status: status.as_str().to_owned(),
        };
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file)?;
        writeln!(f, "{}", serde_json::to_string(&entry)?)?;
        Ok(())
    }

    pub fn update_status(&self, mission_id: &Uuid, status: &MissionStatus) -> Result<()> {
        // Append a new entry — latest entry for a mission_id wins
        self.upsert(mission_id, Path::new(""), "", status)
    }

    pub fn list_all(&self) -> Result<Vec<SessionListItem>> {
        if !self.file.exists() {
            return Ok(Vec::new());
        }

        let f = File::open(&self.file)?;
        let reader = BufReader::new(f);

        // Deduplicate by mission_id, keeping latest entry
        let mut latest: HashMap<String, IndexEntry> = HashMap::new();
        for line in reader.lines() {
            let line = line?;
            if let Ok(entry) = serde_json::from_str::<IndexEntry>(&line) {
                latest.insert(entry.id.clone(), entry);
            }
        }

        let mut items: Vec<SessionListItem> = latest
            .into_values()
            .map(|e| SessionListItem {
                id: Uuid::parse_str(&e.id).unwrap_or_default(),
                created_at: e.created_at,
                updated_at: e.created_at, // Updated from latest entry timestamp
                repo_path: PathBuf::from(&e.repo_path),
                mission_rewrite: e.mission,
                status: e.status,
                final_summary: None,
            })
            .collect();

        items.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(items)
    }

    pub fn latest_for_repo(
        &self,
        repo_path: &Path,
        not_before: chrono::DateTime<chrono::Utc>,
    ) -> Result<Option<SessionListItem>> {
        let all = self.list_all()?;
        let repo_str = repo_path.to_string_lossy();
        Ok(all
            .into_iter()
            .filter(|item| {
                item.repo_path.to_string_lossy() == repo_str && item.created_at >= not_before
            })
            .next())
    }
}

// ─── JSON Parsing Helpers ──────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct MissionRecordFromJson {
    id: Uuid,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
    repo_path: String,
    mission: String,
    mission_rewrite: String,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    final_summary: Option<String>,
}

impl MissionRecordFromJson {
    fn into_record(self) -> MissionRecord {
        MissionRecord {
            id: self.id,
            created_at: self.created_at,
            updated_at: self.updated_at,
            repo_path: PathBuf::from(self.repo_path),
            mission: self.mission,
            mission_rewrite: self.mission_rewrite,
            worker_agent: AgentKind::Qwen, // Not stored in JSONL, default
            supervisor_agent: AgentKind::Qwen,
            worker_count: 0,
            status: self
                .status
                .and_then(|s| MissionStatus::from_str(&s).ok())
                .unwrap_or(MissionStatus::Running),
            final_summary: self.final_summary,
        }
    }
}

fn parse_worker_from_json(val: serde_json::Value) -> Result<WorkerSnapshot> {
    let id = Uuid::parse_str(val["id"].as_str().unwrap_or_default()).unwrap_or_default();
    let mission_id =
        Uuid::parse_str(val["mission_id"].as_str().unwrap_or_default()).unwrap_or_default();
    let role_str = val["role"].as_str().unwrap_or("worker");
    let role = crate::storage::parse_role(role_str).unwrap_or(SessionRole::Worker);
    let status_str = val["status"].as_str().unwrap_or("progressing");
    let status = SessionState::from_directive(status_str).unwrap_or(SessionState::Progressing);
    let agent_str = val["agent"].as_str().unwrap_or("qwen");
    let agent = AgentKind::from_str(agent_str).unwrap_or(AgentKind::Qwen);

    let launch_command = if let Some(arr) = val.get("launch_command").and_then(|v| v.as_array()) {
        arr.iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect()
    } else {
        vec![agent.as_str().to_owned()]
    };

    let packet = if let Some(p) = val.get("packet") {
        serde_json::from_value::<WorkerPacket>(p.clone()).ok()
    } else {
        None
    };

    Ok(WorkerSnapshot {
        session: SessionRecord {
            id,
            mission_id,
            role,
            ordinal: 0,
            agent,
            terminal_id: val["terminal_id"].as_str().unwrap_or("").to_owned(),
            name: val["name"].as_str().unwrap_or("").to_owned(),
            owned_scope: val["owned_scope"].as_str().unwrap_or("").to_owned(),
            status,
            launch_command,
            last_heartbeat_at: chrono::Utc::now(),
            last_summary: val
                .get("last_summary")
                .and_then(|v| v.as_str())
                .map(String::from),
        },
        packet,
    })
}
