/// Terminal palette — a clean purple / bright-white / gray theme.

/// Default foreground: soft bright gray for base text.
pub fn default_fg() -> Option<(u8, u8, u8)> {
    Some((210, 210, 220))
}

/// Default background: deep purple-tinted dark for the shimmer highlight.
pub fn default_bg() -> Option<(u8, u8, u8)> {
    Some((180, 140, 255))
}
