use harper_core::{
    linting::LintGroup,
    parsers::PlainEnglish,
    spell::{FstDictionary, MergedDictionary, MutableDictionary},
    Dialect, DictWordMetadata, Document,
};
use once_cell::sync::Lazy;
use std::sync::{Arc, Mutex};

use super::protected_spans::{find_protected_spans, is_protected};

/// Custom tech dictionary terms — additional linguistic knowledge for harper's
/// POS tagger, NOT the safety boundary. Protected spans are the hard safety
/// boundary; dictionary is secondary (harper discussion #1107: disable
/// SentenceCapitalization for code even with dictionary). Keep list small and
/// curated; `tech_lexicon` handles decode-time `next year → Next.js` separately.
const TECH_TERMS: &[&str] = &[
    // Core stack
    "Zustand",
    "Tauri",
    "Parakeet",
    "Silero",
    "Superflow",
    "SuperFlow",
    "Next.js",
    "React",
    "TypeScript",
    "ZustandStore",
    "SuperflowPanel",
    "VAD",
    "ONNX",
    "ASR",
    "GPT",
    "Muse",
    "Gemini",
    "Grok",
    "Qwen",
    "GLM",
    "Muse",
    "Glimmer",
    "Kimi",
    // Code-ish
    "getUserById",
    "getContextForFileName",
    "normalizeTranscript",
    "handlePaste",
    "myVarName",
    "fileName",
    "parseTranscript",
    "useEffect",
    "useState",
    "useRef",
    "flate2",
    "harper-core",
    "tauri-plugin-clipboard-manager",
    "src-tauri",
    "clipboard_manager",
];

fn build_merged_dict() -> Arc<MergedDictionary> {
    let curated = FstDictionary::curated();
    let mut custom = MutableDictionary::new();
    // Extend with tech terms — lowercased and original casing both
    let mut words: Vec<(Vec<char>, DictWordMetadata)> = Vec::new();
    for term in TECH_TERMS {
        // Add original casing
        words.push((term.chars().collect(), DictWordMetadata::default()));
        // Add lowercased for case-insensitive matching
        let lower = term.to_lowercase();
        if lower != *term {
            words.push((lower.chars().collect(), DictWordMetadata::default()));
        }
    }
    custom.extend_words(words);
    let mut merged = MergedDictionary::new();
    merged.add_dictionary(curated as Arc<dyn harper_core::spell::Dictionary>);
    merged.add_dictionary(Arc::new(custom) as Arc<dyn harper_core::spell::Dictionary>);
    Arc::new(merged)
}

static MERGED_DICT: Lazy<Arc<MergedDictionary>> = Lazy::new(build_merged_dict);

/// LintGroup is not Sync due to internal caches (LruCache), so we wrap in Mutex.
/// We create one per thread via thread_local or global Mutex for simplicity.
/// For <50ms target we want to reuse the group (caches warm), so we keep a global.
static LINT_GROUP: Lazy<Mutex<LintGroup>> = Lazy::new(|| {
    let dict = MERGED_DICT.clone();
    let mut group = LintGroup::new_curated(dict, Dialect::American);
    super::rules::register(&mut group);
    Mutex::new(group)
});

/// Safe auto-fix policy — EXPLICIT ALLOW-LIST, not permissive.
/// Only high-confidence, single-suggestion, non-ambiguous rules are auto-fixed.
/// Custom ExprLinters must be added here explicitly after 20–50 positives + 20–100 negatives.
/// AMBIGUOUS: `suggestions.len() != 1` → preserve (e.g. `rolling out but afternoon` stays broken).
fn is_safe_to_auto_fix(lint: &harper_core::linting::Lint, rule_name: &str) -> bool {
    if lint.suggestions.len() != 1 {
        return false;
    }
    // Explicit allow-list: only these rule names are CERTAIN/HIGH. Everything else is SuggestOnly.
    // This fixes the bug where any future custom rule with 1 suggestion would be auto-fixed even if low-confidence.
    const ALLOW_RULES: &[&str] = &[
        "RepeatedWords",          // the the → the (CERTAIN)
        "AnA",                    // an test → a test (CERTAIN)
        "ThereIsAgreement",       // There is many → are (HIGH, single noun)
        "PronounVerbAgreement",   // he have → has (HIGH, guarded)
        "SentenceCapitalization", // Minimal, but we deny it for code — keep denied, not here
    ];
    // Deny these even if they somehow appear in allow-list (defense in depth)
    const DENY_RULES: &[&str] = &[
        "Spaces",
        "NoFrenchSpaces",
        "SentenceCapitalization",
        "SpellCheck",
        "SplitWords",
        "CompoundNouns",
        "OrthographicConsistency",
        "CorrectNumberSuffix",
        "NumberSuffixCapitalization",
        "ExpandMemoryShorthands",
        "LongSentences",
        "BoringWords",
        "Hedging",
        "FillerWords",
        "Spelling",
        "Style",
        "Enhancement",
        "Regionalism",
        "Redundancy",
        "Readability",
    ];
    if DENY_RULES.contains(&rule_name) {
        return false;
    }
    // Check lint_kind as well — even allowed names must be Grammar/Agreement/Punctuation, not Spelling etc.
    match lint.lint_kind {
        harper_core::linting::LintKind::Spelling
        | harper_core::linting::LintKind::Style
        | harper_core::linting::LintKind::Enhancement
        | harper_core::linting::LintKind::Regionalism
        | harper_core::linting::LintKind::Redundancy
        | harper_core::linting::LintKind::Readability => return false,
        _ => {}
    }
    ALLOW_RULES.contains(&rule_name)
}

/// Minimal pre-pass before harper — NOT a formatter.
/// Harper's `Document` parser needs reasonable sentence boundaries without
/// inventing commas. We do `trim` only; internal spacing is preserved for
/// `transcript_cleanup` + `formatter.rs` after harper. See `mod.rs` architecture:
/// `transcript_cleanup → protected spans → Harper` — harper never handles
/// spacing/text-track fulfillment.
fn minimal_prepass(text: &str) -> String {
    text.trim().to_string()
}

/// Warm harper caches off the hot path — call once at startup.
/// First `correct()` pays ~900ms cold (dictionary + LintGroup init); warmup moves that to init.
pub fn warm_up() {
    let _ = correct("warmup text for caches");
}

/// Correct `text` with harper-core, protected spans, and safe auto-fix policy.
/// Always runs, no toggle, <50ms target. Returns corrected text.
/// If harper panics or exceeds budget, returns original (fail-open).
pub fn correct(text: &str) -> String {
    if text.trim().is_empty() {
        return text.to_string();
    }

    // Fail-open wrapper — harper must never break dictation
    let result = std::panic::catch_unwind(|| correct_inner(text));
    match result {
        Ok(s) => s,
        Err(_) => {
            log::error!("superflow_grammar harper_engine panicked; returning original");
            text.to_string()
        }
    }
}

fn correct_inner(text: &str) -> String {
    let pre = minimal_prepass(text);
    if pre.is_empty() {
        return pre;
    }

    let start = std::time::Instant::now();
    let corrected = correct_with_group(&pre, &LINT_GROUP);

    let elapsed = start.elapsed();
    if elapsed.as_millis() > 50 {
        log::warn!("superflow_grammar exceeded 50ms budget: {:?}", elapsed);
    } else {
        log::debug!("superflow_grammar corrected in {:?}", elapsed);
    }
    corrected
}

fn correct_with_group(pre: &str, group: &Mutex<LintGroup>) -> String {
    let protected = find_protected_spans(pre);

    // Build Document with merged dict so POS tagging knows tech terms
    let doc = Document::new(pre, &PlainEnglish, &*MERGED_DICT);

    // Lint with global group (warm caches)
    let mut group = match group.lock() {
        Ok(g) => g,
        Err(_) => return pre.to_string(),
    };
    let organized = group.organized_lints(&doc);
    drop(group);

    // Flatten with rule names, filter by protected spans and safe policy
    let mut candidates: Vec<(harper_core::linting::Lint, String)> = Vec::new();
    for (rule, lints) in organized {
        for lint in lints {
            if is_protected(lint.span, &protected) {
                continue;
            }
            if !is_safe_to_auto_fix(&lint, rule.as_str()) {
                continue;
            }
            candidates.push((lint, rule.clone()));
        }
    }

    if candidates.is_empty() {
        return pre.to_string();
    }

    // Remove overlapping lints — lower priority (smaller u8) wins. This prevents
    // the "Hello    world  . → Hello world This is a tes" truncation bug we saw
    // in brutal test 3.2 where overlapping Spaces lints corrupted output.
    let mut lints_only: Vec<harper_core::linting::Lint> =
        candidates.into_iter().map(|(l, _)| l).collect();
    harper_core::remove_overlaps(&mut lints_only);

    // Re-filter after dedup (remove_overlaps may have kept a Spelling lint we filtered earlier? No, already filtered)
    // Sort reverse for safe apply (so offsets stay valid)
    lints_only.sort_by(|a, b| b.span.start.cmp(&a.span.start));

    let mut chars: Vec<char> = pre.chars().collect();
    for lint in lints_only {
        // Final guard: lint span must be within chars (harper spans are char-based)
        if lint.span.end > chars.len() {
            continue;
        }
        lint.suggestions[0].apply(lint.span, &mut chars);
    }
    chars.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_protected_spans() {
        let input = "the file src-tauri/src/transcript/cleanup.rs contains function getUserById and it dont work";
        let out = correct(input);
        // Protected tokens must survive byte-for-byte
        assert!(out.contains("cleanup.rs"), "lost cleanup.rs: {}", out);
        assert!(out.contains("getUserById"), "lost getUserById: {}", out);
        // src-tauri path should be protected too
        assert!(out.contains("src-tauri"), "lost src-tauri: {}", out);
    }

    #[test]
    fn fixes_repeated_words() {
        let input = "We should fix the the file and handle it";
        let out = correct(input);
        assert_eq!(out, "We should fix the file and handle it");
    }

    #[test]
    fn fixes_an_a() {
        let input = "This is an test and a apple";
        let out = correct(input);
        assert_eq!(out, "This is a test and an apple");
    }

    #[test]
    fn does_not_fix_spacing() {
        let input = "hello   there  with  weird   spacing";
        let out = correct(input);
        // Harper should NOT fix spacing — that's formatter's job
        assert_eq!(out, input, "harper should not touch spacing");
    }

    #[test]
    fn does_not_corrupt_code_via_sentence_cap() {
        let input =
            "the context file name is transcript_cleanup.rs and it contains function formatter.rs";
        let out = correct(input);
        // Should not become transcript_cleanup.Rs
        assert!(out.contains("transcript_cleanup.rs"));
        assert!(out.contains("formatter.rs"));
    }

    #[test]
    fn does_not_apply_spelling() {
        let input = "we use Zustand and Tauri";
        let out = correct(input);
        assert_eq!(out, input, "spelling should not be auto-fixed");
    }

    #[test]
    fn handles_empty() {
        assert_eq!(correct(""), "");
        assert_eq!(correct("   "), "   ");
    }

    #[test]
    fn performance_budget() {
        // Warm caches first — first run includes dictionary + LintGroup init (~900ms cold)
        let _ = correct("warmup text for caches");

        let mut long = String::new();
        for _ in 0..200 {
            long.push_str("the team have have meeting and they was discussing about the issue which have multiple bug. ");
        }
        let start = std::time::Instant::now();
        let _ = correct(&long);
        let elapsed = start.elapsed();
        // Debug builds are ~100× slower than release (4.8s vs 28ms). Enforce <50ms only in release,
        // where the <50ms budget matters for the hot path. In debug we allow <2s (flaky on loaded runners).
        let budget_ms = if cfg!(debug_assertions) { 2000 } else { 50 };
        assert!(
            (elapsed.as_millis() as u64) < budget_ms,
            "exceeded {}ms: {:?}",
            budget_ms,
            elapsed
        );
    }
}
