use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::agent::AgentKind;

/// Top-level mission record persisted to SQLite.
/// Tracks the user's original mission, the supervisor's rewrite, and final status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionRecord {
    /// Unique mission identifier (UUID v4)
    pub id: Uuid,
    /// When the mission was created
    pub created_at: DateTime<Utc>,
    /// When the mission was last updated
    pub updated_at: DateTime<Utc>,
    /// Absolute path to the repository being worked on
    pub repo_path: PathBuf,
    /// The user's original mission text
    pub mission: String,
    /// The supervisor's rewritten mission (cleaner scope and framing)
    pub mission_rewrite: String,
    /// Which agent kind is used for worker sessions (Qwen, Codex, Claude, Forge)
    pub worker_agent: AgentKind,
    /// Which agent kind is used for the supervisor session
    pub supervisor_agent: AgentKind,
    /// Number of worker sessions to launch (excludes supervisor)
    pub worker_count: usize,
    /// Current mission status (Planned → Running → Completed/Failed)
    pub status: MissionStatus,
    /// Final summary from the supervisor when mission ends
    pub final_summary: Option<String>,
}

/// Mission lifecycle status. Transitions are managed by the watchdog.
/// Terminal states: Completed, Failed
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum MissionStatus {
    /// Mission plan created but not yet launched
    Planned,
    /// Sessions are being booted
    Launching,
    /// Mission is actively running
    Running,
    /// Mission completed successfully
    Completed,
    /// Mission failed irrecoverably
    Failed,
}

impl MissionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::Launching => "launching",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }

    pub fn from_str(s: &str) -> Result<Self, String> {
        match s.trim().to_lowercase().as_str() {
            "planned" => Ok(Self::Planned),
            "launching" => Ok(Self::Launching),
            "running" => Ok(Self::Running),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            _ => Err(format!("unknown mission status: {}", s)),
        }
    }
}

/// Supervisor-generated mission plan. Contains workstream decomposition, risk map, and worker assignments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionPlan {
    /// Supervisor's rewritten mission statement
    pub mission_rewrite: String,
    /// Decomposed work tracks (e.g. "debug", "security", "docs")
    pub workstreams: Vec<Workstream>,
    /// Known risks and mitigations
    pub risk_map: Vec<RiskItem>,
    /// Per-worker assignment packets
    pub worker_packets: Vec<WorkerPacket>,
    /// How the supervisor will monitor and intervene
    pub supervision_strategy: String,
}

/// A single workstream — a decomposed track of work within the mission.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workstream {
    /// Short stable identifier (e.g. "debug", "security")
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// How this workstream executes relative to others
    pub execution: WorkstreamExecution,
    /// What this workstream owns (file paths, modules, concerns)
    pub owned_scope: String,
    /// Criteria that must be met for this workstream to be considered done
    pub success_criteria: Vec<String>,
    /// Other workstream IDs this depends on (empty if parallel)
    pub depends_on: Vec<String>,
}

/// How a workstream executes relative to others.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkstreamExecution {
    /// Can run in parallel with other workstreams
    Parallel,
    /// Depends on another workstream completing first
    Dependent,
    /// Validates work done by another workstream
    Validation,
    /// Final integration of all completed workstreams
    Integration,
}

/// A single identified risk and its planned mitigation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskItem {
    /// Where the risk exists (e.g. "Shared files", "Security claims")
    pub zone: String,
    /// What the risk is
    pub risk: String,
    /// How we plan to mitigate it
    pub mitigation: String,
}

/// Per-worker assignment packet. Defines what a specific worker should do,
/// what's out of scope, and how completion is validated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerPacket {
    /// Stable machine key for template lookup (e.g. "software-engineer", "designer-engineer")
    pub role_type: String,
    /// Runtime display name (e.g. "Engineer-1", "Designer-2")
    pub display_name: String,
    /// Deprecated: use display_name instead. Kept for backward compat with persisted packets.
    #[serde(default)]
    pub worker_id: String,
    /// Human-readable role title (e.g. "Software Engineer"). Backward compat; populated from role_type.
    #[serde(default)]
    pub role: String,
    /// Where to begin work (starting angle/approach)
    #[serde(default)]
    pub starting_angle: String,
    /// What this worker owns (file paths, modules, concerns)
    pub owned_scope: String,
    /// The primary task this worker must complete
    pub explicit_task: String,
    /// What this worker must NOT touch
    pub out_of_scope: String,
    /// Criteria that must be met for this worker to be considered done
    pub definition_of_done: Vec<String>,
    /// Proof/artifacts the worker must produce
    pub required_evidence: Vec<String>,
    /// How to report blockers
    pub blocker_protocol: String,
    /// Warning about file ownership conflicts
    pub conflict_warning: String,
    /// Rules for inter-worker communication
    #[serde(default)]
    pub communication_rules: Vec<String>,
    /// Standard by which this worker's output will be validated
    #[serde(default)]
    pub validation_standard: Vec<String>,
    /// How the worker should format their output
    pub expected_output_format: Vec<String>,
}

/// Runtime session record for a supervisor or worker PTY session.
/// Persisted to SQLite and used for resume/replay operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    /// Unique session identifier (UUID v4)
    pub id: Uuid,
    /// The mission this session belongs to
    pub mission_id: Uuid,
    /// Whether this is a supervisor or worker session
    pub role: SessionRole,
    /// Launch order (0 = supervisor, 1+ = workers)
    pub ordinal: usize,
    /// Which agent kind runs in this session
    pub agent: AgentKind,
    /// PTY terminal identifier
    pub terminal_id: String,
    /// Display name (e.g. "Supervisor", "Engineer-1")
    pub name: String,
    /// What this session owns (empty for supervisor)
    pub owned_scope: String,
    /// Current 16-state lifecycle value
    pub status: SessionState,
    /// CLI command used to launch this session
    pub launch_command: Vec<String>,
    /// Timestamp of last confirmed output
    pub last_heartbeat_at: DateTime<Utc>,
    /// Latest status summary from the session
    pub last_summary: Option<String>,
}

/// Whether a session is a supervisor or worker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionRole {
    Supervisor,
    Worker,
}

/// 16-state session lifecycle managed by the watchdog.
///
/// Lifecycle flow:
/// `Planned → Booting → NotStarted → Progressing → Blocked → Stalled`
/// `→ DoneClaimed → NeedsValidation → WeakOutput → WrongDirection`
/// `→ Contradictory → NeedsRetry → Validated → Failed → Exited`
///
/// Terminal states: `Validated`, `Failed`, `Exited`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionState {
    /// Session planned but not yet booted
    Planned,
    /// PTY session is starting
    Booting,
    /// Session booted but no work started
    NotStarted,
    /// Actively working
    Progressing,
    /// Blocked waiting for dependency or clarification
    Blocked,
    /// No output received beyond stall threshold
    Stalled,
    /// Worker claims task is done
    DoneClaimed,
    /// Validation challenge has been issued
    NeedsValidation,
    /// Output produced but lacks evidence or depth
    WeakOutput,
    /// Worker deviated from assigned scope
    WrongDirection,
    /// File ownership or logic conflict with another worker
    Contradictory,
    /// Needs to retry with corrected scope
    NeedsRetry,
    /// Validated and accepted
    Validated,
    /// Failed irrecoverably
    Failed,
    /// Session exited (normal or error)
    Exited,
}

impl SessionState {
    pub fn from_directive(state: &str) -> Option<Self> {
        match state.trim().to_ascii_lowercase().as_str() {
            "planned" => Some(Self::Planned),
            "booting" => Some(Self::Booting),
            "not_started" => Some(Self::NotStarted),
            "progressing" => Some(Self::Progressing),
            "blocked" => Some(Self::Blocked),
            "stalled" => Some(Self::Stalled),
            "done_claimed" => Some(Self::DoneClaimed),
            "needs_validation" => Some(Self::NeedsValidation),
            "weak_output" => Some(Self::WeakOutput),
            "wrong_direction" => Some(Self::WrongDirection),
            "contradictory" => Some(Self::Contradictory),
            "needs_retry" => Some(Self::NeedsRetry),
            "validated" => Some(Self::Validated),
            "failed" => Some(Self::Failed),
            "exited" => Some(Self::Exited),
            _ => None,
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Validated | Self::Failed | Self::Exited)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Planned => "planned",
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
}

/// Runtime event record — every output chunk, directive, state change,
/// automation event, stall, mail, lease, or validation result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct EventRecord {
    /// Unique event identifier (UUID v4)
    pub id: Uuid,
    /// The mission this event belongs to
    pub mission_id: Uuid,
    /// Which worker session produced this event (None for mission-level events)
    pub worker_id: Option<Uuid>,
    /// When the event occurred
    pub created_at: DateTime<Utc>,
    /// Event type (output, directive, state_change, automation, stall, mail, lease, validation)
    pub kind: String,
    /// Human-readable event body
    pub body: String,
    /// JSON payload for structured data
    pub payload_json: String,
}

/// File ownership lease — tracks which worker owns which files for conflict resolution.
/// Upserted by session+path; a second claim on the same path triggers contradiction handling.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaseRecord {
    /// The mission this lease belongs to
    pub mission_id: Uuid,
    /// File path being claimed (relative to repo root)
    pub path: String,
    /// Which session owns this path
    pub owner_session_id: Uuid,
    /// Intent: read, edit, or review
    pub intent: String,
    /// Status: active, released, conflicted
    pub status: String,
    /// When the lease was last updated
    pub updated_at: DateTime<Utc>,
}

/// Durable inter-worker mail — persisted to SQLite before injection into recipient PTY.
/// Supports threading, priority, ack tracking, and supervisor visibility.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MailRecord {
    /// Unique mail identifier (UUID v4)
    pub id: Uuid,
    /// The mission this mail belongs to
    pub mission_id: Uuid,
    /// Which worker sent this mail
    pub sender_worker_id: Uuid,
    /// Which worker should receive this mail
    pub recipient_worker_id: Uuid,
    /// Message type: task, reply, notification, escalation, scavenge
    pub message_type: String,
    /// Priority level: urgent, high, normal, low
    pub priority: String,
    /// Delivery mode: interrupt (inject immediately) or queue (recipient polls)
    pub delivery_mode: String,
    /// Brief subject line
    pub subject: String,
    /// Delivery status: pending, delivered, acked, done, archived
    pub status: String,
    /// Acknowledgment state: pending, acked, done, cannot_comply
    pub ack_state: String,
    /// Whether this mail is pinned (survives auto-archival)
    pub pinned: bool,
    /// Structured mail body as JSON
    pub body_json: String,
    /// Thread ID for conversation tracking
    pub thread_id: String,
    /// Reply-to mail ID (if this is a reply)
    pub reply_to: Option<String>,
    /// When the mail was created
    pub created_at: DateTime<Utc>,
    /// When the mail was archived (null if still active)
    pub archived_at: Option<DateTime<Utc>>,
}

/// Task assignment per worker — persisted alongside sessions.
/// Stores task title, description, status, dependencies, and definition of done.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRecord {
    /// Unique task identifier (UUID v4)
    pub id: Uuid,
    /// The mission this task belongs to
    pub mission_id: Uuid,
    /// Which worker this task is assigned to
    pub worker_id: Uuid,
    /// Task title
    pub title: String,
    /// Full task description
    pub description: String,
    /// Current task status
    pub status: String,
    /// Task priority
    pub priority: String,
    /// Dependencies as JSON array of task IDs
    pub depends_on_json: String,
    /// Definition of done as JSON array
    pub definition_of_done_json: String,
}

/// Freeform summary — mission-level, per-worker, or special types
/// (plan_source, resume, exit, surface, agents_bootstrap, preflight_failure).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryRecord {
    /// Unique summary identifier (UUID v4)
    pub id: Uuid,
    /// The mission this summary belongs to
    pub mission_id: Uuid,
    /// Which worker this summary is for (None for mission-level)
    pub worker_id: Option<Uuid>,
    /// Summary type (mission, worker, plan_source, resume, exit, etc.)
    pub summary_type: String,
    /// The summary content text
    pub content: String,
    /// When the summary was created
    pub created_at: DateTime<Utc>,
}

/// Launch summary — rendered to the user after mission bootstrap.
/// Not persisted; used for CLI output only.
#[derive(Debug, Clone)]
pub struct LaunchSummary {
    pub mission_id: Uuid,
    pub repo: PathBuf,
    pub state_dir: PathBuf,
    pub dry_run: bool,
    pub worker_agent: AgentKind,
    pub supervisor_agent: AgentKind,
    pub worker_count: usize,
    pub session_names: Vec<String>,
    pub mission_rewrite: String,
    pub workstream_names: Vec<String>,
    pub notes: Vec<String>,
}

/// Adapter-normalized state observation — stores how the adapter layer
/// interpreted raw output when no explicit directive was found.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedUpdateRecord {
    /// Unique update identifier (UUID v4)
    pub id: Uuid,
    /// The mission this update belongs to
    pub mission_id: Uuid,
    /// Which worker session this update is for
    pub worker_id: Uuid,
    /// Source of the observation (output snippet, heuristic, etc.)
    pub source: String,
    /// Raw output excerpt from the PTY
    pub raw_excerpt: String,
    /// What state the adapter inferred
    pub normalized_state: String,
    /// Confidence level (high, medium, low)
    pub confidence: String,
    /// Brief summary of the observation
    pub summary: String,
    /// Which adapter produced this observation (qwen, codex, claude, forge)
    pub adapter: String,
    /// When the observation was recorded
    pub created_at: DateTime<Utc>,
}

/// Validation challenge outcome — persisted when a worker's done claim is validated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResultRecord {
    /// Unique validation result identifier (UUID v4)
    pub id: Uuid,
    /// The mission this validation belongs to
    pub mission_id: Uuid,
    /// Which worker session was validated
    pub worker_id: Uuid,
    /// Which task was validated (if any)
    pub task_id: Option<Uuid>,
    /// Validation outcome (passed, failed, needs_revision)
    pub outcome: String,
    /// Brief validation summary
    pub summary: String,
    /// Evidence as JSON (what was checked, what passed/failed)
    pub evidence_json: String,
    /// When the validation was recorded
    pub created_at: DateTime<Utc>,
}

/// Session restart tracking record (from gastown daemon restart tracker pattern).
/// Persists restart attempts to survive orchestrator restarts and detect crash loops.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct RestartRecord {
    /// Unique restart record identifier (UUID v4)
    pub id: Uuid,
    /// Which session was restarted
    pub session_id: Uuid,
    /// The mission this session belongs to
    pub mission_id: Uuid,
    /// Total number of restart attempts for this session
    pub restart_count: usize,
    /// When the first restart was attempted
    pub first_restart_at: DateTime<Utc>,
    /// When the most recent restart was attempted
    pub last_restart_at: DateTime<Utc>,
    /// Current backoff duration in seconds (exponential: base × 2^(count-1))
    pub backoff_seconds: f64,
}

/// Read-only view of a session for list display.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SessionListItem {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub repo_path: PathBuf,
    pub mission_rewrite: String,
    pub status: String,
    pub final_summary: Option<String>,
}

/// Full snapshot of a mission including plan and timing metadata.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct MissionSnapshot {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub repo_path: PathBuf,
    pub user_mission_raw: String,
    pub mission_rewrite: String,
    pub status: String,
    pub final_summary: Option<String>,
    pub plan: MissionPlan,
}

#[derive(Debug, Clone)]
pub struct WorkerSnapshot {
    pub session: SessionRecord,
    pub packet: Option<WorkerPacket>,
}

#[derive(Debug, Clone)]
pub struct ReplayEntry {
    pub created_at: DateTime<Utc>,
    pub lane: String,
    pub kind: String,
    pub body: String,
}

impl LaunchSummary {
    pub fn render(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!("mission_id: {}", self.mission_id));
        lines.push(format!("repo: {}", self.repo.display()));
        lines.push(format!("state_dir: {}", self.state_dir.display()));
        lines.push(format!(
            "agents: workers={} x{} | supervisor={}",
            self.worker_agent.as_str(),
            self.worker_count,
            self.supervisor_agent.as_str()
        ));
        lines.push(format!(
            "mode: {}",
            if self.dry_run {
                "planned_only"
            } else {
                "launched"
            }
        ));
        lines.push(format!("mission: {}", self.mission_rewrite));
        lines.push(format!("sessions: {}", self.session_names.join(", ")));
        lines.push(format!("workstreams: {}", self.workstream_names.join(", ")));
        if !self.notes.is_empty() {
            lines.push(format!("notes: {}", self.notes.join(" | ")));
        }
        lines.join("\n")
    }
}
