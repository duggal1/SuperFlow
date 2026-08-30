#![allow(dead_code)]

use ratatui::text::{Line, Span};

use crate::internal::ui::theme::theme_main::SapphireTheme;

pub fn branch_title(
    theme: &SapphireTheme,
    branch: &'static str,
    label: &str,
    accent: &str,
) -> Line<'static> {
    Line::from(vec![
        Span::styled(branch.to_owned(), theme.surfaces.panel.rule),
        Span::styled(label.to_owned(), theme.surfaces.panel.title),
        Span::raw(" "),
        Span::styled(accent.to_owned(), theme.surfaces.panel.accent),
    ])
}

pub fn branch_body(
    theme: &SapphireTheme,
    branch: &'static str,
    text: impl Into<String>,
) -> Line<'static> {
    Line::from(vec![
        Span::styled(branch.to_owned(), theme.surfaces.panel.rule),
        Span::styled(text.into(), theme.surfaces.panel.body),
    ])
}

pub fn branch_muted(
    theme: &SapphireTheme,
    branch: &'static str,
    text: impl Into<String>,
) -> Line<'static> {
    Line::from(vec![
        Span::styled(branch.to_owned(), theme.surfaces.panel.rule),
        Span::styled(text.into(), theme.surfaces.panel.dimmed),
    ])
}

pub fn item_branch(index: usize, total: usize) -> &'static str {
    if index + 1 == total {
        "└─ "
    } else {
        "├─ "
    }
}

pub fn child_branch(index: usize, total: usize) -> &'static str {
    if index + 1 == total { "   " } else { "│  " }
}
