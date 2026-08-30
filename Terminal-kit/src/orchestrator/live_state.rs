use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::model::SessionRole;
use crate::tmux::{SessionHealth, Tmux};
use uuid::Uuid;

use super::ActiveSession;

const TMUX_LIVENESS_WINDOW: Duration = Duration::from_secs(180);
const RECENT_OUTPUT_GRACE: Duration = Duration::from_secs(12);
const RECENT_STATUS_GRACE: Duration = Duration::from_secs(45);

pub struct Snapshot {
    labels: HashMap<Uuid, String>,
}

impl Snapshot {
    pub fn build(active_sessions: &HashMap<Uuid, ActiveSession>, now: Instant) -> Self {
        let terminal_live = active_sessions
            .iter()
            .map(|(session_id, session)| (*session_id, session_terminal_live(session)))
            .collect::<HashMap<_, _>>();
        let labels = active_sessions
            .iter()
            .map(|(session_id, session)| {
                (
                    *session_id,
                    effective_state_label_with_terminal_live(
                        session,
                        now,
                        *terminal_live.get(session_id).unwrap_or(&false),
                    ),
                )
            })
            .collect::<HashMap<_, _>>();
        Self { labels }
    }

    pub fn effective_state_label<'a>(&'a self, session: &ActiveSession) -> &'a str {
        self.labels
            .get(&session.record.id)
            .map(String::as_str)
            .unwrap_or_else(|| session.state.as_str())
    }

    pub fn counts_as_blocked(&self, session: &ActiveSession) -> bool {
        matches!(self.effective_state_label(session), "blocked" | "stalled")
    }

    pub fn counts_as_problem(&self, session: &ActiveSession) -> bool {
        matches!(
            self.effective_state_label(session),
            "failed" | "contradictory" | "blocked" | "stalled" | "wrong_direction" | "needs_retry"
        ) || session.validation_pending
    }
}

pub fn session_terminal_live(session: &ActiveSession) -> bool {
    session
        .runtime
        .terminal_target()
        .map(|target| Tmux::new(None).check_session_health(target, TMUX_LIVENESS_WINDOW))
        .is_some_and(|health| {
            matches!(
                health,
                SessionHealth::Healthy | SessionHealth::Hung | SessionHealth::Starting
            )
        })
}

pub fn session_has_recent_activity(session: &ActiveSession, now: Instant) -> bool {
    session.output_chunks > 0 && now.duration_since(session.last_output_at) <= RECENT_OUTPUT_GRACE
        || session
            .last_status_update_at
            .is_some_and(|at| now.duration_since(at) <= RECENT_STATUS_GRACE)
}

pub fn session_is_live(session: &ActiveSession, now: Instant) -> bool {
    session_has_recent_activity(session, now) || session_terminal_live(session)
}

#[allow(dead_code)]
pub fn effective_state_label(session: &ActiveSession, now: Instant) -> String {
    effective_state_label_with_terminal_live(session, now, session_terminal_live(session))
}

fn effective_state_label_with_terminal_live(
    session: &ActiveSession,
    now: Instant,
    terminal_live: bool,
) -> String {
    if session.record.role == SessionRole::Supervisor {
        return effective_supervisor_state_label_with_terminal_live(session, now, terminal_live);
    }
    if session.state.is_terminal() {
        return session.state.as_str().to_owned();
    }
    session.state.as_str().to_owned()
}

#[allow(dead_code)]
pub fn effective_supervisor_state_label(session: &ActiveSession, now: Instant) -> String {
    effective_supervisor_state_label_with_terminal_live(
        session,
        now,
        session_terminal_live(session),
    )
}

fn effective_supervisor_state_label_with_terminal_live(
    session: &ActiveSession,
    now: Instant,
    terminal_live: bool,
) -> String {
    if session.state.is_terminal() {
        return session.state.as_str().to_owned();
    }
    if session_has_recent_activity(session, now) || terminal_live {
        "running".to_owned()
    } else {
        session.state.as_str().to_owned()
    }
}

#[allow(dead_code)]
pub fn counts_as_blocked(session: &ActiveSession, now: Instant) -> bool {
    matches!(
        effective_state_label(session, now).as_str(),
        "blocked" | "stalled"
    )
}

#[allow(dead_code)]
pub fn counts_as_problem(session: &ActiveSession, now: Instant) -> bool {
    matches!(
        effective_state_label(session, now).as_str(),
        "failed" | "contradictory" | "blocked" | "stalled" | "wrong_direction" | "needs_retry"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::AgentKind;
    use crate::model::{SessionRecord, SessionState};
    use crate::orchestrator::dedup::MessageDeduplicator;
    use crate::orchestrator::health::{SessionHealthState, ZombieDebounce};
    use crate::orchestrator::supervision::state::initial_stage;
    use crate::runtime::{ProcessLaunchSpec, RunningSession, SubmitMode};
    use chrono::Utc;
    use std::collections::{BTreeMap, HashSet, VecDeque};
    use std::path::PathBuf;
    use uuid::Uuid;

    fn worker_session(state: SessionState, validation_pending: bool) -> ActiveSession {
        let (runtime, _) = RunningSession::test("Engineer-1", Duration::from_millis(0));
        ActiveSession {
            record: SessionRecord {
                id: Uuid::new_v4(),
                mission_id: Uuid::new_v4(),
                role: SessionRole::Worker,
                ordinal: 1,
                agent: AgentKind::Qwen,
                terminal_id: "worker-pty".to_owned(),
                name: "Engineer-1".to_owned(),
                owned_scope: String::new(),
                status: state,
                launch_command: vec!["qwen".to_owned()],
                last_heartbeat_at: Utc::now(),
                last_summary: None,
            },
            packet: None,
            runtime,
            runtime_slot: None,
            launch_spec: ProcessLaunchSpec {
                program: "qwen".to_owned(),
                args: Vec::new(),
                cwd: PathBuf::from("."),
                env: BTreeMap::new(),
                prompt_delay: Duration::from_millis(0),
                startup_input: None,
                startup_rules: Vec::new(),
                surface_label: "Engineer-1".to_owned(),
                submit_mode: SubmitMode::LineFeed,
            },
            launch_prompt: String::new(),
            state,
            task_id: None,
            line_buffer: String::new(),
            raw_buffer: String::new(),
            started_at: Instant::now(),
            startup_grace_until: Instant::now(),
            last_output_at: Instant::now(),
            output_chunks: 0,
            directive_count: 0,
            initial_status_received: true,
            output_chunks_at_last_status: 0,
            reported_overlap: None,
            stall_count: 0,
            restart_count: 0,
            restart_at: None,
            validation_pending,
            low_confidence_count: 0,
            last_observation_key: None,
            last_supervisor_action_key: None,
            escalation_sent_for_state: None,
            protocol_reminder_sent: false,
            consecutive_stall_failures: 0,
            last_confirmed_alive: Instant::now(),
            last_files: Vec::new(),
            last_risks: Vec::new(),
            intervention_cooldown_until: None,
            last_intervention_type: None,
            total_interventions: 0,
            last_response_time: None,
            last_intervention_at: None,
            queued_prompts: VecDeque::new(),
            queued_prompt_keys: HashSet::new(),
            recent_prompt_keys: VecDeque::new(),
            last_prompt_sent_at: None,
            launch_prompt_sent: true,
            launch_prompt_sent_at: None,
            cleanup_authorized: false,
            last_status_update_at: Some(Instant::now()),
            last_status_file_modified: None,
            last_tmux_health: None,
            last_tmux_health_checked_at: None,
            last_supervisor_notice_key: None,
            recent_supervisor_notice_keys: VecDeque::new(),
            last_supervisor_state_card_key: None,
            zombie_debounce: ZombieDebounce::default(),
            health_state: SessionHealthState::new(),
            message_dedup: MessageDeduplicator::new(),
            task_stage: initial_stage(SessionRole::Worker, state),
            assignment_fingerprint: None,
            plan_only_count: 0,
            last_status_signature: None,
            repeated_status_without_evidence: 0,
            first_status_incident_stage: 0,
            last_first_status_escalation_at: None,
            supervising_supervisor_id: None,
        }
    }

    #[test]
    fn preserves_done_claimed_state_label() {
        let session = worker_session(SessionState::DoneClaimed, true);
        assert_eq!(
            effective_state_label(&session, Instant::now()),
            SessionState::DoneClaimed.as_str()
        );
    }

    #[test]
    fn validation_pending_is_not_a_problem_state() {
        let session = worker_session(SessionState::DoneClaimed, true);
        assert!(!counts_as_problem(&session, Instant::now()));
    }
}

pub fn should_pause_mail_timeout(session: &ActiveSession, now: Instant) -> bool {
    session_is_live(session, now)
        || !session.queued_prompts.is_empty()
        || session
            .last_prompt_sent_at
            .is_some_and(|at| now.duration_since(at) <= Duration::from_secs(20))
}
