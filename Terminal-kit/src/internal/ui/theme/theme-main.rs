//! Core theme palette — two-color design: off-white grey + light bright purple.
//! Fields are prepared incrementally as UI components are wired up.
#![allow(dead_code)]

use ratatui::style::{Color, Modifier, Style};

use super::styles::SurfaceStyles;

#[derive(Clone, Debug)]
pub struct Palette {
    pub purple: Color,
    pub purple_soft: Color,
    pub purple_glow: Color,
    pub text: Color,
    pub soft_white: Color,
    pub muted: Color,
    pub border: Color,
    pub accent: Color,
    pub cyan: Color,
    pub teal: Color,
    pub blue: Color,
    pub success: Color,
    pub rule: Color,
    pub danger: Color,
}

impl Default for Palette {
    fn default() -> Self {
        Self {
            purple: Color::Rgb(206, 165, 255),
            purple_soft: Color::Rgb(229, 214, 255),
            purple_glow: Color::Rgb(167, 128, 255),
            text: Color::Rgb(234, 236, 244),
            soft_white: Color::Rgb(248, 248, 255),
            muted: Color::Rgb(152, 160, 181),
            border: Color::Rgb(83, 78, 113),
            accent: Color::Rgb(206, 165, 255),
            cyan: Color::Rgb(124, 234, 255),
            teal: Color::Rgb(108, 224, 179),
            blue: Color::Rgb(124, 175, 255),
            success: Color::Rgb(123, 231, 146),
            rule: Color::Rgb(102, 92, 140),
            danger: Color::Rgb(255, 138, 169),
        }
    }
}

#[derive(Clone, Debug)]
pub struct FrameTheme {
    pub root_title: Style,
    pub root_border: Style,
    pub focused_border: Style,
    pub subtle_rule: Style,
}

#[derive(Clone, Debug)]
pub struct StatusTheme {
    pub validated: Style,
    pub running: Style,
    pub blocked: Style,
    pub waiting: Style,
    pub failed: Style,
    pub degraded: Style,
    pub neutral: Style,
}

#[derive(Clone, Debug)]
pub struct SapphireTheme {
    pub palette: Palette,
    pub surfaces: SurfaceStyles,
    pub frame: FrameTheme,
    pub status: StatusTheme,
}

impl Default for SapphireTheme {
    fn default() -> Self {
        let palette = Palette::default();
        let surfaces = SurfaceStyles::from_palette(&palette);
        let frame = FrameTheme {
            root_title: Style::default()
                .fg(palette.purple)
                .add_modifier(Modifier::BOLD),
            root_border: Style::default().fg(palette.rule),
            focused_border: Style::default().fg(palette.purple_glow),
            subtle_rule: Style::default().fg(palette.rule),
        };
        let status = StatusTheme {
            validated: Style::default()
                .fg(palette.success)
                .add_modifier(Modifier::BOLD),
            running: Style::default()
                .fg(palette.cyan)
                .add_modifier(Modifier::BOLD),
            blocked: Style::default()
                .fg(palette.teal)
                .add_modifier(Modifier::BOLD),
            waiting: Style::default()
                .fg(palette.blue)
                .add_modifier(Modifier::BOLD),
            failed: Style::default()
                .fg(palette.danger)
                .add_modifier(Modifier::BOLD),
            degraded: Style::default()
                .fg(palette.danger)
                .add_modifier(Modifier::BOLD),
            neutral: Style::default().fg(palette.soft_white),
        };

        Self {
            palette,
            surfaces,
            frame,
            status,
        }
    }
}

impl SapphireTheme {
    pub fn panel_border(&self, focused: bool) -> Style {
        if focused {
            self.surfaces.panel.focused_border
        } else {
            self.surfaces.panel.default_border
        }
    }

    pub fn badge_style(&self, status: &str) -> Style {
        match status.to_ascii_lowercase().as_str() {
            "validated" | "completed" | "healthy" => self.status.validated,
            "running" | "progressing" | "launching" => self.status.running,
            "blocked" | "stalled" => self.status.blocked,
            "needs_validation" | "done_claimed" | "pending" => self.status.waiting,
            "failed" | "wrong_direction" | "contradictory" => self.status.failed,
            "degraded" => self.status.degraded,
            _ => self.status.neutral,
        }
    }

    pub fn state_dot(&self, status: &str) -> (&'static str, Style) {
        match status.to_ascii_lowercase().as_str() {
            "validated" | "completed" | "healthy" => ("●", self.status.validated),
            "running" | "progressing" | "launching" => ("●", self.status.running),
            "blocked" | "stalled" => ("●", self.status.blocked),
            "needs_validation" | "done_claimed" | "pending" => ("●", self.status.waiting),
            "failed" | "wrong_direction" | "contradictory" => ("●", self.status.failed),
            "degraded" => ("●", self.status.degraded),
            _ => ("•", self.status.neutral),
        }
    }

    pub fn key_value_label(&self) -> Style {
        self.surfaces.header.meta_label
    }

    pub fn key_value_value(&self) -> Style {
        self.surfaces.header.meta_value
    }

    pub fn footer_hint(&self) -> Style {
        self.surfaces.panel.footer
    }
}
