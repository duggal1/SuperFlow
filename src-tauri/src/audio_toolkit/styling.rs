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
use crate::audio_toolkit::text::AliasMatcher;

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

fn safe_styling_alias(canonical: &str, alias: &str) -> bool {
    if !safe_alias_for_hex(canonical, alias) {
        return false;
    }
    let normalized = alias.trim().to_lowercase();
    if normalized == "right zero" {
        return false;
    }
    let bare_word = normalized.split_whitespace().count() == 1
        && normalized
            .chars()
            .all(|character| character.is_alphabetic());
    !(bare_word && canonical.to_lowercase() != normalized)
}

type AliasEntries = Vec<(String, Vec<String>)>;

static STYLING: OnceLock<AliasEntries> = OnceLock::new();
static MATCHER: OnceLock<AliasMatcher> = OnceLock::new();

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
                .filter(|alias| safe_styling_alias(&pair.canonical, alias))
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

/// Applies the built-in styling catalogs to transcribed text.
pub fn apply(text: &str) -> String {
    if text.is_empty() {
        return text.to_string();
    }
    MATCHER
        .get_or_init(|| AliasMatcher::new(entries(), MATCH_THRESHOLD))
        .apply(text)
}

pub(crate) fn warm_up() {
    let _ = MATCHER.get_or_init(|| AliasMatcher::new(entries(), MATCH_THRESHOLD));
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
        assert_eq!(
            apply("right now use the normal model"),
            "right now use the normal model"
        );
        assert_eq!(apply("set right position zero"), "set right-0");
    }
}

#[cfg(test)]
mod digit_probe {
    #[test]
    fn digit_shades_fire() {
        assert_eq!(super::apply("use bg stone 200"), "use bg-stone-200");
        // The authored "on hover …" pattern legitimately consumes "on".
        assert_eq!(
            super::apply("border zinc 400 on hover bg stone 300"),
            "border-zinc-400 hover:bg-stone-300"
        );
        // Both class utterances resolve even when a conjunction is absorbed
        // by a nearby pattern window.
        let out = super::apply("border zinc 400 and hover bg stone 300");
        assert!(out.contains("border-zinc-400"), "got: {out}");
        assert!(out.contains("hover:bg-stone-300"), "got: {out}");
        assert_eq!(super::apply("give it gap of sex"), "give it gap-6");
        assert_eq!(super::apply("six p padding"), "p-6 padding");
    }
}
