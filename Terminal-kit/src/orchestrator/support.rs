use super::*;

pub(super) fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        text.to_owned()
    } else {
        let mut rendered = text.chars().take(max_chars).collect::<String>();
        rendered.push_str("...");
        rendered
    }
}

pub(super) fn truncate_id(id: &Uuid) -> String {
    let hex = id.to_string();
    hex[..8].to_owned()
}

pub(super) fn pad_status(status: &str) -> String {
    let padded = format!("{status:<10}");
    match status {
        "running" | "launching" => format!("\x1b[33m{padded}\x1b[0m"),
        "completed" => format!("\x1b[32m{padded}\x1b[0m"),
        "failed" => format!("\x1b[31m{padded}\x1b[0m"),
        "planned" => format!("\x1b[36m{padded}\x1b[0m"),
        _ => padded,
    }
}

pub(super) fn trim_recent_utf8(buffer: &mut String, max_bytes: usize, keep_bytes: usize) {
    if buffer.len() <= max_bytes {
        return;
    }
    let target = buffer.len().saturating_sub(keep_bytes);
    let keep_from = previous_char_boundary(buffer, target);
    buffer.drain(..keep_from);
}

pub(super) fn previous_char_boundary(text: &str, index: usize) -> usize {
    let mut boundary = index.min(text.len());
    while boundary > 0 && !text.is_char_boundary(boundary) {
        boundary -= 1;
    }
    boundary
}

#[allow(dead_code)]
pub(super) fn parse_mail_id(value: &str) -> Option<Uuid> {
    Uuid::parse_str(value.trim()).ok()
}

pub(super) fn first_status_deadline() -> Duration {
    Duration::from_secs(25)
}

pub(super) fn first_status_escalation_interval(stage: u8) -> Duration {
    match stage {
        0 => Duration::from_secs(0),
        1 => Duration::from_secs(15),
        _ => Duration::from_secs(25),
    }
}

pub(super) fn first_status_failure_kind_name(kind: FirstStatusFailureKind) -> &'static str {
    match kind {
        FirstStatusFailureKind::Dispatch => "dispatch",
        FirstStatusFailureKind::Runtime => "runtime",
        FirstStatusFailureKind::StatusPipeline => "status_pipeline",
        FirstStatusFailureKind::Reporting => "reporting",
    }
}

pub(super) fn worker_status_file_path(
    control_surface: &ControlSurface,
    worker_name: &str,
) -> PathBuf {
    control_surface
        .workers_state_dir
        .join(worker_name)
        .join("status.json")
}

pub(super) fn worker_prompt_file_path(
    control_surface: &ControlSurface,
    worker_name: &str,
) -> PathBuf {
    control_surface
        .state_dir
        .join("prompts")
        .join(format!("{worker_name}.md"))
}

pub(super) fn worker_transcript_path(
    control_surface: &ControlSurface,
    worker_name: &str,
) -> PathBuf {
    control_surface
        .transcript_dir
        .join(format!("{worker_name}.log"))
}

pub(super) fn protocol_reminder_grace(agent: crate::agent::AgentKind) -> Duration {
    if agent == crate::agent::AgentKind::Qwen {
        Duration::from_secs(15)
    } else {
        Duration::from_secs(6)
    }
}

pub(super) fn startup_grace(agent: crate::agent::AgentKind) -> Duration {
    protocol_reminder_grace(agent) + Duration::from_secs(8)
}

pub(super) fn max_restart_attempts(role: SessionRole) -> usize {
    match role {
        SessionRole::Supervisor => 4,
        SessionRole::Worker => 3,
    }
}

pub(super) fn should_auto_restart(
    role: SessionRole,
    previous_state: SessionState,
    restart_count: usize,
) -> bool {
    !previous_state.is_terminal() && restart_count < max_restart_attempts(role)
}

pub(super) fn mass_failure_window() -> Duration {
    Duration::from_secs(30)
}

pub(super) fn intervention_cooldown_base() -> Duration {
    Duration::from_secs(30)
}

pub(super) fn intervention_cooldown_max() -> Duration {
    Duration::from_secs(120)
}

pub(super) fn restart_base_secs() -> u64 {
    2
}

pub(super) fn restart_crash_loop_threshold() -> usize {
    5
}

pub(super) fn restart_crash_loop_window() -> Duration {
    Duration::from_secs(600)
}

pub(super) fn zombie_check_max_inactivity() -> Duration {
    Duration::from_secs(180)
}

pub(super) fn trim_recent_failures(recent_failures: &mut Vec<RecentFailure>) {
    let cutoff = Instant::now() - mass_failure_window();
    recent_failures.retain(|entry| entry.recorded_at >= cutoff);
}

pub(super) fn directive_kind(directive: &SapphireDirective) -> &'static str {
    match directive {
        SapphireDirective::Status(_) => "status",
        SapphireDirective::Mail(_) => "mail",
        SapphireDirective::Ack(_) => "ack",
        SapphireDirective::Lease(_) => "lease",
    }
}

pub(super) fn event_type_for_state(state: &SessionState) -> SupervisorEventType {
    match state {
        SessionState::Stalled => SupervisorEventType::Stall,
        SessionState::DoneClaimed | SessionState::NeedsValidation => {
            SupervisorEventType::DoneClaimed
        }
        SessionState::WeakOutput | SessionState::WrongDirection => SupervisorEventType::WeakOutput,
        SessionState::Contradictory => SupervisorEventType::Contradiction,
        SessionState::Blocked => SupervisorEventType::Blocked,
        SessionState::Failed | SessionState::Exited => SupervisorEventType::Failed,
        _ => SupervisorEventType::Notice,
    }
}

pub(super) fn supervisor_decision_key(
    kind: SupervisorDecisionKind,
    target_session_id: Uuid,
) -> String {
    format!("{}:{target_session_id}", kind.as_str())
}

pub(super) fn queue_supervisor_decision(
    pending_supervisor_decisions: &mut HashMap<String, PendingSupervisorDecision>,
    kind: SupervisorDecisionKind,
    target_session_id: Uuid,
    reason: &str,
) -> bool {
    let key = supervisor_decision_key(kind, target_session_id);
    if let Some(existing) = pending_supervisor_decisions.get_mut(&key) {
        existing.reason = reason.to_owned();
        return false;
    }
    let now = Instant::now();
    pending_supervisor_decisions.insert(
        key,
        PendingSupervisorDecision {
            kind,
            target_session_id,
            reason: reason.to_owned(),
            queued_at: now,
            last_notified_at: now,
            notice_count: 0,
            autonomous_action_count: 0,
            last_autonomous_action_at: None,
        },
    );
    true
}

pub(super) fn clear_supervisor_decisions_for_target(
    pending_supervisor_decisions: &mut HashMap<String, PendingSupervisorDecision>,
    target_session_id: Uuid,
) {
    pending_supervisor_decisions
        .retain(|_, pending| pending.target_session_id != target_session_id);
}

pub(super) fn clear_resolved_supervisor_decisions(
    pending_supervisor_decisions: &mut HashMap<String, PendingSupervisorDecision>,
    active_sessions: &HashMap<Uuid, ActiveSession>,
    target_session_id: Uuid,
) {
    let Some(target) = active_sessions.get(&target_session_id) else {
        clear_supervisor_decisions_for_target(pending_supervisor_decisions, target_session_id);
        return;
    };

    pending_supervisor_decisions.retain(|_, pending| {
        if pending.target_session_id != target_session_id {
            return true;
        }
        match pending.kind {
            SupervisorDecisionKind::Validation => target.validation_pending,
            SupervisorDecisionKind::StallRecovery => target.state == SessionState::Stalled,
            SupervisorDecisionKind::LowConfidenceRecovery => target.low_confidence_count > 0,
            SupervisorDecisionKind::OverlapRecovery => target
                .reported_overlap
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty()),
        }
    });
}

pub(super) fn clear_transient_supervisor_decisions_on_output(
    pending_supervisor_decisions: &mut HashMap<String, PendingSupervisorDecision>,
    target_session_id: Uuid,
) {
    pending_supervisor_decisions.retain(|_, pending| {
        pending.target_session_id != target_session_id
            || matches!(
                pending.kind,
                SupervisorDecisionKind::Validation | SupervisorDecisionKind::OverlapRecovery
            )
    });
}

pub(super) fn intervention_cooldown(intervention_count: usize) -> Duration {
    let secs = intervention_cooldown_base()
        .as_secs()
        .saturating_mul(intervention_count.max(1) as u64);
    Duration::from_secs(secs.min(intervention_cooldown_max().as_secs()))
}

pub(super) fn is_in_cooldown(session: &ActiveSession, now: Instant) -> bool {
    session
        .intervention_cooldown_until
        .is_some_and(|until| now < until)
}

pub(super) fn record_intervention(
    session: &mut ActiveSession,
    intervention_type: &str,
    now: Instant,
) {
    session.total_interventions += 1;
    session.last_intervention_type = Some(intervention_type.to_owned());
    session.last_intervention_at = Some(now);
    session.intervention_cooldown_until =
        Some(now + intervention_cooldown(session.total_interventions));
}

pub(super) fn record_intervention_response(session: &mut ActiveSession, now: Instant) {
    if let Some(intervention_at) = session.last_intervention_at {
        session.last_response_time = Some(now.duration_since(intervention_at));
    }
    session.last_intervention_at = None;
}

pub(super) fn is_agent_mid_response(session: &ActiveSession) -> bool {
    session.last_output_at.elapsed() < NUDGE_QUIET_THRESHOLD && session.output_chunks > 0
}

pub(super) fn worker_output_has_settled(session: &ActiveSession, now: Instant) -> bool {
    session.output_chunks > 0
        && now.duration_since(session.last_output_at) >= HEURISTIC_SETTLE_THRESHOLD
}

pub(super) fn prompt_fingerprint(prompt: &str) -> String {
    let normalized = prompt.split_whitespace().collect::<Vec<_>>().join(" ");
    blake3::hash(normalized.as_bytes()).to_hex().to_string()
}

pub(super) fn prompt_repeat_suppression_window() -> Duration {
    Duration::from_secs(180)
}

pub(super) fn supervisor_notice_repeat_suppression_window() -> Duration {
    Duration::from_secs(240)
}

pub(super) fn prompt_queue_limit() -> usize {
    6
}

pub(super) fn prompt_dispatch_interval(session: &ActiveSession) -> Duration {
    Duration::from_secs(12 + (session.total_interventions.min(4) as u64 * 8))
}

pub(super) fn prune_recent_prompt_keys(session: &mut ActiveSession, now: Instant) {
    while session
        .recent_prompt_keys
        .front()
        .is_some_and(|(_, sent_at)| {
            now.duration_since(*sent_at) > prompt_repeat_suppression_window()
        })
    {
        session.recent_prompt_keys.pop_front();
    }
}

pub(super) fn has_recent_prompt_key(session: &ActiveSession, key: &str) -> bool {
    session
        .recent_prompt_keys
        .iter()
        .any(|(existing, _)| existing == key)
}

pub(super) fn remember_prompt_delivery(session: &mut ActiveSession, key: String, now: Instant) {
    session.last_prompt_sent_at = Some(now);
    session.recent_prompt_keys.push_back((key, now));
    while session.recent_prompt_keys.len() > 32 {
        session.recent_prompt_keys.pop_front();
    }
}

pub(super) fn clear_superseded_prompt_queue(session: &mut ActiveSession) {
    if session.queued_prompts.is_empty() {
        return;
    }
    session.queued_prompts.clear();
    session.queued_prompt_keys.clear();
}

pub(super) fn prune_recent_supervisor_notice_keys(session: &mut ActiveSession, now: Instant) {
    while session
        .recent_supervisor_notice_keys
        .front()
        .is_some_and(|(_, sent_at)| {
            now.duration_since(*sent_at) > supervisor_notice_repeat_suppression_window()
        })
    {
        session.recent_supervisor_notice_keys.pop_front();
    }
}

pub(super) fn has_recent_supervisor_notice_key(session: &ActiveSession, key: &str) -> bool {
    session
        .recent_supervisor_notice_keys
        .iter()
        .any(|(existing, _)| existing == key)
}

pub(super) fn remember_supervisor_notice(session: &mut ActiveSession, key: String, now: Instant) {
    session.last_supervisor_notice_key = Some(key.clone());
    session.recent_supervisor_notice_keys.push_back((key, now));
    while session.recent_supervisor_notice_keys.len() > 32 {
        session.recent_supervisor_notice_keys.pop_front();
    }
}

pub(super) fn has_recent_status_activity(session: &ActiveSession, now: Instant) -> bool {
    session.last_status_update_at.is_some_and(|at| {
        now.duration_since(at)
            < Duration::from_secs(supervisor::STATUS_FILE_LIVENESS_GRACE_SECS * 3)
    })
}

pub(super) fn session_tmux_health(session: &ActiveSession) -> Option<tmux::SessionHealth> {
    session.last_tmux_health
}

pub(super) fn session_has_live_terminal(session: &ActiveSession) -> bool {
    matches!(
        session_tmux_health(session),
        Some(
            tmux::SessionHealth::Healthy
                | tmux::SessionHealth::Hung
                | tmux::SessionHealth::Starting
        )
    )
}

pub(super) fn effective_stall_threshold(
    session: &ActiveSession,
    stall_after: Duration,
) -> Duration {
    if session.output_chunks > 0 || session.last_status_update_at.is_some() {
        stall_after.mul_f64(3.0)
    } else {
        stall_after
    }
}

pub(super) fn tmux_health_refresh_interval() -> Duration {
    Duration::from_secs(10)
}

pub(super) fn refresh_tmux_health_cache(active_sessions: &mut HashMap<Uuid, ActiveSession>) {
    let now = Instant::now();
    for session in active_sessions.values_mut() {
        if session.state.is_terminal() {
            continue;
        }
        if session
            .last_tmux_health_checked_at
            .is_some_and(|checked| now.duration_since(checked) < tmux_health_refresh_interval())
        {
            continue;
        }
        session.last_tmux_health_checked_at = Some(now);
        session.last_tmux_health = session.runtime.terminal_target().map(|target| {
            tmux::Tmux::new(None).check_session_health(target, zombie_check_max_inactivity())
        });
    }
}

pub(super) fn recently_prompted(session: &ActiveSession, now: Instant) -> bool {
    session
        .last_prompt_sent_at
        .is_some_and(|last| now.duration_since(last) < prompt_dispatch_interval(session))
}

pub(super) fn send_prompt_immediately(session: &mut ActiveSession, prompt: &str) -> Result<()> {
    let now = Instant::now();
    let key = prompt_fingerprint(prompt);
    prune_recent_prompt_keys(session, now);

    if has_recent_prompt_key(session, &key) {
        warn!(
            worker = %session.record.name,
            key = %key,
            "BLOCKED duplicate prompt delivery (same hash recently sent)"
        );
        return Ok(());
    }

    if is_agent_mid_response(session) {
        let elapsed = now.saturating_duration_since(session.last_output_at);
        warn!(
            worker = %session.record.name,
            elapsed_ms = elapsed.as_millis(),
            "send_prompt_immediately: agent mid-response, delaying 500ms"
        );
        std::thread::sleep(Duration::from_millis(500));
    }

    session.runtime.send_prompt(prompt)?;
    remember_prompt_delivery(session, key, now);
    Ok(())
}

pub(super) fn send_or_queue_prompt(session: &mut ActiveSession, prompt: &str) -> bool {
    let now = Instant::now();
    let key = prompt_fingerprint(prompt);
    prune_recent_prompt_keys(session, now);

    if session.queued_prompt_keys.contains(&key) || has_recent_prompt_key(session, &key) {
        return false;
    }

    if is_agent_mid_response(session) || recently_prompted(session, now) {
        if session.queued_prompts.len() >= prompt_queue_limit() {
            if let Some(evicted) = session.queued_prompts.pop_front() {
                session.queued_prompt_keys.remove(&evicted.key);
            }
        }
        session.queued_prompt_keys.insert(key.clone());
        session.queued_prompts.push_back(QueuedPrompt {
            key,
            body: prompt.to_owned(),
        });
        false
    } else {
        let _ = session.runtime.send_prompt(prompt);
        remember_prompt_delivery(session, key, now);
        true
    }
}

pub(super) fn drain_prompt_queues(active_sessions: &mut HashMap<Uuid, ActiveSession>) {
    let mut drained = Vec::new();
    let now = Instant::now();
    for (session_id, session) in active_sessions.iter_mut() {
        prune_recent_prompt_keys(session, now);
        if is_agent_mid_response(session) || recently_prompted(session, now) {
            continue;
        }
        if let Some(prompt) = session.queued_prompts.pop_front() {
            session.queued_prompt_keys.remove(&prompt.key);
            let _ = session.runtime.send_prompt(&prompt.body);
            remember_prompt_delivery(session, prompt.key, now);
            drained.push((*session_id, prompt.body));
        }
    }
    for (_, prompt) in &drained {
        tracing::debug!(
            "drained queued prompt: {}",
            prompt.chars().take(80).collect::<String>()
        );
    }
}

pub(super) fn append_transcript(
    control_surface: &ControlSurface,
    session_name: &str,
    chunk: &str,
) -> Result<()> {
    let path = control_surface
        .transcript_dir
        .join(format!("{session_name}.log"));
    append_to_file(&path, chunk)
}

pub(super) fn resolve_alias(alias_map: &HashMap<String, Uuid>, alias: &str) -> Option<Uuid> {
    let direct = alias.trim();
    alias_map
        .get(direct)
        .copied()
        .or_else(|| alias_map.get(&direct.to_ascii_lowercase()).copied())
}

pub(super) fn persist_runtime_event(
    store: &Store,
    mission_id: Uuid,
    event: &RuntimeEvent,
) -> Result<()> {
    match event {
        RuntimeEvent::Output { .. } => Ok(()),
        RuntimeEvent::Automation {
            session_id,
            rule_name,
        } => store.append_json_event(
            mission_id,
            Some(*session_id),
            "automation",
            format!("fired {}", rule_name),
            event,
        ),
        RuntimeEvent::Exited {
            session_id,
            exit_code,
        } => store.append_json_event(
            mission_id,
            Some(*session_id),
            "process_exit",
            "session exited",
            &json!({ "exit_code": exit_code }),
        ),
    }
}

pub(super) fn launch_command(program: &str, args: &[String]) -> Vec<String> {
    let mut command = vec![program.to_owned()];
    command.extend(args.iter().cloned());
    command
}

pub(super) fn runtime_capture_dir(mission_id: Uuid) -> PathBuf {
    std::env::temp_dir()
        .join("sp-runtime-capture")
        .join(mission_id.to_string())
}

pub(super) fn write_string_to_file(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content).with_context(|| format!("failed to write {}", path.display()))
}

pub(super) fn append_to_file(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(content.as_bytes())?;
    Ok(())
}

pub(super) fn embed_initial_prompt_if_supported(
    agent: crate::agent::AgentKind,
    spec: ProcessLaunchSpec,
    _prompt: &str,
) -> (ProcessLaunchSpec, bool) {
    let _ = agent;
    (spec, false)
}

pub(super) fn harden_supervisor_launch_spec(
    agent: crate::agent::AgentKind,
    mut spec: ProcessLaunchSpec,
) -> ProcessLaunchSpec {
    if agent == crate::agent::AgentKind::Qwen
        && !spec.args.iter().any(|arg| arg == "--screen-reader")
    {
        spec.args.insert(0, "--screen-reader".to_owned());
    }
    spec
}

pub(super) fn rebuild_launch_spec(
    session: &SessionRecord,
    repo: &Path,
    state_dir: &Path,
) -> ProcessLaunchSpec {
    let mut base = session.agent.build_launch_spec(repo, state_dir, &[]);
    if let Some((program, args)) = session.launch_command.split_first() {
        base.program = program.clone();
        base.args = args.to_vec();
    }
    if session.role == SessionRole::Supervisor {
        base = harden_supervisor_launch_spec(session.agent, base);
    }
    base.surface_label = session.name.clone();
    base
}
