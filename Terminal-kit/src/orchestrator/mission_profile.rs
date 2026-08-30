use crate::cli::LaunchConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MissionProfile {
    /// Whether the mission requires multi-agent coordination (affects prompt content).
    pub coordination_focused: bool,
    /// Whether to run multiple active supervisors for scale.
    pub enable_supervisor_team: bool,
    /// Always true — state cards always sent to supervisor.
    pub enable_state_cards: bool,
    /// Always true — protocol reminders always sent.
    pub enable_protocol_reminders: bool,
    /// Always true — health probes always active before stall.
    pub enable_health_probes: bool,
}

impl MissionProfile {
    pub fn from_launch(config: &LaunchConfig) -> Self {
        let lowered = config.mission.to_ascii_lowercase();
        let coordination_focused = contains_any(
            &lowered,
            &[
                "teammate",
                "coordinate",
                "coordination",
                "work together",
                "collaborate",
                "talk to your teammate",
                "prove",
                "mail",
                "agent-to-agent",
                "inter-agent",
            ],
        );

        // ALL safety nets always enabled. No "lean" mode. No cheaping out.
        Self {
            coordination_focused,
            enable_supervisor_team: true,
            enable_state_cards: true,
            enable_protocol_reminders: true,
            enable_health_probes: true,
        }
    }
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}
