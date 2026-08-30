use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// In-memory live state for watchdog tracking.
/// Lost on orchestrator restart — rebuilt from agent memory files on resume.
#[derive(Default)]
pub struct LiveState {
    /// Active session heartbeats and intervention tracking
    pub sessions: HashMap<Uuid, ActiveSession>,
    /// Restart tracking for crash loop detection
    pub restarts: HashMap<Uuid, RestartState>,
}

#[derive(Debug, Clone)]
pub struct ActiveSession {
    pub last_heartbeat: std::time::Instant,
    pub intervention_cooldown_until: Option<std::time::Instant>,
    pub last_intervention_type: Option<String>,
    pub total_interventions: usize,
    pub last_response_time: Option<Duration>,
    pub last_intervention_at: Option<std::time::Instant>,
    pub consecutive_stall_failures: usize,
    pub last_confirmed_alive: std::time::Instant,
    pub queued_prompts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestartState {
    pub session_id: Uuid,
    pub mission_id: Uuid,
    pub restart_count: usize,
    pub first_restart_at: chrono::DateTime<Utc>,
    pub last_restart_at: chrono::DateTime<Utc>,
    pub backoff_seconds: f64,
}

impl LiveState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn touch_session(&mut self, session_id: Uuid) {
        let now = std::time::Instant::now();
        let entry = self
            .sessions
            .entry(session_id)
            .or_insert_with(|| ActiveSession {
                last_heartbeat: now,
                intervention_cooldown_until: None,
                last_intervention_type: None,
                total_interventions: 0,
                last_response_time: None,
                last_intervention_at: None,
                consecutive_stall_failures: 0,
                last_confirmed_alive: now,
                queued_prompts: Vec::new(),
            });
        entry.last_heartbeat = now;
        entry.last_confirmed_alive = now;
        // Reset cooldown on output
        if entry.total_interventions > 0 {
            entry.intervention_cooldown_until = None;
        }
    }

    pub fn get_session(&self, session_id: &Uuid) -> Option<&ActiveSession> {
        self.sessions.get(session_id)
    }

    pub fn get_session_mut(&mut self, session_id: &Uuid) -> Option<&mut ActiveSession> {
        self.sessions.get_mut(session_id)
    }

    pub fn remove_session(&mut self, session_id: &Uuid) {
        self.sessions.remove(session_id);
    }

    // ─── Restart Tracking ──────────────────────────────────────────────

    pub fn upsert_restart_attempt(
        &mut self,
        session_id: Uuid,
        mission_id: Uuid,
    ) -> Result<crate::model::RestartRecord> {
        let now = Utc::now();
        let state = self
            .restarts
            .entry(session_id)
            .or_insert_with(|| RestartState {
                session_id,
                mission_id,
                restart_count: 0,
                first_restart_at: now,
                last_restart_at: now,
                backoff_seconds: restart_base_secs() as f64,
            });

        state.restart_count += 1;
        state.last_restart_at = now;
        let backoff = (restart_base_secs() as f64 * 2.0f64.powi((state.restart_count - 1) as i32))
            .min(restart_max_secs() as f64);
        state.backoff_seconds = backoff;

        Ok(crate::model::RestartRecord {
            id: Uuid::new_v4(),
            session_id,
            mission_id,
            restart_count: state.restart_count,
            first_restart_at: state.first_restart_at,
            last_restart_at: state.last_restart_at,
            backoff_seconds: state.backoff_seconds,
        })
    }

    pub fn load_restart_state(&self, session_id: &Uuid) -> Option<&RestartState> {
        self.restarts.get(session_id)
    }

    pub fn reset_restart_tracker(&mut self, session_id: &Uuid) {
        self.restarts.remove(session_id);
    }

    pub fn is_crash_loop(&self, session_id: &Uuid, threshold: usize, window: Duration) -> bool {
        if let Some(state) = self.restarts.get(session_id) {
            if state.restart_count >= threshold {
                let elapsed = Utc::now().signed_duration_since(state.first_restart_at);
                if let Ok(elapsed_std) = elapsed.to_std() {
                    return elapsed_std <= window;
                }
            }
        }
        false
    }

    pub fn get_crash_loop_sessions(
        &self,
        mission_id: &Uuid,
        threshold: usize,
        window: Duration,
    ) -> Vec<(Uuid, usize)> {
        let now = Utc::now();
        self.restarts
            .values()
            .filter(|s| {
                s.mission_id == *mission_id
                    && s.restart_count >= threshold
                    && now
                        .signed_duration_since(s.first_restart_at)
                        .to_std()
                        .map_or(false, |d| d <= window)
            })
            .map(|s| (s.session_id, s.restart_count))
            .collect()
    }
}

fn restart_base_secs() -> u64 {
    2
}
fn restart_max_secs() -> u64 {
    300
}
