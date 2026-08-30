use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use tokio::task::JoinSet;
use uuid::Uuid;

use crate::agent::AgentKind;
use crate::cli::NoSupervisorLaunchConfig;
use crate::internal::ui::theme::{ansi, unicode::Symbol};
use crate::runtime::{ProcessLaunchSpec, SessionRuntime};
use crate::tmux::Tmux;

const PANES_PER_SESSION: usize = 10;
const DEFAULT_FAST_PROMPT_DELAY: Duration = Duration::from_millis(1_500);
const QWEN_FAST_PROMPT_DELAY: Duration = Duration::from_millis(4_500);
const QWEN_STARTUP_WAKE_DELAY: Duration = Duration::from_millis(1_000);
const STARTUP_SETTLE_BUFFER: Duration = Duration::from_millis(100);
const TERMINAL_OPEN_DELAY: Duration = Duration::from_millis(120);

pub async fn launch(config: NoSupervisorLaunchConfig) -> Result<String> {
    if !Tmux::is_available() {
        anyhow::bail!("tmux is required for `sp ns`");
    }

    let tmux = Tmux::new(None);
    let session_names = tmux
        .create_batch_sessions(
            &config.session_name,
            &config.repo.to_string_lossy(),
            config.count,
            PANES_PER_SESSION,
        )
        .map_err(anyhow::Error::msg)
        .context("failed to create tmux sessions for `sp ns`")?;

    let runtime_root = ns_runtime_root(&config);
    let transcript_dir = runtime_root.join("transcripts");
    std::fs::create_dir_all(&transcript_dir)?;

    let runtimes = session_names
        .iter()
        .map(|session_name| SessionRuntime::with_tmux(session_name.clone(), transcript_dir.clone()))
        .collect::<Vec<_>>();

    let mut launches = Vec::with_capacity(config.count);
    let mut prompt_targets = Vec::with_capacity(config.count);
    for index in 0..config.count {
        let runtime_index = index / PANES_PER_SESSION;
        let prompt = config.prompts[index].clone();
        let mut spec =
            config
                .agent
                .build_launch_spec(&config.repo, &runtime_root, &config.worker_args);
        spec.surface_label = format!("{}-{}", config.agent.as_str(), index + 1);
        apply_fast_ns_profile(config.agent, &mut spec);
        let prompt_delay = spec.prompt_delay;
        let running = runtimes[runtime_index]
            .spawn(Uuid::new_v4(), spec)
            .with_context(|| format!("failed to spawn terminal {}", index + 1))?;
        if let Some(pane_id) = running.terminal_target() {
            prompt_targets.push((pane_id.to_owned(), prompt.clone()));
        }
        launches.push((running, prompt, prompt_delay));
    }

    let terminal_notice = open_external_terminals(&tmux, &session_names).err();

    if let Some(delay) = launches.iter().map(|(_, _, delay)| *delay).max() {
        tokio::time::sleep(delay + STARTUP_SETTLE_BUFFER).await;
    } else {
        tokio::time::sleep(Duration::from_millis(250) + STARTUP_SETTLE_BUFFER).await;
    }

    dispatch_prompts_parallel(&prompt_targets).await?;

    tokio::time::sleep(TERMINAL_OPEN_DELAY).await;

    Ok(render_summary(
        &config,
        &session_names,
        terminal_notice.as_ref(),
    ))
}

fn apply_fast_ns_profile(agent: AgentKind, spec: &mut ProcessLaunchSpec) {
    match agent {
        AgentKind::Qwen => {
            spec.prompt_delay = QWEN_FAST_PROMPT_DELAY;
            spec.startup_input = Some((QWEN_STARTUP_WAKE_DELAY, "\n".to_owned()));
        }
        AgentKind::Forge | AgentKind::Codex | AgentKind::Claude => {
            spec.prompt_delay = spec.prompt_delay.min(DEFAULT_FAST_PROMPT_DELAY);
        }
    }
}

fn ns_runtime_root(config: &NoSupervisorLaunchConfig) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    std::env::temp_dir().join(format!(
        "sp-ns-{}-{}-{stamp}",
        config.agent.as_str(),
        std::process::id()
    ))
}

#[cfg(target_os = "macos")]
fn open_external_terminals(tmux: &Tmux, session_names: &[String]) -> Result<()> {
    if session_names.is_empty() {
        return Ok(());
    }

    tmux.open_ghostty_batch_tabs(session_names)
        .map_err(anyhow::Error::msg)
        .context("could not auto-open Ghostty tabs")
}

#[cfg(not(target_os = "macos"))]
fn open_external_terminals(_tmux: &Tmux, _session_names: &[String]) -> Result<()> {
    Ok(())
}

fn render_summary(
    config: &NoSupervisorLaunchConfig,
    session_names: &[String],
    terminal_notice: Option<&anyhow::Error>,
) -> String {
    let repo_name = config
        .repo
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("repo");
    let width = summary_width();
    let rule = ansi::rule(&"-".repeat(width));
    let details = [
        format!(
            "{}  {}",
            ansi::brand_bold("agent"),
            ansi::text(&format!("{} × {}", config.agent.as_str(), config.count))
        ),
        format!("{}   {}", ansi::brand_bold("repo"), ansi::text(repo_name)),
        format!(
            "{}   {}",
            ansi::brand_bold("tmux"),
            ansi::muted(&session_names.join(", "))
        ),
    ];

    let mut lines = Vec::new();
    lines.push(String::new());
    lines.push(format!("  {rule}"));
    lines.push(format!(
        "  {} {}",
        ansi::success_bold(Symbol::Success.as_str()),
        ansi::text_bold("Ready")
    ));
    for line in details {
        lines.push(format!("  {line}"));
    }
    if terminal_notice.is_some() {
        lines.push(format!(
            "  {} {}",
            ansi::muted("open"),
            ansi::text("tmux attach manually if the terminal did not open")
        ));
    }
    lines.push(format!("  {rule}"));
    lines.join("\n")
}

fn summary_width() -> usize {
    terminal_width()
        .map(|width| width.saturating_sub(10).clamp(36, 72))
        .unwrap_or(56)
}

fn terminal_width() -> Option<usize> {
    let output = std::process::Command::new("tput")
        .arg("cols")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<usize>()
        .ok()
}

async fn dispatch_prompts_parallel(prompt_targets: &[(String, String)]) -> Result<()> {
    let mut tasks = JoinSet::new();
    for (pane_id, prompt) in prompt_targets {
        let pane_id = pane_id.clone();
        let prompt = prompt.clone();
        tasks.spawn_blocking(move || submit_prompt_fast(&pane_id, &prompt));
    }

    while let Some(result) = tasks.join_next().await {
        result
            .context("parallel prompt dispatch task failed")?
            .context("failed to submit no-supervisor prompt")?;
    }

    Ok(())
}

fn submit_prompt_fast(pane_id: &str, prompt: &str) -> Result<()> {
    let tmux = Tmux::new(None);
    let body = prompt.trim_end_matches(['\r', '\n']);
    if body.is_empty() {
        tmux.send_enter(pane_id).map_err(anyhow::Error::msg)?;
        return Ok(());
    }

    if body.contains('\n') || body.contains('\r') {
        tmux.paste_text_via_buffer(pane_id, body)
            .map_err(anyhow::Error::msg)?;
    } else {
        tmux.send_keys_literal(pane_id, body)
            .map_err(anyhow::Error::msg)?;
    }
    tmux.send_enter(pane_id).map_err(anyhow::Error::msg)?;
    Ok(())
}
