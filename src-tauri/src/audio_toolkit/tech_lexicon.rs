//! Built-in technical vocabulary for post-transcription correction.
//!
//! Ships a curated lexicon of technology terms with their common mishearings
//! ("next year" → "Next.js", "tail winds" → "Tailwind CSS") and applies it to
//! every transcription through the same n-gram fuzzy engine used for user
//! custom words. Entirely local — no network, no model, no API key.

use serde::Deserialize;
use std::sync::OnceLock;

/// Embedded at compile time from the catalog; see `catalog/tech_lexicon.json`.
const LEXICON_JSON: &str = include_str!("../catalog/tech_lexicon.json");

/// Stricter than the user custom-word threshold (default 0.18 is user-chosen):
/// built-in entries must not fire on loose matches, only near-exact alias or
/// canonical hits.
const MATCH_THRESHOLD: f64 = 0.2;

#[derive(Deserialize)]
struct LexiconFile {
    #[allow(dead_code)]
    version: u32,
    entries: Vec<LexiconEntry>,
}

#[derive(Deserialize)]
struct LexiconEntry {
    canonical: String,
    #[serde(default)]
    aliases: Vec<String>,
}

type LexiconEntries = Vec<(String, Vec<String>)>;

static LEXICON: OnceLock<LexiconEntries> = OnceLock::new();

fn entries() -> &'static LexiconEntries {
    LEXICON.get_or_init(|| {
        match serde_json::from_str::<LexiconFile>(LEXICON_JSON) {
            Ok(file) => file
                .entries
                .into_iter()
                .map(|entry| (entry.canonical, entry.aliases))
                .collect(),
            Err(e) => {
                // A malformed embedded lexicon must never break dictation;
                // log and run with an empty list.
                log::error!("Failed to parse embedded tech lexicon: {e}");
                Vec::new()
            }
        }
    })
}

/// Number of built-in technical terms available for correction.
pub fn len() -> usize {
    entries().len()
}

/// Applies the built-in technical lexicon to transcribed text.
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
    fn lexicon_parses_and_loads() {
        assert!(len() > 100, "expected a substantial lexicon, got {}", len());
    }

    #[test]
    fn corrects_misheard_framework_names() {
        assert_eq!(apply("built with next year"), "built with Next.js");
        assert_eq!(
            apply("styled using tail winds"),
            "styled using Tailwind CSS"
        );
    }

    #[test]
    fn corrects_spelled_out_acronyms() {
        assert_eq!(apply("the a p i returns json"), "the API returns JSON");
    }

    #[test]
    fn preserves_case_pattern() {
        assert_eq!(apply("NEXT YEAR app"), "NEXT.JS app");
        assert_eq!(apply("Next year app"), "Next.js app");
    }

    #[test]
    fn leaves_plain_text_untouched() {
        let text = "I will see you tomorrow at the market";
        assert_eq!(apply(text), text);
    }

    #[test]
    fn empty_text_stays_empty() {
        assert_eq!(apply(""), "");
    }

    #[test]
    fn handles_punctuation_boundaries() {
        assert_eq!(
            apply("we used redis, postgres, and lang chain."),
            "we used Redis, PostgreSQL, and LangChain."
        );
    }

    #[test]
    fn corrects_spoken_file_extensions() {
        assert_eq!(apply("edit hero dot tsx"), "edit hero .tsx");
        assert_eq!(apply("open main dot rs"), "open main .rs");
        assert_eq!(
            apply("check the package dot json"),
            "check the package .json"
        );
    }

    #[test]
    fn corrects_spoken_path_separators() {
        assert_eq!(
            apply("components slash hero dot tsx"),
            "components / hero .tsx"
        );
        assert_eq!(apply("src forward slash lib"), "src / lib");
    }

    #[test]
    fn does_not_fire_on_nearby_prose_words() {
        assert_eq!(apply("he slashed prices today"), "he slashed prices today");
        assert_eq!(apply("the dot of the sentence"), "the dot of the sentence");
    }
}
