//! Markdown rendering theme with termimad fallback support.
//! Fields are prepared incrementally as rendering components are wired up.
#![allow(dead_code)]

use ratatui::style::{Modifier, Style};

use crate::internal::ui::theme::theme_main::SapphireTheme;

#[derive(Clone, Debug)]
pub struct MarkdownTheme {
    pub body: Style,
    pub muted: Style,
    pub strong: Style,
    pub emphasis: Style,
    pub link: Style,
    pub inline_code: Style,
    pub heading_1: Style,
    pub heading_2: Style,
    pub heading_3: Style,
    pub heading_4: Style,
    pub bullet_marker: Style,
    pub quote_text: Style,
    pub quote_border: Style,
    pub divider: Style,
    pub code_fence: Style,
    pub code_text: Style,
    pub code_lang: Style,
    pub table_header: Style,
    pub table_border: Style,
    pub table_cell: Style,
    pub block_label: Style,
}

impl MarkdownTheme {
    pub fn from_app_theme(theme: &SapphireTheme) -> Self {
        Self {
            body: theme.surfaces.markdown.text,
            muted: theme.surfaces.markdown.muted,
            strong: theme.surfaces.markdown.strong,
            emphasis: theme.surfaces.markdown.emphasis,
            link: theme.surfaces.markdown.link,
            inline_code: theme.surfaces.markdown.inline_code,
            heading_1: theme.surfaces.markdown.heading_1,
            heading_2: theme.surfaces.markdown.heading_2,
            heading_3: theme.surfaces.markdown.heading_3,
            heading_4: Style::default()
                .fg(theme.palette.soft_white)
                .add_modifier(Modifier::BOLD),
            bullet_marker: theme.surfaces.markdown.bullet,
            quote_text: theme.surfaces.markdown.quote,
            quote_border: theme.surfaces.markdown.quote_border,
            divider: theme.surfaces.markdown.divider,
            code_fence: theme.surfaces.markdown.code_fence,
            code_text: theme.surfaces.markdown.code_text,
            code_lang: Style::default()
                .fg(theme.palette.accent)
                .add_modifier(Modifier::BOLD),
            table_header: theme.surfaces.markdown.table_header,
            table_border: theme.surfaces.markdown.table_border,
            table_cell: theme.surfaces.markdown.table_cell,
            block_label: Style::default()
                .fg(theme.palette.purple_soft)
                .add_modifier(Modifier::BOLD),
        }
    }
}
