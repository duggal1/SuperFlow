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

use serde_json::Value;
use std::collections::HashSet;
use std::sync::OnceLock;

use super::catalog::{harvest, AliasPair};

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

type AliasEntries = Vec<(String, Vec<String>)>;

static PROGRAMMING: OnceLock<AliasEntries> = OnceLock::new();

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
        for pair in pairs {
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
pub fn is_known_term(word: &str) -> bool {{
    use std::sync::OnceLock;
    use std::collections::HashSet;
    static KNOWN: OnceLock<HashSet<String>> = OnceLock::new();
    let normalized: String = word.chars().filter(|c| c.is_alphanumeric()).collect();
    let known = KNOWN.get_or_init(|| {{
        entries()
            .iter()
            .filter_map(|(canonical, _)| {{
                let k: String = canonical.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase();
                (!k.is_empty()).then_some(k)
            }})
            .collect()
    }});
    known.contains(&normalized)
}}

/// Applies the built-in programming-syntax catalog to transcribed text.
/// Path-shaped canonicals ("./", "~/", "/api/users") are re-joined with the
/// following word so spoken segmentation ("dot slash src") yields real paths.
pub fn apply(text: &str) -> String {
    if text.is_empty() {
        return text.to_string();
    }
    let corrected =
        crate::audio_toolkit::text::apply_alias_entries(text, entries(), MATCH_THRESHOLD);
    crate::audio_toolkit::text::join_path_tokens(&corrected)
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
}

#[cfg(test)]
mod debug_probe {
    #[test]
    fn probe_env_var() {
        let all = super::entries();
        eprintln!("PROBE owners of envvar:");
        for (canonical, aliases) in all.iter() {
            for a in aliases {
                let k: String = a
                    .split_whitespace()
                    .map(crate::audio_toolkit::text::public_build_match_key)
                    .collect();
                if k == "envvar" || k == "nevertype" {
                    eprintln!("  {:?} <- alias {:?} key {:?}", canonical, a, k);
                }
            }
        }
        eprintln!(
            "PROBE apply: {:?}",
            super::apply("set the database url env var")
        );
    }
}
