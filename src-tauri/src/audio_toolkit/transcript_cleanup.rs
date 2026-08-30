const FILLERS: &[&str] = &["um", "uh", "erm", "hmm", "hm", "mmm", "mm"];

const PROTECTED_WORDS: &[&str] = &[
    "not", "never", "no", "n't", "don't", "dont", "can't", "cant", "cannot", "shouldn't",
    "shouldnt", "won't", "wont", "wouldn't", "wouldnt", "couldn't", "couldnt", "isn't",
    "isnt", "aren't", "arent", "wasn't", "wasnt", "weren't", "werent", "haven't",
    "havent", "hasn't", "hasnt", "hadn't", "hadnt", "didn't", "didnt", "doesn't",
    "doesnt",
];

const RESTART_BRIDGES: &[&str] = &[
    "okay", "ok", "right", "sorry", "actually", "well", "so", "like", "that", "basically",
    "anyway", "anyways", "mean", "means", "no", "i",
];

const MAX_REPEAT_NGRAM: usize = 16;
const STREAMING_MAX_REPEAT_NGRAM: usize = 4;
const MAX_RESTART_PREFIX: usize = 8;
const MAX_RESTART_GAP: usize = 3;
const MAX_RESTART_LOOKAHEAD: usize = 14;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Token {
    raw: String,
    normalized: String,
    filler: bool,
    protected: bool,
    comma_boundary: bool,
    clause_boundary: bool,
    sentence_boundary: bool,
}

impl Token {
    fn new(raw: String) -> Self {
        let normalized = normalize_lexeme(&raw);
        let filler = is_filler_normalized(&normalized);
        let protected = is_protected_word_normalized(&normalized)
            || contains_digit(&raw)
            || looks_like_url(&raw)
            || looks_like_email(&raw)
            || looks_like_path(&raw)
            || looks_like_code_token(&raw);
        let comma_boundary = has_comma_boundary_raw(&raw);
        let clause_boundary = has_clause_boundary_raw(&raw);
        let sentence_boundary = has_sentence_boundary_raw(&raw);

        Self {
            raw,
            normalized,
            filler,
            protected,
            comma_boundary,
            clause_boundary,
            sentence_boundary,
        }
    }

    fn with_punctuation_from(&self, source: &Self) -> Self {
        Self::new(replace_punctuation_suffix(&self.raw, &source.raw))
    }
}

fn normalize_lexeme(token: &str) -> String {
    token
        .trim_matches(|c: char| !c.is_alphanumeric() && c != '\'' && c != '-')
        .to_lowercase()
}

fn is_protected_word_normalized(normalized: &str) -> bool {
    PROTECTED_WORDS.contains(&normalized)
}

fn is_filler_normalized(normalized: &str) -> bool {
    if FILLERS.contains(&normalized) {
        return true;
    }

    let bytes = normalized.as_bytes();
    if bytes.len() < 2 || bytes.len() > 12 || bytes[0] != b'u' {
        return false;
    }

    let rest = &bytes[1..];

    if rest.iter().all(|b| *b == b'm') || rest.iter().all(|b| *b == b'h') {
        return true;
    }

    let mut seen_m = false;
    for byte in rest {
        match *byte {
            b'h' if !seen_m => {}
            b'm' => seen_m = true,
            _ => return false,
        }
    }

    seen_m
}

fn looks_like_url(token: &str) -> bool {
    let lower = token.to_ascii_lowercase();
    lower.contains("://") || lower.starts_with("www.")
}

fn looks_like_email(token: &str) -> bool {
    let trimmed = token.trim_matches(|c: char| ",.;:!?()[]{}<>\"'".contains(c));
    let Some((local, domain)) = trimmed.split_once('@') else {
        return false;
    };

    !local.is_empty() && domain.contains('.') && !domain.ends_with('.')
}

fn looks_like_path(token: &str) -> bool {
    token.contains('/') || token.contains('\\')
}

fn looks_like_code_token(token: &str) -> bool {
    let trimmed = token.trim_matches(|c: char| ",;:!?()[]{}<>\"'".contains(c));

    trimmed.contains("::")
        || trimmed.contains("->")
        || trimmed.contains("=>")
        || trimmed.contains('_')
        || trimmed.starts_with('@')
        || trimmed.starts_with('#')
        || trimmed.starts_with('`')
        || trimmed.ends_with('`')
        || (trimmed.contains('.') && !trimmed.ends_with('.'))
}

fn contains_digit(token: &str) -> bool {
    token.chars().any(|c| c.is_ascii_digit())
}

fn trim_closing_wrappers(token: &str) -> &str {
    token.trim_end_matches(|c: char| matches!(c, '\'' | '"' | ')' | ']' | '}'))
}

fn has_comma_boundary_raw(token: &str) -> bool {
    trim_closing_wrappers(token).ends_with(',')
}

fn has_clause_boundary_raw(token: &str) -> bool {
    let trimmed = trim_closing_wrappers(token);
    trimmed.ends_with(',') || trimmed.ends_with(';') || trimmed.ends_with(':')
}

fn has_sentence_boundary_raw(token: &str) -> bool {
    let trimmed = trim_closing_wrappers(token);
    trimmed.ends_with('.') || trimmed.ends_with('!') || trimmed.ends_with('?')
}

fn punctuation_suffix(token: &str) -> &str {
    let mut start = token.len();

    for (index, ch) in token.char_indices().rev() {
        if ch.is_alphanumeric()
            || ch == '\''
            || ch == '-'
            || ch == '_'
            || ch == '/'
            || ch == '\\'
            || ch == '@'
        {
            break;
        }
        start = index;
    }

    &token[start..]
}

fn replace_punctuation_suffix(base: &str, source: &str) -> String {
    let source_suffix = punctuation_suffix(source);
    let base_suffix = punctuation_suffix(base);
    if base_suffix == source_suffix {
        return base.to_string();
    }

    let cutoff = base.len().saturating_sub(base_suffix.len());
    let mut output = String::with_capacity(cutoff + source_suffix.len());
    output.push_str(&base[..cutoff]);
    output.push_str(source_suffix);
    output
}

fn tokenize(input: &str) -> Vec<Token> {
    input
        .split_whitespace()
        .filter(|token| !token.is_empty())
        .map(|token| Token::new(token.to_owned()))
        .collect()
}

fn remove_fillers(tokens: Vec<Token>) -> Vec<Token> {
    if tokens.is_empty() {
        return tokens;
    }

    let mut result: Vec<Token> = Vec::with_capacity(tokens.len());

    for token in tokens {
        if !token.filler {
            result.push(token);
            continue;
        }

        if token.sentence_boundary {
            if let Some(previous) = result.last_mut() {
                if !previous.sentence_boundary {
                    *previous = previous.with_punctuation_from(&token);
                }
            }
        }
    }

    result
}

fn can_collapse_duplicate_run(tokens: &[Token], start: usize, end: usize) -> bool {
    if end <= start + 1 {
        return false;
    }

    let protected_run = tokens[start..end].iter().any(|token| token.protected);

    // A single repeated negation word ("don't", "not", "no") is preserved when it
    // is a short 2x emphasis ("don't don't", "no no"). But a 3x+ run of a
    // protected word is an unambiguous stutter, not meaningful negation, so it
    // may collapse. This is what fixes "I don't, don't, don't, don't, don't".
    if protected_run && (end - start) < 3 {
        return false;
    }

    // Never collapse across a real sentence boundary (. ! ?) — that would merge
    // distinct utterances.
    if tokens[start..end - 1]
        .iter()
        .any(|token| token.sentence_boundary)
    {
        return false;
    }

    let has_comma = tokens[start..end - 1]
        .iter()
        .any(|token| token.comma_boundary);

    // Adjacent duplicates (no comma) always collapse. Duplicates separated only
    // by commas collapse too, but only when the run is clearly a stutter (>=4):
    // this removes accidental speech stutter while preserving intentional 2-3x
    // emphasis the spec requires ("very, very", "no, no", "go, go, go").
    if has_comma && (end - start) < 4 {
        return false;
    }

    true
}

fn collapse_duplicate_tokens(tokens: Vec<Token>) -> Vec<Token> {
    if tokens.len() < 2 {
        return tokens;
    }

    let mut result = Vec::with_capacity(tokens.len());
    let mut index = 0;

    while index < tokens.len() {
        let normalized = &tokens[index].normalized;
        if normalized.is_empty() {
            result.push(tokens[index].clone());
            index += 1;
            continue;
        }

        let mut end = index + 1;
        while end < tokens.len() && tokens[end].normalized == *normalized {
            end += 1;
        }

        if can_collapse_duplicate_run(&tokens, index, end) {
            result.push(tokens[index].with_punctuation_from(&tokens[end - 1]));
        } else {
            result.extend(tokens[index..end].iter().cloned());
        }

        index = end;
    }

    result
}

fn normalized_windows_equal(tokens: &[Token], left: usize, right: usize, length: usize) -> bool {
    if left + length > tokens.len() || right + length > tokens.len() {
        return false;
    }

    for offset in 0..length {
        let a = &tokens[left + offset].normalized;
        let b = &tokens[right + offset].normalized;
        if a.is_empty() || a != b {
            return false;
        }
    }

    true
}

fn phrase_contains_protected(tokens: &[Token]) -> bool {
    tokens.iter().any(|token| token.protected)
}

fn can_collapse_phrase(tokens: &[Token], start: usize, length: usize) -> bool {
    if phrase_contains_protected(&tokens[start..start + length]) {
        return false;
    }

    let first_end = &tokens[start + length - 1];
    !first_end.clause_boundary && !first_end.sentence_boundary
}

fn repeated_copy_count(tokens: &[Token], start: usize, length: usize) -> usize {
    let mut copies = 1;

    while start + length * (copies + 1) <= tokens.len()
        && normalized_windows_equal(tokens, start, start + length * copies, length)
    {
        copies += 1;
    }

    copies
}

fn collapse_repeated_phrases_with_limit(tokens: Vec<Token>, max_ngram: usize) -> Vec<Token> {
    if tokens.len() < 4 {
        return tokens;
    }

    let mut current = tokens;
    let upper = max_ngram.min(MAX_REPEAT_NGRAM);

    for length in (2..=upper).rev() {
        if current.len() < length * 2 {
            continue;
        }

        let mut next = Vec::with_capacity(current.len());
        let mut index = 0;

        while index < current.len() {
            let enough_tokens = index + length * 2 <= current.len();
            let repeated = enough_tokens
                && can_collapse_phrase(&current, index, length)
                && normalized_windows_equal(&current, index, index + length, length);

            if !repeated {
                next.push(current[index].clone());
                index += 1;
                continue;
            }

            let copies = repeated_copy_count(&current, index, length);
            let mut kept = current[index..index + length].to_vec();
            let source_last = &current[index + length * copies - 1];
            let kept_last = kept.len() - 1;
            kept[kept_last] = kept[kept_last].with_punctuation_from(source_last);
            next.extend(kept);
            index += length * copies;
        }

        current = next;
    }

    current
}

fn common_prefix_len(tokens: &[Token], left: usize, right: usize, max_len: usize) -> usize {
    let limit = max_len
        .min(tokens.len().saturating_sub(left))
        .min(tokens.len().saturating_sub(right));
    let mut matched = 0;

    while matched < limit {
        let a = &tokens[left + matched].normalized;
        let b = &tokens[right + matched].normalized;
        if a.is_empty() || a != b {
            break;
        }
        matched += 1;
    }

    matched
}

fn restart_gap_is_safe(gap: &[Token]) -> bool {
    !gap.is_empty()
        && gap.len() <= MAX_RESTART_GAP
        && gap
            .iter()
            .all(|token| RESTART_BRIDGES.contains(&token.normalized.as_str()))
}

fn restart_candidate_is_safe(
    tokens: &[Token],
    start: usize,
    restart: usize,
    prefix_len: usize,
) -> bool {
    if prefix_len < 2 || restart <= start + prefix_len {
        return false;
    }

    if phrase_contains_protected(&tokens[start..start + prefix_len]) {
        return false;
    }

    let gap = &tokens[start + prefix_len..restart];
    restart_gap_is_safe(gap)
}

fn remove_restarts(tokens: Vec<Token>) -> Vec<Token> {
    if tokens.len() < 5 {
        return tokens;
    }

    let mut result = Vec::with_capacity(tokens.len());
    let mut index = 0;
    // True when we dropped a prefix that began the sentence AND the original
    // first token was capitalized (a real sentence start), so the new first
    // token must be capitalized too (e.g. "Third thing is that third thing…"
    // → "Third thing…"). A lowercase start ("when we like when we…") stays
    // lowercase, since the correction is not a sentence start.
    let mut dropped_sentence_start = false;
    let original_start_capitalized = tokens
        .first()
        .and_then(|t| t.raw.chars().next())
        .map(|c| c.is_alphabetic() && c.is_uppercase())
        .unwrap_or(false);

    while index < tokens.len() {
        let search_end = (index + MAX_RESTART_LOOKAHEAD).min(tokens.len());
        let mut best_restart: Option<(usize, usize)> = None;

        for restart in index + 3..search_end {
            let prefix_len = common_prefix_len(&tokens, index, restart, MAX_RESTART_PREFIX);
            if !restart_candidate_is_safe(&tokens, index, restart, prefix_len) {
                continue;
            }

            match best_restart {
                Some((_, best_prefix_len)) if best_prefix_len >= prefix_len => {}
                _ => best_restart = Some((restart, prefix_len)),
            }
        }

        if let Some((restart, _)) = best_restart {
            if index == 0 && original_start_capitalized {
                dropped_sentence_start = true;
            }
            index = restart;
            continue;
        }

        result.push(tokens[index].clone());
        index += 1;
    }

    if dropped_sentence_start {
        if let Some(first) = result.first_mut() {
            let c: Vec<char> = first.raw.chars().collect();
            if let Some(ch) = c.first() {
                if ch.is_alphabetic() && !ch.is_uppercase() {
                    first.raw = c
                        .iter()
                        .enumerate()
                        .map(|(i, x)| {
                            if i == 0 {
                                x.to_uppercase().next().unwrap()
                            } else {
                                *x
                            }
                        })
                        .collect::<String>();
                }
            }
        }
    }

    result
}

fn is_closing_punctuation_token(token: &str) -> bool {
    !token.is_empty()
        && token
            .chars()
            .all(|ch| matches!(ch, ',' | '.' | ';' | ':' | '!' | '?' | '%' | ')' | ']' | '}'))
}

fn is_opening_punctuation_token(token: &str) -> bool {
    !token.is_empty() && token.chars().all(|ch| matches!(ch, '(' | '[' | '{'))
}

fn starts_with_digit(token: &str) -> bool {
    token.chars().next().is_some_and(|ch| ch.is_ascii_digit())
}

fn ends_with_digit(token: &str) -> bool {
    token.chars().next_back().is_some_and(|ch| ch.is_ascii_digit())
}

fn normalize_spacing_and_punctuation(tokens: Vec<Token>) -> String {
    if tokens.is_empty() {
        return String::new();
    }

    let mut output = String::new();
    let mut suppress_space_before_next = false;

    for index in 0..tokens.len() {
        let raw = &tokens[index].raw;
        if raw.is_empty() {
            continue;
        }

        if is_closing_punctuation_token(raw) {
            while output.ends_with(' ') {
                output.pop();
            }
            output.push_str(raw);

            let joins_numeric_literal = matches!(raw.as_str(), ":" | ".")
                && index > 0
                && index + 1 < tokens.len()
                && ends_with_digit(&tokens[index - 1].raw)
                && starts_with_digit(&tokens[index + 1].raw);

            suppress_space_before_next = joins_numeric_literal;
            continue;
        }

        if is_opening_punctuation_token(raw) {
            if !output.is_empty() && !output.ends_with(' ') {
                output.push(' ');
            }
            output.push_str(raw);
            suppress_space_before_next = true;
            continue;
        }

        if !output.is_empty() && !output.ends_with(' ') && !suppress_space_before_next {
            output.push(' ');
        }

        output.push_str(raw);
        suppress_space_before_next = false;
    }

    output.trim().to_string()
}

fn cleanup_final_tokens(tokens: Vec<Token>) -> Vec<Token> {
    let tokens = remove_fillers(tokens);
    let tokens = collapse_duplicate_tokens(tokens);
    let tokens = collapse_repeated_phrases_with_limit(tokens, MAX_REPEAT_NGRAM);
    let tokens = remove_restarts(tokens);
    let tokens = collapse_self_correction_bridges(tokens);
    let tokens = collapse_no_corrections(tokens);
    let tokens = collapse_duplicate_tokens(tokens);
    let tokens = collapse_repeated_phrases_with_limit(tokens, MAX_REPEAT_NGRAM);
    collapse_modifier_stacking(tokens)
}

/// Self-correction restarts gated by an explicit discourse bridge
/// ("I mean", "actually", "wait", "rather", "instead", "sorry") where the
/// abandoned clause and the correction share the SAME subject pronoun but not
/// an exact token prefix (e.g. "I can [I mean] I can't" → "I can't").
///
/// CERTAIN when the abandoned fragment is short (<= `MAX_SELF_CORRECT_FRAG`)
/// and contains no protected token. Longer/ambiguous fragments are preserved
/// (AMBIGUOUS policy: prefer imperfect text over changed meaning).
fn collapse_self_correction_bridges(tokens: Vec<Token>) -> Vec<Token> {
    if tokens.len() < 5 {
        return tokens;
    }

    const BRIDGES: &[&str] = &[
        "mean", "meant", "actually", "wait", "rather", "instead", "sorry", "basically",
    ];
    const SUBJECTS: &[&str] = &["i", "we", "you", "he", "she", "they", "it"];
    const MAX_SELF_CORRECT_FRAG: usize = 5;

    let mut out: Vec<Token> = Vec::with_capacity(tokens.len());
    let mut i = 0;

    while i < tokens.len() {
        let is_bridge = BRIDGES.contains(&tokens[i].normalized.as_str());
        if is_bridge && i >= 1 {
            // Correction subject: first subject pronoun after the bridge.
            let mut q = None;
            for j in i + 1..tokens.len().min(i + 1 + MAX_RESTART_LOOKAHEAD) {
                if SUBJECTS.contains(&tokens[j].normalized.as_str()) && !tokens[j].protected {
                    q = Some(j);
                    break;
                }
            }
            if let Some(q) = q {
                // Abandoned subject: farthest-back subject pronoun before the bridge
                // that matches the correction subject (smallest index wins).
                let mut p = None;
                for j in 0..i {
                    if SUBJECTS.contains(&tokens[j].normalized.as_str())
                        && !tokens[j].protected
                        && tokens[j].normalized == tokens[q].normalized
                    {
                        p = Some(j);
                        break;
                    }
                }
                if let Some(p) = p {
                    let frag_len = i - p;
                    let safe = frag_len >= 1
                        && frag_len <= MAX_SELF_CORRECT_FRAG
                        && !tokens[p..i].iter().any(|t| t.protected);
                    if safe {
                        // Already emitted the abandoned fragment [p..i); drop it,
                        // then skip the bridge (current token) and resume at the correction.
                        for _ in 0..frag_len {
                            out.pop();
                        }
                        i = q;
                        continue;
                    }
                }
            }
        }
        out.push(tokens[i].clone());
        i += 1;
    }

    out
}

/// Self-correction of a mutually-exclusive choice spoken as `X no Y`
/// (e.g. "ship Tuesday no Wednesday" → "ship Wednesday").
///
/// CERTAIN only when both neighbours are members of a closed set of
/// alternatives (days / months) and differ — real corrections always pick a
/// distinct alternative, while genuine "no" usage ("there is no time") has a
/// non-alternative predecessor and is preserved.
fn collapse_no_corrections(tokens: Vec<Token>) -> Vec<Token> {
    if tokens.len() < 3 {
        return tokens;
    }

    const ALT_CHOICES: &[&str] = &[
        "monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday",
        "january", "february", "march", "april", "may", "june", "july", "august",
        "september", "october", "november", "december", "tomorrow", "today", "tonight",
    ];

    let mut out: Vec<Token> = Vec::with_capacity(tokens.len());
    let mut i = 0;

    while i < tokens.len() {
        let is_no = tokens[i].normalized == "no";
        if is_no && i >= 1 && i + 1 < tokens.len() {
            let prev = tokens[i - 1].normalized.as_str();
            let next = tokens[i + 1].normalized.as_str();
            if ALT_CHOICES.contains(&prev)
                && ALT_CHOICES.contains(&next)
                && prev != next
                && !tokens[i - 1].protected
                && !tokens[i + 1].protected
            {
                // Drop the abandoned alternative and the "no" bridge, keep the correction.
                // `tokens[i-1]` (the abandoned alternative) was already emitted into `out`,
                // so pop it; advance past only "no" so `tokens[i+1]` (the correction) is
                // emitted on the next iteration.
                out.pop();
                i += 1;
                continue;
            }
        }
        out.push(tokens[i].clone());
        i += 1;
    }

    out
}

fn collapse_modifier_stacking(tokens: Vec<Token>) -> Vec<Token> {
    const INTENSIFIERS: &[&str] = &[
        "really", "extremely", "very", "quite", "rather", "fairly", "highly", "super", "ultra",
        "absolutely", "completely", "totally", "utterly",
    ];
    if tokens.len() < 3 {
        return tokens;
    }
    let mut out: Vec<Token> = Vec::with_capacity(tokens.len());
    let mut i = 0;
    while i < tokens.len() {
        if i + 2 < tokens.len()
            && INTENSIFIERS.contains(&tokens[i].normalized.as_str())
            && INTENSIFIERS.contains(&tokens[i + 1].normalized.as_str())
            && INTENSIFIERS.contains(&tokens[i + 2].normalized.as_str())
        {
            // Collapse 3+ consecutive intensifiers to single last one (keep most recent)
            // e.g., "really extremely very extremely important" → "extremely important"
            // But preserve intentional "very, very" (has comma boundary)
            let mut j = i + 2;
            while j + 1 < tokens.len()
                && INTENSIFIERS.contains(&tokens[j + 1].normalized.as_str())
                && !tokens[j].comma_boundary
                && !tokens[j].clause_boundary
            {
                j += 1;
            }
            out.push(tokens[j].clone());
            i = j + 1;
        } else {
            out.push(tokens[i].clone());
            i += 1;
        }
    }
    out
}

fn cleanup_streaming_tokens(tokens: Vec<Token>) -> Vec<Token> {
    let tokens = remove_fillers(tokens);
    let tokens = collapse_duplicate_tokens(tokens);
    collapse_repeated_phrases_with_limit(tokens, STREAMING_MAX_REPEAT_NGRAM)
}

pub fn normalize_transcript(input: &str) -> String {
    if input.trim().is_empty() {
        return String::new();
    }

    normalize_spacing_and_punctuation(cleanup_final_tokens(tokenize(input)))
}

pub fn normalize_streaming_preview(input: &str) -> String {
    if input.trim().is_empty() {
        return String::new();
    }

    normalize_spacing_and_punctuation(cleanup_streaming_tokens(tokenize(input)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(input: &str, expected: &str) {
        let output = normalize_transcript(input);
        assert_eq!(
            output, expected,
            "\ninput:    {:?}\noutput:   {:?}\nexpected: {:?}",
            input, output, expected
        );
    }

    #[test]
    fn removes_simple_duplicate_words() {
        check("when when model dialog render", "when model dialog render");
        check("I I want this", "I want this");
        check("buddy buddy buddy you good", "buddy you good");
        check("really really really really miss you", "really miss you");
    }

    #[test]
    fn carries_final_punctuation_when_collapsing_duplicates() {
        check("when when, model dialog render", "when, model dialog render");
        check("really really really. Stop.", "really. Stop.");
    }

    #[test]
    fn preserves_explicit_rhetorical_repetition() {
        check("very, very important", "very, very important");
        check("hello, hello there", "hello, hello there");
        check("No, no, keep that", "No, no, keep that");
        check("I know, I know this is weird", "I know, I know this is weird");
    }

    #[test]
    fn removes_fillers() {
        check("um I want you to fix this", "I want you to fix this");
        check("Okay, um, I want to fix this.", "Okay, I want to fix this.");
        check("uhh this is broken", "this is broken");
        check("uhmmmm this is broken", "this is broken");
        check("I think erm this is wrong", "I think this is wrong");
        check("hmm I think this is broken", "I think this is broken");
    }

    #[test]
    fn does_not_remove_valid_words_containing_filler_text() {
        check("umbrella human humming", "umbrella human humming");
        check("I am working", "I am working");
    }

    #[test]
    fn preserves_negation() {
        check("I don't know", "I don't know");
        check("I don't want this", "I don't want this");
        check("not working", "not working");
        check("never remove this", "never remove this");
        check("not not working", "not not working");
        check("don't don't change this", "don't don't change this");
    }

    #[test]
    fn preserves_numbers_times_paths_urls_emails_and_code_tokens() {
        check("src-tauri/src/managers/mlx.rs", "src-tauri/src/managers/mlx.rs");
        check("@/components/ui/Badge.tsx", "@/components/ui/Badge.tsx");
        check("bg-rose-600", "bg-rose-600");
        check("10:00 AM for 30 minutes", "10:00 AM for 30 minutes");
        check("3.14 is a number", "3.14 is a number");
        check("https://example.com/a/b", "https://example.com/a/b");
        check("alex@example.com", "alex@example.com");
        check("TanStack Query", "TanStack Query");
        check("Badge.tsx", "Badge.tsx");
    }

    #[test]
    fn collapses_repeated_phrases() {
        check(
            "the front end is the front end is fucked",
            "the front end is fucked",
        );
        check("on the on the on the card itself", "on the card itself");
        check("if I if I right now", "if I right now");
        check("I know I know I know I know", "I know");
    }

    #[test]
    fn collapses_long_repeated_phrases() {
        check(
            "there will be a time when we have to suffer a lot there will be a time when we have to suffer a lot",
            "there will be a time when we have to suffer a lot",
        );
    }

    #[test]
    fn preserves_repetition_across_explicit_boundaries() {
        check("we tried. we tried again", "we tried. we tried again");
        check("go, go now", "go, go now");
    }

    #[test]
    fn removes_restart_with_discourse_bridge() {
        check(
            "The second issue is that okay The second issue is that right now",
            "The second issue is that right now",
        );
        check(
            "when we like when we close the window",
            "when we close the window",
        );
    }

    #[test]
    fn removes_restart_with_short_weak_fragment() {
        check(
            "Third thing is that third thing is very important",
            "Third thing is very important",
        );
    }

    #[test]
    fn does_not_collapse_parallel_clauses() {
        check(
            "the app is fast, the app is reliable",
            "the app is fast, the app is reliable",
        );
        check(
            "the frontend is broken, the frontend is ugly",
            "the frontend is broken, the frontend is ugly",
        );
    }

    #[test]
    fn preserves_normal_command_language() {
        check("send it when it is ready", "send it when it is ready");
        check(
            "Tell Alex I will send it tomorrow",
            "Tell Alex I will send it tomorrow",
        );
        check("please reply to this email", "please reply to this email");
    }

    #[test]
    fn normalizes_only_standalone_punctuation_spacing() {
        check("hello , world !", "hello, world!");
        check("this is ( clean ) now", "this is (clean) now");
        check("10 : 00", "10:00");
        check("3 . 14", "3.14");
        check("https://example.com", "https://example.com");
    }

    #[test]
    fn preserves_unicode_names() {
        check("José José is here", "José is here");
        check("Renée is here", "Renée is here");
    }

    #[test]
    fn handles_empty_input() {
        check("", "");
        check("   \n\t ", "");
    }

    #[test]
    fn cleanup_is_idempotent() {
        let inputs = [
            "when when model dialog render",
            "um I know I know this is broken",
            "the front end is the front end is fucked",
            "I don't want this",
            "src-tauri/src/managers/mlx.rs",
            "when we like when we close the window",
        ];

        for input in inputs {
            let once = normalize_transcript(input);
            let twice = normalize_transcript(&once);
            assert_eq!(once, twice, "input: {input:?}");
        }
    }

    #[test]
    fn streaming_preview_is_conservative() {
        assert_eq!(normalize_streaming_preview("when when hello"), "when hello");
        assert_eq!(normalize_streaming_preview("um when when hello"), "when hello");
        assert_eq!(
            normalize_streaming_preview("I don't I don't know"),
            "I don't I don't know"
        );
    }

    #[test]
    fn real_transcript_duplicate_patterns() {
        check(
            "right now if I if I right now want to fix this",
            "right now if I right now want to fix this",
        );
        check(
            "I know I know I know I know it's been a long journey",
            "I know it's been a long journey",
        );
        check(
            "Buddy buddy buddy you good I really really really miss you",
            "Buddy you good I really miss you",
        );
    }

    #[test]
    fn collapses_self_correction_alternative() {
        check("we should ship Tuesday no Wednesday", "we should ship Wednesday");
        check(
            "the demo is Monday no Tuesday",
            "the demo is Tuesday",
        );
        check(
            "lets meet Friday no Thursday next week",
            "lets meet Thursday next week",
        );
    }

    #[test]
    fn collapses_comma_stutter_but_keeps_intentional_emphasis() {
        // 5x stutter across commas → collapse to one.
        check("I don't, don't, don't, don't, don't wanna", "I don't wanna");
        // 2-3x intentional emphasis across commas → preserved (spec).
        check("very, very important", "very, very important");
        check("go, go, go", "go, go, go");
        check("no, no, that's not what I meant", "no, no, that's not what I meant");
    }

    #[test]
    fn preserves_genuine_no_negation() {
        check("there is no time left", "there is no time left");
        check("I have no idea why", "I have no idea why");
    }

    #[test]
    fn collapses_self_correction_bridges() {
        check("I can I mean I can't do that", "I can't do that");
        check(
            "it could have I mean it couldn't have happened",
            "it couldn't have happened",
        );
        check("he said I can I mean I can't", "he said I can't");
        check("I will I mean I won't go", "I won't go");
    }

    #[test]
    fn preserves_ambiguous_self_corrections() {
        // Long abandoned clause (> frag limit) — meaning ambiguous, keep original.
        check(
            "I went to the store and I mean I also went to the park",
            "I went to the store and I mean I also went to the park",
        );
        // Genuine "mean" usage, not a correction bridge.
        check("I mean this is important", "I mean this is important");
    }

    #[test]
    fn large_input_stays_bounded() {
        let mut input = String::new();
        for _ in 0..500 {
            input.push_str("the frontend is fast and reliable and the model works correctly. ");
        }

        let start = std::time::Instant::now();
        let output = normalize_transcript(&input);
        let elapsed = start.elapsed();

        assert!(elapsed.as_secs_f32() < 1.0, "cleanup took {:?}", elapsed);
        assert!(!output.is_empty());
    }
}
