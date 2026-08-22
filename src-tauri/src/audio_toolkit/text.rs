use natural::phonetics::soundex;
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashSet;
use strsim::levenshtein;

/// Builds an n-gram string by cleaning and concatenating words
///
/// Strips punctuation from each word, lowercases, and joins without spaces.
/// This allows matching "Charge B" against "ChargeBee".
fn build_ngram(words: &[&str]) -> String {
    words
        .iter()
        .map(|w| build_match_key(w))
        .collect::<Vec<_>>()
        .concat()
}

pub(crate) fn public_build_match_key(word: &str) -> String {
    build_match_key(word)
}

fn build_match_key(word: &str) -> String {
    word.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

struct CustomWordMatchKey {
    word_index: usize,
    key: String,
    /// Vowel-collapsed consonant skeleton of `key` for phonetic fallback
    /// matching (built-in lexicon only). Empty when the key is too short.
    skeleton: String,
    /// Number of whitespace-separated words the phrase had before keying.
    /// A multi-word n-gram must only match a key with the SAME word count —
    /// comparing concatenated keys lets filler words ("make it flex layout"
    /// vs "flex layout") get absorbed into a match and deleted.
    word_count: usize,
}

/// Maps ASR-equivalent consonants to a shared letter and drops vowels and
/// glide consonants: c/k/q → "k", z → "s", vowels/h/w/y removed, adjacent
/// duplicates collapsed. "kubernetes" and "coobernetees" both become
/// "kbrnts", so one entry covers mishearings no alias list can enumerate.
fn consonant_skeleton(key: &str) -> String {
    let mut out = String::new();
    let mut prev = '\0';
    for ch in key.chars() {
        let mapped = match ch {
            'c' | 'k' | 'q' => 'k',
            'z' => 's',
            'a' | 'e' | 'i' | 'o' | 'u' | 'h' | 'w' | 'y' => continue,
            other => other,
        };
        if mapped != prev {
            out.push(mapped);
            prev = mapped;
        }
    }
    out
}

/// Phonetic fallback fires only on substantial words; short candidates are
/// left to the strict orthographic path so prose stays untouched.
const SKELETON_MIN_CHARS: usize = 5;
/// Max normalized skeleton edit distance for a phonetic match.
const SKELETON_THRESHOLD: f64 = 0.25;

fn build_custom_word_match_keys(word: &str, word_index: usize) -> Vec<CustomWordMatchKey> {
    let primary_key = build_match_key(word);
    let mut keys = Vec::with_capacity(2);

    // The fallback matcher is intentionally limited to ASCII terms. Its
    // whitespace tokenization and Soundex scoring are not suitable for CJK
    // scripts. Unicode custom words remain available to models that accept
    // them as native decode prompts; they are simply skipped by this fallback.
    if is_supported_fuzzy_key(&primary_key) {
        let skeleton = if primary_key.chars().count() >= SKELETON_MIN_CHARS {
            consonant_skeleton(&primary_key)
        } else {
            String::new()
        };
        keys.push(CustomWordMatchKey {
            word_index,
            key: primary_key.clone(),
            skeleton,
            // Custom words keep the legacy glue semantics (#1406): a spoken
            // "Mac Book Pro" must fuzzy-match the authored phrase
            // "MacBook Pro" across variable word counts, unlike authored
            // lexicon aliases which align strictly.
            word_count: 1,
        });
    }

    if word.contains('&') {
        let expanded_key = build_match_key(&word.replace('&', " and "));
        if is_supported_fuzzy_key(&expanded_key) && expanded_key != primary_key {
            keys.push(CustomWordMatchKey {
                word_index,
                key: expanded_key,
                skeleton: String::new(),
                word_count: 1,
            });
        }
    }

    keys
}

fn is_supported_fuzzy_key(key: &str) -> bool {
    !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric())
}

fn supports_soundex(key: &str) -> bool {
    !key.is_empty() && key.chars().all(|c| c.is_ascii_alphabetic())
}

/// Finds the best matching custom word for a candidate string
///
/// Uses Levenshtein distance and Soundex phonetic matching to find
/// the best match above the given threshold.
///
/// # Arguments
/// * `candidate` - The cleaned/lowercased candidate string to match
/// * `custom_words` - Original custom words (for returning the replacement)
/// * `custom_word_match_keys` - Normalized custom-word keys for comparison
/// * `threshold` - Maximum similarity score to accept
///
/// # Returns
/// The best matching custom word and its score, if any match was found
fn find_best_match<'a>(
    candidate: &str,
    custom_words: &'a [String],
    custom_word_match_keys: &[&CustomWordMatchKey],
    window_word_count: usize,
    threshold: f64,
    allow_phonetic_boost: bool,
) -> Option<(&'a String, f64)> {
    if !is_supported_fuzzy_key(candidate) || candidate.chars().count() > 50 {
        return None;
    }

    let mut best_match: Option<&String> = None;
    let mut best_score = f64::MAX;

    for custom_word_key in custom_word_match_keys {
        // Authored multi-word phrases must align with the window's word
        // count: a filler-bearing window ("it flex layout") must never be
        // absorbed by a shorter alias ("flex layout"). Single-word keys are
        // exempt — glued or split speech ("charge bee" → ChargeBee) matches
        // by concatenation.
        if custom_word_key.word_count > 1 && custom_word_key.word_count != window_word_count {
            continue;
        }
        // Glue keys may span at most one extra spoken word unless the window
        // is a pure re-segmentation of the same letters ("mac book pro" ≡
        // "macbookpro"): "Charge B" → ChargeBee stays legal, while a leading
        // small word ("a type script" absorbed by the single token
        // "TypeScript") is rejected.
        if custom_word_key.word_count == 1 && window_word_count > 1 {
            let exact_resegmentation = candidate == custom_word_key.key;
            if window_word_count > custom_word_key.word_count + 1 && !exact_resegmentation {
                continue;
            }
        }
        // Skip if lengths are too different (optimization + prevents over-matching)
        // Use percentage-based check: max 25% length difference (prevents n-grams from
        // matching significantly shorter custom words, e.g., "openaigpt" vs "openai")
        let candidate_len = candidate.chars().count();
        let custom_word_len = custom_word_key.key.chars().count();
        let len_diff = candidate_len.abs_diff(custom_word_len) as f64;
        let max_len = candidate_len.max(custom_word_len) as f64;
        let max_allowed_diff = (max_len * 0.25).max(2.0); // At least 2 chars difference allowed
        if len_diff > max_allowed_diff {
            continue;
        }

        // Calculate Levenshtein distance (normalized by length)
        let levenshtein_dist = levenshtein(candidate, &custom_word_key.key);
        let levenshtein_score = if max_len > 0.0 {
            levenshtein_dist as f64 / max_len
        } else {
            1.0
        };

        // Phonetic fallback (built-in lexicon only): compare consonant
        // skeletons so unenumerated mishearings ("coober netees" →
        // Kubernetes) still match. Guards: substantial words only, similar
        // raw lengths (a stray small word glued into the n-gram must not
        // sneak past), matching first skeleton consonant (the anchor), and
        // a sane orthographic ceiling so wild guesses never fire.
        let score = if !custom_word_key.skeleton.is_empty() {
            let candidate_skeleton = consonant_skeleton(candidate);
            let raw_len_ok = {
                let diff = (candidate.chars().count() as i64
                    - custom_word_key.key.chars().count() as i64)
                    .abs();
                diff as f64 <= (max_len * 0.2).max(2.0)
            };
            let anchored = candidate_skeleton.starts_with(&custom_word_key.skeleton);
            if raw_len_ok
                && anchored
                && candidate_skeleton.chars().count() >= 3
                && candidate.len() >= SKELETON_MIN_CHARS
                && levenshtein_score <= 0.5
            {
                let skel_dist = levenshtein(&candidate_skeleton, &custom_word_key.skeleton);
                let skel_max = (candidate_skeleton.chars().count())
                    .max(custom_word_key.skeleton.chars().count())
                    as f64;
                let skeleton_score = skel_dist as f64 / skel_max.max(1.0);
                if skeleton_score < SKELETON_THRESHOLD && skeleton_score < levenshtein_score {
                    skeleton_score * 1.1
                } else {
                    levenshtein_score
                }
            } else {
                levenshtein_score
            }
        } else {
            levenshtein_score
        };

        // Soundex is an English/ASCII phonetic algorithm. Numeric terms can
        // still use edit distance, but must not receive a phonetic boost.
        // The boost is disabled for the built-in lexicon: on short common
        // words ("see", "app") it turns near-anything into a "match".
        let phonetic_match = allow_phonetic_boost
            && supports_soundex(candidate)
            && supports_soundex(&custom_word_key.key)
            && soundex(candidate, &custom_word_key.key);

        // Combine scores: favor phonetic matches, but also consider string similarity
        let combined_score = if phonetic_match {
            score * 0.3 // Give significant boost to phonetic matches
        } else {
            score
        };

        // Accept if the score is good enough (configurable threshold)
        if combined_score < threshold && combined_score < best_score {
            best_match = Some(&custom_words[custom_word_key.word_index]);
            best_score = combined_score;
        }
    }

    best_match.map(|m| (m, best_score))
}

/// Applies custom word corrections to transcribed text using fuzzy matching
///
/// This function corrects words in the input text by finding the best matches
/// from a list of custom words using a combination of:
/// - Levenshtein distance for string similarity
/// - Soundex phonetic matching for pronunciation similarity
/// - N-gram matching for multi-word speech artifacts (e.g., "Charge B" -> "ChargeBee")
///
/// # Arguments
/// * `text` - The input text to correct
/// * `custom_words` - List of custom words to match against
/// * `threshold` - Maximum similarity score to accept (0.0 = exact match, 1.0 = any match)
///
/// # Returns
/// The corrected text with custom words applied
pub fn apply_custom_words(text: &str, custom_words: &[String], threshold: f64) -> String {
    if custom_words.is_empty() {
        return text.to_string();
    }

    // Pre-compute normalized comparison keys to avoid repeated allocations.
    let custom_word_match_keys: Vec<CustomWordMatchKey> = custom_words
        .iter()
        .enumerate()
        .flat_map(|(index, word)| build_custom_word_match_keys(word, index))
        .collect();

    apply_match_entries(text, custom_words, &custom_word_match_keys, threshold, true)
}

/// Applies corrections driven by explicit (display form → spoken aliases) pairs.
///
/// Each entry's aliases are normalized with the same n-gram rules used for
/// transcript matching (punctuation stripped, lowercased, whitespace removed),
/// so a spoken alias like "next year" matches the two-word n-gram "next year"
/// in a transcript and replaces it with the display form ("Next.js"). This is
/// the engine behind the built-in technical lexicon; user custom words use the
/// fuzzy-only path in [`apply_custom_words`].
///
/// # Arguments
/// * `text` - The input text to correct
/// * `entries` - Tuples of (display form, spoken alias phrases)
/// * `threshold` - Maximum similarity score to accept per alias key
pub fn apply_alias_entries(
    text: &str,
    entries: &[(String, Vec<String>)],
    threshold: f64,
) -> String {
    if entries.is_empty() {
        return text.to_string();
    }

    let displays: Vec<String> = entries.iter().map(|(d, _)| d.clone()).collect();
    let mut seen: HashSet<(String, usize)> = HashSet::new();
    let mut match_keys: Vec<CustomWordMatchKey> = Vec::new();

    for (entry_index, (_, aliases)) in entries.iter().enumerate() {
        let mut push_key = |phrase: &str, phonetic: bool| {
            // Normalize exactly like transcript n-grams: clean each word, join.
            let key = phrase
                .split_whitespace()
                .map(build_match_key)
                .collect::<Vec<_>>()
                .concat();
            let word_count = phrase.split_whitespace().count();
            if !is_supported_fuzzy_key(&key) {
                return;
            }
            // Dedupe by (key, word count): every spoken segmentation of an
            // entry coexists ("api"@1 for written form, "a p i"@3 for
            // speech), while exact duplicate spellings collapse.
            if !seen.insert((key.clone(), word_count)) {
                return;
            }
            let skeleton = if phonetic && key.chars().count() >= SKELETON_MIN_CHARS {
                consonant_skeleton(&key)
            } else {
                String::new()
            };
            match_keys.push(CustomWordMatchKey {
                word_index: entry_index,
                key,
                skeleton,
                word_count,
            });
        };

        for alias in aliases {
            push_key(alias, true);
        }
        // The display form self-key enables casing normalization ("nextjs" →
        // Next.js). Symbol-bearing displays ("src/", "./") are excluded:
        // stripping their punctuation would mint phantom keys that rewrite
        // ordinary words ("src" → "src/"). Self-keys are orthographic-only —
        // no skeleton — so loose consonant matches can never borrow the
        // display's identity ("envvar" ≁ "never").
        let display = &displays[entry_index];
        if display
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '.' || c == '_')
            && display
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic())
        {
            push_key(display, false);
        }
    }

    apply_match_entries(text, &displays, &match_keys, threshold, false)
}

fn apply_match_entries(
    text: &str,
    displays: &[String],
    match_keys: &[CustomWordMatchKey],
    threshold: f64,
    allow_phonetic_boost: bool,
) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();

    // Bucket keys by word count and size the n-gram window from the longest
    // alias. A hardcoded cap of 3 made 4+-word spoken patterns ("background
    // stone six hundred") unmatchable, leaving trailing words dangling.
    // Single-word keys glue across windows ("Charge B" → ChargeBee), so the
    // legacy 3-word window stays available even when every alias is short.
    let longest_alias = match_keys.iter().map(|k| k.word_count).max().unwrap_or(1);
    let max_n = longest_alias.max(3).clamp(1, 6);
    let mut buckets: Vec<Vec<&CustomWordMatchKey>> = vec![Vec::new(); max_n + 1];
    for key in match_keys {
        if key.word_count == 1 {
            // Single-word keys may glue onto any window size ("Charge B" →
            // ChargeBee), so they participate in every bucket.
            for bucket in buckets.iter_mut().skip(1) {
                bucket.push(key);
            }
        } else if key.word_count <= max_n {
            buckets[key.word_count].push(key);
        }
    }

    let mut result = Vec::new();
    let mut i = 0;

    while i < words.len() {
        let mut best_match: Option<(usize, &String, f64)> = None;

        // Consider word-aligned n-grams, longest first, choosing the closest
        // match. An n-gram of n words may only match a key built from the
        // same number of words (see `word_count`).
        for n in (1..=max_n).rev() {
            if i + n > words.len() || buckets[n].is_empty() {
                continue;
            }

            let ngram_words = &words[i..i + n];
            // Do not consume across a punctuation boundary. In
            // "Charge B, che", the comma closes the candidate at "B,".
            if ngram_words[..n.saturating_sub(1)]
                .iter()
                .any(|word| !extract_punctuation(word).1.is_empty())
            {
                continue;
            }
            let ngram = build_ngram(ngram_words);

            if let Some((replacement, score)) = find_best_match(
                &ngram,
                displays,
                buckets[n].as_slice(),
                n,
                threshold,
                allow_phonetic_boost,
            ) {
                let is_better = best_match
                    .as_ref()
                    .is_none_or(|(_, _, best_score)| score < *best_score);
                if is_better {
                    best_match = Some((n, replacement, score));
                }
            }
        }

        if let Some((n, replacement, _)) = best_match {
            let ngram_words = &words[i..i + n];
            // Extract punctuation from first and last words of the n-gram.
            let (prefix, _) = extract_punctuation(ngram_words[0]);
            let (_, suffix) = extract_punctuation(ngram_words[n - 1]);

            // Preserve case from first word.
            let corrected = preserve_case_pattern(ngram_words[0], replacement);

            result.push(format!("{}{}{}", prefix, corrected, suffix));
            i += n;
        } else {
            result.push(words[i].to_string());
            i += 1;
        }
    }

    result.join(" ")
}

/// Preserves the case pattern of the original word when applying a replacement
fn preserve_case_pattern(original: &str, replacement: &str) -> String {
    if original.chars().all(|c| c.is_uppercase()) {
        replacement.to_uppercase()
    } else if original.chars().next().is_some_and(|c| c.is_uppercase()) {
        let mut chars: Vec<char> = replacement.chars().collect();
        if let Some(first_char) = chars.get_mut(0) {
            *first_char = first_char.to_uppercase().next().unwrap_or(*first_char);
        }
        chars.into_iter().collect()
    } else {
        replacement.to_string()
    }
}

/// Extracts punctuation prefix and suffix from a word
fn extract_punctuation(word: &str) -> (&str, &str) {
    // String slices use byte offsets. Derive both boundaries from char_indices
    // so multibyte punctuation such as `。` and `「」` can never be split.
    let prefix_end = word
        .char_indices()
        .find(|(_, c)| c.is_alphanumeric())
        .map(|(index, _)| index)
        .unwrap_or(word.len());
    let suffix_start = word
        .char_indices()
        .rev()
        .find(|(_, c)| c.is_alphanumeric())
        .map(|(index, c)| index + c.len_utf8())
        .unwrap_or(0);

    let prefix = if prefix_end > 0 {
        &word[..prefix_end]
    } else {
        ""
    };

    let suffix = if suffix_start < word.len() {
        &word[suffix_start..]
    } else {
        ""
    };

    (prefix, suffix)
}

/// Evidence for the language of the text being cleaned.
///
/// This intentionally describes the transcription output, not SuperFlow's UI
/// language. Unknown output languages fail closed: built-in filler removal is
/// skipped rather than applying a language profile speculatively.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OutputLanguageEvidence {
    UserSelected(String),
    ModelConstrained(String),
    /// The transcription model itself identified the language (audio-based
    /// LID, e.g. Whisper in auto mode).
    ModelDetected(String),
    /// Detected from the transcribed text with high confidence, constrained to
    /// the model's supported languages. Weakest accepted evidence.
    TextDetected(String),
    TranslatedToEnglish,
    Unknown,
}

impl OutputLanguageEvidence {
    fn language(&self) -> Option<&str> {
        match self {
            Self::UserSelected(language)
            | Self::ModelConstrained(language)
            | Self::ModelDetected(language)
            | Self::TextDetected(language) => Some(language),
            Self::TranslatedToEnglish => Some("en"),
            Self::Unknown => None,
        }
    }
}

/// Filler tokens that are not lexical words in any language SuperFlow's models can
/// output, so removing them cannot corrupt text regardless of the (possibly
/// unknown) output language. Kept deliberately conservative: anything that is a
/// real word somewhere ("um" pt/de, "ha" es, "ah"/"eh" interjections, "mm"
/// millimetres) belongs in the language-gated lists instead.
const UNIVERSAL_FILLER_WORDS: &[&str] = &[
    "uh", "uhm", "umm", "uhh", "uhhh", "ehh", "ehm", "ahm", "hmm", "hm", "mmm", "хм", "ммм",
];

/// Filler words that are only safe to remove with evidence for the output
/// language, because the same token is a real word elsewhere (e.g. Portuguese
/// "um" = "a/an", German "um" = "at/around", Spanish "ha" = "has").
fn gated_filler_words_for_language(lang: &str) -> &'static [&'static str] {
    let base_lang = lang.split(&['-', '_'][..]).next().unwrap_or(lang);

    match base_lang {
        "en" => &["um", "ah", "eh", "ha"],
        "de" => &["äh", "ähm"],
        "fr" => &["euh"],
        _ => &[],
    }
}

static MULTI_SPACE_PATTERN: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s{2,}").unwrap());

/// Collapses repeated words (3+ repetitions) to a single instance.
/// E.g., "wh wh wh wh" -> "wh", "I I I I" -> "I"
fn collapse_stutters(text: &str) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return text.to_string();
    }

    let mut result: Vec<&str> = Vec::new();
    let mut i = 0;

    while i < words.len() {
        let word = words[i];
        let word_lower = word.to_lowercase();

        if word_lower.chars().all(|c| c.is_alphabetic()) {
            // Count consecutive repetitions (case-insensitive)
            let mut count = 1;
            while i + count < words.len() && words[i + count].to_lowercase() == word_lower {
                count += 1;
            }

            // If 3+ repetitions, collapse to single instance
            if count >= 3 {
                result.push(word);
                i += count;
            } else {
                result.push(word);
                i += 1;
            }
        } else {
            result.push(word);
            i += 1;
        }
    }

    result.join(" ")
}

/// Removes filler words from transcription output when enabled.
///
/// Built-in removal is two-tiered: [`UNIVERSAL_FILLER_WORDS`] apply regardless
/// of language evidence, while [`gated_filler_words_for_language`] tokens are
/// only removed when the output language is known. A custom list is an
/// explicit user override and replaces both tiers without requiring language
/// evidence. `Some(empty vec)` disables removal, preserving the legacy
/// power-user setting. The master toggle takes precedence over both built-in
/// and custom lists.
///
/// # Arguments
/// * `text` - The raw transcription text to filter
/// * `language` - Evidence for the language of the transcription output
/// * `custom_filler_words` - Optional user-provided filler word list. `Some(vec)` overrides
///   language defaults; `Some(empty vec)` disables filtering; `None` uses language defaults.
/// * `enabled` - Whether filler-word removal is enabled
///
/// # Returns
/// The text with configured filler words removed
pub fn remove_filler_words(
    text: &str,
    language: &OutputLanguageEvidence,
    custom_filler_words: &Option<Vec<String>>,
    enabled: bool,
) -> String {
    if !enabled {
        return text.to_string();
    }

    // Build filler patterns from custom list or the built-in tiers
    let patterns: Vec<Regex> = match custom_filler_words {
        Some(words) => words
            .iter()
            .filter_map(|word| Regex::new(&format!(r"(?i)\b{}\b[,.]?", regex::escape(word))).ok())
            .collect(),
        None => UNIVERSAL_FILLER_WORDS
            .iter()
            .chain(
                language
                    .language()
                    .map(gated_filler_words_for_language)
                    .unwrap_or_default(),
            )
            .map(|word| Regex::new(&format!(r"(?i)\b{}\b[,.]?", regex::escape(word))).unwrap())
            .collect(),
    };

    // Remove filler words
    let mut filtered = text.to_string();
    for pattern in &patterns {
        filtered = pattern.replace_all(&filtered, "").to_string();
    }

    filtered
}

/// Applies non-filler transcription cleanup.
///
/// Kept separate from [`remove_filler_words`] so disabling filler deletion
/// does not also disable the existing repeated-word and whitespace cleanup.
pub fn normalize_transcription_output(text: &str) -> String {
    let mut normalized = collapse_stutters(text);

    // Clean up multiple spaces to single space
    normalized = MULTI_SPACE_PATTERN
        .replace_all(&normalized, " ")
        .to_string();

    // Trim leading/trailing whitespace
    normalized.trim().to_string()
}

/// File extensions that participate in spoken-path joining. Only tokens whose
/// extension is in this list are glued to the preceding word, so ordinary
/// prose around a stray period ("end . of sentence") is never mangled.
const PATH_EXTENSIONS: &[&str] = &[
    "ts", "tsx", "js", "jsx", "mjs", "cjs", "json", "rs", "py", "rb", "go", "java", "kt", "kts",
    "swift", "php", "cs", "cpp", "c", "h", "hpp", "md", "mdx", "txt", "css", "scss", "less",
    "html", "htm", "xml", "yml", "yaml", "toml", "sql", "sh", "bash", "zsh", "env", "lock", "svg",
    "csv",
];

fn ends_with_alphanumeric(token: &str) -> bool {
    token
        .chars()
        .next_back()
        .is_some_and(|c| c.is_alphanumeric())
}

/// Splits trailing punctuation off a whitespace token: `".tsx,"` →
/// `(".tsx", ",")`. The core keeps any leading punctuation.
fn split_trailing_punctuation(token: &str) -> (&str, &str) {
    let split = token
        .char_indices()
        .rev()
        .take_while(|(_, c)| !c.is_alphanumeric())
        .last()
        .map(|(i, _)| i)
        .unwrap_or(token.len());
    (&token[..split], &token[split..])
}

/// Glues path fragments produced by the lexicon back into single file-path
/// tokens: `"hero .tsx"` → `"hero.tsx"`, `"src / components"` →
/// `"src/components"`, `"components /auth"` → `"components/auth"`.
///
/// Runs after alias application; purely local, no fuzzy matching, and every
/// rule requires an alphanumeric boundary on at least one side so ordinary
/// sentences pass through unchanged.
pub fn join_path_tokens(text: &str) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.len() < 2 {
        return text.to_string();
    }

    let mut out: Vec<String> = Vec::with_capacity(words.len());
    for word in words {
        let mut merged = false;
        if let Some(last) = out.last_mut() {
            // "src/ components" or "src / components": a pending trailing
            // slash meets an alphanumeric continuation.
            if last.ends_with('/') && word.chars().next().is_some_and(char::is_alphanumeric) {
                last.push_str(word);
                merged = true;
            } else if ends_with_alphanumeric(last) {
                if word == "/" {
                    // Pending separator; the next word closes the join.
                    last.push('/');
                    merged = true;
                } else {
                    let (core, _punctuation) = split_trailing_punctuation(word);
                    if core.len() > 1 && core.starts_with('/') {
                        // "components /auth" — slash fused to the next fragment.
                        last.push_str(word);
                        merged = true;
                    } else if let Some(extension) = core.strip_prefix('.') {
                        // "hero .tsx," — lexicon-produced extension token.
                        if PATH_EXTENSIONS.contains(&extension) {
                            last.push_str(word);
                            merged = true;
                        }
                    }
                }
            }
        }
        if !merged {
            out.push(word.to_string());
        }
    }

    out.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exercise the complete cleanup sequence with an explicitly selected
    /// language. Individual tests below predate the split between filler
    /// removal and non-filler normalization.
    fn filter_transcription_output(
        text: &str,
        language: &str,
        custom_filler_words: &Option<Vec<String>>,
    ) -> String {
        let language = OutputLanguageEvidence::UserSelected(language.to_string());
        let filtered = remove_filler_words(text, &language, custom_filler_words, true);
        normalize_transcription_output(&filtered)
    }

    #[test]
    fn test_apply_custom_words_exact_match() {
        let text = "hello world";
        let custom_words = vec!["Hello".to_string(), "World".to_string()];
        let result = apply_custom_words(text, &custom_words, 0.5);
        assert_eq!(result, "Hello World");
    }

    #[test]
    fn test_apply_custom_words_fuzzy_match() {
        let text = "helo wrold";
        let custom_words = vec!["hello".to_string(), "world".to_string()];
        let result = apply_custom_words(text, &custom_words, 0.5);
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_preserve_case_pattern() {
        assert_eq!(preserve_case_pattern("HELLO", "world"), "WORLD");
        assert_eq!(preserve_case_pattern("Hello", "world"), "World");
        assert_eq!(preserve_case_pattern("hello", "WORLD"), "WORLD");
    }

    #[test]
    fn test_extract_punctuation() {
        assert_eq!(extract_punctuation("hello"), ("", ""));
        assert_eq!(extract_punctuation("!hello?"), ("!", "?"));
        assert_eq!(extract_punctuation("...hello..."), ("...", "..."));
    }

    #[test]
    fn test_extract_punctuation_uses_unicode_boundaries() {
        assert_eq!(extract_punctuation("你好。"), ("", "。"));
        assert_eq!(extract_punctuation("「你好」"), ("「", "」"));
        assert_eq!(extract_punctuation("你好！"), ("", "！"));
    }

    #[test]
    fn test_empty_custom_words() {
        let text = "hello world";
        let custom_words = vec![];
        let result = apply_custom_words(text, &custom_words, 0.5);
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_filter_filler_words() {
        let text = "So uhm I was thinking uh about this";
        let result = filter_transcription_output(text, "en", &None);
        assert_eq!(result, "So I was thinking about this");
    }

    #[test]
    fn test_filter_filler_words_case_insensitive() {
        let text = "UHM this is UH a test";
        let result = filter_transcription_output(text, "en", &None);
        assert_eq!(result, "this is a test");
    }

    #[test]
    fn test_filter_filler_words_with_punctuation() {
        let text = "Well, uhm, I think, uh. that's right";
        let result = filter_transcription_output(text, "en", &None);
        assert_eq!(result, "Well, I think, that's right");
    }

    #[test]
    fn test_filter_cleans_whitespace() {
        let text = "Hello    world   test";
        let result = filter_transcription_output(text, "en", &None);
        assert_eq!(result, "Hello world test");
    }

    #[test]
    fn test_filter_trims() {
        let text = "  Hello world  ";
        let result = filter_transcription_output(text, "en", &None);
        assert_eq!(result, "Hello world");
    }

    #[test]
    fn test_filter_combined() {
        let text = "  Uhm, so I was, uh, thinking about this  ";
        let result = filter_transcription_output(text, "en", &None);
        assert_eq!(result, "so I was, thinking about this");
    }

    #[test]
    fn test_filter_preserves_valid_text() {
        let text = "This is a completely normal sentence.";
        let result = filter_transcription_output(text, "en", &None);
        assert_eq!(result, "This is a completely normal sentence.");
    }

    #[test]
    fn test_filter_stutter_collapse() {
        let text = "w wh wh wh wh wh wh wh wh wh why";
        let result = filter_transcription_output(text, "en", &None);
        assert_eq!(result, "w wh why");
    }

    #[test]
    fn test_filter_stutter_short_words() {
        let text = "I I I I think so so so so";
        let result = filter_transcription_output(text, "en", &None);
        assert_eq!(result, "I think so");
    }

    #[test]
    fn test_filter_stutter_longer_words() {
        let text = "Check data doc doc doc doc documentation.";
        let result = filter_transcription_output(text, "en", &None);
        assert_eq!(result, "Check data doc documentation.");
    }

    #[test]
    fn test_filter_stutter_mixed_case() {
        let text = "No NO no NO no";
        let result = filter_transcription_output(text, "en", &None);
        assert_eq!(result, "No");
    }

    #[test]
    fn test_filter_stutter_preserves_two_repetitions() {
        let text = "no no is fine";
        let result = filter_transcription_output(text, "en", &None);
        assert_eq!(result, "no no is fine");
    }

    #[test]
    fn test_filter_english_removes_um() {
        let text = "um I think um this is good";
        let result = filter_transcription_output(text, "en", &None);
        assert_eq!(result, "I think this is good");
    }

    #[test]
    fn test_filter_portuguese_preserves_um() {
        // "um" means "a/an" in Portuguese
        let text = "um gato bonito";
        let result = filter_transcription_output(text, "pt", &None);
        assert_eq!(result, "um gato bonito");
    }

    #[test]
    fn test_filter_spanish_preserves_ha() {
        // "ha" means "has" in Spanish
        let text = "ha sido un buen día";
        let result = filter_transcription_output(text, "es", &None);
        assert_eq!(result, "ha sido un buen día");
    }

    #[test]
    fn test_filter_language_code_with_region() {
        // "pt-BR" should normalize to "pt"
        let text = "um gato bonito";
        let result = filter_transcription_output(text, "pt-BR", &None);
        assert_eq!(result, "um gato bonito");
    }

    #[test]
    fn test_filter_custom_filler_words_override() {
        let custom = Some(vec!["okay".to_string(), "right".to_string()]);
        let text = "okay so I think right this works";
        let result = filter_transcription_output(text, "en", &custom);
        assert_eq!(result, "so I think this works");
    }

    #[test]
    fn test_filter_custom_filler_words_empty_disables() {
        let custom = Some(vec![]);
        let text = "So uhm I was thinking uh about this";
        let result = filter_transcription_output(text, "en", &custom);
        // No filler words removed since custom list is empty
        assert_eq!(result, "So uhm I was thinking uh about this");
    }

    #[test]
    fn test_filter_unknown_language_still_removes_universal_fillers() {
        let text = "uh I think uhm this works";
        let result = filter_transcription_output(text, "xx", &None);
        assert_eq!(result, "I think this works");
    }

    #[test]
    fn test_filter_unknown_language_does_not_remove_um() {
        let text = "um I think this works";
        let result = filter_transcription_output(text, "xx", &None);
        assert_eq!(result, "um I think this works");
    }

    #[test]
    fn test_filter_unknown_evidence_removes_universal_keeps_gated() {
        let filtered = remove_filler_words(
            "uhh bueno hmm creo que um ha llegado",
            &OutputLanguageEvidence::Unknown,
            &None,
            true,
        );
        assert_eq!(
            normalize_transcription_output(&filtered),
            "bueno creo que um ha llegado"
        );

        let cyrillic = remove_filler_words(
            "хм я думаю ммм это работает",
            &OutputLanguageEvidence::Unknown,
            &None,
            true,
        );
        assert_eq!(
            normalize_transcription_output(&cyrillic),
            "я думаю это работает"
        );
    }

    #[test]
    fn test_filter_german_gated_fillers_require_evidence() {
        let text = "äh ich glaube ähm das passt";

        let unknown = remove_filler_words(text, &OutputLanguageEvidence::Unknown, &None, true);
        assert_eq!(normalize_transcription_output(&unknown), text);

        let result = filter_transcription_output(text, "de", &None);
        assert_eq!(result, "ich glaube das passt");
    }

    #[test]
    fn test_filter_preserves_millimetre_unit() {
        // "mm" was removed from the filler lists because it eats units.
        let text = "the screw is 5 mm long";
        let result = filter_transcription_output(text, "en", &None);
        assert_eq!(result, "the screw is 5 mm long");
    }

    #[test]
    fn test_filter_detected_evidence_unlocks_gated_fillers() {
        let model = remove_filler_words(
            "um I think this works",
            &OutputLanguageEvidence::ModelDetected("en".to_string()),
            &None,
            true,
        );
        assert_eq!(normalize_transcription_output(&model), "I think this works");

        let text = remove_filler_words(
            "euh je pense que ça marche",
            &OutputLanguageEvidence::TextDetected("fr".to_string()),
            &None,
            true,
        );
        assert_eq!(
            normalize_transcription_output(&text),
            "je pense que ça marche"
        );
    }

    #[test]
    fn test_filter_master_toggle_disables_custom_and_builtin_removal() {
        let text = "um customword I think";
        let language = OutputLanguageEvidence::UserSelected("en".to_string());
        let custom = Some(vec!["customword".to_string()]);

        let result = remove_filler_words(text, &language, &custom, false);

        assert_eq!(result, text);
    }

    #[test]
    fn test_filter_custom_words_apply_without_language_evidence() {
        let custom = Some(vec!["customword".to_string()]);
        let text = "customword should be removed but um should remain";

        let filtered = remove_filler_words(text, &OutputLanguageEvidence::Unknown, &custom, true);
        let result = normalize_transcription_output(&filtered);

        assert_eq!(result, "should be removed but um should remain");
    }

    #[test]
    fn test_apply_custom_words_ngram_two_words() {
        let text = "il cui nome è Charge B, che permette";
        let custom_words = vec!["ChargeBee".to_string()];
        let result = apply_custom_words(text, &custom_words, 0.5);
        assert!(result.contains("ChargeBee,"), "unexpected result: {result}");
        assert!(!result.contains("Charge B"));
    }

    #[test]
    fn test_apply_custom_words_ngram_three_words() {
        let text = "use Chat G P T for this";
        let custom_words = vec!["ChatGPT".to_string()];
        let result = apply_custom_words(text, &custom_words, 0.5);
        assert!(result.contains("ChatGPT"));
    }

    #[test]
    fn test_apply_custom_words_prefers_longer_ngram() {
        let text = "Open AI GPT model";
        let custom_words = vec!["OpenAI".to_string(), "GPT".to_string()];
        let result = apply_custom_words(text, &custom_words, 0.5);
        assert_eq!(result, "OpenAI GPT model");
    }

    #[test]
    fn test_apply_custom_words_ngram_preserves_case() {
        let text = "CHARGE B is great";
        let custom_words = vec!["ChargeBee".to_string()];
        let result = apply_custom_words(text, &custom_words, 0.5);
        assert!(result.contains("CHARGEBEE"));
    }

    #[test]
    fn test_apply_custom_words_ngram_with_spaces_in_custom() {
        // Custom word with space should also match against split words
        let text = "using Mac Book Pro";
        let custom_words = vec!["MacBook Pro".to_string()];
        let result = apply_custom_words(text, &custom_words, 0.5);
        assert_eq!(result, "using MacBook Pro");
    }

    #[test]
    fn test_apply_custom_words_trailing_number_not_doubled() {
        // Verify that trailing non-alpha chars (like numbers) aren't double-counted
        // between build_ngram stripping them and extract_punctuation capturing them
        let text = "use GPT4 for this";
        let custom_words = vec!["GPT-4".to_string()];
        let result = apply_custom_words(text, &custom_words, 0.5);
        // Should NOT produce "GPT-44" (double-counting the trailing 4)
        assert!(
            !result.contains("GPT-44"),
            "got double-counted result: {}",
            result
        );
    }

    #[test]
    fn test_apply_custom_words_matches_ampersand_word() {
        let text = "send it to RD for review";
        let custom_words = vec!["R&D".to_string()];
        let result = apply_custom_words(text, &custom_words, 0.18);
        assert_eq!(result, "send it to R&D for review");
    }

    #[test]
    fn test_apply_custom_words_matches_spoken_ampersand_word() {
        let text = "send it to R and D for review";
        let custom_words = vec!["R&D".to_string()];
        let result = apply_custom_words(text, &custom_words, 0.18);
        assert_eq!(result, "send it to R&D for review");
    }

    #[test]
    fn test_apply_custom_words_preserves_ampersand_word() {
        let text = "send it to R&D for review";
        let custom_words = vec!["R&D".to_string()];
        let result = apply_custom_words(text, &custom_words, 0.18);
        assert_eq!(result, "send it to R&D for review");
    }

    #[test]
    fn test_apply_custom_words_handles_unicode_punctuation() {
        // The candidate must be a legitimate fuzzy/exact match for the custom
        // word; CJK brackets must survive the replacement untouched.
        let text = "「superflow。」";
        let custom_words = vec!["SuperFlow".to_string()];
        let result = apply_custom_words(text, &custom_words, 0.5);
        assert_eq!(result, "「SuperFlow。」");
    }

    #[test]
    fn test_apply_custom_words_skips_cjk_fuzzy_matching() {
        let text = "你好。";
        let custom_words = vec!["你号".to_string()];
        let result = apply_custom_words(text, &custom_words, 1.0);
        assert_eq!(result, text);
    }

    #[test]
    fn join_merges_extension_tokens() {
        assert_eq!(join_path_tokens("edit hero .tsx"), "edit hero.tsx");
        assert_eq!(join_path_tokens("edit hero .tsx,"), "edit hero.tsx,");
        assert_eq!(
            join_path_tokens("open app .json file"),
            "open app.json file"
        );
    }

    #[test]
    fn join_merges_slash_separators() {
        assert_eq!(
            join_path_tokens("src / components / hero .tsx"),
            "src/components/hero.tsx"
        );
        assert_eq!(
            join_path_tokens("components /auth guard"),
            "components/auth guard"
        );
        assert_eq!(join_path_tokens("and / or"), "and/or");
    }

    #[test]
    fn join_leaves_prose_and_unknown_extensions_alone() {
        assert_eq!(join_path_tokens("end . of sentence"), "end . of sentence");
        assert_eq!(join_path_tokens("version .5 release"), "version .5 release");
        assert_eq!(join_path_tokens("a .unknown b"), "a .unknown b");
        assert_eq!(join_path_tokens("one two"), "one two");
    }

    #[test]
    fn join_handles_trailing_punctuated_slash() {
        assert_eq!(join_path_tokens("pick a /, any"), "pick a /, any");
    }
}
