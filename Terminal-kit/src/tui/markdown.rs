use pulldown_cmark::{Event, Options, Parser, Tag};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use unicode_width::UnicodeWidthStr;

use crate::internal::ui::theme::fallback::markdown::theme::MarkdownTheme;

pub struct MarkdownRenderer {
    theme: MarkdownTheme,
}

impl MarkdownRenderer {
    pub fn new(theme: MarkdownTheme) -> Self {
        Self { theme }
    }

    pub fn render(&self, markdown: &str, width: u16) -> Text<'static> {
        let mut output = Vec::new();
        let mut lines = markdown.lines().peekable();

        while let Some(line) = lines.next() {
            let trimmed = line.trim_end();
            if trimmed.is_empty() {
                output.push(Line::default());
                continue;
            }

            if let Some(fence_lang) = trimmed.strip_prefix("```") {
                let mut code = Vec::new();
                for candidate in lines.by_ref() {
                    if candidate.trim_start().starts_with("```") {
                        break;
                    }
                    code.push(candidate.to_owned());
                }
                output.extend(self.render_code_block(fence_lang.trim(), &code));
                output.push(Line::default());
                continue;
            }

            if is_table_header(trimmed, lines.peek().copied()) {
                let mut table_lines = vec![trimmed.to_owned()];
                let _ = lines.next();
                while let Some(candidate) = lines.peek().copied() {
                    if !candidate.contains('|') || candidate.trim().is_empty() {
                        break;
                    }
                    table_lines.push(candidate.trim().to_owned());
                    let _ = lines.next();
                }
                output.extend(self.render_table(&table_lines, width));
                output.push(Line::default());
                continue;
            }

            if is_rule(trimmed) {
                output.push(Line::from(Span::styled(
                    "─".repeat(width.max(8) as usize),
                    self.theme.divider,
                )));
                output.push(Line::default());
                continue;
            }

            if let Some(level) = heading_level(trimmed) {
                let content = trimmed[level + 1..].trim();
                output.push(Line::from(self.render_inline(
                    content,
                    self.heading_style(level),
                )));
                output.push(Line::default());
                continue;
            }

            if let Some(quote) = trimmed.strip_prefix('>') {
                let mut quote_lines = vec![quote.trim().to_owned()];
                while let Some(candidate) = lines.peek().copied() {
                    if let Some(next) = candidate.trim_start().strip_prefix('>') {
                        quote_lines.push(next.trim().to_owned());
                        let _ = lines.next();
                    } else {
                        break;
                    }
                }
                output.extend(self.render_quote(&quote_lines));
                output.push(Line::default());
                continue;
            }

            if let Some((marker, content)) = list_marker(trimmed) {
                output.push(Line::from({
                    let mut spans = vec![Span::styled(marker, self.theme.bullet_marker)];
                    spans.push(Span::raw(" "));
                    spans.extend(self.render_inline(content, self.theme.body));
                    spans
                }));
                continue;
            }

            let mut paragraph = trimmed.to_owned();
            while let Some(candidate) = lines.peek().copied() {
                let candidate_trimmed = candidate.trim();
                if candidate_trimmed.is_empty()
                    || is_rule(candidate_trimmed)
                    || heading_level(candidate_trimmed).is_some()
                    || candidate_trimmed.starts_with('>')
                    || candidate_trimmed.starts_with("```")
                    || is_table_header(candidate_trimmed, None)
                    || list_marker(candidate_trimmed).is_some()
                {
                    break;
                }
                paragraph.push(' ');
                paragraph.push_str(candidate_trimmed);
                let _ = lines.next();
            }
            output.push(Line::from(self.render_inline(&paragraph, self.theme.body)));
            output.push(Line::default());
        }

        Text::from(output)
    }

    fn render_code_block(&self, language: &str, lines: &[String]) -> Vec<Line<'static>> {
        let mut out = Vec::new();
        let label = if language.is_empty() { "code" } else { language };
        out.push(Line::from(vec![
            Span::styled("┌ ", self.theme.code_fence),
            Span::styled(label.to_owned(), self.theme.code_lang),
        ]));
        for line in lines {
            out.push(Line::from(vec![
                Span::styled("│ ", self.theme.code_fence),
                Span::styled(line.clone(), self.theme.code_text),
            ]));
        }
        out.push(Line::from(Span::styled("└", self.theme.code_fence)));
        out
    }

    fn render_quote(&self, lines: &[String]) -> Vec<Line<'static>> {
        lines.iter()
            .map(|line| {
                Line::from(vec![
                    Span::styled("▍ ", self.theme.quote_border),
                    Span::styled(line.clone(), self.theme.quote_text),
                ])
            })
            .collect()
    }

    fn render_table(&self, lines: &[String], width: u16) -> Vec<Line<'static>> {
        let rows = lines
            .iter()
            .map(|line| split_table_row(line))
            .collect::<Vec<_>>();
        if rows.is_empty() {
            return Vec::new();
        }
        let header = &rows[0];
        let body = &rows[1..];
        let mut widths = vec![0usize; header.len()];

        for row in rows.iter() {
            for (index, cell) in row.iter().enumerate() {
                widths[index] = widths[index].max(UnicodeWidthStr::width(cell.as_str()));
            }
        }

        let max_total = width.saturating_sub(6) as usize;
        let current_total = widths.iter().sum::<usize>() + widths.len().saturating_sub(1) * 3;
        if current_total > max_total && !widths.is_empty() {
            let overflow = current_total - max_total;
            let last = widths.len() - 1;
            widths[last] = widths[last].saturating_sub(overflow.min(widths[last].saturating_sub(8)));
        }

        let mut out = Vec::new();
        out.push(self.render_table_row(header, &widths, true));
        out.push(self.render_table_divider(&widths));
        for row in body {
            out.push(self.render_table_row(row, &widths, false));
        }
        out
    }

    fn render_table_row(&self, row: &[String], widths: &[usize], header: bool) -> Line<'static> {
        let mut spans = Vec::new();
        spans.push(Span::styled("│ ", self.theme.table_border));
        for (index, width) in widths.iter().enumerate() {
            let value = row.get(index).cloned().unwrap_or_default();
            let text = pad_cell(&value, *width);
            spans.push(Span::styled(
                text,
                if header {
                    self.theme.table_header
                } else {
                    self.theme.table_cell
                },
            ));
            if index + 1 == widths.len() {
                spans.push(Span::styled(" │", self.theme.table_border));
            } else {
                spans.push(Span::styled(" │ ", self.theme.table_border));
            }
        }
        Line::from(spans)
    }

    fn render_table_divider(&self, widths: &[usize]) -> Line<'static> {
        let mut text = String::from("├");
        for (index, width) in widths.iter().enumerate() {
            text.push_str(&"─".repeat(width + 2));
            if index + 1 == widths.len() {
                text.push('┤');
            } else {
                text.push('┼');
            }
        }
        Line::from(Span::styled(text, self.theme.table_border))
    }

    fn render_inline(&self, text: &str, base: Style) -> Vec<Span<'static>> {
        let parser = Parser::new_ext(
            text,
            Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES | Options::ENABLE_TASKLISTS,
        );
        let mut stack = vec![base];
        let mut spans = Vec::new();

        for event in parser {
            match event {
                Event::Text(value) => spans.push(Span::styled(
                    value.to_string(),
                    *stack.last().unwrap_or(&base),
                )),
                Event::Code(value) => spans.push(Span::styled(
                    value.to_string(),
                    self.theme.inline_code,
                )),
                Event::SoftBreak | Event::HardBreak => spans.push(Span::raw(" ")),
                Event::Start(tag) => stack.push(self.apply_tag(*stack.last().unwrap_or(&base), &tag)),
                Event::End(_tag) => {
                    let _ = stack.pop();
                    if stack.is_empty() {
                        stack.push(base);
                    }
                }
                _ => {}
            }
        }

        spans
    }

    fn apply_tag(&self, current: Style, tag: &Tag<'_>) -> Style {
        match tag {
            Tag::Strong => current.patch(self.theme.strong),
            Tag::Emphasis => current.patch(self.theme.emphasis),
            Tag::Strikethrough => current.add_modifier(Modifier::CROSSED_OUT),
            Tag::Link { .. } => current.patch(self.theme.link),
            _ => current,
        }
    }

    fn heading_style(&self, level: usize) -> Style {
        match level {
            1 => self.theme.heading_1,
            2 => self.theme.heading_2,
            3 => self.theme.heading_3,
            _ => self.theme.heading_4,
        }
    }
}

fn heading_level(line: &str) -> Option<usize> {
    let hashes = line.chars().take_while(|ch| *ch == '#').count();
    if hashes > 0 && hashes <= 6 && line.chars().nth(hashes) == Some(' ') {
        Some(hashes)
    } else {
        None
    }
}

fn is_rule(line: &str) -> bool {
    matches!(line.trim(), "---" | "***" | "___")
}

fn list_marker(line: &str) -> Option<(String, &str)> {
    let trimmed = line.trim_start();
    for marker in ["- ", "* ", "+ "] {
        if let Some(content) = trimmed.strip_prefix(marker) {
            return Some((marker.trim().to_owned(), content.trim()));
        }
    }
    let digits = trimmed.chars().take_while(|ch| ch.is_ascii_digit()).count();
    if digits > 0 && trimmed.get(digits..digits + 2) == Some(". ") {
        return Some((trimmed[..digits + 1].to_owned(), trimmed[digits + 2..].trim()));
    }
    None
}

fn is_table_header(line: &str, next_line: Option<&str>) -> bool {
    line.contains('|')
        && next_line
            .map(|next| {
                let stripped = next.trim().replace('|', "").replace(':', "").replace('-', "");
                next.contains('|') && stripped.is_empty()
            })
            .unwrap_or(false)
}

fn split_table_row(line: &str) -> Vec<String> {
    line.trim()
        .trim_matches('|')
        .split('|')
        .map(|cell| cell.trim().to_owned())
        .collect()
}

fn pad_cell(value: &str, width: usize) -> String {
    let display_width = UnicodeWidthStr::width(value);
    if display_width >= width {
        truncate_to_width(value, width)
    } else {
        format!("{value}{}", " ".repeat(width - display_width))
    }
}

fn truncate_to_width(value: &str, width: usize) -> String {
    let mut out = String::new();
    let mut used = 0;
    for ch in value.chars() {
        let ch_width = ch.width().unwrap_or(0);
        if used + ch_width > width.saturating_sub(1) {
            break;
        }
        out.push(ch);
        used += ch_width;
    }
    if UnicodeWidthStr::width(value) > width {
        out.push('…');
    }
    if UnicodeWidthStr::width(out.as_str()) < width {
        out.push_str(&" ".repeat(width - UnicodeWidthStr::width(out.as_str())));
    }
    out
}

trait CharWidth {
    fn width(self) -> Option<usize>;
}

impl CharWidth for char {
    fn width(self) -> Option<usize> {
        unicode_width::UnicodeWidthChar::width(self)
    }
}
