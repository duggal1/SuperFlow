use std::collections::HashMap;
use std::sync::OnceLock;

use serde_json::Value;

/// Embedded emoji alias catalog. Same embedding convention as every other
/// speech-correction catalog: compile-time `include_str!`, graceful degrade.
const EMOJIES_JSON: &str = include_str!("../catalog/emojies.json");

/// Hard ceiling on repeats produced from one spoken count. Prevents absurd
/// output like "one hundred fire emojis" → 100 flames from a mishearing.
const MAX_REPEATS: usize = 10;

/// Largest number word accepted as a count ("twenty"). Anything above the
/// words listed here only ever yields the fallback repeat count.
const COUNT_WORDS: &[&str] = &[
    "zero",
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
    "twenty",
];

struct EmojiIndex {
    /// Lowercased alias phrase → emoji. Keys exist in both singular and
    /// plural-spoken form ("rocket emoji" / "rocket emojis").
    aliases: HashMap<String, String>,
}

static INDEX: OnceLock<EmojiIndex> = OnceLock::new();

fn parse_emoji_value(value: &Value, out: &mut Vec<(String, String)>) {
    let Some(entries) = value.as_array() else {
        return;
    };
    for entry in entries {
        let Some(emoji) = entry.get("emoji").and_then(Value::as_str) else {
            continue;
        };
        let Some(aliases) = entry.get("aliases").and_then(Value::as_array) else {
            continue;
        };
        for alias in aliases {
            if let Some(alias) = alias.as_str() {
                out.push((alias.trim().to_lowercase(), emoji.to_string()));
            }
        }
    }
}

fn index() -> &'static EmojiIndex {
    INDEX.get_or_init(|| {
        let mut pairs: Vec<(String, String)> = Vec::new();
        match serde_json::from_str::<Value>(EMOJIES_JSON) {
            Ok(document) => parse_emoji_value(&document, &mut pairs),
            Err(e) => log::error!("Failed to parse embedded emoji catalog: {e}"),
        }

        // Plural spoken forms ride along automatically: aliases ending in
        // "emoji" gain an "emojis" twin so "three rocket emojis" matches the
        // same entry as "three rocket emoji".
        let mut aliases = HashMap::with_capacity(pairs.len() * 2);
        for (alias, emoji) in pairs {
            if !aliases.contains_key(&alias) {
                aliases.insert(alias.clone(), emoji.clone());
            }
            if alias.ends_with("emoji") {
                let plural = format!("{alias}s");
                aliases.entry(plural).or_insert(emoji);
            }
        }
        EmojiIndex { aliases }
    })
}

/// Parses a count token: either plain digits or the number words in
/// `COUNT_WORDS`. Everything else fails closed to a default repeat of one.
fn parse_count(token: Option<&str>) -> usize {
    const DEFAULT: usize = 1;
    if token.is_none() {
        return DEFAULT;
    }
    let token = token.unwrap().to_lowercase();
    if let Ok(digits) = token.parse::<usize>() {
        return digits.clamp(DEFAULT, MAX_REPEATS);
    }
    COUNT_WORDS
        .iter()
        .position(|word| *word == token)
        .map_or(DEFAULT, |index| index.max(DEFAULT))
        .clamp(DEFAULT, MAX_REPEATS)
}

/// Longest alias phrase (by whitespace-separated token count) that starts at
/// `tokens[from]`, or None when no catalog alias begins there. Tokens are
/// lowercased before joining so spoken casing never blocks a match.
fn longest_alias_at(tokens: &[&str], from: usize) -> Option<(usize, String)> {
    const MAX_PHRASE_TOKENS: usize = 5;
    for span in (1..=MAX_PHRASE_TOKENS).rev() {
        if from + span > tokens.len() {
            continue;
        }
        let phrase = tokens[from..from + span].join(" ").to_lowercase();
        if let Some(emoji) = index().aliases.get(&phrase) {
            return Some((span, emoji.clone()));
        }
    }
    None
}

/// Expands spoken emoji requests into real repeated emojis: the transcript
/// phrase "three rocket emojis" becomes "🚀🚀🚀", never "3 🚀". Purely local,
/// deterministic dead math — counts pair with entries instead of printing a
/// numeral beside one emoji.
pub fn apply(text: &str) -> String {
    if text.is_empty() || !text.chars().any(|c| c.is_alphabetic()) {
        return text.to_string();
    }
    let tokens: Vec<&str> = text.split_whitespace().collect();
    if tokens.len() < 2 {
        return text.to_string();
    }

    let mut out: Vec<String> = Vec::with_capacity(tokens.len());
    let mut replaced = false;
    let mut cursor = 0usize;
    while cursor < tokens.len() {
        // Optional count token immediately before the alias phrase.
        let token_is_digit = tokens[cursor].chars().all(|c| c.is_ascii_digit());
        let count_offset = if (token_is_digit || parse_count(Some(tokens[cursor])) != 1)
            && cursor + 1 < tokens.len()
        {
            1
        } else {
            0
        };

        if let Some((span, emoji)) = longest_alias_at(&tokens, cursor + count_offset) {
            let count = parse_count(if count_offset == 1 {
                Some(tokens[cursor])
            } else {
                None
            });
            out.push(emoji.repeat(count));
            replaced = true;
            cursor += count_offset + span;
            continue;
        }
        out.push(tokens[cursor].to_string());
        cursor += 1;
    }
    if !replaced {
        return text.to_string();
    }
    out.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_count_expands_to_repeated_emojis() {
        assert_eq!(apply("three rocket emojis"), "🚀🚀🚀");
        assert_eq!(apply("five fire emojis"), "🔥🔥🔥🔥🔥");
    }

    #[test]
    fn digit_count_pairs_real_logic_not_a_numeral() {
        assert_eq!(apply("3 rocket emojis"), "🚀🚀🚀");
        assert_eq!(apply("2 thumbs up"), "👍👍");
    }

    #[test]
    fn bare_alias_becomes_single_emoji() {
        assert_eq!(apply("heart emoji"), "❤️");
        assert_eq!(apply("add a fire emoji"), "add a 🔥");
    }

    #[test]
    fn casing_does_not_block_matching() {
        assert_eq!(apply("Three Rocket Emojis"), "🚀🚀🚀");
    }

    #[test]
    fn unknown_phrases_pass_through_untouched() {
        let text = "the blue widget emoji spec ships today";
        assert_eq!(apply(text), text);
    }

    #[test]
    fn counts_above_catalog_ceiling_are_capped() {
        let out = apply("twenty star emojis");
        assert_eq!(out, "⭐".repeat(MAX_REPEATS));
    }

    #[test]
    fn running_gags_do_not_explode_cap_at_ten() {
        let out = apply("celebrate with 99 party popper emojis now");
        assert!(out.starts_with("celebrate with "));
        assert!(out.contains("🎉".repeat(10).as_str()));
        assert!(!out.contains("🎉".repeat(11).as_str()));
    }

    #[test]
    fn surrounding_prose_is_preserved() {
        assert_eq!(
            apply("ship it with three rocket emojis then deploy"),
            "ship it with 🚀🚀🚀 then deploy"
        );
    }

    #[test]
    fn applying_twice_is_idempotent() {
        let once = apply("two fire emojis");
        assert_eq!(apply(&once), once);
    }
}
