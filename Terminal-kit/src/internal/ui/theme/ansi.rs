#![allow(dead_code)]

use std::io::IsTerminal;

#[derive(Clone, Copy)]
struct Rgb(u8, u8, u8);

#[derive(Default)]
struct StyleSpec {
    fg: Option<Rgb>,
    bold: bool,
    dim: bool,
}

const PURPLE: Rgb = Rgb(206, 165, 255);
const PURPLE_SOFT: Rgb = Rgb(229, 214, 255);
const BLUE: Rgb = Rgb(124, 175, 255);
const TEAL: Rgb = Rgb(108, 224, 179);
const SUCCESS: Rgb = Rgb(123, 231, 146);
const TEXT: Rgb = Rgb(248, 248, 255);
const MUTED: Rgb = Rgb(152, 160, 181);
const RULE: Rgb = Rgb(102, 92, 140);
const DANGER: Rgb = Rgb(255, 138, 169);

fn color_enabled() -> bool {
    if std::env::var("SAPPHIRE_NO_COLOR").is_ok_and(|value| value != "0") {
        return false;
    }

    if std::env::var("SAPPHIRE_FORCE_COLOR").is_ok_and(|value| value != "0")
        || std::env::var("FORCE_COLOR").is_ok_and(|value| value != "0")
        || std::env::var("CLICOLOR_FORCE").is_ok_and(|value| value != "0")
    {
        return true;
    }

    if supports_color::on_cached(supports_color::Stream::Stdout).is_some() {
        return true;
    }

    let term = std::env::var("TERM").unwrap_or_default();
    std::io::stdout().is_terminal() && term != "dumb"
}

fn paint(text: &str, spec: StyleSpec) -> String {
    if !color_enabled() {
        return text.to_owned();
    }

    let mut codes = Vec::new();
    if spec.bold {
        codes.push("1".to_owned());
    }
    if spec.dim {
        codes.push("2".to_owned());
    }
    if let Some(Rgb(r, g, b)) = spec.fg {
        codes.push(format!("38;2;{r};{g};{b}"));
    }

    if codes.is_empty() {
        return text.to_owned();
    }

    format!("\x1b[{}m{}\x1b[0m", codes.join(";"), text)
}

fn fg(color: Rgb, text: &str) -> String {
    paint(
        text,
        StyleSpec {
            fg: Some(color),
            ..StyleSpec::default()
        },
    )
}

fn fg_bold(color: Rgb, text: &str) -> String {
    paint(
        text,
        StyleSpec {
            fg: Some(color),
            bold: true,
            ..StyleSpec::default()
        },
    )
}

pub fn brand(text: &str) -> String {
    fg(PURPLE, text)
}

pub fn brand_bold(text: &str) -> String {
    fg_bold(PURPLE, text)
}

pub fn brand_soft(text: &str) -> String {
    fg(PURPLE_SOFT, text)
}

pub fn brand_soft_bold(text: &str) -> String {
    fg_bold(PURPLE_SOFT, text)
}

pub fn blue(text: &str) -> String {
    fg(BLUE, text)
}

pub fn blue_bold(text: &str) -> String {
    fg_bold(BLUE, text)
}

pub fn teal(text: &str) -> String {
    fg(TEAL, text)
}

pub fn teal_bold(text: &str) -> String {
    fg_bold(TEAL, text)
}

pub fn success(text: &str) -> String {
    fg(SUCCESS, text)
}

pub fn success_bold(text: &str) -> String {
    fg_bold(SUCCESS, text)
}

pub fn danger(text: &str) -> String {
    fg(DANGER, text)
}

pub fn danger_bold(text: &str) -> String {
    fg_bold(DANGER, text)
}

pub fn text(text: &str) -> String {
    fg(TEXT, text)
}

pub fn text_bold(text: &str) -> String {
    fg_bold(TEXT, text)
}

pub fn muted(text: &str) -> String {
    fg(MUTED, text)
}

pub fn muted_bold(text: &str) -> String {
    fg_bold(MUTED, text)
}

pub fn dim(text: &str) -> String {
    paint(
        text,
        StyleSpec {
            dim: true,
            fg: Some(MUTED),
            ..StyleSpec::default()
        },
    )
}

pub fn rule(text: &str) -> String {
    fg(RULE, text)
}
