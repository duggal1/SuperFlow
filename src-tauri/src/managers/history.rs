use anyhow::{anyhow, Result};
use chrono::{DateTime, Local, Utc};
use log::{debug, error, info};
use rusqlite::{params, Connection, OptionalExtension};
use rusqlite_migration::{Migrations, M};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::fs;
use std::path::PathBuf;
use tauri::AppHandle;
use tauri_specta::Event;

/// Database migrations for transcription history.
/// Each migration is applied in order. The library tracks which migrations
/// have been applied using SQLite's user_version pragma.
///
/// Note: For users upgrading from tauri-plugin-sql, migrate_from_tauri_plugin_sql()
/// converts the old _sqlx_migrations table tracking to the user_version pragma,
/// ensuring migrations don't re-run on existing databases.
static MIGRATIONS: &[M] = &[
    M::up(
        "CREATE TABLE IF NOT EXISTS transcription_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            file_name TEXT NOT NULL,
            timestamp INTEGER NOT NULL,
            saved BOOLEAN NOT NULL DEFAULT 0,
            title TEXT NOT NULL,
            transcription_text TEXT NOT NULL
        );",
    ),
    M::up("ALTER TABLE transcription_history ADD COLUMN post_processed_text TEXT;"),
    M::up("ALTER TABLE transcription_history ADD COLUMN post_process_prompt TEXT;"),
    M::up("ALTER TABLE transcription_history ADD COLUMN post_process_requested BOOLEAN NOT NULL DEFAULT 0;"),
    M::up("ALTER TABLE transcription_history ADD COLUMN word_count INTEGER NOT NULL DEFAULT 0;"),
    M::up("ALTER TABLE transcription_history ADD COLUMN audio_duration_secs REAL;"),
    M::up("ALTER TABLE transcription_history ADD COLUMN avg_wpm REAL;"),
    M::up("ALTER TABLE transcription_history ADD COLUMN time_saved_secs REAL;"),
    M::up(
        "CREATE TABLE IF NOT EXISTS ai_cleanup_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp INTEGER NOT NULL,
            source TEXT NOT NULL,
            input_text TEXT NOT NULL,
            output_text TEXT NOT NULL,
            model TEXT NOT NULL,
            thinking_level TEXT NOT NULL
        );",
    ),
];

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct AiCleanupHistoryEntry {
    pub id: i64,
    pub timestamp: i64,
    pub source: String,
    pub input_text: String,
    pub output_text: String,
    pub model: String,
    pub thinking_level: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct PaginatedHistory {
    pub entries: Vec<HistoryEntry>,
    pub has_more: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type, tauri_specta::Event)]
#[serde(tag = "action")]
pub enum HistoryUpdatePayload {
    #[serde(rename = "added")]
    Added { entry: HistoryEntry },
    #[serde(rename = "updated")]
    Updated { entry: HistoryEntry },
    #[serde(rename = "deleted")]
    Deleted { id: i64 },
    #[serde(rename = "toggled")]
    Toggled { id: i64 },
}

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct HistoryEntry {
    pub id: i64,
    pub file_name: String,
    pub timestamp: i64,
    pub saved: bool,
    pub title: String,
    pub transcription_text: String,
    pub post_processed_text: Option<String>,
    pub post_process_prompt: Option<String>,
    pub post_process_requested: bool,
    /// Words in the final (post-processed when present) text. Persisted so
    /// stats never depend on re-parsing text in the UI.
    pub word_count: i64,
    /// Real recorded audio length in seconds, captured at save time from the
    /// sample count. `None` for legacy rows.
    pub audio_duration_secs: Option<f64>,
    /// Speaking speed in words/minute derived at save time. `None` when no
    /// duration or no words.
    pub avg_wpm: Option<f64>,
    /// Seconds saved versus typing the same words at TYPING_WPM (40). `None`
    /// when no duration or no words.
    pub time_saved_secs: Option<f64>,
}

/// Baseline typing speed for "time saved" math — mirrors TYPING_WPM in
/// src/lib/utils/journalStats.ts.
const TYPING_WPM: f64 = 40.0;

/// Derive the persisted per-entry stats from the final text and the real audio
/// duration. Shared by insert and retry-update paths so both store identical
/// math.
fn compute_entry_stats(
    text: &str,
    audio_duration_secs: Option<f64>,
) -> (i64, Option<f64>, Option<f64>) {
    let word_count = text.trim().split_whitespace().count() as i64;
    match audio_duration_secs {
        Some(duration) if duration > 0.0 && word_count > 0 => {
            let avg_wpm = word_count as f64 / (duration / 60.0);
            let time_saved_secs = ((word_count as f64 / TYPING_WPM) * 60.0 - duration).max(0.0);
            (word_count, Some(avg_wpm), Some(time_saved_secs))
        }
        _ => (word_count, None, None),
    }
}

pub struct HistoryManager {
    app_handle: AppHandle,
    recordings_dir: PathBuf,
    db_path: PathBuf,
}

impl HistoryManager {
    pub fn new(app_handle: &AppHandle) -> Result<Self> {
        // Create recordings directory in app data dir
        let app_data_dir = crate::portable::app_data_dir(app_handle)?;
        let recordings_dir = app_data_dir.join("recordings");
        let db_path = app_data_dir.join("history.db");

        // Ensure recordings directory exists
        if !recordings_dir.exists() {
            fs::create_dir_all(&recordings_dir)?;
            debug!("Created recordings directory: {:?}", recordings_dir);
        }

        let manager = Self {
            app_handle: app_handle.clone(),
            recordings_dir,
            db_path,
        };

        // Initialize database and run migrations synchronously
        manager.init_database()?;

        Ok(manager)
    }

    fn init_database(&self) -> Result<()> {
        info!("Initializing database at {:?}", self.db_path);

        let mut conn = Connection::open(&self.db_path)?;

        // Handle migration from tauri-plugin-sql to rusqlite_migration
        // tauri-plugin-sql used _sqlx_migrations table, rusqlite_migration uses user_version pragma
        self.migrate_from_tauri_plugin_sql(&conn)?;

        // Create migrations object and run to latest version
        let migrations = Migrations::new(MIGRATIONS.to_vec());

        // Validate migrations in debug builds
        #[cfg(debug_assertions)]
        migrations.validate().expect("Invalid migrations");

        // Get current version before migration
        let version_before: i32 =
            conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
        debug!("Database version before migration: {}", version_before);

        // Apply any pending migrations
        migrations.to_latest(&mut conn)?;

        // Get version after migration
        let version_after: i32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;

        if version_after > version_before {
            info!(
                "Database migrated from version {} to {}",
                version_before, version_after
            );
        } else {
            debug!("Database already at latest version {}", version_after);
        }

        // Databases upgraded from tauri-plugin-sql carry that system's version
        // count in user_version, which can mark stats-column migrations as
        // applied even though the columns are missing. Reconcile so history
        // queries never fail after an upgrade.
        Self::ensure_stats_columns(&conn)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS ai_cleanup_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp INTEGER NOT NULL,
                source TEXT NOT NULL,
                input_text TEXT NOT NULL,
                output_text TEXT NOT NULL,
                model TEXT NOT NULL,
                thinking_level TEXT NOT NULL
            );",
        )?;

        Ok(())
    }

    /// Column definitions every transcription_history table must have for the
    /// current queries to work: name paired with its ADD COLUMN definition.
    const REQUIRED_COLUMNS: &[(&str, &str)] = &[
        ("word_count", "INTEGER NOT NULL DEFAULT 0"),
        ("audio_duration_secs", "REAL"),
        ("avg_wpm", "REAL"),
        ("time_saved_secs", "REAL"),
    ];

    /// Add any required column missing from transcription_history.
    /// Idempotent: columns already present are left untouched.
    fn ensure_stats_columns(conn: &Connection) -> Result<()> {
        let mut existing: Vec<String> = Vec::new();
        {
            let mut stmt = conn.prepare("PRAGMA table_info(transcription_history)")?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
            for name in rows {
                existing.push(name?);
            }
        }

        for (name, definition) in Self::REQUIRED_COLUMNS {
            if !existing.iter().any(|column| column == name) {
                conn.execute(
                    &format!("ALTER TABLE transcription_history ADD COLUMN {name} {definition}"),
                    [],
                )?;
                info!("Added missing history column: {name}");
            }
        }

        Ok(())
    }

    /// Migrate from tauri-plugin-sql's migration tracking to rusqlite_migration's.
    /// tauri-plugin-sql used a _sqlx_migrations table, while rusqlite_migration uses
    /// SQLite's user_version pragma. This function checks if the old system was in use
    /// and sets the user_version accordingly so migrations don't re-run.
    fn migrate_from_tauri_plugin_sql(&self, conn: &Connection) -> Result<()> {
        // Check if the old _sqlx_migrations table exists
        let has_sqlx_migrations: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='_sqlx_migrations'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if !has_sqlx_migrations {
            return Ok(());
        }

        // Check current user_version
        let current_version: i32 =
            conn.pragma_query_value(None, "user_version", |row| row.get(0))?;

        if current_version > 0 {
            // Already migrated to rusqlite_migration system
            return Ok(());
        }

        // Get the highest version from the old migrations table
        let old_version: i32 = conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations WHERE success = 1",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if old_version > 0 {
            info!(
                "Migrating from tauri-plugin-sql (version {}) to rusqlite_migration",
                old_version
            );

            // Set user_version to match the old migration state
            conn.pragma_update(None, "user_version", old_version)?;

            // Optionally drop the old migrations table (keeping it doesn't hurt)
            // conn.execute("DROP TABLE IF EXISTS _sqlx_migrations", [])?;

            info!(
                "Migration tracking converted: user_version set to {}",
                old_version
            );
        }

        Ok(())
    }

    fn get_connection(&self) -> Result<Connection> {
        Ok(Connection::open(&self.db_path)?)
    }

    pub fn save_ai_cleanup(
        &self,
        source: &str,
        input_text: &str,
        output_text: &str,
        model: &str,
        thinking_level: &str,
    ) -> Result<AiCleanupHistoryEntry> {
        let timestamp = Utc::now().timestamp();
        let conn = self.get_connection()?;
        conn.execute(
            "INSERT INTO ai_cleanup_history (
                timestamp, source, input_text, output_text, model, thinking_level
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                timestamp,
                source,
                input_text,
                output_text,
                model,
                thinking_level
            ],
        )?;
        Ok(AiCleanupHistoryEntry {
            id: conn.last_insert_rowid(),
            timestamp,
            source: source.to_string(),
            input_text: input_text.to_string(),
            output_text: output_text.to_string(),
            model: model.to_string(),
            thinking_level: thinking_level.to_string(),
        })
    }

    pub fn get_ai_cleanup_history(&self, limit: usize) -> Result<Vec<AiCleanupHistoryEntry>> {
        let conn = self.get_connection()?;
        let mut statement = conn.prepare(
            "SELECT id, timestamp, source, input_text, output_text, model, thinking_level
             FROM ai_cleanup_history ORDER BY id DESC LIMIT ?1",
        )?;
        let entries = statement
            .query_map(params![limit.clamp(1, 100) as i64], |row| {
                Ok(AiCleanupHistoryEntry {
                    id: row.get("id")?,
                    timestamp: row.get("timestamp")?,
                    source: row.get("source")?,
                    input_text: row.get("input_text")?,
                    output_text: row.get("output_text")?,
                    model: row.get("model")?,
                    thinking_level: row.get("thinking_level")?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(entries)
    }

    fn map_history_entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<HistoryEntry> {
        Ok(HistoryEntry {
            id: row.get("id")?,
            file_name: row.get("file_name")?,
            timestamp: row.get("timestamp")?,
            saved: row.get("saved")?,
            title: row.get("title")?,
            transcription_text: row.get("transcription_text")?,
            post_processed_text: row.get("post_processed_text")?,
            post_process_prompt: row.get("post_process_prompt")?,
            post_process_requested: row.get("post_process_requested")?,
            word_count: row.get("word_count")?,
            audio_duration_secs: row.get("audio_duration_secs")?,
            avg_wpm: row.get("avg_wpm")?,
            time_saved_secs: row.get("time_saved_secs")?,
        })
    }

    pub fn recordings_dir(&self) -> &std::path::Path {
        &self.recordings_dir
    }

    /// Save a new history entry to the database.
    /// The WAV file should already have been written to the recordings directory.
    /// `audio_duration_secs` is the real recorded length (sample count / 16 kHz),
    /// persisted alongside derived WPM / time-saved stats so the home-page stats
    /// never depend on re-reading audio files in the UI.
    pub fn save_entry(
        &self,
        file_name: String,
        transcription_text: String,
        post_process_requested: bool,
        post_processed_text: Option<String>,
        post_process_prompt: Option<String>,
        audio_duration_secs: f64,
    ) -> Result<HistoryEntry> {
        let timestamp = Utc::now().timestamp();
        self.save_entry_at(
            file_name,
            transcription_text,
            post_process_requested,
            post_processed_text,
            post_process_prompt,
            audio_duration_secs,
            timestamp,
        )
    }

    /// [`Self::save_entry`] with an explicit recording timestamp — used by
    /// crash recovery so a restored dictation lands on the day it actually
    /// happened, not the day it was recovered.
    pub fn save_entry_at(
        &self,
        file_name: String,
        transcription_text: String,
        post_process_requested: bool,
        post_processed_text: Option<String>,
        post_process_prompt: Option<String>,
        audio_duration_secs: f64,
        timestamp: i64,
    ) -> Result<HistoryEntry> {
        let title = self.format_timestamp_title(timestamp);

        // Stats key off the final text (post-processed when present) — that is
        // what the user actually produced.
        let final_text = post_processed_text
            .as_deref()
            .unwrap_or(&transcription_text);
        let duration_secs = Some(audio_duration_secs).filter(|d| *d > 0.0);
        let (word_count, avg_wpm, time_saved_secs) = compute_entry_stats(final_text, duration_secs);

        let conn = self.get_connection()?;
        conn.execute(
            "INSERT INTO transcription_history (
                file_name,
                timestamp,
                saved,
                title,
                transcription_text,
                post_processed_text,
                post_process_prompt,
                post_process_requested,
                word_count,
                audio_duration_secs,
                avg_wpm,
                time_saved_secs
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                &file_name,
                timestamp,
                false,
                &title,
                &transcription_text,
                &post_processed_text,
                &post_process_prompt,
                post_process_requested,
                word_count,
                duration_secs,
                avg_wpm,
                time_saved_secs,
            ],
        )?;

        let entry = HistoryEntry {
            id: conn.last_insert_rowid(),
            file_name,
            timestamp,
            saved: false,
            title,
            transcription_text,
            post_processed_text,
            post_process_prompt,
            post_process_requested,
            word_count,
            audio_duration_secs: duration_secs,
            avg_wpm,
            time_saved_secs,
        };

        debug!("Saved history entry with id {}", entry.id);

        self.cleanup_old_entries()?;

        // Emit typed event for real-time frontend updates
        if let Err(e) = (HistoryUpdatePayload::Added {
            entry: entry.clone(),
        })
        .emit(&self.app_handle)
        {
            error!("Failed to emit history-updated event: {}", e);
        }

        Ok(entry)
    }

    /// Update an existing history entry with new transcription results (used by retry).
    pub fn update_transcription(
        &self,
        id: i64,
        transcription_text: String,
        post_processed_text: Option<String>,
        post_process_prompt: Option<String>,
    ) -> Result<HistoryEntry> {
        let conn = self.get_connection()?;

        // Recompute the derived stats from the stored duration so a retried
        // entry's WPM / time-saved reflect its new text.
        let duration_secs: Option<f64> = conn
            .query_row(
                "SELECT audio_duration_secs FROM transcription_history WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .optional()?;
        let final_text = post_processed_text
            .as_deref()
            .unwrap_or(&transcription_text);
        let (word_count, avg_wpm, time_saved_secs) = compute_entry_stats(final_text, duration_secs);

        let updated = conn.execute(
            "UPDATE transcription_history
             SET transcription_text = ?1,
                 post_processed_text = ?2,
                 post_process_prompt = ?3,
                 word_count = ?4,
                 avg_wpm = ?5,
                 time_saved_secs = ?6
             WHERE id = ?7",
            params![
                transcription_text,
                post_processed_text,
                post_process_prompt,
                word_count,
                avg_wpm,
                time_saved_secs,
                id
            ],
        )?;

        if updated == 0 {
            return Err(anyhow!("History entry {} not found", id));
        }

        let entry = conn
            .query_row(
                "SELECT id, file_name, timestamp, saved, title, transcription_text, post_processed_text, post_process_prompt, post_process_requested,
                 word_count, audio_duration_secs, avg_wpm, time_saved_secs
                 FROM transcription_history WHERE id = ?1",
                params![id],
                Self::map_history_entry,
            )?;

        debug!("Updated transcription for history entry {}", id);

        if let Err(e) = (HistoryUpdatePayload::Updated {
            entry: entry.clone(),
        })
        .emit(&self.app_handle)
        {
            error!("Failed to emit history-updated event: {}", e);
        }

        Ok(entry)
    }

    pub fn cleanup_old_entries(&self) -> Result<()> {
        let retention_period = crate::settings::get_recording_retention_period(&self.app_handle);

        match retention_period {
            crate::settings::RecordingRetentionPeriod::Never => {
                // Don't delete anything
                Ok(())
            }
            crate::settings::RecordingRetentionPeriod::PreserveLimit => {
                // Use the old count-based logic with history_limit
                let limit = crate::settings::get_history_limit(&self.app_handle);
                self.cleanup_by_count(limit)
            }
            _ => {
                // Use time-based logic
                self.cleanup_by_time(retention_period)
            }
        }
    }

    fn delete_entries_and_files(&self, entries: &[(i64, String)]) -> Result<usize> {
        if entries.is_empty() {
            return Ok(0);
        }

        let conn = self.get_connection()?;
        let mut deleted_count = 0;

        for (id, file_name) in entries {
            // Delete database entry
            conn.execute(
                "DELETE FROM transcription_history WHERE id = ?1",
                params![id],
            )?;

            // Delete WAV file
            let file_path = self.recordings_dir.join(file_name);
            if file_path.exists() {
                if let Err(e) = fs::remove_file(&file_path) {
                    error!("Failed to delete WAV file {}: {}", file_name, e);
                } else {
                    debug!("Deleted old WAV file: {}", file_name);
                    deleted_count += 1;
                }
            }
        }

        Ok(deleted_count)
    }

    fn cleanup_by_count(&self, limit: usize) -> Result<()> {
        let conn = self.get_connection()?;

        // Get all entries that are not saved, ordered by timestamp desc
        let mut stmt = conn.prepare(
            "SELECT id, file_name FROM transcription_history WHERE saved = 0 ORDER BY timestamp DESC"
        )?;

        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, i64>("id")?, row.get::<_, String>("file_name")?))
        })?;

        let mut entries: Vec<(i64, String)> = Vec::new();
        for row in rows {
            entries.push(row?);
        }

        if entries.len() > limit {
            let entries_to_delete = &entries[limit..];
            let deleted_count = self.delete_entries_and_files(entries_to_delete)?;

            if deleted_count > 0 {
                debug!("Cleaned up {} old history entries by count", deleted_count);
            }
        }

        Ok(())
    }

    fn cleanup_by_time(
        &self,
        retention_period: crate::settings::RecordingRetentionPeriod,
    ) -> Result<()> {
        let conn = self.get_connection()?;

        // Calculate cutoff timestamp (current time minus retention period)
        let now = Utc::now().timestamp();
        let cutoff_timestamp = match retention_period {
            crate::settings::RecordingRetentionPeriod::Days3 => now - (3 * 24 * 60 * 60), // 3 days in seconds
            crate::settings::RecordingRetentionPeriod::Weeks2 => now - (2 * 7 * 24 * 60 * 60), // 2 weeks in seconds
            crate::settings::RecordingRetentionPeriod::Months3 => now - (3 * 30 * 24 * 60 * 60), // 3 months in seconds (approximate)
            _ => unreachable!("Should not reach here"),
        };

        // Get all unsaved entries older than the cutoff timestamp
        let mut stmt = conn.prepare(
            "SELECT id, file_name FROM transcription_history WHERE saved = 0 AND timestamp < ?1",
        )?;

        let rows = stmt.query_map(params![cutoff_timestamp], |row| {
            Ok((row.get::<_, i64>("id")?, row.get::<_, String>("file_name")?))
        })?;

        let mut entries_to_delete: Vec<(i64, String)> = Vec::new();
        for row in rows {
            entries_to_delete.push(row?);
        }

        let deleted_count = self.delete_entries_and_files(&entries_to_delete)?;

        if deleted_count > 0 {
            debug!(
                "Cleaned up {} old history entries based on retention period",
                deleted_count
            );
        }

        Ok(())
    }

    pub async fn get_history_entries(
        &self,
        cursor: Option<i64>,
        limit: Option<usize>,
    ) -> Result<PaginatedHistory> {
        let conn = self.get_connection()?;
        let limit = limit.map(|l| l.min(100));

        let mut entries: Vec<HistoryEntry> = match (cursor, limit) {
            (Some(cursor_id), Some(lim)) => {
                let fetch_count = (lim + 1) as i64;
                let mut stmt = conn.prepare(
                    "SELECT id, file_name, timestamp, saved, title, transcription_text, post_processed_text, post_process_prompt, post_process_requested,
                     word_count, audio_duration_secs, avg_wpm, time_saved_secs
                     FROM transcription_history
                     WHERE id < ?1
                     ORDER BY id DESC
                     LIMIT ?2",
                )?;
                let result = stmt
                    .query_map(params![cursor_id, fetch_count], Self::map_history_entry)?
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                result
            }
            (None, Some(lim)) => {
                let fetch_count = (lim + 1) as i64;
                let mut stmt = conn.prepare(
                    "SELECT id, file_name, timestamp, saved, title, transcription_text, post_processed_text, post_process_prompt, post_process_requested,
                     word_count, audio_duration_secs, avg_wpm, time_saved_secs
                     FROM transcription_history
                     ORDER BY id DESC
                     LIMIT ?1",
                )?;
                let result = stmt
                    .query_map(params![fetch_count], Self::map_history_entry)?
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                result
            }
            (_, None) => {
                let mut stmt = conn.prepare(
                    "SELECT id, file_name, timestamp, saved, title, transcription_text, post_processed_text, post_process_prompt, post_process_requested,
                     word_count, audio_duration_secs, avg_wpm, time_saved_secs
                     FROM transcription_history
                     ORDER BY id DESC",
                )?;
                let result = stmt
                    .query_map([], Self::map_history_entry)?
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                result
            }
        };

        let has_more = limit.is_some_and(|lim| entries.len() > lim);
        if has_more {
            entries.pop();
        }

        Ok(PaginatedHistory { entries, has_more })
    }

    /// Every history entry, oldest first — the order an export document reads
    /// in. Includes empty (failed) rows; the export layer filters those.
    pub fn get_all_entries(&self) -> Result<Vec<HistoryEntry>> {
        let conn = self.get_connection()?;
        Self::get_all_entries_with_conn(&conn)
    }

    fn get_all_entries_with_conn(conn: &Connection) -> Result<Vec<HistoryEntry>> {
        let mut stmt = conn.prepare(
            "SELECT id, file_name, timestamp, saved, title, transcription_text, post_processed_text, post_process_prompt, post_process_requested,
             word_count, audio_duration_secs, avg_wpm, time_saved_secs
             FROM transcription_history
             ORDER BY id ASC",
        )?;
        let entries = stmt
            .query_map([], Self::map_history_entry)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(entries)
    }

    #[cfg(test)]
    fn get_latest_entry_with_conn(conn: &Connection) -> Result<Option<HistoryEntry>> {
        let mut stmt = conn.prepare(
            "SELECT
                id,
                file_name,
                timestamp,
                saved,
                title,
                transcription_text,
                post_processed_text,
                post_process_prompt,
                post_process_requested,
                word_count,
                audio_duration_secs,
                avg_wpm,
                time_saved_secs
             FROM transcription_history
             ORDER BY timestamp DESC
             LIMIT 1",
        )?;

        let entry = stmt.query_row([], Self::map_history_entry).optional()?;
        Ok(entry)
    }

    /// Get the latest entry with non-empty transcription text.
    pub fn get_latest_completed_entry(&self) -> Result<Option<HistoryEntry>> {
        let conn = self.get_connection()?;
        Self::get_latest_completed_entry_with_conn(&conn)
    }

    fn get_latest_completed_entry_with_conn(conn: &Connection) -> Result<Option<HistoryEntry>> {
        let mut stmt = conn.prepare(
            "SELECT
                id,
                file_name,
                timestamp,
                saved,
                title,
                transcription_text,
                post_processed_text,
                post_process_prompt,
                post_process_requested,
                word_count,
                audio_duration_secs,
                avg_wpm,
                time_saved_secs
             FROM transcription_history
             WHERE transcription_text != ''
             ORDER BY timestamp DESC
             LIMIT 1",
        )?;

        let entry = stmt.query_row([], Self::map_history_entry).optional()?;
        Ok(entry)
    }

    pub async fn toggle_saved_status(&self, id: i64) -> Result<()> {
        let conn = self.get_connection()?;

        // Get current saved status
        let current_saved: bool = conn.query_row(
            "SELECT saved FROM transcription_history WHERE id = ?1",
            params![id],
            |row| row.get("saved"),
        )?;

        let new_saved = !current_saved;

        conn.execute(
            "UPDATE transcription_history SET saved = ?1 WHERE id = ?2",
            params![new_saved, id],
        )?;

        debug!("Toggled saved status for entry {}: {}", id, new_saved);

        // Emit history updated event
        if let Err(e) = (HistoryUpdatePayload::Toggled { id }).emit(&self.app_handle) {
            error!("Failed to emit history-updated event: {}", e);
        }

        Ok(())
    }

    pub fn get_audio_file_path(&self, file_name: &str) -> PathBuf {
        self.recordings_dir.join(file_name)
    }

    pub async fn get_entry_by_id(&self, id: i64) -> Result<Option<HistoryEntry>> {
        let conn = self.get_connection()?;
        Self::get_entry_by_id_with_conn(&conn, id)
    }

    fn get_entry_by_id_with_conn(conn: &Connection, id: i64) -> Result<Option<HistoryEntry>> {
        let mut stmt = conn.prepare(
            "SELECT
                id,
                file_name,
                timestamp,
                saved,
                title,
                transcription_text,
                post_processed_text,
                post_process_prompt,
                post_process_requested,
                word_count,
                audio_duration_secs,
                avg_wpm,
                time_saved_secs
             FROM transcription_history
             WHERE id = ?1",
        )?;

        let entry = stmt.query_row([id], Self::map_history_entry).optional()?;

        Ok(entry)
    }

    pub async fn delete_entry(&self, id: i64) -> Result<()> {
        let conn = self.get_connection()?;

        // Get the entry to find the file name
        if let Some(entry) = self.get_entry_by_id(id).await? {
            // Delete the audio file first
            let file_path = self.get_audio_file_path(&entry.file_name);
            if file_path.exists() {
                if let Err(e) = fs::remove_file(&file_path) {
                    error!("Failed to delete audio file {}: {}", entry.file_name, e);
                    // Continue with database deletion even if file deletion fails
                }
            }
        }

        // Delete from database
        conn.execute(
            "DELETE FROM transcription_history WHERE id = ?1",
            params![id],
        )?;

        debug!("Deleted history entry with id: {}", id);

        // Emit history updated event
        if let Err(e) = (HistoryUpdatePayload::Deleted { id }).emit(&self.app_handle) {
            error!("Failed to emit history-updated event: {}", e);
        }

        Ok(())
    }

    fn format_timestamp_title(&self, timestamp: i64) -> String {
        if let Some(utc_datetime) = DateTime::from_timestamp(timestamp, 0) {
            // Convert UTC to local timezone
            let local_datetime = utc_datetime.with_timezone(&Local);
            local_datetime.format("%B %e, %Y - %l:%M%p").to_string()
        } else {
            format!("Recording {}", timestamp)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::{params, Connection};

    fn setup_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        conn.execute_batch(
            "CREATE TABLE transcription_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                file_name TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                saved BOOLEAN NOT NULL DEFAULT 0,
                title TEXT NOT NULL,
                transcription_text TEXT NOT NULL,
                post_processed_text TEXT,
                post_process_prompt TEXT,
                post_process_requested BOOLEAN NOT NULL DEFAULT 0,
                word_count INTEGER NOT NULL DEFAULT 0,
                audio_duration_secs REAL,
                avg_wpm REAL,
                time_saved_secs REAL
            );",
        )
        .expect("create transcription_history table");
        conn
    }

    fn setup_legacy_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("open legacy in-memory db");
        conn.execute_batch(
            "CREATE TABLE transcription_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                file_name TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                saved BOOLEAN NOT NULL DEFAULT 0,
                title TEXT NOT NULL,
                transcription_text TEXT NOT NULL,
                post_processed_text TEXT,
                post_process_prompt TEXT,
                post_process_requested BOOLEAN NOT NULL DEFAULT 0
            );",
        )
        .expect("create legacy transcription_history table");
        conn
    }

    fn insert_entry(conn: &Connection, timestamp: i64, text: &str, post_processed: Option<&str>) {
        conn.execute(
            "INSERT INTO transcription_history (
                file_name,
                timestamp,
                saved,
                title,
                transcription_text,
                post_processed_text,
                post_process_prompt,
                post_process_requested
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                format!("superflow-{}.wav", timestamp),
                timestamp,
                false,
                format!("Recording {}", timestamp),
                text,
                post_processed,
                Option::<String>::None,
                false,
            ],
        )
        .expect("insert history entry");
    }

    #[test]
    fn get_latest_entry_returns_none_when_empty() {
        let conn = setup_conn();
        let entry = HistoryManager::get_latest_entry_with_conn(&conn).expect("fetch latest entry");
        assert!(entry.is_none());
    }

    #[test]
    fn get_latest_entry_returns_newest_entry() {
        let conn = setup_conn();
        insert_entry(&conn, 100, "first", None);
        insert_entry(&conn, 200, "second", Some("processed"));

        let entry = HistoryManager::get_latest_entry_with_conn(&conn)
            .expect("fetch latest entry")
            .expect("entry exists");

        assert_eq!(entry.timestamp, 200);
        assert_eq!(entry.transcription_text, "second");
        assert_eq!(entry.post_processed_text.as_deref(), Some("processed"));
    }

    #[test]
    fn get_latest_completed_entry_skips_empty_entries() {
        let conn = setup_conn();
        insert_entry(&conn, 100, "completed", None);
        insert_entry(&conn, 200, "", None);

        let entry = HistoryManager::get_latest_completed_entry_with_conn(&conn)
            .expect("fetch latest completed entry")
            .expect("completed entry exists");

        assert_eq!(entry.timestamp, 100);
        assert_eq!(entry.transcription_text, "completed");
    }

    /// Reproduce the tauri-plugin-sql upgrade desync: user_version copied from
    /// the old system marks the stats migrations as applied while the columns
    /// are absent. ensure_stats_columns must heal the schema so history
    /// queries succeed instead of emptying the journal on every launch.
    #[test]
    fn ensure_stats_columns_heals_upgraded_database() {
        let conn = setup_legacy_conn();

        // Simulate the upgraded database: version claims migrations applied.
        conn.pragma_update(None, "user_version", 6)
            .expect("set user_version");

        HistoryManager::ensure_stats_columns(&conn).expect("reconcile columns");

        // All required columns now exist and a full stats select works.
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM transcription_history
                 WHERE word_count >= 0
                   AND audio_duration_secs IS NULL
                   AND avg_wpm IS NULL
                   AND time_saved_secs IS NULL",
                [],
                |row| row.get(0),
            )
            .expect("stats select succeeds");
        assert_eq!(count, 0);

        // Idempotent: running again must not fail on duplicate columns.
        HistoryManager::ensure_stats_columns(&conn).expect("second reconcile");
    }

    #[test]
    fn ensure_stats_columns_keeps_existing_values() {
        let conn = setup_conn();
        HistoryManager::ensure_stats_columns(&conn).expect("add columns");
        conn.execute(
            "INSERT INTO transcription_history (
                file_name, timestamp, saved, title, transcription_text,
                post_processed_text, post_process_prompt, post_process_requested,
                word_count, audio_duration_secs, avg_wpm, time_saved_secs
             ) VALUES ('a.wav', 1, 0, 't', 'hello world', NULL, NULL, 0, 2, 4.0, 30.0, 1.0)",
            [],
        )
        .expect("insert with stats");

        HistoryManager::ensure_stats_columns(&conn).expect("reconcile is a no-op");

        let words: i64 = conn
            .query_row(
                "SELECT word_count FROM transcription_history WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .expect("read persisted word_count");
        assert_eq!(words, 2);
    }

    #[test]
    fn get_entry_by_id_reads_all_stats_columns() {
        let conn = setup_conn();
        conn.execute(
            "INSERT INTO transcription_history (
                file_name, timestamp, saved, title, transcription_text,
                post_processed_text, post_process_prompt, post_process_requested,
                word_count, audio_duration_secs, avg_wpm, time_saved_secs
             ) VALUES ('a.wav', 1, 0, 't', 'hello world', NULL, NULL, 0, 2, 4.0, 30.0, 1.0)",
            [],
        )
        .expect("insert with stats");

        let entry = HistoryManager::get_entry_by_id_with_conn(&conn, 1)
            .expect("read entry")
            .expect("entry exists");
        assert_eq!(entry.word_count, 2);
        assert_eq!(entry.audio_duration_secs, Some(4.0));
        assert_eq!(entry.avg_wpm, Some(30.0));
        assert_eq!(entry.time_saved_secs, Some(1.0));
    }

    #[test]
    fn get_all_entries_returns_chronological_order() {
        let conn = setup_conn();
        insert_entry(&conn, 100, "first", None);
        insert_entry(&conn, 200, "second", None);
        insert_entry(&conn, 300, "", None);

        let entries = HistoryManager::get_all_entries_with_conn(&conn).expect("fetch all");

        let ids: Vec<i64> = entries.iter().map(|e| e.id).collect();
        assert_eq!(ids, vec![1, 2, 3]);
        assert_eq!(entries[0].transcription_text, "first");
    }

    #[test]
    fn failed_entry_ids_returns_only_empty_transcriptions_newest_first() {
        let conn = setup_conn();
        insert_entry(&conn, 100, "completed", None);
        insert_entry(&conn, 200, "   ", None); // whitespace-only counts as failed
        insert_entry(&conn, 300, "", None);

        let mut stmt = conn
            .prepare("SELECT id FROM transcription_history WHERE TRIM(transcription_text) = '' ORDER BY id DESC")
            .expect("prepare");
        let ids = stmt
            .query_map([], |row| row.get::<_, i64>(0))
            .expect("query")
            .collect::<Result<Vec<i64>, rusqlite::Error>>()
            .expect("collect");

        assert_eq!(ids, vec![3, 2]);
    }
}
