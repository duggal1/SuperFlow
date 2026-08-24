//! Built-in programming-syntax vocabulary for post-transcription correction.
//!
//! Loads `catalog/programming-syntax.json` (namespace
//! `developer-voice-normalization-expanded`) through the shared catalog
//! harvester: every token-level {spoken → canonical} mapping across JS/TS,
//! JSON/YAML/TOML/XML, SQL, git, Docker, npm, terminal paths, environment
//! variables, API syntax, Markdown, testing, and ORM sections becomes an
//! alias entry in one n-gram fuzzy pass.
//!
//! The programming-language detection table is handled explicitly so spoken
//! language aliases ("jay ess", "type script") resolve to properly cased
//! display names (JavaScript, TypeScript, …).

use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::Value;
use std::collections::HashSet;
use std::sync::OnceLock;

use super::catalog::{harvest, AliasPair};
use crate::audio_toolkit::text::AliasMatcher;

const PROGRAMMING_SYNTAX_JSON: &str = include_str!("../catalog/programming-syntax.json");

/// Properly cased display names for the language-alias table.
fn language_display_name(key: &str) -> Option<&'static str> {
    Some(match key {
        "javascript" => "JavaScript",
        "typescript" => "TypeScript",
        "python" => "Python",
        "rust" => "Rust",
        "go" => "Go",
        "java" => "Java",
        "kotlin" => "Kotlin",
        "swift" => "Swift",
        "php" => "PHP",
        "ruby" => "Ruby",
        "c" => "C",
        "cpp" => "C++",
        "csharp" => "C#",
        "sql" => "SQL",
        "shell" => "Shell",
        _ => return None,
    })
}

/// Spoken forms that must never become ALL-CAPS technical tokens on their
/// own. They are ordinary English words ("what I did TODAY and RIGHT NOW"),
/// so a bare alias for them is only safe inside an explicitly detected
/// technical span — which the pipeline does not fabricate. Phrase-shaped
/// aliases ("get request", "right join", "count if") carry their own evidence
/// and stay eligible. T2.3/T2.4.
const AMBIGUOUS_BARE_ALIASES: &[&str] = &[
    "and", "or", "not", "in", "as", "if", "is", "by", "to", "at", "on", "get", "put", "set", "let",
    "date", "left", "right", "inner", "outer", "cross", "today", "now", "match", "search",
    "filter", "sort", "upper", "lower", "trim", "round", "max", "min", "sum", "count", "mean",
    "index", "value", "values", "text", "like", "generic", "normal", "medium", "thin", "bold",
    "light", "flex", "grid", "block", "inline", "hidden", "delete", "post", "patch", "head",
    "options",
];

const AMBIGUOUS_SQL_CANONICALS: &[&str] = &["LIKE", "IS NULL", "IS NOT NULL", "NOT NULL"];

/// True when `alias` is a single bare word that would rewrite ordinary prose.
fn alias_is_ambiguous_bare_word(alias: &str) -> bool {
    let normalized = alias.trim().to_lowercase();
    !normalized.is_empty()
        && !normalized.contains(' ')
        && AMBIGUOUS_BARE_ALIASES.contains(&normalized.as_str())
}

/// Drops pairs whose canonical form is itself an ambiguous bare English
/// word (AND, SUM, ROUND, TODAY, …) from the global fuzzy pass entirely,
/// plus any remaining ambiguous bare-word aliases (T2.3). Multiword and
/// symbol-shaped aliases are untouched elsewhere.
fn denylist_ambiguous_aliases(pairs: Vec<AliasPair>) -> Vec<AliasPair> {
    pairs
        .into_iter()
        .filter(|pair| {
            let canonical = pair.canonical.trim();
            // Multiword or symbol-shaped canonicals ("RIGHT JOIN",
            // "./src", "bg-stone-500") carry their own evidence.
            !(AMBIGUOUS_SQL_CANONICALS.contains(&canonical)
                || (canonical.split_whitespace().count() == 1
                    && !canonical.contains(['-', '_', '/', '.', '(', ')'])
                    && AMBIGUOUS_BARE_ALIASES.contains(&canonical.to_lowercase().as_str())))
        })
        .map(|pair| {
            let aliases: Vec<String> = pair
                .aliases
                .into_iter()
                .filter(|alias| !alias_is_ambiguous_bare_word(alias))
                .collect();
            AliasPair {
                canonical: pair.canonical,
                aliases,
            }
        })
        .filter(|pair| !pair.aliases.is_empty())
        .collect()
}

/// High-precision, evidence-backed technical spans (T2.4). Every rule needs
/// an explicit multiword cue or formula marker, so ordinary prose can never
/// match: "send a get request", "a right join", "today()".
fn apply_technical_spans(text: &str) -> String {
    static HTTP_METHOD: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"(?i)\b(get|post|put|patch|delete|head|options)\s+(requests?|endpoints?)\b")
            .unwrap()
    });
    static SQL_JOIN: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"(?i)\b(left|right|inner|outer|cross)\s+joins?\b").unwrap());
    static SHEET_FN: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\b(today|now)\s*\(\)").unwrap());
    static SQL_NULL: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"(?i)\b(where|having)\s+([a-z_][a-z0-9_.]*)\s+is\s+(not\s+)?null\b").unwrap()
    });
    static SQL_LIKE: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"(?i)\b(where|having)\s+([a-z_][a-z0-9_.]*)\s+like\b").unwrap());

    let text = HTTP_METHOD.replace_all(text, |caps: &regex::Captures| {
        format!("{} {}", caps[1].to_uppercase(), &caps[2])
    });
    let text = SQL_JOIN.replace_all(&text, |caps: &regex::Captures| {
        format!("{} JOIN", caps[1].to_uppercase())
    });
    let text = SHEET_FN
        .replace_all(&text, |caps: &regex::Captures| {
            format!("{}()", caps[1].to_uppercase())
        })
        .into_owned();
    let text = SQL_NULL.replace_all(&text, |caps: &regex::Captures| {
        let not = caps.get(3).map_or("", |_| "NOT ");
        format!("{} {} IS {not}NULL", &caps[1], &caps[2])
    });
    SQL_LIKE
        .replace_all(&text, |caps: &regex::Captures| {
            format!("{} {} LIKE", &caps[1], &caps[2])
        })
        .into_owned()
}

type AliasEntries = Vec<(String, Vec<String>)>;

static PROGRAMMING: OnceLock<AliasEntries> = OnceLock::new();
static MATCHER: OnceLock<AliasMatcher> = OnceLock::new();

fn entries() -> &'static AliasEntries {
    PROGRAMMING.get_or_init(|| {
        let mut pairs: Vec<AliasPair> = Vec::new();
        match serde_json::from_str::<Value>(PROGRAMMING_SYNTAX_JSON) {
            Ok(document) => {
                // Language aliases first so their keys win dedupe conflicts.
                if let Some(languages) = document
                    .get("programming_language_detection")
                    .and_then(Value::as_object)
                {
                    for (key, aliases) in languages {
                        let Some(display) = language_display_name(key) else {
                            continue;
                        };
                        let spoken: Vec<String> = aliases
                            .as_array()
                            .map(|list| {
                                list.iter()
                                    .filter_map(Value::as_str)
                                    .map(str::to_string)
                                    .collect()
                            })
                            .unwrap_or_default();
                        if !spoken.is_empty() {
                            pairs.push(AliasPair {
                                canonical: display.to_string(),
                                aliases: spoken,
                            });
                        }
                    }
                }
                harvest(&document, &mut pairs);
            }
            Err(e) => log::error!("Failed to parse embedded programming-syntax catalog: {e}"),
        }

        let mut seen: HashSet<String> = HashSet::new();
        let mut out: AliasEntries = Vec::with_capacity(pairs.len());
        for pair in denylist_ambiguous_aliases(pairs) {
            if seen.insert(pair.canonical.clone()) {
                out.push((pair.canonical, pair.aliases));
            }
        }
        out
    })
}

/// Number of programming-syntax corrections available.
pub fn len() -> usize {
    entries().len()
}

/// Canonical display forms (syntax tokens, commands, env vars) for
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

/// Applies the built-in programming-syntax catalog to transcribed text.
/// Path-shaped canonicals ("./", "~/", "/api/users") are re-joined with the
/// following word so spoken segmentation ("dot slash src") yields real paths.
/// Ambiguous bare English words are denylisted from the fuzzy pass; explicit
/// evidence-backed spans (get request, right join, today()) are restored by
/// high-precision rules instead.
pub fn apply(text: &str) -> String {
    if text.is_empty() {
        return text.to_string();
    }
    let corrected = MATCHER
        .get_or_init(|| AliasMatcher::new(entries(), MATCH_THRESHOLD))
        .apply(text);
    let corrected = apply_technical_spans(&corrected);
    crate::audio_toolkit::text::join_path_tokens(&corrected)
}

pub(crate) fn warm_up() {
    let _ = MATCHER.get_or_init(|| AliasMatcher::new(entries(), MATCH_THRESHOLD));
}

/// Same strictness as the other catalogs: near-exact hits only.
const MATCH_THRESHOLD: f64 = 0.2;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_loads_substantially() {
        let total = len();
        assert!(total > 150, "expected substantial entries, got {total}");
    }

    #[test]
    fn corrects_spoken_syntax_tokens() {
        assert_eq!(apply("cd dot slash src"), "cd ./src");
        assert_eq!(
            apply("set the database url env var"),
            "set the DATABASE_URL env var"
        );
    }

    #[test]
    fn resolves_spoken_language_names() {
        assert_eq!(apply("built with jay ess"), "built with JavaScript");
        assert_eq!(apply("a type script project"), "a TypeScript project");
    }

    #[test]
    fn plain_prose_passes_through() {
        let text = "let us grab lunch after the meeting";
        assert_eq!(apply(text), text);
    }

    #[test]
    fn bare_common_english_words_never_become_tokens() {
        // The exact reported failure family (plan T2): ordinary speech must
        // not acquire ALL-CAPS spreadsheet/SQL/API/shell tokens.
        for word in [
            "and", "right", "today", "now", "left", "match", "search", "filter", "sort", "upper",
            "lower", "trim", "round", "get",
        ] {
            let text = format!("what I did {} {} o'clock", word, word);
            let out = apply(&text);
            assert!(
                !out.contains(word.to_uppercase().as_str()),
                "bare {word:?} became a token: {out}"
            );
        }
        assert_eq!(
            apply("what I did today and right now"),
            "what I did today and right now"
        );
        assert_eq!(
            apply("I goofed around with my mom all day"),
            "I goofed around with my mom all day"
        );
        assert_eq!(
            apply("like this is null generic normal delete right now"),
            "like this is null generic normal delete right now"
        );
        assert_eq!(apply("and I like this"), "and I like this");
        assert_eq!(apply("and it is null today"), "and it is null today");
    }

    #[test]
    fn explicit_technical_phrases_still_normalize() {
        assert_eq!(
            apply("send a get request to the api"),
            "send a GET request to the api"
        );
        assert_eq!(
            apply("query the right join first"),
            "query the RIGHT JOIN first"
        );
        assert_eq!(
            apply("call today() on the sheet"),
            "call TODAY() on the sheet"
        );
        assert_eq!(
            apply("set the database url env var"),
            "set the DATABASE_URL env var"
        );
        assert_eq!(apply("cd dot slash src"), "cd ./src");
        assert_eq!(apply("where email is null"), "where email IS NULL");
        assert_eq!(apply("where name like"), "where name LIKE");
        assert_eq!(apply("use a generic type"), "use a <T>");
    }

    #[test]
    fn formula_markers_are_required_for_sheet_functions() {
        // No parens, no function: ordinary words stay ordinary.
        assert_eq!(apply("what today means now"), "what today means now");
    }
}
