use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Context, Result};
use chrono::Utc;
use serde_json::json;
use tracing::{info, warn};
use uuid::Uuid;

use crate::adapter::{
    Confidence, FinalEnvelope, NormalizedObservation, SupervisorAction, SupervisorEventType,
    adapter_for,
};
use crate::cli::{LaunchConfig, ResumeConfig};
use crate::model::{
    LaunchSummary, MissionPlan, MissionRecord, MissionStatus, NormalizedUpdateRecord,
    SessionRecord, SessionRole, SessionState, TaskRecord, ValidationResultRecord, WorkerPacket,
};
use crate::protocol::{
    AckDirective, LeaseDirective, MailDirective, SapphireDirective, StatusDirective,
    consume_directives,
};
use crate::runtime::{ProcessLaunchSpec, RunningSession, RuntimeEvent, SessionRuntime};
use crate::store::Store;
use crate::templates::PromptLibrary;
use crate::tmux;

mod communication_policy;
mod coordination;
mod dedup;
mod enforcement;
mod final_synthesis;
mod finalization;
mod health;
mod launch_prompt;
mod live_state;
mod mail;
mod meetings;
mod memory;
mod mission_profile;
pub(crate) mod planning;
mod prompt_contracts;
mod status_files;
mod supervision;
mod supervisor;
mod supervisor_team;

mod bootstrap;
mod completion;
mod directives;
mod health_control;
mod launch;
mod mission_run;
mod resume;
mod runtime_events;
mod runtime_failures;
mod session_control;
mod status_handling;
mod supervisor_actions;
mod support;
mod surface;
mod views;

use bootstrap::{
    ensure_agents_bootstrap, ensure_worker_status_path, render_supervisor_prompt_with_agents,
    render_worker_prompt_with_agents, write_prompt_file,
};
use completion::reject_completion_without_artifacts;
use health_control::assess_worker_liveness;
use mission_profile::MissionProfile;
use session_control::register_session;
use supervisor_actions::queue_supervisor_state_card;
use support::*;

pub struct Orchestrator {
    pub(super) store: Store,
    pub(super) prompts: PromptLibrary,
}

#[derive(Default)]
pub(super) struct WatchdogStats {
    pub(super) runtime_events: usize,
    pub(super) directives: usize,
    pub(super) mails_routed: usize,
    pub(super) validation_challenges: usize,
    pub(super) stall_interventions: usize,
    pub(super) lease_conflicts: usize,
    pub(super) protocol_reminders: usize,
    pub(super) supervisor_health_events: usize,
    pub(super) critical_failures: usize,
    pub(super) crash_loops_detected: usize,
    pub(super) mass_deaths_detected: usize,
}

pub(super) struct ActiveSession {
    pub(super) record: SessionRecord,
    pub(super) packet: Option<WorkerPacket>,
    pub(super) runtime: RunningSession,
    pub(super) runtime_slot: Option<usize>,
    pub(super) launch_spec: ProcessLaunchSpec,
    pub(super) launch_prompt: String,
    pub(super) state: SessionState,
    pub(super) task_id: Option<Uuid>,
    pub(super) line_buffer: String,
    pub(super) raw_buffer: String,
    pub(super) started_at: Instant,
    pub(super) startup_grace_until: Instant,
    pub(super) last_output_at: Instant,
    pub(super) output_chunks: usize,
    pub(super) directive_count: usize,
    pub(super) initial_status_received: bool,
    pub(super) output_chunks_at_last_status: usize,
    pub(super) reported_overlap: Option<String>,
    pub(super) stall_count: usize,
    pub(super) restart_count: usize,
    pub(super) restart_at: Option<Instant>,
    pub(super) validation_pending: bool,
    pub(super) low_confidence_count: usize,
    pub(super) last_observation_key: Option<String>,
    pub(super) last_supervisor_action_key: Option<String>,
    pub(super) escalation_sent_for_state: Option<SessionState>,
    pub(super) protocol_reminder_sent: bool,
    pub(super) consecutive_stall_failures: usize,
    pub(super) last_confirmed_alive: Instant,
    #[allow(dead_code)]
    pub(super) last_files: Vec<String>,
    #[allow(dead_code)]
    pub(super) last_risks: Vec<String>,
    pub(super) intervention_cooldown_until: Option<Instant>,
    pub(super) last_intervention_type: Option<String>,
    pub(super) total_interventions: usize,
    pub(super) last_response_time: Option<Duration>,
    pub(super) last_intervention_at: Option<Instant>,
    pub(super) queued_prompts: VecDeque<QueuedPrompt>,
    pub(super) queued_prompt_keys: HashSet<String>,
    pub(super) recent_prompt_keys: VecDeque<(String, Instant)>,
    pub(super) last_prompt_sent_at: Option<Instant>,
    pub(super) launch_prompt_sent: bool,
    pub(super) launch_prompt_sent_at: Option<Instant>,
    pub(super) cleanup_authorized: bool,
    pub(super) last_status_update_at: Option<Instant>,
    pub(super) last_status_file_modified: Option<SystemTime>,
    pub(super) last_tmux_health: Option<tmux::SessionHealth>,
    pub(super) last_tmux_health_checked_at: Option<Instant>,
    pub(super) last_supervisor_notice_key: Option<String>,
    pub(super) recent_supervisor_notice_keys: VecDeque<(String, Instant)>,
    pub(super) last_supervisor_state_card_key: Option<String>,
    pub(super) zombie_debounce: health::ZombieDebounce,
    pub(super) health_state: health::SessionHealthState,
    pub(super) message_dedup: dedup::MessageDeduplicator,
    pub(super) task_stage: supervision::state::TaskStage,
    #[allow(dead_code)]
    pub(super) assignment_fingerprint: Option<String>,
    pub(super) plan_only_count: usize,
    pub(super) last_status_signature: Option<enforcement::StatusSignature>,
    pub(super) repeated_status_without_evidence: usize,
    pub(super) first_status_incident_stage: u8,
    pub(super) last_first_status_escalation_at: Option<Instant>,
    pub(super) supervising_supervisor_id: Option<Uuid>,
}

pub(super) struct QueuedPrompt {
    pub(super) key: String,
    pub(super) body: String,
}

pub(super) struct WorkerLaunch {
    pub(super) session: SessionRecord,
    pub(super) launch_spec: ProcessLaunchSpec,
    pub(super) prompt: String,
    pub(super) packet: WorkerPacket,
    pub(super) task_id: Option<Uuid>,
}

pub(super) struct LeaseOwner {
    pub(super) session_id: Uuid,
    pub(super) intent: String,
}

pub struct PendingMail {
    pub(super) message_id: Uuid,
    pub(super) thread_id: String,
    #[allow(dead_code)]
    pub(super) intent: String,
    #[allow(dead_code)]
    pub(super) thread_state: String,
    #[allow(dead_code)]
    pub(super) duplicate_key: String,
    pub(super) sender_session_id: Uuid,
    pub(super) recipient_session_id: Uuid,
    pub(super) cc_session_ids: Vec<Uuid>,
    #[allow(dead_code)]
    pub(super) sender_pod: String,
    #[allow(dead_code)]
    pub(super) recipient_pod: String,
    #[allow(dead_code)]
    pub(super) routing_class: String,
    pub(super) subject: String,
    pub(super) message_type: String,
    pub(super) priority: String,
    pub(super) routed_at: Instant,
    pub(super) acked: bool,
    pub(super) timeout_stage: u8,
    pub(super) last_timeout_at: Option<Instant>,
    pub(super) reply_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum SupervisorDecisionKind {
    Validation,
    StallRecovery,
    LowConfidenceRecovery,
    OverlapRecovery,
}

impl SupervisorDecisionKind {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Validation => "validation",
            Self::StallRecovery => "stall_recovery",
            Self::LowConfidenceRecovery => "low_confidence_recovery",
            Self::OverlapRecovery => "overlap_recovery",
        }
    }
}

pub(super) struct PendingSupervisorDecision {
    pub(super) kind: SupervisorDecisionKind,
    pub(super) target_session_id: Uuid,
    pub(super) reason: String,
    pub(super) queued_at: Instant,
    pub(super) last_notified_at: Instant,
    pub(super) notice_count: usize,
    pub(super) autonomous_action_count: usize,
    pub(super) last_autonomous_action_at: Option<Instant>,
}

pub(super) struct RecentFailure {
    pub(super) recorded_at: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FirstStatusFailureKind {
    Dispatch,
    Runtime,
    StatusPipeline,
    Reporting,
}

#[derive(Debug, Clone)]
pub(super) struct WorkerLivenessAssessment {
    pub(super) state: health::WorkerLivenessState,
    pub(super) incident_scope: health::IncidentScope,
    pub(super) first_status_overdue: bool,
    pub(super) failure_kind: Option<FirstStatusFailureKind>,
    pub(super) prompt_path_ready: bool,
    pub(super) status_path_ready: bool,
    pub(super) transcript_path_ready: bool,
    pub(super) transcript_has_output: bool,
    pub(super) runtime_live: bool,
    pub(super) diagnosis: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SupervisorMode {
    Healthy,
    Recovering,
    Degraded,
}

pub(super) struct ControlSurface {
    pub(super) state_dir: PathBuf,
    pub(super) status_file: PathBuf,
    pub(super) dashboard_file: PathBuf,
    pub(super) transcript_dir: PathBuf,
    pub(super) workers_state_dir: PathBuf,
    pub(super) persist_transcripts: bool,
    pub(super) tmux_session_names: Vec<String>,
}

pub(super) struct AgentsBootstrap {
    pub(super) path: PathBuf,
    pub(super) existed: bool,
}

#[derive(Clone)]
pub(super) struct PlanOutcome {
    pub(super) plan: MissionPlan,
    pub(super) source: &'static str,
}

#[derive(Default)]
pub(super) struct CompletionExpectation {
    pub(super) exact_files: Vec<String>,
    pub(super) require_readme: bool,
    pub(super) require_script: bool,
}

pub(super) const NUDGE_QUIET_THRESHOLD: Duration = Duration::from_secs(3);
pub(super) const HEURISTIC_SETTLE_THRESHOLD: Duration = Duration::from_secs(8);
