//! Built-in Tailwind/styling vocabulary for post-transcription correction.
//!
//! Loads `catalog/styling.json` (namespace `tailwind-css-speech-normalization`)
//! and exposes the same (display form → spoken aliases) contract the technical
//! lexicon uses, so both catalogs flow through one n-gram fuzzy engine.
//!
//! Hex-color aliases are filtered: bare color words ("white", "red") would
//! rewrite ordinary prose, so only unambiguous spoken forms survive
//! ("hex white", "standard red", "tailwind blue").

use serde::Deserialize;
use std::sync::OnceLock;

const STYLING_JSON: &str = include_str!("../catalog/styling.json");

#[derive(Deserialize)]
struct StylingFile {
    #[allow(dead_code)]
    version: u32,
    common_classes: Vec<StylingEntry>,
    hex_colors: Vec<StylingEntry>,
    high_value_speech_patterns: Vec<SpeechPattern>,
}

#[derive(Deserialize)]
struct StylingEntry {
    canonical: String,
    aliases: Vec<String>,
}

#[derive(Deserialize)]
struct SpeechPattern {
    canonical: String,
    input_patterns: Vec<String>,
}

/// Bare color words must never become hex codes in normal sentences.
fn safe_hex_alias(alias: &str) -> bool {
    let lowered = alias.to_lowercase();
    !(lowered == "white"
        || lowered == "black"
        || lowered == "red"
        || lowered == "orange"
        || lowered == "yellow"
        || lowered == "green"
        || lowered == "cyan"
        || lowered == "blue"
        || lowered == "violet"
        || lowered == "pink")
}

type AliasEntries = Vec<(String, Vec<String>)>;

static STYLING: OnceLock<AliasEntries> = OnceLock::new();

fn entries() -> &'static AliasEntries {
    STYLING.get_or_init(|| {
        match serde_json::from_str::<StylingFile>(STYLING_JSON) {
            Ok(file) => {
                let mut out: AliasEntries = Vec::new();
                for entry in file.common_classes {
                    out.push((entry.canonical, entry.aliases));
                }
                for entry in file.hex_colors {
                    let aliases: Vec<String> = entry
                        .aliases
                        .into_iter()
                        .filter(|alias| safe_hex_alias(alias))
                        .collect();
                    // An entry whose every alias was unsafe still keeps its
                    // canonical self-key so explicit "hex white" style speech
                    // via other entries stays consistent.
                    out.push((entry.canonical, aliases));
                }
                for pattern in file.high_value_speech_patterns {
                    out.push((pattern.canonical, pattern.input_patterns));
                }
                out
            }
            Err(e) => {
                log::error!("Failed to parse embedded styling catalog: {e}");
                Vec::new()
            }
        }
    })
}

/// Number of styling corrections available.
pub fn len() -> usize {
    entries().len()
}

/// Canonical display forms (Tailwind utilities, class names) for decode-time
/// vocabulary biasing.
pub fn canonical_names() -> impl Iterator<Item = &'static str> {
    entries().iter().map(|(canonical, _)| canonical.as_str())
}

/// Same strictness as the technical lexicon: near-exact alias or canonical
/// hits only.
const MATCH_THRESHOLD: f64 = 0.2;

/// Applies the built-in styling lexicon to transcribed text.
pub fn apply(text: &str) -> String {
    if text.is_empty() {
        return text.to_string();
    }
    crate::audio_toolkit::text::apply_alias_entries(text, entries(), MATCH_THRESHOLD)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_parses_with_substantial_coverage() {
        assert!(len() > 200, "expected substantial styling entries, got {}", len());
    }

    #[test]
    fn corrects_spoken_tailwind_utilities() {
        assert_eq!(apply("make it flex layout"), "make it flex");
        assert_eq!(apply("add tracking wide"), "add tracking-wide");
    }

    #[test]
    fn corrects_high_value_speech_patterns() {
        assert_eq!(
            apply("set background stone six hundred"),
            "set bg-stone-600"
        );
    }

    #[test]
    fn bare_color_words_never_become_hex_codes() {
        assert_eq!(apply("the white house"), "the white house");
        assert_eq!(apply("paint it hex white"), "paint it #ffffff");
    }

    #[test]
    fn plain_prose_passes_through() {
        let text = "please review the layout tomorrow";
        assert_eq!(apply(text), text);
    }
}
