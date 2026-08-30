//! Trait-based styling contracts for terminal UI elements.
//! These fields are consumed incrementally as the TUI surface is wired up.
#![allow(dead_code)]

use ratatui::style::{Modifier, Style};

use super::theme_main::Palette;

#[derive(Clone, Debug)]
pub struct PanelStyles {
    pub default_border: Style,
    pub focused_border: Style,
    pub title: Style,
    pub accent: Style,
    pub body: Style,
    pub dimmed: Style,
    pub footer: Style,
    pub rule: Style,
}

#[derive(Clone, Debug)]
pub struct HeaderStyles {
    pub brand: Style,
    pub mission: Style,
    pub meta_label: Style,
    pub meta_value: Style,
    pub timer: Style,
}

#[derive(Clone, Debug)]
pub struct BadgeStyles {
    pub running: Style,
    pub healthy: Style,
    pub pending: Style,
    pub waiting: Style,
    pub failed: Style,
    pub neutral: Style,
}

#[derive(Clone, Debug)]
pub struct TableStyles {
    pub header: Style,
    pub border: Style,
    pub cell: Style,
    pub muted_cell: Style,
}

#[derive(Clone, Debug)]
pub struct MarkdownStyles {
    pub text: Style,
    pub muted: Style,
    pub strong: Style,
    pub emphasis: Style,
    pub heading_1: Style,
    pub heading_2: Style,
    pub heading_3: Style,
    pub bullet: Style,
    pub quote: Style,
    pub quote_border: Style,
    pub inline_code: Style,
    pub code_text: Style,
    pub code_fence: Style,
    pub divider: Style,
    pub link: Style,
    pub table_header: Style,
    pub table_border: Style,
    pub table_cell: Style,
}

#[derive(Clone, Debug)]
pub struct SurfaceStyles {
    pub header: HeaderStyles,
    pub panel: PanelStyles,
    pub badges: BadgeStyles,
    pub table: TableStyles,
    pub markdown: MarkdownStyles,
}

impl SurfaceStyles {
    pub fn from_palette(palette: &Palette) -> Self {
        Self {
            header: HeaderStyles {
                brand: Style::default()
                    .fg(palette.purple)
                    .add_modifier(Modifier::BOLD),
                mission: Style::default()
                    .fg(palette.soft_white)
                    .add_modifier(Modifier::BOLD),
                meta_label: Style::default().fg(palette.muted),
                meta_value: Style::default().fg(palette.text),
                timer: Style::default()
                    .fg(palette.accent)
                    .add_modifier(Modifier::BOLD),
            },
            panel: PanelStyles {
                default_border: Style::default().fg(palette.border),
                focused_border: Style::default().fg(palette.purple),
                title: Style::default()
                    .fg(palette.purple_soft)
                    .add_modifier(Modifier::BOLD),
                accent: Style::default()
                    .fg(palette.cyan)
                    .add_modifier(Modifier::BOLD),
                body: Style::default().fg(palette.text),
                dimmed: Style::default().fg(palette.muted),
                footer: Style::default().fg(palette.soft_white),
                rule: Style::default().fg(palette.rule),
            },
            badges: BadgeStyles {
                running: Style::default()
                    .fg(palette.accent)
                    .add_modifier(Modifier::BOLD),
                healthy: Style::default()
                    .fg(palette.success)
                    .add_modifier(Modifier::BOLD),
                pending: Style::default()
                    .fg(palette.blue)
                    .add_modifier(Modifier::BOLD),
                waiting: Style::default()
                    .fg(palette.teal)
                    .add_modifier(Modifier::BOLD),
                failed: Style::default()
                    .fg(palette.danger)
                    .add_modifier(Modifier::BOLD),
                neutral: Style::default().fg(palette.soft_white),
            },
            table: TableStyles {
                header: Style::default()
                    .fg(palette.purple)
                    .add_modifier(Modifier::BOLD),
                border: Style::default().fg(palette.border),
                cell: Style::default().fg(palette.text),
                muted_cell: Style::default().fg(palette.muted),
            },
            markdown: MarkdownStyles {
                text: Style::default().fg(palette.text),
                muted: Style::default().fg(palette.muted),
                strong: Style::default()
                    .fg(palette.soft_white)
                    .add_modifier(Modifier::BOLD),
                emphasis: Style::default()
                    .fg(palette.blue)
                    .add_modifier(Modifier::ITALIC),
                heading_1: Style::default()
                    .fg(palette.purple)
                    .add_modifier(Modifier::BOLD),
                heading_2: Style::default()
                    .fg(palette.blue)
                    .add_modifier(Modifier::BOLD),
                heading_3: Style::default()
                    .fg(palette.teal)
                    .add_modifier(Modifier::BOLD),
                bullet: Style::default()
                    .fg(palette.cyan)
                    .add_modifier(Modifier::BOLD),
                quote: Style::default().fg(palette.soft_white),
                quote_border: Style::default().fg(palette.teal),
                inline_code: Style::default().fg(palette.cyan),
                code_text: Style::default().fg(palette.soft_white),
                code_fence: Style::default().fg(palette.rule),
                divider: Style::default().fg(palette.rule),
                link: Style::default()
                    .fg(palette.blue)
                    .add_modifier(Modifier::UNDERLINED),
                table_header: Style::default()
                    .fg(palette.purple_soft)
                    .add_modifier(Modifier::BOLD),
                table_border: Style::default().fg(palette.rule),
                table_cell: Style::default().fg(palette.text),
            },
        }
    }
}
