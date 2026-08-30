use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::model::LeaseRecord;

/// JSON-based lease store for file ownership claims.
///
/// Format: `.sp/leases/<mission_id>/leases.jsonl`
/// Upsert semantics: latest claim for a path wins.
pub struct LeaseStore {
    base_dir: PathBuf,
}

impl LeaseStore {
    pub fn open(base_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&base_dir)
            .with_context(|| format!("failed to create lease dir {}", base_dir.display()))?;
        Ok(Self { base_dir })
    }

    pub fn mission_file(&self, mission_id: &Uuid) -> PathBuf {
        self.base_dir
            .join(mission_id.to_string())
            .join("leases.jsonl")
    }

    pub fn upsert_lease(&self, mission_id: &Uuid, lease: &LeaseRecord) -> Result<()> {
        let dir = self.base_dir.join(mission_id.to_string());
        std::fs::create_dir_all(&dir)?;

        let line = serde_json::json!({
            "path": lease.path,
            "owner_worker_id": lease.owner_session_id,
            "intent": lease.intent,
            "status": lease.status,
            "updated_at": lease.updated_at,
        });
        let mut f = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.mission_file(mission_id))?;
        writeln!(f, "{}", serde_json::to_string(&line)?)?;
        Ok(())
    }

    pub fn get_lease(&self, mission_id: &Uuid, path: &str) -> Result<Option<LeaseRecord>> {
        let file = self.mission_file(mission_id);
        if !file.exists() {
            return Ok(None);
        }

        let f = fs::File::open(&file)?;
        let reader = BufReader::new(f);
        let mut latest: Option<LeaseRecord> = None;

        for line in reader.lines() {
            let line = line?;
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
                if val.get("path").and_then(|v| v.as_str()) == Some(path) {
                    latest = Some(LeaseRecord {
                        mission_id: *mission_id,
                        path: val
                            .get("path")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_owned(),
                        owner_session_id: val
                            .get("owner_worker_id")
                            .and_then(|v| v.as_str())
                            .and_then(|s| Uuid::parse_str(s).ok())
                            .unwrap_or_default(),
                        intent: val
                            .get("intent")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_owned(),
                        status: val
                            .get("status")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_owned(),
                        updated_at: val
                            .get("updated_at")
                            .and_then(|v| v.as_str())
                            .and_then(|s| serde_json::from_str(s).ok())
                            .unwrap_or_else(|| chrono::Utc::now()),
                    });
                }
            }
        }

        Ok(latest)
    }
}
