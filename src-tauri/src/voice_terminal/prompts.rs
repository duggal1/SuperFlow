//! Supervisor bootstrap prompt for voice-launched terminal teams.

use super::grammar::AgentKind;

const TEAMMATE_FOOTER: &str = "You are Agent {n} on a concurrent team. Own only your assigned scope. Preserve teammate changes; never revert, reset, delete, or overwrite work you did not create.";

pub(crate) fn single_line(text: &str) -> String {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed.chars().take(4_000).collect()
}

pub fn supervisor_prompt(
    transcript_mission: &str,
    agent: AgentKind,
    workers: usize,
    session: &str,
) -> String {
    let mission = single_line(transcript_mission);
    format!(
        r#"You are the sole supervisor for this engineering mission. Act as the senior technical authority: maximize correctness and useful parallel throughput, protect the shared repository, and return one verified result.

Mission: {mission}
Worker CLI: {agent}
Workers requested: {workers}
Current tmux session: {session}

Operating contract:
1. Privately turn the mission into an execution plan. Launch exactly {workers} workers. You alone decompose work, write worker prompts, launch terminals, supervise, validate, and integrate. Workers must never launch other workers.
2. Before launch, inspect repository instructions and the dirty working tree. Give every worker a distinct role, skills, starting point, owned files or domain, exact task, out-of-scope boundary, definition of done, required evidence, blocker protocol, and output format. Avoid overlapping write ownership.
3. Write each complete assignment to an absolute temporary file, then start the worker CLI in its tmux pane and paste only a short loader that identifies Agent N and tells it to read @<absolute-assignment-file>. End every assignment with this exact team rule, replacing {{n}}: "{teammate_footer}"
4. Use only this tmux session. Never open another macOS terminal window and never create one external tab per worker. Keep at most six panes in each tmux window, including your supervisor pane. Put yourself plus up to five workers in the first window; put overflow workers in additional tmux windows, at most six workers each. Use a side-by-side split for two total panes and a tiled 2x2 layout for three or four total panes; tile larger windows.
5. Record every worker pane ID. Inspect progress with `tmux capture-pane`. Send every answer, correction, retry, or validation request back to that same pane using a named tmux buffer plus Enter. Never create a new pane, window, tab, or agent for a follow-up. Never send the same unchanged prompt twice.
6. Supervise in bounded, evidence-driven rounds. Do not start a perpetual polling loop or an autonomous supervisor-worker conversation loop. Intervene only for a blocker, drift, conflict, weak evidence, or completion claim. Stop when all workers are validated or failed, or when a real user decision is required.
7. A worker saying "done" is only a claim. Require exact changed files, commands and tests, observed results, remaining risks, and overlap status. Preserve concurrent teammate changes; never use git restore/reset, never discard unrelated work, and never push unless the user explicitly asked.
8. Integrate the result, run the repository's relevant checks, resolve contradictions, then give the user one concise final report: completed work, validation, failures, and remaining risk. Do not keep supervising after finalization.

Start now. Do not restate this brief and do not ask for confirmation unless execution is genuinely blocked."#,
        agent = agent.executable(),
        teammate_footer = TEAMMATE_FOOTER,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supervisor_owns_all_prompting_and_launches() {
        let prompt = supervisor_prompt(
            "Fix the backend and verify it",
            AgentKind::Codex,
            4,
            "sf-supervisor-1",
        );
        assert!(prompt.contains("Mission: Fix the backend and verify it"));
        assert!(prompt.contains("Worker CLI: codex"));
        assert!(prompt.contains("Workers requested: 4"));
        assert!(prompt.contains("You alone decompose work, write worker prompts"));
        assert!(prompt.contains("read @<absolute-assignment-file>"));
    }

    #[test]
    fn layout_follow_up_and_stop_rules_are_explicit() {
        let prompt = supervisor_prompt("ship it", AgentKind::Claude, 7, "sf-supervisor-2");
        assert!(prompt.contains("at most six panes in each tmux window"));
        assert!(prompt.contains("same pane"));
        assert!(prompt.contains("Never create a new pane, window, tab, or agent for a follow-up"));
        assert!(prompt.contains("Do not start a perpetual polling loop"));
        assert!(prompt.contains("Stop when all workers are validated or failed"));
    }

    #[test]
    fn teammate_footer_is_between_twenty_and_thirty_words() {
        let rendered = TEAMMATE_FOOTER.replace("{n}", "1");
        let word_count = rendered.split_whitespace().count();
        assert!((20..=30).contains(&word_count), "word count: {word_count}");
        assert!(rendered.contains("never revert, reset, delete, or overwrite"));
    }

    #[test]
    fn mission_is_collapsed_to_one_bounded_line() {
        let prompt = supervisor_prompt("fix\n\n  this\tthing", AgentKind::Kilo, 1, "session");
        assert!(prompt.contains("Mission: fix this thing\n"));
    }
}
