//! Live punctuation for streaming dictation output.
//!
//! Runs locally on every emitted stream segment while the user speaks:
//! sentence capitalization, terminal marks (`.` / `?`), standalone-"I"
//! fixing, and in [`PunctuationStyle::Formal`] mode light comma restoration.
//! Purely deterministic — microseconds per segment, never blocks, never
//! fails — and deliberately conservative around code tokens (camelCase,
//! PascalCase, snake_case are left untouched).
//!
//! A neural restoration backend can replace [`punctuate`] later without any
//! call-site change: callers pass text plus style and receive styled text.

use crate::settings::PunctuationStyle;

/// Words whose presence at a sentence start makes the sentence a question.
const QUESTION_CUES: &[&str] = &[
    "what", "why", "how", "when", "where", "who", "whom", "whose", "which", "is", "are", "am",
    "was", "were", "do", "does", "did", "can", "could", "will", "would", "should", "shall",
    "may", "might", "have", "has", "had",
];

/// Discourse markers that take a following comma in formal mode.
const FORMAL_LEADING_MARKERS: &[&str] = &[
    "so", "and", "but", "actually", "basically", "however", "meanwhile", "also", "then",
    "well", "yes", "no", "okay", "ok",
];

fn is_terminal_char(c: char) -> bool {
    matches!(c, '.' | '!' | '?' | ',' | ';' | ':')
}

/// The first word is capitalized only when it carries no internal casing of
/// its own — camelCase/PascalCase/snake_case identifiers survive verbatim.
fn capitalize_first(word: &str) -> String {
    let mut chars = word.char_indices();
    if let Some((_, first)) = chars.next() {
        let rest_starts_uppercase = word.chars().skip(1).any(|c| c.is_uppercase());
        let has_underscore = word.contains('_');
        if rest_starts_uppercase || has_underscore {
            return word.to_string();
        }
        let mut out = String::with_capacity(word.len());
        out.extend(first.to_uppercase());
        out.push_str(&word[first.len_utf8()..]);
        return out;
    }
    word.to_string()
}

fn first_word(sentence: &str) -> String {
    sentence
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .trim_end_matches(|c: char| !c.is_alphanumeric())
        .to_lowercase()
}

fn is_question(sentence: &str) -> bool {
    let first = first_word(sentence);
    QUESTION_CUES.contains(&first.as_str())
}

fn apply_formal_commas(sentence: &str) -> String {
    let mut words: Vec<String> = Vec::new();
    for (index, word) in sentence.split_whitespace().enumerate() {
        if index == 0 {
            let lowered_first = word
                .trim_end_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase();
            if FORMAL_LEADING_MARKERS.contains(&lowered_first.as_str()) {
                // Recase the marker's canonical form with the original casing
                // pattern preserved by capitalize_first below anyway.
                words.push(format!("{word},"));
                continue;
            }
        }
        if lowered_is_bare_conjunction_but(word) && words.len() > 1 {
            // Comma goes on the previous word: "ship it ,but"? No — attach
            // before: previous word gets the comma instead.
            if let Some(previous) = words.last_mut() {
                if previous
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_alphanumeric())
                {
                    previous.push(',');
                }
            }
        }
        words.push(word.to_string());
    }
    words.join(" ")
}

fn lowered_is_bare_conjunction_but(word: &str) -> bool {
    word.eq_ignore_ascii_case("but")
}

fn punctuate_sentence(raw: &str, style: PunctuationStyle) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let mut sentence = match style {
        PunctuationStyle::Formal => apply_formal_commas(trimmed),
        PunctuationStyle::Informal => trimmed.to_string(),
    };

    // Sentence-start capitalization (code-token aware).
    let mut fixed = String::with_capacity(sentence.len());
    let (leading, core) = sentence
        .char_indices()
        .find(|(_, c)| c.is_alphanumeric())
        .map(|(index, _)| sentence.split_at(index))
        .unwrap_or(("", sentence.as_str()));
    fixed.push_str(leading);
    if let Some(space_after) = core.split_once(' ') {
        let (first_word, remainder) = space_after;
        fixed.push_str(&capitalize_first(first_word));
        fixed.push(' ');
        fixed.push_str(&fix_standalone_i(remainder));
    } else {
        fixed.push_str(&capitalize_first(core));
    }
    sentence = fixed;

    // Terminal mark.
    let ends_terminal = sentence
        .chars()
        .last()
        .is_some_and(is_terminal_char);
    if !ends_terminal && sentence.chars().last().is_some_and(|c| c.is_alphanumeric()) {
        sentence.push(if is_question(trimmed) { '?' } else { '.' });
    }

    sentence
}

/// Standalone lowercase "i" is always the pronoun in dictation.
fn fix_standalone_i(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for (index, token) in text.split_whitespace().enumerate() {
        if index > 0 {
            out.push(' ');
        }
        if token == "i" {
            out.push('I');
        } else {
            out.push_str(token);
        }
    }
    out
}

/// Punctuates streaming display text for the given style. Existing terminal
/// marks are always respected; nothing is ever removed.
pub fn punctuate(text: &str, style: PunctuationStyle) -> String {
    if text.trim().is_empty() {
        return text.to_string();
    }
    let pieces: Vec<String> = split_sentences(text)
        .into_iter()
        .map(|sentence| punctuate_sentence(sentence, style))
        .collect();
    let joined = pieces.join(" ");
    // Preserve any trailing whitespace shape of the original.
    let trailing = text.len() - text.trim_end().len();
    if trailing > 0 {
        format!("{joined}{}", " ".repeat(trailing.min(1)))
    } else {
        joined
    }
}

fn split_sentences(text: &str) -> Vec<&str> {
    let mut sentences = Vec::new();
    let mut start = 0usize;
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if matches!(bytes[index], b'.' | b'!' | b'?') {
            let next = text[index + 1..].chars().next();
            if next.is_none_or(|c| c.is_whitespace()) {
                let end = (index + 1).min(text.len());
                sentences.push(&text[start..end]);
                start = end;
            }
        }
        index += 1;
    }
    if start < text.len() {
        sentences.push(&text[start..]);
    }
    sentences
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn informal_adds_caps_and_terminals_only() {
        assert_eq!(punctuate("hello world", PunctuationStyle::Informal), "Hello world.");
        assert_eq!(
            punctuate("what is this thing", PunctuationStyle::Informal),
            "What is this thing?"
        );
    }

    #[test]
    fn formal_restores_light_commas() {
        assert_eq!(
            punctuate("so we ship it today", PunctuationStyle::Formal),
            "So, we ship it today."
        );
        assert_eq!(
            punctuate("we could ship it but it is rough", PunctuationStyle::Formal),
            "We could ship it, but it is rough."
        );
    }

    #[test]
    fn informal_never_adds_commas() {
        assert_eq!(
            punctuate("so we ship it today", PunctuationStyle::Informal),
            "So we ship it today."
        );
    }

    #[test]
    fn code_tokens_keep_their_casing() {
        assert_eq!(
            punctuate("useEffect runs after render", PunctuationStyle::Informal),
            "useEffect runs after render."
        );
        assert_eq!(
            punctuate("the user_id field", PunctuationStyle::Informal),
            "The user_id field."
        );
    }

    #[test]
    fn existing_marks_are_respected() {
        assert_eq!(punctuate("ready!", PunctuationStyle::Informal), "Ready!");
        assert_eq!(
            punctuate("first one. second one", PunctuationStyle::Informal),
            "First one. Second one."
        );
    }

    #[test]
    fn standalone_i_is_fixed_everywhere() {
        assert_eq!(
            punctuate("i think i can", PunctuationStyle::Informal),
            "I think I can."
        );
    }

    #[test]
    fn empty_input_passthrough() {
        assert_eq!(punctuate("", PunctuationStyle::Formal), "");
        assert_eq!(punctuate("   ", PunctuationStyle::Formal), "   ");
    }
}
