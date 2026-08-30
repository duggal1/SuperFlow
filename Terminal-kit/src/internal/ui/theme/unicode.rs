use std::fmt;

/// Strongly typed application banners.
///
/// Keep banner assets centralized so the rest of the codebase
/// does not duplicate raw unicode blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Banner {
    Sapphire,
}

impl Banner {
    /// Returns the exact banner text, preserved byte-for-byte.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sapphire => SAPPHIRE_BANNER,
        }
    }

    /// Returns the banner as exact individual lines.
    #[must_use]
    pub const fn lines(self) -> &'static [&'static str] {
        match self {
            Self::Sapphire => &SAPPHIRE_BANNER_LINES,
        }
    }

    #[allow(dead_code)]
    #[must_use]
    pub const fn height(self) -> usize {
        match self {
            Self::Sapphire => SAPPHIRE_BANNER_LINES.len(),
        }
    }

    #[must_use]
    pub fn width(self) -> usize {
        self.lines()
            .iter()
            .map(|line| line.chars().count())
            .max()
            .unwrap_or(0)
    }

    pub fn write_to<W>(self, out: &mut W) -> fmt::Result
    where
        W: fmt::Write,
    {
        out.write_str(self.as_str())
    }

    #[must_use]
    pub fn padded_left(self, padding: usize) -> String {
        let pad = " ".repeat(padding);
        self.lines()
            .iter()
            .map(|line| format!("{pad}{line}"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[must_use]
    pub fn centered(self, container_width: usize) -> String {
        let width = self.width();
        if container_width <= width {
            return self.as_str().to_owned();
        }

        let left_padding = (container_width - width) / 2;
        self.padded_left(left_padding)
    }
}

impl fmt::Display for Banner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.write_to(f)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Symbol {
    Warning,
    Info,
    Success,
    Error,
    Prompt,
    Field,
}

impl Symbol {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Warning => "△",
            Self::Info => "•",
            Self::Success => "✓",
            Self::Error => "✕",
            Self::Prompt => "›",
            Self::Field => "↗",
        }
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// IMPORTANT:
/// This banner is intentionally stored as exact lines instead of being
/// regenerated. Leading spaces are part of the glyph alignment.
pub const SAPPHIRE_BANNER_LINES: [&str; 7] = [
    " ░██████      ░███    ░█████████  ░█████████   ░██     ░██ ░██████░█████████   ░█████████ ",
    " ░██   ░██    ░██░██   ░██     ░██ ░██     ░██ ░██     ░██   ░██  ░██     ░██ ░██         ",
    "░██          ░██  ░██  ░██     ░██ ░██     ░██ ░██     ░██   ░██  ░██     ░██ ░██         ",
    " ░████████  ░█████████ ░█████████  ░█████████  ░██████████   ░██  ░█████████  ░█████████  ",
    "        ░██ ░██    ░██ ░██         ░██         ░██     ░██   ░██  ░██   ░██   ░██         ",
    " ░██   ░██  ░██    ░██ ░██         ░██         ░██     ░██   ░██  ░██    ░██  ░██         ",
    "  ░██████   ░██    ░██ ░██         ░██         ░██     ░██ ░██████░██     ░██  ░██████████ ",
];

pub const SAPPHIRE_BANNER: &str = concat!(
    " ░██████      ░███    ░█████████  ░█████████   ░██     ░██ ░██████░█████████   ░█████████ \n",
    " ░██   ░██    ░██░██   ░██     ░██ ░██     ░██ ░██     ░██   ░██  ░██     ░██ ░██         \n",
    "░██          ░██  ░██  ░██     ░██ ░██     ░██ ░██     ░██   ░██  ░██     ░██ ░██         \n",
    " ░████████  ░█████████ ░█████████  ░█████████  ░██████████   ░██  ░█████████  ░█████████  \n",
    "        ░██ ░██    ░██ ░██         ░██         ░██     ░██   ░██  ░██   ░██   ░██         \n",
    " ░██   ░██  ░██    ░██ ░██         ░██         ░██     ░██   ░██  ░██    ░██  ░██         \n",
    "  ░██████   ░██    ░██ ░██         ░██         ░██     ░██ ░██████░██     ░██  ░██████████ "
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banner_text_matches_joined_lines() {
        let joined = SAPPHIRE_BANNER_LINES.join("\n");
        assert_eq!(joined, SAPPHIRE_BANNER);
    }

    #[test]
    fn banner_has_expected_height() {
        assert_eq!(Banner::Sapphire.height(), 7);
    }

    #[test]
    fn banner_width_is_non_zero() {
        assert!(Banner::Sapphire.width() > 0);
    }

    #[test]
    fn centered_returns_raw_when_container_is_too_small() {
        assert_eq!(Banner::Sapphire.centered(5), Banner::Sapphire.as_str());
    }

    #[test]
    fn display_renders_exact_banner() {
        assert_eq!(format!("{}", Banner::Sapphire), SAPPHIRE_BANNER);
    }

    #[test]
    fn padded_left_adds_spaces() {
        let padded = Banner::Sapphire.padded_left(4);
        let lines: Vec<&str> = padded.lines().collect();
        assert_eq!(lines.len(), Banner::Sapphire.height());
        assert!(lines[0].starts_with("    "));
    }

    #[test]
    fn centered_pads_when_container_is_large() {
        let rendered = Banner::Sapphire.centered(120);
        assert!(rendered.contains("\n"));
    }

    #[test]
    fn symbols_render_stable_unicode() {
        assert_eq!(Symbol::Success.as_str(), "✓");
        assert_eq!(format!("{}", Symbol::Prompt), "›");
    }
}
