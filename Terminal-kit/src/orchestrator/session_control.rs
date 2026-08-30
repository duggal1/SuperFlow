use super::*;

pub(super) fn register_session(
    active_sessions: &mut HashMap<Uuid, ActiveSession>,
    alias_map: &mut HashMap<String, Uuid>,
    record: SessionRecord,
    packet: Option<WorkerPacket>,
    runtime: RunningSession,
    runtime_slot: Option<usize>,
    launch_spec: ProcessLaunchSpec,
    launch_prompt: String,
    task_id: Option<Uuid>,
    aliases: Vec<String>,
) {
    let session_id = record.id;
    let initial_status = record.status;
    let role = record.role;
    let assignment_fingerprint = supervision::state::assignment_fingerprint(packet.as_ref());
    let startup_grace_until = Instant::now() + startup_grace(record.agent);
    for alias in &aliases {
        alias_map.insert(alias.clone(), session_id);
        alias_map.insert(alias.to_ascii_lowercase(), session_id);
    }
    active_sessions.insert(
        session_id,
        ActiveSession {
            state: initial_status,
            record,
            packet,
            runtime,
            runtime_slot,
            launch_spec,
            launch_prompt,
            task_id,
            line_buffer: String::new(),
            raw_buffer: String::new(),
            started_at: Instant::now(),
            startup_grace_until,
            last_output_at: Instant::now(),
            output_chunks: 0,
            directive_count: 0,
            initial_status_received: false,
            output_chunks_at_last_status: 0,
            reported_overlap: None,
            stall_count: 0,
            restart_count: 0,
            restart_at: None,
            validation_pending: matches!(
                initial_status,
                SessionState::DoneClaimed | SessionState::NeedsValidation
            ),
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
            launch_prompt_sent: false,
            launch_prompt_sent_at: None,
            cleanup_authorized: false,
            last_status_update_at: None,
            last_status_file_modified: None,
            last_tmux_health: None,
            last_tmux_health_checked_at: None,
            last_supervisor_notice_key: None,
            recent_supervisor_notice_keys: VecDeque::new(),
            last_supervisor_state_card_key: None,
            zombie_debounce: health::ZombieDebounce::default(),
            health_state: health::SessionHealthState::new(),
            message_dedup: dedup::MessageDeduplicator::new(),
            task_stage: supervision::state::initial_stage(role, initial_status),
            assignment_fingerprint,
            plan_only_count: 0,
            last_status_signature: None,
            repeated_status_without_evidence: 0,
            first_status_incident_stage: 0,
            last_first_status_escalation_at: None,
            supervising_supervisor_id: None,
        },
    );
}
