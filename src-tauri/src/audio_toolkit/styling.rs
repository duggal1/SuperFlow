//! Built-in Tailwind/styling vocabulary for post-transcription correction.
//!
//! Loads BOTH styling catalogs — `catalog/styling.json` (v3, class aliases +
//! hex colors + high-value speech patterns) and `catalog/stylings.json` (v1,
//! prefixes, variants, semantic instructions) — through the shared catalog
//! harvester so every token-level mapping flows into one n-gram fuzzy pass.
//!
//! Hex-color aliases are filtered: bare color words ("white", "red") would
//! rewrite ordinary prose, so only unambiguous spoken forms survive
//! ("hex white", "standard red", "tailwind blue").

use std::collections::HashSet;
use std::sync::OnceLock;

use super::catalog::{harvest, AliasPair};

const STYLING_JSON: &str = include_str!("../catalog/styling.json");
const STYLINGS_JSON: &str = include_str!("../catalog/stylings.json");

/// Bare color words must never become hex codes in normal sentences.
fn safe_alias_for_hex(canonical: &str, alias: &str) -> bool {
    if !canonical.starts_with('#') {
        return true;
    }
    let lowered = alias.to_lowercase();
    !matches!(
        lowered.as_str(),
        "white"
            | "black"
            | "red"
            | "orange"
            | "yellow"
            | "green"
            | "cyan"
            | "blue"
            | "violet"
            | "pink"
    )
}

type AliasEntries = Vec<(String, Vec<String>)>;

static STYLING: OnceLock<AliasEntries> = OnceLock::new();

fn entries() -> &'static AliasEntries {
    STYLING.get_or_init(|| {
        // Both files are validated up front: a malformed embedded catalog
        // must never break dictation, so a parse failure degrades to
        // whatever the other file provided.
        let mut pairs: Vec<AliasPair> = Vec::new();
        for source in [STYLING_JSON, STYLINGS_JSON] {
            match serde_json::from_str::<serde_json::Value>(source) {
                Ok(document) => harvest(&document, &mut pairs),
                Err(e) => log::error!("Failed to parse embedded styling catalog: {e}"),
            }
        }

        let mut seen: HashSet<String> = HashSet::new();
        let mut out: AliasEntries = Vec::with_capacity(pairs.len());
        for pair in pairs {
            if !seen.insert(pair.canonical.clone()) {
                continue;
            }
            let aliases: Vec<String> = pair
                .aliases
                .into_iter()
                .filter(|alias| safe_alias_for_hex(&pair.canonical, alias))
                .collect();
            out.push((pair.canonical, aliases));
        }
        out
    })
}

/// Number of styling corrections available.
pub fn len() -> usize {
    entries().len()
}

/// Canonical display forms (Tailwind utilities and class tokens) for
/// decode-time vocabulary biasing.
pub fn canonical_names() -> impl Iterator<Item = &'static str> {
    entries().iter().map(|(canonical, _)| canonical.as_str())
}

/// Applies the built-in styling catalogs to transcribed text.
pub fn apply(text: &str) -> String {
    if text.is_empty() {
        return text.to_string();
    }
    crate::audio_toolkit::text::apply_alias_entries(text, entries(), MATCH_THRESHOLD)
}

/// Same strictness as the technical lexicon: near-exact alias or canonical
/// hits only.
const MATCH_THRESHOLD: f64 = 0.2;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_catalogs_load_substantially() {
        let total = len();
        assert!(
            total > 200,
            "expected substantial styling entries, got {total}"
        );
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
