use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Per-agent memory files.
///
/// Format: `.sp/memory/<mission_id>/<display_name>.json`
/// Each agent has a memory file that persists across the session.
/// Read on resume to inject context: "Last session, you worked on X..."
pub struct AgentMemoryStore {
    base_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMemory {
    pub mission_id: Uuid,
    pub display_name: String,
    pub role_type: String,
    pub owned_scope: Vec<String>,
    pub decisions: Vec<String>,
    pub blockers: Vec<String>,
    pub learnings: Vec<String>,
    pub files_touched: Vec<String>,
    pub final_state: String,
    pub summary: String,
    #[serde(default = "chrono::Utc::now")]
    pub created_at: DateTime<Utc>,
}

impl AgentMemoryStore {
    pub fn open(base_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&base_dir)
            .with_context(|| format!("failed to create memory dir {}", base_dir.display()))?;
        Ok(Self { base_dir })
    }

    pub fn mission_dir(&self, mission_id: &Uuid) -> PathBuf {
        self.base_dir.join(mission_id.to_string())
    }

    pub fn memory_file(&self, mission_id: &Uuid, display_name: &str) -> PathBuf {
        self.mission_dir(mission_id)
            .join(format!("{}.json", display_name))
    }

    pub fn save_memory(
        &self,
        mission_id: &Uuid,
        display_name: &str,
        memory: &AgentMemory,
    ) -> Result<()> {
        let dir = self.mission_dir(mission_id);
        std::fs::create_dir_all(&dir)?;

        let file = self.memory_file(mission_id, display_name);
        let mut f = fs::File::create(&file)?;
        serde_json::to_writer_pretty(&mut f, memory)?;
        f.flush()?;
        Ok(())
    }

    pub fn load_memory(
        &self,
        mission_id: &Uuid,
        display_name: &str,
    ) -> Result<Option<AgentMemory>> {
        let file = self.memory_file(mission_id, display_name);
        if !file.exists() {
            return Ok(None);
        }
        let f = fs::File::open(&file)?;
        let memory: AgentMemory = serde_json::from_reader(f)?;
        Ok(Some(memory))
    }

    pub fn list_agents(&self, mission_id: &Uuid) -> Result<Vec<String>> {
        let dir = self.mission_dir(mission_id);
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut names = Vec::new();
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            if entry.file_type().map_or(false, |ft| ft.is_file()) {
                let name = entry
                    .file_name()
                    .to_string_lossy()
                    .strip_suffix(".json")
                    .map(String::from)
                    .unwrap_or_default();
                if !name.is_empty() {
                    names.push(name);
                }
            }
        }
        Ok(names)
    }

    pub fn update_state(&self, mission_id: &Uuid, display_name: &str, state: &str) -> Result<()> {
        if let Some(mut memory) = self.load_memory(mission_id, display_name)? {
            memory.final_state = state.to_owned();
            self.save_memory(mission_id, display_name, &memory)
        } else {
            Ok(())
        }
    }

    pub fn append_files_touched(
        &self,
        mission_id: &Uuid,
        display_name: &str,
        files: &[String],
    ) -> Result<()> {
        if let Some(mut memory) = self.load_memory(mission_id, display_name)? {
            for f in files {
                if !memory.files_touched.contains(f) {
                    memory.files_touched.push(f.clone());
                }
            }
            self.save_memory(mission_id, display_name, &memory)
        } else {
            Ok(())
        }
    }

    pub fn format_for_resume(
        &self,
        mission_id: &Uuid,
        display_name: &str,
    ) -> Result<Option<String>> {
        match self.load_memory(mission_id, display_name)? {
            Some(m) => Ok(Some(format!(
                "PREVIOUS SESSION MEMORY ({display_name}):\n\
                 - Role: {role}\n\
                 - Owned scope: {scope}\n\
                 - Files touched: {files}\n\
                 - Decisions: {decisions}\n\
                 - Blockers: {blockers}\n\
                 - Learnings: {learnings}\n\
                 - Final state: {state}\n\
                 - Summary: {summary}",
                role = m.role_type,
                scope = m.owned_scope.join(", "),
                files = m.files_touched.join(", "),
                decisions = m.decisions.join("; "),
                blockers = if m.blockers.is_empty() {
                    "none".to_owned()
                } else {
                    m.blockers.join("; ")
                },
                learnings = m.learnings.join("; "),
                state = m.final_state,
                summary = m.summary,
            ))),
            None => Ok(None),
        }
    }

    /// Scan ALL missions and return chronologically sorted memory entries
    /// for a specific agent (by display_name + role_type). Returns last N entries.
    /// This is how Engineer-1 on mission B learns what Engineer-1 did on mission A.
    pub fn list_agent_history(
        &self,
        display_name: &str,
        role_type: &str,
        limit: usize,
    ) -> Result<Vec<(DateTime<Utc>, AgentMemory)>> {
        if !self.base_dir.exists() {
            return Ok(Vec::new());
        }

        let mut entries = Vec::new();

        // Each subdirectory under base_dir is a mission_id
        for entry in fs::read_dir(&self.base_dir)? {
            let entry = entry?;
            if !entry.file_type().map_or(false, |ft| ft.is_dir()) {
                continue;
            }
            let mission_id_str = entry.file_name().to_string_lossy().to_string();
            let Ok(mission_id) = Uuid::parse_str(&mission_id_str) else {
                continue;
            };

            let file = self.memory_file(&mission_id, display_name);
            if !file.exists() {
                continue;
            }

            let Ok(f) = fs::File::open(&file) else {
                continue;
            };
            let Ok(memory) = serde_json::from_reader::<_, AgentMemory>(f) else {
                continue;
            };

            if memory.role_type != role_type {
                continue;
            }

            // Use summary as a proxy for timestamp — we'll sort by mission directory order
            // which is chronological since missions are created sequentially
            entries.push((memory.created_at, memory));
        }

        entries.sort_by(|a, b| b.0.cmp(&a.0));
        entries.truncate(limit);
        Ok(entries)
    }

    /// Format recent memory entries as a concise block for prompt injection.
    /// Returns None if no history exists.
    pub fn format_history_for_injection(
        &self,
        display_name: &str,
        role_type: &str,
        limit: usize,
    ) -> Result<Option<String>> {
        let history = self.list_agent_history(display_name, role_type, limit)?;
        if history.is_empty() {
            return Ok(None);
        }

        let mut lines = Vec::new();
        lines.push(format!("PREVIOUS SESSIONS ({display_name}):"));

        for (i, (_, memory)) in history.iter().enumerate() {
            let mission_short = memory
                .mission_id
                .to_string()
                .chars()
                .take(8)
                .collect::<String>();
            lines.push(format!("\n  Session {mission_short}…:"));
            if !memory.owned_scope.is_empty() {
                lines.push(format!("    Scope: {}", memory.owned_scope.join(", ")));
            }
            if !memory.files_touched.is_empty() {
                lines.push(format!("    Files: {}", memory.files_touched.join(", ")));
            }
            if !memory.decisions.is_empty() {
                lines.push(format!("    Decisions: {}", memory.decisions.join("; ")));
            }
            if !memory.blockers.is_empty() {
                lines.push(format!("    Blockers: {}", memory.blockers.join("; ")));
            }
            if !memory.learnings.is_empty() {
                lines.push(format!("    Learnings: {}", memory.learnings.join("; ")));
            }
            lines.push(format!("    State: {state}", state = memory.final_state));
            if i == 0 && !memory.summary.is_empty() {
                lines.push(format!("    Summary: {summary}", summary = memory.summary));
            }
        }

        Ok(Some(lines.join("\n")))
    }
}
