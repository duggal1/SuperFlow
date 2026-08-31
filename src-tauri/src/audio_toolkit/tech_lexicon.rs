//! Built-in technical vocabulary for pre-cleanup correction.
//!
//! Ships a curated lexicon of technology terms with their common mishearings
//! ("next year" → "Next.js", "tail winds" → "Tailwind CSS") and applies it to
//! every eligible English transcript through the same n-gram fuzzy engine used
//! for user custom words before S1-mini. Entirely local — no network or API key.

use std::sync::OnceLock;

use crate::audio_toolkit::text::AliasMatcher;

/// Embedded at compile time from the catalog; see `catalog/tech_lexicon.json`.
const LEXICON_JSON: &str = include_str!("../catalog/tech_lexicon.json");

/// Stricter than the user custom-word threshold (default 0.18 is user-chosen):
/// built-in entries must not fire on loose matches, only near-exact alias or
/// canonical hits.
const MATCH_THRESHOLD: f64 = 0.2;

type LexiconEntries = Vec<(String, Vec<String>)>;

static LEXICON: OnceLock<LexiconEntries> = OnceLock::new();
static MATCHER: OnceLock<AliasMatcher> = OnceLock::new();

const AMBIGUOUS_PROSE_ALIASES: &[&str] = &[
    "render",
    "go",
    "bun",
    "react",
    "swift",
    "rust",
    "yarn",
    "spring",
    "motion",
    "dart",
    "flutter",
    "solid",
    "remix",
    "express",
    "linear",
    "notion",
    "railway",
    "use transition",
    "transition",
    "transaction",
];

fn safe_alias(canonical: &str, alias: &str) -> bool {
    let normalized = alias.trim().to_lowercase();
    if AMBIGUOUS_PROSE_ALIASES.contains(&normalized.as_str()) {
        return false;
    }
    if matches!(canonical, "useEffect" | "useState") {
        return true;
    }
    if canonical.starts_with("use") {
        return normalized.contains(" hook") || normalized.contains("react ");
    }
    if canonical == "Qwik" && normalized == "quick" {
        return false;
    }
    if canonical == "OpenSearch" && normalized == "open search" {
        return false;
    }
    true
}

fn entries() -> &'static LexiconEntries {
    LEXICON.get_or_init(|| {
        match serde_json::from_str::<serde_json::Value>(LEXICON_JSON) {
            Ok(document) => {
                let mut pairs = Vec::new();
                crate::audio_toolkit::catalog::harvest(&document, &mut pairs);
                pairs
                    .into_iter()
                    .filter_map(|pair| {
                        let aliases = pair
                            .aliases
                            .into_iter()
                            .filter(|alias| safe_alias(&pair.canonical, alias))
                            .collect::<Vec<_>>();
                        (!aliases.is_empty()).then_some((pair.canonical, aliases))
                    })
                    .collect()
            }
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
    MATCHER
        .get_or_init(|| AliasMatcher::new(entries(), MATCH_THRESHOLD))
        .apply(text)
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
        assert_eq!(apply("a quick status update"), "a quick status update");
        assert_eq!(apply("an open source project"), "an open source project");
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
    fn corrects_environment_config_tokens() {
        for input in ["dot env local", "dot e n v local", "env local"] {
            assert_eq!(apply(input), ".env.local", "input: {input}");
        }
        for input in ["dot env", "dot e n v", "env file"] {
            assert_eq!(apply(input), ".env", "input: {input}");
        }
    }

    #[test]
    fn corrects_global_high_confidence_framework_apis() {
        for input in ["use effect", "use-effect", "useeffect"] {
            assert_eq!(apply(input), "useEffect", "input: {input}");
        }
        for input in ["use state", "use-state", "usestate"] {
            assert_eq!(apply(input), "useState", "input: {input}");
        }
        for input in ["get user by id", "get user by i d"] {
            assert_eq!(apply(input), "getUserById", "input: {input}");
        }
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
        assert_eq!(apply("a bun in the oven"), "a bun in the oven");
    }

    #[test]
    fn ambiguous_product_and_hook_words_need_technical_context() {
        assert_eq!(
            apply("render it and use transition while we go"),
            "render it and use transition while we go"
        );
        assert_eq!(
            apply("deploy on render hosting with the use transition hook"),
            "deploy on Render with the useTransition"
        );
    }

    #[test]
    fn vocabulary_hint_covers_the_modern_stack() {
        let hint = vocabulary_hint();
        // Capped by MAX_CHARS, so bound the count instead of entry totals.
        assert!(!hint.is_empty());
        assert!(hint.len() < 200);
        assert!(hint.iter().any(|t| t == "Next.js"));
    }

    #[test]
    fn corrects_2026_model_names() {
        assert!(apply("i am using opers five today").contains("Claude Opus 5"));
        assert!(apply("switch to opus 5 now").contains("Claude Opus 5"));
        assert!(apply("run it on he cows 4.5").contains("Claude Haiku 4.5"));
        assert!(apply("the hekau model is fast").contains("Claude Haiku 4.5"));
        assert!(apply("hikau 4.5 is outdated now").contains("Claude Haiku 4.5"));
        assert!(apply("try chatgpt 5.6 seoul").contains("GPT-5.6 Sol"));
        assert!(apply("gpt 5.6 soul vs gpt 5.6 terror").contains("GPT-5.6 Sol"));
        assert!(apply("gpt 5.6 terror benchmark").contains("GPT-5.6 Terra"));
        assert!(apply("deploy deepseek v4 flash").contains("DeepSeek-V4-Flash"));
        assert!(apply("compare kimi k two point five and kimi k three").contains("Kimi K2.5"));
        assert!(apply("grok four point six released").contains("Grok 4.6"));
        assert!(apply("glm five point three scores").contains("GLM-5.3"));
        assert!(apply("the jamnia model by google").contains("Gemini"));
        assert!(apply("sonnet five vs fable five").contains("Claude Sonnet 5"));
    }

    #[test]
    fn corrects_august_2026_wave() {
        assert!(apply("what about moose park 1.2 from meta").contains("Muse Spark 1.2"));
        assert!(apply("moose spark benchmarks dropped").contains("Muse Spark"));
        assert!(apply("muse glimmer is multimodal").contains("Muse Glimmer"));
        assert!(apply("kimmy k three launched today").contains("Kimi K3"));
        assert!(apply("kimmy k two point five will launch").contains("Kimi K2.5"));
        assert!(apply("kimi k2.7 code is out").contains("Kimi K2.7 Code"));
        assert!(apply("quinn 3.7 plus pricing").contains("Qwen3.7-Plus"));
        assert!(
            apply("queen 3.8 max vs q when 3.7").contains("Qwen3.8-Max")
                && apply("queen 3.8 max vs q when 3.7").contains("Qwen3.7")
        );
        assert!(apply("glm five point two shipped").contains("GLM-5.2"));
    }

    #[test]
    fn everyday_prose_is_never_mangled_by_model_aliases() {
        // The user guardrail: non-developer daily speech stays untouched.
        assert_eq!(apply("my soul left the room"), "my soul left the room");
        assert_eq!(
            apply("she read a fable to the kids"),
            "she read a fable to the kids"
        );
        assert_eq!(apply("the cows graze all day"), "the cows graze all day");
        assert_eq!(apply("seoul is sunny today"), "seoul is sunny today");
        assert_eq!(apply("he wrote a haiku"), "he wrote a haiku");
        assert_eq!(
            apply("terra firma under our feet"),
            "terra firma under our feet"
        );
        assert_eq!(apply("luna lit the sky"), "luna lit the sky");
        assert_eq!(apply("an opera about mythos"), "an opera about mythos");
        assert_eq!(
            apply("a queen rules the kingdom"),
            "a queen rules the kingdom"
        );
        assert_eq!(apply("a moose in the forest"), "a moose in the forest");
        assert_eq!(apply("the glimmer of hope"), "the glimmer of hope");
    }
}
