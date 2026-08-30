use blake3::hash;

use crate::adapter::SupervisorEventType;
use crate::model::SessionState;
use crate::tmux::SessionHealth;

pub const STATE_CARD_INTERVAL_SECS: u64 = 90;
pub const STATUS_FILE_LIVENESS_GRACE_SECS: u64 = 20;

pub fn notice_key(event_type: SupervisorEventType, body: &str) -> String {
    let normalized = body.split_whitespace().collect::<Vec<_>>().join(" ");
    format!(
        "{}:{}",
        event_type.as_str(),
        hash(normalized.as_bytes()).to_hex()
    )
}

pub fn state_card_key(card: &str) -> String {
    hash(card.as_bytes()).to_hex().to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupervisorCondition {
    Healthy,
    ProbeNeeded,
    Unavailable,
}

pub fn classify_supervisor(
    state: SessionState,
    elapsed: std::time::Duration,
    stall_after: std::time::Duration,
    tmux_health: SessionHealth,
) -> SupervisorCondition {
    if matches!(state, SessionState::Failed | SessionState::Exited)
        || matches!(tmux_health, SessionHealth::Dead | SessionHealth::Zombie)
    {
        SupervisorCondition::Unavailable
    } else if matches!(
        tmux_health,
        SessionHealth::Healthy | SessionHealth::Starting
    ) {
        SupervisorCondition::Healthy
    } else if matches!(tmux_health, SessionHealth::Hung) && elapsed >= stall_after.mul_f64(3.0) {
        SupervisorCondition::ProbeNeeded
    } else if elapsed >= stall_after.mul_f64(4.0) {
        SupervisorCondition::ProbeNeeded
    } else {
        SupervisorCondition::Healthy
    }
}

pub fn build_takeover_prompt(card: &str, failed_supervisor_name: &str) -> String {
    format!(
        "TAKEOVER NOW.\nThe active supervisor {failed_supervisor_name} is unavailable or unhealthy.\nYou are now the acting supervisor. Resume active supervision immediately, diagnose liveness and reporting incidents, and force workers toward terminal states with evidence.\n\n{card}"
    )
}

pub fn summarize_notice(event_type: SupervisorEventType, body: &str) -> String {
    let prefix = match event_type {
        SupervisorEventType::Stall => "Stall incident",
        SupervisorEventType::DoneClaimed => "Validation required",
        SupervisorEventType::WeakOutput => "Weak execution",
        SupervisorEventType::Contradiction => "Ownership conflict",
        SupervisorEventType::Blocked => "Blocker incident",
        SupervisorEventType::Failed => "Runtime failure",
        SupervisorEventType::Notice => "Supervision update",
    };
    format!("{prefix}: {}", truncate_inline(body, 160))
}

pub fn summarize_action(action: &str, target: Option<&str>, summary: &str) -> String {
    let target = target.unwrap_or("mission");
    format!(
        "{} {}: {}",
        action.trim().replace('_', " "),
        target,
        truncate_inline(summary, 140)
    )
}

pub fn summarize_state_card(card: &str) -> String {
    let branch = card
        .lines()
        .find(|line| line.starts_with("Your branch:"))
        .map(|line| line.trim_start_matches("Your branch:").trim())
        .unwrap_or("supervision branch active");
    let global = card
        .lines()
        .find(|line| line.starts_with("Global mission:"))
        .map(|line| line.trim_start_matches("Global mission:").trim())
        .unwrap_or("workers=0");
    format!(
        "{} | {}",
        truncate_inline(branch, 96),
        truncate_inline(global, 96)
    )
}

fn truncate_inline(text: &str, max_chars: usize) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= max_chars {
        normalized
    } else {
        let mut rendered = normalized.chars().take(max_chars).collect::<String>();
        rendered.push_str("...");
        rendered
    }
}
