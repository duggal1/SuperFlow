use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;
use tauri::AppHandle;

// Reuse HistoryManager's DB file and portable path logic
fn db_path(app: &AppHandle) -> Result<PathBuf> {
    let dir = crate::portable::app_data_dir(app).map_err(|e| anyhow!("app data dir: {e}"))?;
    Ok(dir.join("history.db"))
}

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct MeetingSegment {
    pub speaker: String, // "You" | "Speaker 1" | "Speaker 2"
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct MeetingRecord {
    pub id: String,
    pub title: String,
    pub started_at: i64, // unix ms
    pub ended_at: i64,
    pub duration_ms: i64,
    pub transcript: Vec<MeetingSegment>,
    pub created_at: i64,
    pub intelligence: Option<MeetingIntelligence>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct MeetingListEntry {
    pub id: String,
    pub title: String,
    pub started_at: i64,
    pub duration_ms: i64,
    pub has_intelligence: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct MeetingIntelligence {
    pub meeting_type: String,
    pub outcome: String,
    #[serde(default)]
    pub what_went_well: Vec<IntelligenceItem>,
    #[serde(default)]
    pub mistakes: Vec<IntelligenceItem>,
    #[serde(default)]
    pub missed_opportunities: Vec<IntelligenceItem>,
    #[serde(default)]
    pub communication_issues: Vec<IntelligenceItem>,
    #[serde(default)]
    pub important_decisions: Vec<String>,
    #[serde(default)]
    pub action_items: Vec<String>,
    #[serde(default)]
    pub risks: Vec<String>,
    #[serde(default)]
    pub lessons: Vec<String>,
    #[serde(default)]
    pub next_time: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct IntelligenceItem {
    pub issue: String,
    pub timestamp: Option<String>,
    pub evidence: Option<String>,
    pub why_it_matters: Option<String>,
    pub better_approach: Option<String>,
}

pub struct MeetingManager {
    db_path: PathBuf,
}

impl MeetingManager {
    pub fn new(app: &AppHandle) -> Result<Self> {
        let path = db_path(app)?;
        let conn = Connection::open(&path)?;
        Self::migrate(&conn)?;
        Ok(Self { db_path: path })
    }

    fn migrate(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS meetings (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                started_at INTEGER NOT NULL,
                ended_at INTEGER NOT NULL,
                duration_ms INTEGER NOT NULL,
                transcript_json TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                intelligence_json TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_meetings_started_at ON meetings(started_at DESC);
            "#,
        )?;
        Ok(())
    }

    fn conn(&self) -> Result<Connection> {
        let conn = Connection::open(&self.db_path)?;
        Self::migrate(&conn)?;
        Ok(conn)
    }

    pub fn save_meeting(&self, record: &MeetingRecord) -> Result<()> {
        let conn = self.conn()?;
        let transcript_json = serde_json::to_string(&record.transcript)?;
        let intel_param = record
            .intelligence
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;
        conn.execute(
            "INSERT OR REPLACE INTO meetings (id, title, started_at, ended_at, duration_ms, transcript_json, created_at, intelligence_json) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![record.id, record.title, record.started_at, record.ended_at, record.duration_ms, transcript_json, record.created_at, intel_param],
        )?;
        Ok(())
    }

    pub fn get_meeting(&self, id: &str) -> Result<Option<MeetingRecord>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare("SELECT id, title, started_at, ended_at, duration_ms, transcript_json, created_at, intelligence_json FROM meetings WHERE id = ?1")?;
        let row = stmt
            .query_row(params![id], |row| {
                let transcript_json: String = row.get(5)?;
                let intel_json: Option<String> = row.get(7)?;
                Ok(MeetingRecord {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    started_at: row.get(2)?,
                    ended_at: row.get(3)?,
                    duration_ms: row.get(4)?,
                    transcript: serde_json::from_str(&transcript_json).unwrap_or_default(),
                    created_at: row.get(6)?,
                    intelligence: intel_json.and_then(|s| serde_json::from_str(&s).ok()),
                })
            })
            .optional()?;
        Ok(row)
    }

    pub fn list_meetings(&self, limit: usize, offset: usize) -> Result<Vec<MeetingListEntry>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare("SELECT id, title, started_at, duration_ms, intelligence_json FROM meetings ORDER BY started_at DESC LIMIT ?1 OFFSET ?2")?;
        let rows = stmt.query_map(params![limit as i64, offset as i64], |row| {
            let intel: Option<String> = row.get(4)?;
            Ok(MeetingListEntry {
                id: row.get(0)?,
                title: row.get(1)?,
                started_at: row.get(2)?,
                duration_ms: row.get(3)?,
                has_intelligence: intel.is_some(),
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn delete_meeting(&self, id: &str) -> Result<()> {
        let conn = self.conn()?;
        conn.execute("DELETE FROM meetings WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn save_intelligence(&self, id: &str, intel: &MeetingIntelligence) -> Result<()> {
        let conn = self.conn()?;
        let json = serde_json::to_string(intel)?;
        conn.execute(
            "UPDATE meetings SET intelligence_json = ?1 WHERE id = ?2",
            params![json, id],
        )?;
        Ok(())
    }

    pub fn export_markdown(&self, id: &str) -> Result<String> {
        let rec = self
            .get_meeting(id)?
            .ok_or_else(|| anyhow!("meeting not found"))?;
        let started =
            DateTime::<Utc>::from_timestamp_millis(rec.started_at).unwrap_or_else(|| Utc::now());
        let duration = format_duration(rec.duration_ms);
        let mut md = format!(
            "# {}\n\nDate: {}\nDuration: {}\n\n## Transcript\n\n",
            rec.title,
            started.format("%B %d, %Y — %I:%M %p"),
            duration
        );
        for seg in &rec.transcript {
            let ts = format_ms(seg.start_ms);
            md.push_str(&format!("### {} — {}\n\n{}\n\n", seg.speaker, ts, seg.text));
        }
        if let Some(intel) = rec.intelligence {
            md.push_str("\n## Meeting Intelligence\n\n");
            md.push_str(&format!(
                "**Type:** {}\n\n**Outcome:** {}\n\n",
                intel.meeting_type, intel.outcome
            ));
            // Add other sections if needed
        }
        Ok(md)
    }
}

fn format_duration(ms: i64) -> String {
    let total_secs = ms / 1000;
    let h = total_secs / 3600;
    let m = (total_secs % 3600) / 60;
    let s = total_secs % 60;
    if h > 0 {
        format!("{:02}:{:02}:{:02}", h, m, s)
    } else {
        format!("{:02}:{:02}", m, s)
    }
}

fn format_ms(ms: i64) -> String {
    let total_secs = ms / 1000;
    let h = total_secs / 3600;
    let m = (total_secs % 3600) / 60;
    let s = total_secs % 60;
    if h > 0 {
        format!("{:02}:{:02}:{:02}", h, m, s)
    } else {
        format!("{:02}:{:02}", m, s)
    }
}
