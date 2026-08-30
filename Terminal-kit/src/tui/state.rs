//! TUI state layer — runtime-aligned types. Every field maps to real backend data.
//! No UI-only abstractions. No hardcoded zeros.
#![allow(dead_code)]

use chrono::{DateTime, Utc};
use std::time::Instant;
use uuid::Uuid;

// ─── Agent Status (direct mirror of SessionState) ────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentStatus {
    Booting,
    NotStarted,
    Progressing,
    Blocked,
    Stalled,
    DoneClaimed,
    NeedsValidation,
    WeakOutput,
    WrongDirection,
    Contradictory,
    NeedsRetry,
    Validated,
    Failed,
    Exited,
}

impl AgentStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Booting => "booting",
            Self::NotStarted => "not_started",
            Self::Progressing => "progressing",
            Self::Blocked => "blocked",
            Self::Stalled => "stalled",
            Self::DoneClaimed => "done_claimed",
            Self::NeedsValidation => "needs_validation",
            Self::WeakOutput => "weak_output",
            Self::WrongDirection => "wrong_direction",
            Self::Contradictory => "contradictory",
            Self::NeedsRetry => "needs_retry",
            Self::Validated => "validated",
            Self::Failed => "failed",
            Self::Exited => "exited",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "booting" => Self::Booting,
            "not_started" | "planned" => Self::NotStarted,
            "progressing" | "running" => Self::Progressing,
            "blocked" => Self::Blocked,
            "stalled" => Self::Stalled,
            "done_claimed" => Self::DoneClaimed,
            "needs_validation" => Self::NeedsValidation,
            "weak_output" => Self::WeakOutput,
            "wrong_direction" => Self::WrongDirection,
            "contradictory" => Self::Contradictory,
            "needs_retry" => Self::NeedsRetry,
            "validated" | "completed" => Self::Validated,
            "failed" => Self::Failed,
            "exited" => Self::Exited,
            _ => Self::NotStarted,
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Validated | Self::Failed | Self::Exited)
    }
}

// ─── Agent Node ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AgentNode {
    pub id: Uuid,
    pub name: String,
    pub role_type: String,
    pub display_role: String,
    pub status: AgentStatus,
    pub summary: String,
    pub liveness: Option<String>,
    pub owner_supervisor: Option<String>,
    pub branch_label: Option<String>,
    pub incident_scope: Option<String>,
    pub failure_kind: Option<String>,
    pub owned_agent_count: usize,
    pub blocked_agent_count: usize,
    pub validating_agent_count: usize,
    pub owned_scope: String,
    pub explicit_task: String,
    pub files_touched: Vec<String>,
    pub stall_count: usize,
    pub intervention_count: usize,
    pub consecutive_stall_failures: usize,
    pub output_chunks: usize,
    pub mail_thread_count: usize,
    pub started_at: Option<Instant>,
    pub last_output_at: Option<Instant>,
    pub is_supervisor: bool,
    pub is_standby: bool,
    pub is_active_supervisor: bool,
}

impl AgentNode {
    pub fn elapsed(&self) -> Option<std::time::Duration> {
        self.started_at.map(|s| s.elapsed())
    }
    pub fn time_since_output(&self) -> Option<std::time::Duration> {
        self.last_output_at.map(|t| t.elapsed())
    }
}

// ─── Supervisor Activity Log ─────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SupervisorLogEntry {
    pub timestamp: DateTime<Utc>,
    pub kind: SupervisorLogKind,
    pub message: String,
    pub target: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupervisorLogKind {
    Startup,
    Planning,
    Dispatch,
    Action,
    Validation,
    Escalation,
    Mail,
    Health,
    Completion,
    Error,
    Restart,
}

impl SupervisorLogKind {
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Startup => ">",
            Self::Planning => "*",
            Self::Dispatch => "->",
            Self::Action => "!",
            Self::Validation => "V",
            Self::Escalation => "^",
            Self::Mail => "M",
            Self::Health => "H",
            Self::Completion => "D",
            Self::Error => "X",
            Self::Restart => "R",
        }
    }
}

// ─── Mail Thread ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct MailThread {
    pub thread_id: String,
    pub subject: String,
    pub from: String,
    pub to: String,
    pub message_type: String,
    pub priority: String,
    pub state: String,
    pub acked: bool,
}

// ─── Pod Summary (from coordination module) ─────────────────────────────

#[derive(Debug, Clone)]
pub struct PodSummary {
    pub name: String,
    pub members: Vec<String>,
    pub blocked_members: Vec<String>,
    pub open_threads: usize,
}

// ─── Meeting Artifact ────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct MeetingArtifact {
    pub id: String,
    pub kind: String,
    pub participants: Vec<String>,
    pub reason: String,
}

// ─── Watchdog Stats (real, from status file) ─────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct WatchdogStats {
    pub worker_count: usize,
    pub directives: usize,
    pub mail_routed: usize,
    pub validation_challenges: usize,
    pub stall_interventions: usize,
    pub lease_conflicts: usize,
    pub protocol_reminders: usize,
    pub supervisor_health_events: usize,
    pub critical_failures: usize,
    pub crash_loops_detected: usize,
    pub blocked: Vec<String>,
    pub validation_queue: Vec<String>,
    pub contradictions: Vec<String>,
    pub mail_pressure: Vec<String>,
    pub problems: Vec<String>,
    pub ownership_gaps: Vec<String>,
    pub first_status_incidents: Vec<String>,
    pub systemic_incidents: Vec<String>,
    pub crash_loop_sessions: Vec<String>,
    pub pods: Vec<PodSummary>,
    pub memory_summaries: Vec<AgentMemorySummary>,
}

#[derive(Debug, Clone)]
pub struct AgentMemorySummary {
    pub display_name: String,
    pub pod: String,
    pub active_threads: usize,
}

// ─── Execution Summary ───────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct ExecutionSummary {
    pub mission_rewrite: String,
    pub agents_deployed: usize,
    pub agents_completed: usize,
    pub agents_failed: usize,
    pub mail_threads_total: usize,
    pub mail_threads_resolved: usize,
    pub lease_conflicts: usize,
    pub stall_interventions: usize,
    pub protocol_reminders: usize,
    pub supervisor_health_events: usize,
    pub critical_failures: usize,
    pub crash_loops_detected: usize,
    pub supervisor_mode: String,
    pub final_summary: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
}

impl ExecutionSummary {
    pub fn elapsed(&self) -> Option<std::time::Duration> {
        match (self.started_at, self.ended_at) {
            (Some(start), Some(end)) => Some((end - start).to_std().unwrap_or_default()),
            (Some(start), None) => Some((Utc::now() - start).to_std().unwrap_or_default()),
            _ => None,
        }
    }
}

// ─── Runtime Snapshot (complete backend-aligned state) ───────────────────

#[derive(Debug, Clone)]
pub struct RuntimeSnapshot {
    pub mission_id: Option<Uuid>,
    pub mission_status: String,
    pub supervisor: Option<AgentNode>,
    pub standby_supervisor: Option<AgentNode>,
    pub supervisors: Vec<AgentNode>,
    pub agents: Vec<AgentNode>,
    pub supervisor_logs: Vec<SupervisorLogEntry>,
    pub mail_threads: Vec<MailThread>,
    pub meetings: Vec<MeetingArtifact>,
    pub watchdog: WatchdogStats,
    pub execution_summary: ExecutionSummary,
    pub is_done: bool,
}

impl RuntimeSnapshot {
    pub fn agent_count(&self) -> usize {
        self.agents.len()
    }

    pub fn problem_agent_count(&self) -> usize {
        self.agents
            .iter()
            .filter(|a| {
                matches!(
                    a.status,
                    AgentStatus::Blocked
                        | AgentStatus::Stalled
                        | AgentStatus::Contradictory
                        | AgentStatus::Failed
                        | AgentStatus::WrongDirection
                        | AgentStatus::NeedsRetry
                )
            })
            .count()
    }

    pub fn active_agent_count(&self) -> usize {
        self.agents
            .iter()
            .filter(|a| {
                matches!(
                    a.status,
                    AgentStatus::Booting
                        | AgentStatus::NotStarted
                        | AgentStatus::Progressing
                        | AgentStatus::Blocked
                        | AgentStatus::Stalled
                        | AgentStatus::WeakOutput
                        | AgentStatus::WrongDirection
                        | AgentStatus::Contradictory
                        | AgentStatus::NeedsRetry
                )
            })
            .count()
    }

    pub fn empty_planning(mission_text: &str) -> Self {
        Self {
            mission_id: None,
            mission_status: "planning".to_owned(),
            supervisor: None,
            standby_supervisor: None,
            supervisors: Vec::new(),
            agents: Vec::new(),
            supervisor_logs: vec![SupervisorLogEntry {
                timestamp: Utc::now(),
                kind: SupervisorLogKind::Startup,
                message: format!("Planning: {mission_text}"),
                target: None,
            }],
            mail_threads: Vec::new(),
            meetings: Vec::new(),
            watchdog: WatchdogStats::default(),
            execution_summary: ExecutionSummary {
                mission_rewrite: mission_text.to_owned(),
                ..Default::default()
            },
            is_done: false,
        }
    }

    pub fn empty_waiting() -> Self {
        Self {
            mission_id: None,
            mission_status: "launching".to_owned(),
            supervisor: None,
            standby_supervisor: None,
            supervisors: Vec::new(),
            agents: Vec::new(),
            supervisor_logs: vec![SupervisorLogEntry {
                timestamp: Utc::now(),
                kind: SupervisorLogKind::Startup,
                message: "supervisor's still waking up".to_owned(),
                target: None,
            }],
            mail_threads: Vec::new(),
            meetings: Vec::new(),
            watchdog: WatchdogStats::default(),
            execution_summary: ExecutionSummary::default(),
            is_done: false,
        }
    }
}
