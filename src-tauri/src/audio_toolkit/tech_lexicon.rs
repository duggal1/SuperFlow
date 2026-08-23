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

/// Canonical display forms for decode-time vocabulary biasing (whisper
/// initial_prompt). Merges the technical lexicon with the styling catalog
/// (Tailwind utilities are exactly the vocabulary vibe-coding dictation
/// needs spelled correctly). Capped so the prompt stays a small fraction of
/// the decoder's context budget.
pub fn vocabulary_hint() -> Vec<String> {
    const MAX_CHARS: usize = 900;
    let mut out = Vec::new();
    let mut total = 0usize;
    let push = |canonical: &str, out: &mut Vec<String>, total: &mut usize| {
        if *total + canonical.len() + 2 > MAX_CHARS {
            return;
        }
        *total += canonical.len() + 2;
        out.push(canonical.to_string());
    };
    for (canonical, _) in entries() {
        push(canonical, &mut out, &mut total);
    }
    for canonical in crate::audio_toolkit::styling::canonical_names() {
        push(canonical, &mut out, &mut total);
    }
    out
}

/// True when `word` (already lowercased) matches one of this catalog's
/// canonical display forms — used by the formatter's de-shout pass to leave
/// real terms untouched before lexicon replacement.
pub fn is_known_term(word: &str) -> bool {
    {
        use std::collections::HashSet;
        use std::sync::OnceLock;
        static KNOWN: OnceLock<HashSet<String>> = OnceLock::new();
        let normalized: String = word.chars().filter(|c| c.is_alphanumeric()).collect();
        let known = KNOWN.get_or_init(|| {
            {
                entries()
                    .iter()
                    .filter_map(|(canonical, _)| {
                        let k: String = canonical
                            .chars()
                            .filter(|c| c.is_alphanumeric())
                            .collect::<String>()
                            .to_lowercase();
                        (!k.is_empty()).then_some(k)
                    })
                    .collect()
            }
        });
        known.contains(&normalized)
    }
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
        // v2 dropped bare "next year" (prose collision risk); live aliases
        // carry the same intent.
        assert_eq!(apply("built with next jays"), "built with Next.js");
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
    fn canonical_terms_ignore_shouted_input() {
        assert_eq!(apply("NEXT JAYS app"), "Next.js app");
        assert_eq!(apply("PLAYWRIGHT test"), "Playwright test");
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
        // v2 carries a dedicated package.json entry that consumes the phrase.
        assert_eq!(
            apply("check the package dot json"),
            "check the package.json"
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

    #[test]
    fn skeleton_matching_does_not_overfire() {
        // A stray small word glued into the n-gram must not be swallowed.
        assert_eq!(
            apply("deploy it to coober netees"),
            "deploy it to Kubernetes"
        );
        // Generic words must not morph into longer entries via skeletons.
        assert_eq!(apply("list the components"), "list the components");
    }

    #[test]
    fn phonetic_matching_never_fires_on_short_prose() {
        assert_eq!(
            apply("i will rest at the sea shore"),
            "i will rest at the sea shore"
        );
        // Single common-word entries deliberately win in dev dictation:
        // "Bun" the runtime vastly outweighs bread in this context.
        assert_eq!(apply("a bun in the oven"), "a Bun in the oven");
    }

    #[test]
    fn vocabulary_hint_covers_the_modern_stack() {
        let hint = vocabulary_hint();
        // Capped by MAX_CHARS, so bound the count instead of entry totals.
        assert!(!hint.is_empty());
        assert!(hint.len() < 200);
        assert!(hint.iter().any(|t| t == "Next.js"));
    }
}
