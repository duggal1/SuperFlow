//! Prompt construction for the voice-launched terminal team.
//!
//! Every worker receives the full faithful mission. The brain coordinates and
//! deduplicates work across the team.

use log::info;

/// Prompts are typed into interactive TUI readlines; embedded newlines would
/// submit early. Voice missions are prose, so collapsing runs of whitespace
/// to single spaces is lossless in practice.
fn single_line(text: &str) -> String {
    let collapsed: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed.chars().take(4000).collect()
}

/// One faithful prompt per worker; the brain terminal coordinates ownership.
pub async fn split_worker_prompts(mission: &str, workers: usize) -> Vec<String> {
    let mission = single_line(mission);
    info!(target: "voice_terminal", "Using faithful full-mission prompts for every worker");
    vec![mission; workers.max(1)]
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
