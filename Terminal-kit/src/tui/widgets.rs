//! TUI widgets — restrained palette, status dots, line builders.
#![allow(dead_code)]

use super::state::AgentStatus;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

// Bright purple theme — matches internal/ui/theme/ansi.rs exactly
pub const PURPLE: Color = Color::Rgb(206, 165, 255);
pub const PURPLE_SOFT: Color = Color::Rgb(229, 214, 255);
pub const PURPLE_GLOW: Color = Color::Rgb(167, 128, 255);
pub const GREEN: Color = Color::Rgb(123, 231, 146);
pub const GREEN_SOFT: Color = Color::Rgb(74, 222, 128);
pub const RED: Color = Color::Rgb(255, 138, 169);
pub const YELLOW: Color = Color::Rgb(245, 158, 11);
pub const WHITE: Color = Color::Rgb(248, 248, 255);
pub const GRAY: Color = Color::Rgb(152, 160, 181);
pub const DARK: Color = Color::Rgb(100, 116, 139);
pub const BORDER: Color = Color::Rgb(83, 78, 113);
pub const CYAN: Color = Color::Rgb(124, 234, 255);
pub const TEAL: Color = Color::Rgb(108, 224, 179);

// ─── Status Dots ──────────────────────────────────────────────────────────

pub fn status_dot(status: AgentStatus) -> (&'static str, Color) {
    match status {
        AgentStatus::Progressing | AgentStatus::Booting => ("●", GREEN),
        AgentStatus::Validated => ("●", GREEN),
        AgentStatus::Blocked | AgentStatus::Stalled => ("●", YELLOW),
        AgentStatus::Failed | AgentStatus::Exited => ("●", RED),
        AgentStatus::Contradictory | AgentStatus::WrongDirection => ("●", RED),
        AgentStatus::DoneClaimed => ("●", GREEN_SOFT),
        AgentStatus::NeedsValidation => ("●", PURPLE),
        AgentStatus::WeakOutput | AgentStatus::NeedsRetry => ("●", PURPLE_SOFT),
        AgentStatus::NotStarted => ("○", GRAY),
    }
}

pub fn status_label(status: AgentStatus) -> &'static str {
    match status {
        AgentStatus::Booting => "boot",
        AgentStatus::NotStarted => "queued",
        AgentStatus::Progressing => "running",
        AgentStatus::Blocked => "blocked",
        AgentStatus::Stalled => "stalled",
        AgentStatus::DoneClaimed => "done_claimed",
        AgentStatus::NeedsValidation => "reviewing",
        AgentStatus::WeakOutput => "weak",
        AgentStatus::WrongDirection => "drift",
        AgentStatus::Contradictory => "conflict",
        AgentStatus::NeedsRetry => "retry",
        AgentStatus::Validated => "done",
        AgentStatus::Failed => "failed",
        AgentStatus::Exited => "exited",
    }
}

pub fn status_color(status: AgentStatus) -> Color {
    status_dot(status).1
}

// ─── Line Builders ────────────────────────────────────────────────────────

pub fn muted(text: &str) -> Line<'static> {
    Line::from(Span::styled(text.to_owned(), Style::default().fg(GRAY)))
}

pub fn accent(text: &str) -> Line<'static> {
    Line::from(Span::styled(
        text.to_owned(),
        Style::default().fg(PURPLE).add_modifier(Modifier::BOLD),
    ))
}

pub fn section_title(title: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            title.to_owned(),
            Style::default().fg(WHITE).add_modifier(Modifier::BOLD),
        ),
        Span::styled("  ", Style::default().fg(DARK)),
        Span::styled("────────────────", Style::default().fg(BORDER)),
    ])
}

pub fn kv(label: &str, value: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label}: "), Style::default().fg(DARK)),
        Span::styled(value.to_owned(), Style::default().fg(WHITE)),
    ])
}

pub fn kv_green(label: &str, value: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label}: "), Style::default().fg(DARK)),
        Span::styled(value.to_owned(), Style::default().fg(GREEN)),
    ])
}

pub fn kv_yellow(label: &str, value: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label}: "), Style::default().fg(DARK)),
        Span::styled(value.to_owned(), Style::default().fg(YELLOW)),
    ])
}

pub fn kv_red(label: &str, value: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label}: "), Style::default().fg(DARK)),
        Span::styled(
            value.to_owned(),
            Style::default().fg(RED).add_modifier(Modifier::BOLD),
        ),
    ])
}

pub fn truncate(v: &str, max: usize) -> String {
    if v.chars().count() <= max {
        v.to_owned()
    } else {
        v.chars().take(max.saturating_sub(1)).collect::<String>() + ".."
    }
}
