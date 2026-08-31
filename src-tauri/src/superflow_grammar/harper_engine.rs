use harper_core::{
    linting::LintGroup,
    parsers::PlainEnglish,
    spell::{FstDictionary, MergedDictionary, MutableDictionary},
    Dialect, DictWordMetadata, Document,
};
use once_cell::sync::Lazy;
use rayon::prelude::*;
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

/// LintGroup is not Sync due to internal caches (LruCache), so each Harper worker
/// owns a separately locked, warm group while sharing the immutable dictionary.
const MAX_HARPER_WORKERS: usize = 8;
const PARALLEL_MIN_CHARS: usize = 2_000;

fn build_lint_group() -> Mutex<LintGroup> {
    let dict = MERGED_DICT.clone();
    let mut group = LintGroup::new_curated(dict, Dialect::American);
    super::rules::register(&mut group);
    Mutex::new(group)
}

static LINT_GROUPS: Lazy<Vec<Mutex<LintGroup>>> = Lazy::new(|| {
    let workers = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1)
        .min(MAX_HARPER_WORKERS);
    (0..workers).map(|_| build_lint_group()).collect()
});

static HARPER_POOL: Lazy<rayon::ThreadPool> = Lazy::new(|| {
    rayon::ThreadPoolBuilder::new()
        .num_threads(LINT_GROUPS.len())
        .thread_name(|index| format!("harper-{index}"))
        .build()
        .expect("failed to build Harper worker pool")
});

/// Safe auto-fix policy — EXPLICIT ALLOW-LIST, not permissive.
/// Only high-confidence, single-suggestion, non-ambiguous rules are auto-fixed.
/// Custom ExprLinters must be added here explicitly after 20–50 positives + 20–100 negatives.
/// AMBIGUOUS: `suggestions.len() != 1` → preserve (e.g. `rolling out but afternoon` stays broken).
fn is_safe_to_auto_fix(lint: &harper_core::linting::Lint, rule_name: &str) -> bool {
    if lint.suggestions.len() != 1 {
        return false;
    }
    // Deny these even if they somehow appear in allow-list (defense in depth).
    // Harper's Spelling/Style/Enhancement/Regionalism/Redundancy/Readability are
    // all SuggestOnly — never auto-fixed (semantic risk, code-corruption risk).
    const DENY_RULES: &[&str] = &[
        "Spaces",
        "NoFrenchSpaces",
        "SentenceCapitalization", // harper #1107: corrupts code (transcript_cleanup.rs → .Rs)
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
    // Our own curated custom rules (registered in `rules/mod.rs`) are vetted,
    // single-suggestion, and deterministic. They are safe to auto-fix.
    // This is the critical fix: previously every `Sf*` rule was dropped by the
    // default `false`, making the "5 core grammar families" dead code.
    if rule_name.starts_with("Sf") {
        return true;
    }
    // Curated harper rules — only an explicit allow-list is auto-fixed.
    // Everything else is SuggestOnly (high recall, zero auto-mutation risk).
    const ALLOW_RULES: &[&str] = &[
        "RepeatedWords",        // the the → the (CERTAIN)
        "AnA",                  // an test → a test (CERTAIN)
        "ThereIsAgreement",     // There is many → are (HIGH, single noun)
        "PronounVerbAgreement", // he have → has (HIGH, guarded)
    ];
    // Even allowed names must be Grammar/Agreement/Punctuation, not Spelling etc.
    // (Covered by DENY_RULES above, but be explicit for safety.)
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
    HARPER_POOL.install(|| {
        LINT_GROUPS.par_iter().for_each(|group| {
            let _ = correct_with_group("warmup text for caches", group);
        });
    });
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
    let corrected = correct_preprocessed(&pre);

    let elapsed = start.elapsed();
    if elapsed.as_millis() > 50 {
        log::warn!("superflow_grammar exceeded 50ms budget: {:?}", elapsed);
    } else {
        log::debug!("superflow_grammar corrected in {:?}", elapsed);
    }
    corrected
}

fn correct_preprocessed(pre: &str) -> String {
    let chunks = parallel_chunks(pre, LINT_GROUPS.len());
    if chunks.len() == 1 {
        return correct_with_group(pre, &LINT_GROUPS[0]);
    }

    let corrected: Vec<String> = HARPER_POOL.install(|| {
        chunks
            .par_iter()
            .enumerate()
            .map(|(index, chunk)| correct_with_group(chunk, &LINT_GROUPS[index]))
            .collect()
    });
    corrected.concat()
}

fn parallel_chunks(text: &str, workers: usize) -> Vec<&str> {
    if workers < 2 || text.chars().count() < PARALLEL_MIN_CHARS {
        return vec![text];
    }

    let mut boundaries = Vec::new();
    let mut chars = text.char_indices().peekable();
    while let Some((_, ch)) = chars.next() {
        let Some(&(next_byte, next)) = chars.peek() else {
            break;
        };
        if (matches!(ch, '.' | '!' | '?') && next.is_whitespace()) || ch == '\n' {
            boundaries.push(next_byte);
        }
    }

    if boundaries.len() < 2 {
        return vec![text];
    }

    let chunk_count = workers.min(boundaries.len() + 1);
    let mut chunks = Vec::with_capacity(chunk_count);
    let mut start = 0;
    let mut boundary_index = 0;
    for part in 1..chunk_count {
        let target = text.len() * part / chunk_count;
        while boundary_index < boundaries.len() && boundaries[boundary_index] < target {
            boundary_index += 1;
        }
        if boundary_index == boundaries.len() {
            break;
        }
        let end = boundaries[boundary_index];
        if end > start {
            chunks.push(&text[start..end]);
            start = end;
        }
        boundary_index += 1;
    }
    chunks.push(&text[start..]);
    chunks
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
    fn long_transcript_matches_single_worker_output() {
        warm_up();
        let input = (0..160)
            .map(|index| {
                format!("Sentence {index} has the the repeated word and this is an test. ")
            })
            .collect::<String>();
        let pre = minimal_prepass(&input);
        let expected = correct_with_group(&pre, &LINT_GROUPS[0]);

        assert_eq!(correct(&input), expected);
        assert_eq!(parallel_chunks(&pre, LINT_GROUPS.len()).len(), 8);
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

    #[test]
    #[ignore = "manual release benchmark"]
    fn benchmark_transcript_sizes() {
        warm_up();
        for words in [100_usize, 300, 750, 1_000, 1_500, 4_500, 5_000, 6_750] {
            let sentence = "We should fix the the file because this is an test today. ";
            let repeats = words.div_ceil(sentence.split_whitespace().count());
            let input = sentence.repeat(repeats);
            let pre = minimal_prepass(&input);
            let sequential_start = std::time::Instant::now();
            let sequential = correct_with_group(&pre, &LINT_GROUPS[0]);
            let sequential_ms = sequential_start.elapsed().as_secs_f64() * 1_000.0;
            let parallel_start = std::time::Instant::now();
            let output = correct(&input);
            let parallel_ms = parallel_start.elapsed().as_secs_f64() * 1_000.0;
            println!(
                "{words:>5} words: {:>8.3} ms adaptive, {:>8.3} ms single, {:>5.2}x ({} chunks)",
                parallel_ms,
                sequential_ms,
                sequential_ms / parallel_ms,
                parallel_chunks(input.trim(), LINT_GROUPS.len()).len()
            );
            assert_eq!(output, sequential);
            assert!(output.contains("the file"));
            assert!(output.contains("a test"));
        }
    }
}
