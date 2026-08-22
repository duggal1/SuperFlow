//! Prompt construction for the voice-launched terminal team.
//!
//! Tier A (Apple Intelligence, on-device) splits a spoken mission into one
//! self-contained prompt per worker terminal. If Apple Intelligence is
//! unavailable or fails, every worker receives the full mission — a
//! deterministic fallback that always works offline.

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
use crate::apple_intelligence;
use log::{info, warn};

/// Prompts are typed into interactive TUI readlines; embedded newlines would
/// submit early. Voice missions are prose, so collapsing runs of whitespace
/// to single spaces is lossless in practice.
fn single_line(text: &str) -> String {
    let collapsed: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed.chars().take(4000).collect()
}

const SPLIT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);
const SPLIT_MAX_TOKENS: i32 = 2048;

fn split_system_prompt(workers: usize) -> String {
    format!(
        "You decompose a spoken software mission into exactly {workers} independent worker prompts. \
Each prompt must be fully self-contained (the worker sees no other context), reference only what \
the mission states, and be phrased as an instruction to a coding agent. Cover the whole mission \
across the set without duplication. Never ask questions. \
Reply with ONLY a JSON array of exactly {workers} strings. No markdown, no commentary."
    )
}

fn extract_json_array(raw: &str) -> Option<Vec<String>> {
    let start = raw.find('[')?;
    let end = raw.rfind(']')?;
    if end < start {
        return None;
    }
    let parsed: Vec<String> = serde_json::from_str(&raw[start..=end]).ok()?;
    let cleaned: Vec<String> = parsed
        .into_iter()
        .map(|s| single_line(&s))
        .filter(|s| !s.is_empty())
        .collect();
    (!cleaned.is_empty()).then_some(cleaned)
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
async fn apple_split(mission: &str, workers: usize) -> Option<Vec<String>> {
    if !apple_intelligence::check_apple_intelligence_availability() {
        return None;
    }
    let system = split_system_prompt(workers);
    let user = mission.to_string();
    let call = tokio::task::spawn_blocking(move || {
        apple_intelligence::process_text_with_system_prompt(&system, &user, SPLIT_MAX_TOKENS)
    });
    match tokio::time::timeout(SPLIT_TIMEOUT, call).await {
        Ok(Ok(Ok(raw))) => {
            let prompts = extract_json_array(&raw);
            if prompts.is_none() {
                warn!(target: "voice_terminal", "Apple Intelligence returned unparseable split output");
            }
            prompts
        }
        Ok(Ok(Err(err))) => {
            warn!(target: "voice_terminal", "Apple Intelligence split failed: {err}");
            None
        }
        Ok(Err(err)) => {
            warn!(target: "voice_terminal", "Apple Intelligence split task panicked: {err}");
            None
        }
        Err(_) => {
            warn!(target: "voice_terminal", "Apple Intelligence split timed out");
            None
        }
    }
}

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
async fn apple_split(_mission: &str, _workers: usize) -> Option<Vec<String>> {
    None
}

/// One prompt per worker terminal. Primary path: Apple Intelligence on-device.
/// Fallback: the full mission to every worker (the brain terminal dedupes).
pub async fn split_worker_prompts(mission: &str, workers: usize) -> Vec<String> {
    let mission = single_line(mission);
    if mission.is_empty() || workers <= 1 {
        return vec![mission; workers.max(1)];
    }

    if let Some(mut prompts) = apple_split(&mission, workers).await {
        while prompts.len() < workers {
            let last = prompts.last().cloned().unwrap_or_else(|| mission.clone());
            prompts.push(last);
        }
        prompts.truncate(workers);
        info!(target: "voice_terminal", "Apple Intelligence produced {} worker prompts", prompts.len());
        return prompts;
    }

    info!(target: "voice_terminal", "Using deterministic fallback prompts (full mission per worker)");
    vec![mission; workers]
}

/// The prompt typed into the brain terminal: full mission + the roster of
/// tmux pane targets it supervises.
pub fn brain_prompt(
    transcript_mission: &str,
    worker_briefs: &[String],
    roster_lines: &[String],
) -> String {
    let mission = single_line(transcript_mission);
    let mut prompt = String::new();
    prompt.push_str(
        "You are the BRAIN (supervisor) of a local voice-driven coding team running in tmux panes. ",
    );
    if mission.is_empty() {
        prompt.push_str(
            "No spoken mission was given yet — stand by; the user will dictate the mission here. ",
        );
    } else {
        prompt.push_str(&format!("Spoken mission from the user: \"{mission}\". "));
    }

    if !worker_briefs.is_empty() && !roster_lines.is_empty() {
        prompt.push_str("Workers already received their own briefs and are working now:\n");
        for (i, line) in roster_lines.iter().enumerate() {
            let brief = worker_briefs
                .get(i)
                .map(|p| {
                    let chars: String = p.chars().take(300).collect();
                    chars
                })
                .unwrap_or_default();
            prompt.push_str(&format!("{line} brief: {brief}\n"));
        }
        prompt.push_str(
            "Your job: supervise, do not implement. Periodically inspect each worker with \
`tmux capture-pane -p -t <pane>` and coordinate by typing into panes with \
`tmux send-keys -l -t <pane> '<text>'` followed by `tmux send-keys -t <pane> Enter`. \
Redirect workers that drift, resolve file conflicts between them, integrate finished work, \
and run the project's typecheck/lint when the mission looks complete. \
Keep your own terminal as the live status board.",
        );
    } else {
        prompt.push_str(
            "Your job: await instructions from the user (they will speak through SuperFlow and the \
transcript lands here) and drive this terminal yourself.",
        );
    }
    prompt
}
