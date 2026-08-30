//! Minimal terminal renderer for Sapphire control.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use super::state::*;
use super::widgets::*;
use std::time::Instant;

use crate::internal::ui::shimmer::current_spinner_frame;
use crate::internal::ui::theme::unicode::Symbol;

const WARNING_ORANGE: Color = Color::Rgb(255, 138, 169);
const SHIMMER_IDLE_RGB: (u8, u8, u8) = (156, 143, 187);
const SHIMMER_BASE_RGB: (u8, u8, u8) = (193, 136, 255);
const SHIMMER_HOT_RGB: (u8, u8, u8) = (245, 233, 255);

// One static instant for smooth time-based shimmer
fn shimmer_start() -> &'static Instant {
    use std::sync::OnceLock;
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now)
}

const LOADING_PHRASES: &[&str] = &[
    "Discombobulating...",
    "Recombobulating...",
    "Boondoggling...",
    "Flibbertigibbeting...",
    "Prestidigitating...",
    "Hullaballooing...",
    "Tomfoolering...",
    "Shenaniganing...",
    "Razzledazzling...",
    "Fiddlefaddling...",
    "Skedaddling...",
    "Canoodling...",
    "Whatchamacalliting...",
    "Bebopping...",
    "Spelunking...",
    "Gallivanting...",
    "Osmosing...",
    "Nebulizing...",
    "Nucleating...",
    "Transmuting...",
    "Caramelizing...",
    "Fermenting...",
    "Sockhopping...",
    "Topsyturvying...",
    "Wibbling...",
    "Schlepping...",
    "Jitterbugging...",
    "Moonwalking...",
    "Quantumizing...",
    "Hyperspacing...",
    "Smooshing...",
    "Orbitalizing...",
    "Galaxifying...",
    "Supernovaing...",
    "Wormholing...",
    "Constellating...",
    "Cosmifying...",
    "Quasaring...",
    "Pulsaring...",
    "Singularitizing...",
    "Asteroiding...",
    "Darkmattering...",
    "Redshifting...",
    "Moonquaking...",
    "Starforging...",
    "Voidmaxxing...",
    "Planetizing...",
    "Celestializing...",
    "Magnetaring...",
    "Parallaxing...",
    "Gravwaving...",
    "Spectralizing...",
    "Exoplaneting...",
    "Cosmoscrutinizing...",
    "Nebulonizing...",
    "Blackholing...",
    "Starglitching...",
    "Vacuumizing...",
    "Eclipsifying...",
    "Lagranging...",
    "Novafrying...",
    "Cometizing...",
    "Peculiarizing...",
    "Meandering...",
    "Shapeshifting...",
    "Mischiefing...",
    "Goblinizing...",
    "Gremlining...",
    "Crypticizing...",
    "Befuddling...",
    "Bamboozling...",
    "Snickering...",
    "Hijinksing...",
    "Wonkifying...",
    "Unhinging...",
    "Yapping...",
    "Scampering...",
    "Frolicking...",
    "Glitchifying...",
    "Confounding...",
    "Warping...",
    "Fractaling...",
    "Mutating...",
    "Thingamabobbing...",
    "Contraptioning...",
    "Doodading...",
    "Kerfuffling...",
    "Absurdifying...",
    "Chaosengineering...",
    "Plasmatizing...",
    "Hyperventilating...",
    "Crystallizing...",
    "Unrealitying...",
];

const LOADING_PHRASE_INTERVAL_SECS: u64 = 6;

fn loading_phrase(timer_seconds: u64) -> &'static str {
    let phrase_index = (timer_seconds / LOADING_PHRASE_INTERVAL_SECS) as usize;
    LOADING_PHRASES[phrase_index % LOADING_PHRASES.len()]
}

pub fn render(
    frame: &mut Frame<'_>,
    snapshot: &RuntimeSnapshot,
    timer_seconds: u64,
    scroll: u16,
    frame_count: usize,
    show_quit_warning: bool,
) {
    let area = frame.area();
    let has_summary = snapshot.is_done && snapshot.execution_summary.final_summary.is_some();

    let mut constraints = Vec::new();
    constraints.push(Constraint::Length(3));
    constraints.push(Constraint::Min(if has_summary { 10 } else { 14 }));
    constraints.push(Constraint::Length(4));
    if has_summary {
        constraints.push(Constraint::Min(5));
    }
    constraints.push(Constraint::Length(1));

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(constraints)
        .split(area);

    let mut idx = 0;
    render_header(frame, chunks[idx], snapshot, timer_seconds);
    idx += 1;
    render_team(frame, chunks[idx], snapshot, scroll);
    idx += 1;
    render_control(frame, chunks[idx], snapshot);
    idx += 1;
    if has_summary {
        render_summary(frame, chunks[idx], snapshot);
        idx += 1;
    }
    render_footer(frame, chunks[idx], snapshot, timer_seconds, frame_count);

    if show_quit_warning {
        render_quit_warning(frame, area);
    }
}

fn render_header(
    frame: &mut Frame<'_>,
    area: Rect,
    snapshot: &RuntimeSnapshot,
    timer_seconds: u64,
) {
    let status = snapshot.mission_status.as_str();
    let status_color = match status {
        "running" | "launching" => PURPLE,
        "completed" => GREEN,
        "failed" => RED,
        "planning" => PURPLE,
        _ => GRAY,
    };

    let supervisor_count = snapshot
        .supervisors
        .len()
        .max(snapshot.supervisor.iter().count());
    let wd = &snapshot.watchdog;

    let mut top = vec![
        Span::styled(
            "Sapphire",
            Style::default().fg(WHITE).add_modifier(Modifier::BOLD),
        ),
        Span::styled("  ", Style::default()),
        Span::styled(
            format!("{} ", Symbol::Info),
            Style::default().fg(status_color),
        ),
        Span::styled(
            status.to_ascii_uppercase(),
            Style::default()
                .fg(status_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  ", Style::default()),
        Span::styled("⏱ ", Style::default().fg(DARK)),
        Span::styled(
            format!("{} elapsed", format_clock(timer_seconds)),
            Style::default().fg(GRAY),
        ),
    ];

    if snapshot.problem_agent_count() > 0 {
        top.push(Span::styled("  ", Style::default()));
        top.push(Span::styled(
            format!("{} attention", snapshot.problem_agent_count()),
            Style::default().fg(YELLOW).add_modifier(Modifier::BOLD),
        ));
    }

    let bottom = vec![
        Span::styled("Supervisors ", Style::default().fg(DARK)),
        Span::styled(supervisor_count.to_string(), Style::default().fg(WHITE)),
        Span::styled("  Agents ", Style::default().fg(DARK)),
        Span::styled(
            snapshot.agent_count().to_string(),
            Style::default().fg(WHITE),
        ),
        Span::styled("  Working ", Style::default().fg(DARK)),
        Span::styled(
            snapshot.active_agent_count().to_string(),
            Style::default().fg(WHITE),
        ),
        Span::styled("  Review ", Style::default().fg(DARK)),
        Span::styled(
            wd.validation_queue.len().to_string(),
            Style::default().fg(PURPLE),
        ),
        Span::styled("  Mail ", Style::default().fg(DARK)),
        Span::styled(wd.mail_routed.to_string(), Style::default().fg(WHITE)),
        Span::styled("  Stalls ", Style::default().fg(DARK)),
        Span::styled(
            wd.stall_interventions.to_string(),
            Style::default().fg(if wd.stall_interventions > 0 {
                YELLOW
            } else {
                WHITE
            }),
        ),
    ];

    frame.render_widget(
        Paragraph::new(vec![
            Line::from(top),
            Line::from(bottom),
            Line::from(Span::styled(
                "─".repeat(area.width.saturating_sub(1) as usize),
                Style::default().fg(BORDER),
            )),
        ]),
        area,
    );
}

fn render_team(frame: &mut Frame<'_>, area: Rect, snapshot: &RuntimeSnapshot, scroll: u16) {
    let mut lines = vec![section_title("People")];

    let mut supervisors = snapshot.supervisors.clone();
    if supervisors.is_empty() {
        if let Some(supervisor) = snapshot.supervisor.clone() {
            supervisors.push(supervisor);
        }
    }
    supervisors.sort_by(|left, right| {
        right
            .is_active_supervisor
            .cmp(&left.is_active_supervisor)
            .then_with(|| left.name.cmp(&right.name))
    });

    if supervisors.is_empty() && snapshot.agents.is_empty() {
        lines.push(Line::from(Span::styled(
            "Awaiting agent launch",
            Style::default().fg(GRAY),
        )));
    } else {
        if !supervisors.is_empty() {
            lines.push(Line::from(Span::styled(
                "Supervisors",
                Style::default().fg(DARK).add_modifier(Modifier::BOLD),
            )));
        }
        for supervisor in &supervisors {
            let workers: Vec<_> = snapshot
                .agents
                .iter()
                .filter(|agent| agent.owner_supervisor.as_deref() == Some(supervisor.name.as_str()))
                .collect();
            lines.push(render_supervisor_row(supervisor, &workers));
        }

        lines.push(Line::default());
        lines.push(Line::from(Span::styled(
            "Agents",
            Style::default().fg(DARK).add_modifier(Modifier::BOLD),
        )));
        let mut agents = snapshot.agents.clone();
        agents.sort_by(|a, b| a.name.cmp(&b.name));
        for agent in &agents {
            let (dot, color) = status_dot(agent.status);
            lines.push(Line::from(vec![
                Span::styled(format!("{dot} "), Style::default().fg(color)),
                Span::styled(
                    truncate(&agent.name, 18),
                    Style::default().fg(WHITE).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" · {}", pretty_status(agent.status)),
                    Style::default().fg(color),
                ),
                Span::styled(
                    format!(" · {}", short_role(&agent.display_role, 18)),
                    Style::default().fg(DARK),
                ),
            ]));
        }
    }

    if area.width >= 110 && !snapshot.agents.is_empty() {
        lines.push(Line::default());
        lines.push(section_title("Work"));
        lines.extend(render_agent_matrix(snapshot, area.width as usize));
    }

    let wd = &snapshot.watchdog;
    if !wd.blocked.is_empty()
        || !wd.contradictions.is_empty()
        || !wd.mail_pressure.is_empty()
        || !wd.crash_loop_sessions.is_empty()
    {
        lines.push(Line::default());
        lines.push(section_title("Attention"));
        if !wd.blocked.is_empty() {
            lines.push(kv_yellow("blocked", &wd.blocked.join(", ")));
        }
        if !wd.contradictions.is_empty() {
            lines.push(kv_red("conflicts", &wd.contradictions.join(", ")));
        }
        if !wd.mail_pressure.is_empty() {
            lines.push(kv_yellow("mail", &wd.mail_pressure.join(", ")));
        }
        if !wd.crash_loop_sessions.is_empty() {
            lines.push(kv_red("crash loops", &wd.crash_loop_sessions.join(", ")));
        }
    }

    if !wd.pods.is_empty() || !snapshot.mail_threads.is_empty() || !snapshot.meetings.is_empty() {
        lines.push(Line::default());
        lines.push(section_title("Coordination"));
        for pod in wd.pods.iter().take(4) {
            let blocked = if pod.blocked_members.is_empty() {
                String::new()
            } else {
                format!(" blocked:{}", pod.blocked_members.join(", "))
            };
            lines.push(kv(
                &pod.name,
                &format!(
                    "{}{} [{} threads]",
                    pod.members.join(", "),
                    blocked,
                    pod.open_threads
                ),
            ));
        }
        for thread in snapshot
            .mail_threads
            .iter()
            .filter(|thread| matches!(thread.state.as_str(), "open" | "routed" | "pending"))
            .take(3)
        {
            lines.push(kv(
                "mail",
                &format!(
                    "{} -> {} · {}",
                    thread.from,
                    thread.to,
                    truncate(&thread.subject, 56)
                ),
            ));
        }
    }

    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .scroll((scroll, 0)),
        area,
    );
}

fn render_supervisor_row(supervisor: &AgentNode, workers: &[&AgentNode]) -> Line<'static> {
    let blocked = workers
        .iter()
        .filter(|worker| matches!(worker.status, AgentStatus::Blocked | AgentStatus::Stalled))
        .count();
    let validating = workers
        .iter()
        .filter(|worker| {
            matches!(
                worker.status,
                AgentStatus::DoneClaimed | AgentStatus::NeedsValidation
            )
        })
        .count();
    let label = if supervisor.is_active_supervisor {
        "Active"
    } else if supervisor.is_standby {
        "Standby"
    } else {
        "Branch"
    };
    let (_, color) = status_dot(supervisor.status);
    Line::from(vec![
        Span::styled("● ", Style::default().fg(color)),
        Span::styled(
            supervisor.name.clone(),
            Style::default().fg(WHITE).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" · {label}"), Style::default().fg(color)),
        Span::styled(
            format!(" · {} agents", workers.len()),
            Style::default().fg(DARK),
        ),
        Span::styled(format!(" · {blocked} blocked"), Style::default().fg(DARK)),
        Span::styled(
            format!(" · {validating} in review"),
            Style::default().fg(DARK),
        ),
    ])
}

fn render_agent_matrix(snapshot: &RuntimeSnapshot, width: usize) -> Vec<Line<'static>> {
    let name_w = 14;
    let role_w = 10;
    let status_w = 12;
    let owner_w = 14;
    let task_w = width
        .saturating_sub(name_w + role_w + status_w + owner_w + 12)
        .max(16);

    let mut lines = vec![
        Line::from(Span::styled(
            format!(
                "{:<name_w$}  {:<role_w$}  {:<status_w$}  {:<owner_w$}  {}",
                "Agent",
                "Role",
                "State",
                "Owner",
                "Focus",
                name_w = name_w,
                role_w = role_w,
                status_w = status_w,
                owner_w = owner_w,
            ),
            Style::default().fg(DARK).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "─".repeat(width.saturating_sub(2)),
            Style::default().fg(BORDER),
        )),
    ];

    let mut agents = snapshot.agents.clone();
    agents.sort_by(|left, right| left.name.cmp(&right.name));
    for agent in agents.iter().take(12) {
        let status = pretty_status(agent.status);
        let owner = agent
            .owner_supervisor
            .clone()
            .unwrap_or_else(|| "unassigned".to_owned());
        lines.push(Line::from(vec![
            Span::styled(
                format!("{:<name_w$}", truncate(&agent.name, name_w)),
                Style::default().fg(WHITE),
            ),
            Span::raw("  "),
            Span::styled(
                format!("{:<role_w$}", short_role(&agent.display_role, role_w)),
                Style::default().fg(DARK),
            ),
            Span::raw("  "),
            Span::styled(
                format!("{:<status_w$}", truncate(status, status_w)),
                Style::default().fg(status_color(agent.status)),
            ),
            Span::raw("  "),
            Span::styled(
                format!("{:<owner_w$}", truncate(&owner, owner_w)),
                Style::default().fg(PURPLE_SOFT),
            ),
            Span::raw("  "),
            Span::styled(first_task(agent, task_w), Style::default().fg(GRAY)),
        ]));
    }
    lines
}

fn render_control(frame: &mut Frame<'_>, area: Rect, snapshot: &RuntimeSnapshot) {
    let wd = &snapshot.watchdog;
    let mut lines = vec![section_title("System")];
    lines.push(Line::from(vec![
        Span::styled("mode ", Style::default().fg(DARK)),
        Span::styled(
            snapshot.execution_summary.supervisor_mode.clone(),
            Style::default().fg(if snapshot.execution_summary.supervisor_mode == "healthy" {
                GREEN
            } else {
                YELLOW
            }),
        ),
        Span::styled(" · review ", Style::default().fg(DARK)),
        Span::styled(
            wd.validation_queue.len().to_string(),
            Style::default().fg(PURPLE),
        ),
        Span::styled(" · blocked ", Style::default().fg(DARK)),
        Span::styled(wd.blocked.len().to_string(), Style::default().fg(YELLOW)),
        Span::styled(" · contradictions ", Style::default().fg(DARK)),
        Span::styled(
            wd.contradictions.len().to_string(),
            Style::default().fg(RED),
        ),
    ]));

    if let Some(entry) = snapshot.supervisor_logs.last() {
        lines.push(Line::from(vec![
            Span::styled("Latest  ", Style::default().fg(DARK)),
            Span::raw(" "),
            Span::styled(
                truncate(&entry.message, area.width.saturating_sub(10) as usize),
                Style::default().fg(GRAY),
            ),
        ]));
    } else {
        lines.push(Line::from(vec![Span::styled(
            "No supervisor events yet".to_owned(),
            Style::default().fg(GRAY),
        )]));
    }

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), area);
}

fn render_summary(frame: &mut Frame<'_>, area: Rect, snapshot: &RuntimeSnapshot) {
    let s = &snapshot.execution_summary;
    let mut lines = vec![section_title("Summary")];
    lines.push(kv("mission", &s.mission_rewrite));
    lines.push(kv("deployed", &s.agents_deployed.to_string()));
    lines.push(kv_green("completed", &s.agents_completed.to_string()));
    if s.agents_failed > 0 {
        lines.push(kv_red("failed", &s.agents_failed.to_string()));
    }
    lines.push(kv(
        "mail",
        &format!(
            "{}/{} resolved",
            s.mail_threads_resolved, s.mail_threads_total
        ),
    ));
    lines.push(kv("conflicts", &s.lease_conflicts.to_string()));
    lines.push(kv("stalls", &s.stall_interventions.to_string()));
    if let Some(duration) = s.elapsed() {
        lines.push(kv("elapsed", &format_duration(duration)));
    }
    if let Some(summary) = s.final_summary.as_ref() {
        lines.push(Line::default());
        lines.push(Line::from(Span::styled(
            truncate(summary, area.width.saturating_sub(4) as usize),
            Style::default().fg(WHITE),
        )));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), area);
}

fn render_footer(
    frame: &mut Frame<'_>,
    area: Rect,
    snapshot: &RuntimeSnapshot,
    timer_seconds: u64,
    _frame_count: usize,
) {
    let shimmer_text = loading_phrase(timer_seconds);

    let mut spans = vec![Span::styled(
        format!("{} ", current_spinner_frame()),
        Style::default().fg(PURPLE),
    )];
    spans.extend(footer_shimmer_spans(shimmer_text));
    spans.push(Span::raw("  "));
    spans.push(Span::styled(
        if snapshot.is_done {
            "press q to close".to_owned()
        } else {
            "scroll j/k  ·  refresh r  ·  quit Ctrl+C".to_owned()
        },
        Style::default().fg(DARK),
    ));
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_quit_warning(frame: &mut Frame<'_>, area: Rect) {
    let warning_area = Rect {
        x: 1,
        y: area.height.saturating_sub(2),
        width: area.width.saturating_sub(2),
        height: 1,
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!("{} ", Symbol::Warning),
                Style::default()
                    .fg(WARNING_ORANGE)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "Press Ctrl+C again to quit",
                Style::default()
                    .fg(WARNING_ORANGE)
                    .add_modifier(Modifier::BOLD),
            ),
        ])),
        warning_area,
    );
}

fn footer_shimmer_spans(text: &str) -> Vec<Span<'static>> {
    let chars: Vec<_> = text.chars().collect();
    let len = chars.len().max(1);
    let elapsed_ms = shimmer_start().elapsed().as_millis() as f32;
    let period_ms = 2200.0;
    let position = ((elapsed_ms % period_ms) / period_ms) * len as f32;
    let spread = (len as f32 * 0.42).max(3.5);

    chars
        .into_iter()
        .enumerate()
        .map(|(i, ch)| {
            let index = i as f32;
            let direct = (index - position).abs();
            let wrapped = len as f32 - direct;
            let distance = direct.min(wrapped);
            let intensity = smoothstep(1.0 - (distance / spread).clamp(0.0, 1.0));
            let color = shimmer_color(intensity);
            Span::styled(ch.to_string(), Style::default().fg(color))
        })
        .collect()
}

fn smoothstep(value: f32) -> f32 {
    let clamped = value.clamp(0.0, 1.0);
    clamped * clamped * (3.0 - 2.0 * clamped)
}

fn shimmer_color(intensity: f32) -> Color {
    if intensity < 0.72 {
        blend_rgb(SHIMMER_IDLE_RGB, SHIMMER_BASE_RGB, intensity / 0.72)
    } else {
        blend_rgb(SHIMMER_BASE_RGB, SHIMMER_HOT_RGB, (intensity - 0.72) / 0.28)
    }
}

fn blend_rgb(from: (u8, u8, u8), to: (u8, u8, u8), amount: f32) -> Color {
    let mix = amount.clamp(0.0, 1.0);
    Color::Rgb(
        blend_channel(from.0, to.0, mix),
        blend_channel(from.1, to.1, mix),
        blend_channel(from.2, to.2, mix),
    )
}

fn blend_channel(from: u8, to: u8, amount: f32) -> u8 {
    let start = from as f32;
    let end = to as f32;
    (start + ((end - start) * amount)).round() as u8
}

fn pretty_status(status: AgentStatus) -> &'static str {
    match status {
        AgentStatus::Booting => "Booting",
        AgentStatus::NotStarted => "Queued",
        AgentStatus::Progressing => "Working",
        AgentStatus::Blocked => "Blocked",
        AgentStatus::Stalled => "Stalled",
        AgentStatus::DoneClaimed => "Done Claimed",
        AgentStatus::NeedsValidation => "Reviewing",
        AgentStatus::WeakOutput => "Weak",
        AgentStatus::WrongDirection => "Drift",
        AgentStatus::Contradictory => "Conflict",
        AgentStatus::NeedsRetry => "Waiting",
        AgentStatus::Validated => "Done",
        AgentStatus::Failed => "Failed",
        AgentStatus::Exited => "Exited",
    }
}

fn first_task(agent: &AgentNode, width: usize) -> String {
    let source = if !agent.explicit_task.trim().is_empty() {
        &agent.explicit_task
    } else if !agent.owned_scope.trim().is_empty() {
        &agent.owned_scope
    } else if !agent.summary.trim().is_empty() {
        &agent.summary
    } else {
        "-"
    };
    truncate(
        &source.split_whitespace().collect::<Vec<_>>().join(" "),
        width.max(8),
    )
}

fn short_role(role: &str, width: usize) -> String {
    let compact = role.split_whitespace().next().unwrap_or(role);
    truncate(compact, width)
}

fn format_duration(duration: std::time::Duration) -> String {
    let seconds = duration.as_secs();
    if seconds < 60 {
        format!("{seconds}s")
    } else if seconds < 3600 {
        format!("{}m {}s", seconds / 60, seconds % 60)
    } else {
        format!("{}h {}m", seconds / 3600, (seconds % 3600) / 60)
    }
}

fn format_clock(seconds: u64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;

    if hours > 0 {
        format!("{hours}h{minutes:02}m")
    } else if minutes > 0 {
        format!("{minutes}m{secs:02}s")
    } else {
        format!("{secs}s")
    }
}

pub fn render_exit_banner(snapshot: &RuntimeSnapshot, timer_seconds: u64) -> Vec<String> {
    let agent_count = snapshot.agent_count();
    let completed = snapshot
        .agents
        .iter()
        .filter(|agent| agent.status == AgentStatus::Validated)
        .count();
    let failed = snapshot
        .agents
        .iter()
        .filter(|agent| agent.status == AgentStatus::Failed)
        .count();

    let status_line = if completed > 0 && failed == 0 {
        format!(
            "{agent_count} agent{} completed in {}",
            if agent_count == 1 { "" } else { "s" },
            format_duration(std::time::Duration::from_secs(timer_seconds)),
        )
    } else if failed > 0 {
        format!("{completed} of {agent_count} completed, {failed} failed")
    } else {
        format!(
            "{agent_count} agent{} tracked for {}",
            if agent_count == 1 { "" } else { "s" },
            format_duration(std::time::Duration::from_secs(timer_seconds)),
        )
    };

    let mut lines = vec![String::new(), format!("  session closed  ·  {status_line}")];
    if !snapshot.execution_summary.mission_rewrite.is_empty() {
        lines.push(format!(
            "  mission: {}",
            truncate(&snapshot.execution_summary.mission_rewrite, 80)
        ));
    }
    lines.push(String::new());
    lines
}
