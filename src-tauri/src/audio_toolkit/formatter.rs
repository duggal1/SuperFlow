//! Formatting and output normalization — the layer that turns normalized
//! transcript text into clean, intentional-looking technical writing.
//!
//! Runs strictly locally after the catalog correction chain, always on.
//! Deterministic only: it formats numbers, decimals, percentages, currency,
//! units, and times; infers sentence capitalization and terminals; inserts
//! coordinate commas for repeated-conjunction lists; converts explicitly
//! ordinal speech ("first … second …") into numbered Markdown lists; and
//! wraps literal technical tokens in inline code. It never invents values,
//! never adds headings or bold, and preserves upstream canonical terms.

use crate::settings::PunctuationStyle;

const UNITS: &[&str] = &[
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
];
const TENS: &[&str] = &[
    "", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
];

fn clean(word: &str) -> String {
    word.trim_matches(|c: char| !c.is_alphanumeric())
        .to_lowercase()
}

fn number_clean(word: &str) -> String {
    crate::audio_toolkit::normalization::number_word(word)
}

fn unit_value(word: &str) -> Option<u64> {
    UNITS.iter().position(|u| *u == word).map(|i| i as u64)
}

fn tens_value(word: &str) -> Option<u64> {
    TENS.iter().position(|t| *t == word).map(|i| i as u64 * 10)
}

/// A parsed spoken number. `scale` records the last large multiplier so a
/// spoken fraction can be rendered compactly ("one point five million").
#[derive(Debug, Clone, Copy)]
struct SpokenNumber {
    value: u128,
    had_large_scale: bool,
    used: usize,
}

fn parse_spoken_integer(words: &[&str], start: usize) -> Option<SpokenNumber> {
    let mut total = 0u128;
    let mut index = start;
    let mut previous_scale = u128::MAX;
    let mut had_large_scale = false;

    loop {
        let (small, small_used) = parse_under_thousand(words, index)?;
        index += small_used;

        let scale_word = words.get(index).map(|word| number_clean(word));
        let scale = scale_word.as_deref().and_then(large_scale);
        if let Some(scale) = scale {
            if scale >= previous_scale {
                return None;
            }
            total = total.checked_add(small.checked_mul(scale)?)?;
            previous_scale = scale;
            had_large_scale = true;
            index += 1;

            if number_clean(words.get(index).copied().unwrap_or("")) == "and"
                && parse_under_thousand(words, index + 1).is_some()
            {
                index += 1;
            }

            if parse_under_thousand(words, index).is_some() {
                continue;
            }
        } else {
            total = total.checked_add(small)?;
        }
        break;
    }

    Some(SpokenNumber {
        value: total,
        had_large_scale,
        used: index - start,
    })
}

fn parse_under_hundred(words: &[&str], start: usize) -> Option<(u128, usize)> {
    let first = number_clean(words.get(start)?);
    if let Ok(value) = first.replace(',', "").parse::<u128>() {
        return Some((value, 1));
    }
    if let Some(value) = unit_value(&first) {
        return Some((value as u128, 1));
    }
    let tens = tens_value(&first)? as u128;
    let next = words.get(start + 1).map(|word| number_clean(word));
    if let Some(unit) = next
        .as_deref()
        .and_then(unit_value)
        .filter(|value| *value < 10)
    {
        Some((tens + unit as u128, 2))
    } else {
        Some((tens, 1))
    }
}

fn parse_under_thousand(words: &[&str], start: usize) -> Option<(u128, usize)> {
    let first = number_clean(words.get(start)?);
    let hundred_prefix = if first == "a" {
        Some(1u128)
    } else {
        unit_value(&first)
            .filter(|value| (1..=9).contains(value))
            .map(u128::from)
    };

    if let Some(prefix) = hundred_prefix {
        if number_clean(words.get(start + 1).copied().unwrap_or("")) == "hundred" {
            let mut used = 2usize;
            let mut value = prefix.checked_mul(100)?;
            let has_and = number_clean(words.get(start + used).copied().unwrap_or("")) == "and";
            if has_and {
                used += 1;
            }
            if let Some((tail, tail_used)) = parse_under_hundred(words, start + used) {
                if tail < 100 {
                    value = value.checked_add(tail)?;
                    used += tail_used;
                } else if has_and {
                    return None;
                }
            } else if has_and {
                return None;
            }
            return Some((value, used));
        }
    }

    parse_under_hundred(words, start)
}

fn large_scale(word: &str) -> Option<u128> {
    Some(match word {
        "thousand" => 1_000,
        "lakh" => 100_000,
        "million" => 1_000_000,
        "crore" => 10_000_000,
        "billion" => 1_000_000_000,
        "trillion" => 1_000_000_000_000,
        _ => return None,
    })
}

/// Consumes spoken fraction digits starting at the word "point":
/// "point one four" → digits "14".
fn parse_decimal_digits(words: &[&str], start: usize) -> Option<(String, usize)> {
    if number_clean(words.get(start)?) != "point" {
        return None;
    }
    let mut digits = String::new();
    let mut used = 1usize;
    let mut index = start + 1;
    while index < words.len() {
        let word = number_clean(words[index]);
        if matches!(word.as_str(), "oh" | "zero") {
            digits.push('0');
        } else if let Some(digit) = unit_value(&word) {
            digits.push_str(&digit.to_string());
        } else if !word.is_empty() && word.chars().all(|c| c.is_ascii_digit()) {
            digits.push_str(&word);
        } else {
            break;
        }
        used += 1;
        index += 1;
    }
    (!digits.is_empty()).then_some((digits, used))
}

fn group_digits(value: u128) -> String {
    let raw = value.to_string();
    let bytes = raw.as_bytes();
    let mut grouped = String::new();
    for (index, byte) in bytes.iter().enumerate() {
        if index > 0 && (bytes.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(*byte as char);
    }
    grouped
}

fn group_indian_digits(value: u128) -> String {
    let raw = value.to_string();
    if raw.len() <= 3 {
        return raw;
    }
    let split = raw.len() - 3;
    let mut prefix = raw[..split].to_string();
    let mut groups = Vec::new();
    while prefix.len() > 2 {
        let cut = prefix.len() - 2;
        groups.push(prefix[cut..].to_string());
        prefix.truncate(cut);
    }
    if !prefix.is_empty() {
        groups.push(prefix);
    }
    groups.reverse();
    format!("{},{}", groups.join(","), &raw[split..])
}

fn viewport_unit(first: &str, second: &str) -> Option<&'static str> {
    match (first, second) {
        ("viewport", "height") | ("view", "height") => Some("vh"),
        ("viewport", "width") | ("view", "width") => Some("vw"),
        _ => None,
    }
}

/// Parses clock times: hour + minute (+ optional am/pm). A bare hour:minute
/// pair needs a preceding time cue ("meet at ten twenty") so ordinary counts
/// never become clocks.
fn try_parse_time(words: &[&str], start: usize) -> Option<(String, usize)> {
    let hour_word = clean(words.get(start)?);
    let hour = unit_value(&hour_word)
        .filter(|h| (1..=24).contains(h))
        .or_else(|| {
            hour_word
                .parse::<u64>()
                .ok()
                .filter(|h| (1..=24).contains(h))
        })?;

    let mut index = start + 1;
    let minute_word = clean(words.get(index)?);
    let oh_prefix = matches!(minute_word.as_str(), "oh" | "zero");
    let minutes;
    let mut extra_words = 0usize;
    if oh_prefix {
        let digit = clean(words.get(index + 1)?);
        minutes = unit_value(&digit).filter(|m| *m < 10)?;
        extra_words = 1;
    } else if let Some(tens) = tens_value(&minute_word) {
        // "thirty", optionally composed: "forty five".
        let unit_part = words.get(index + 1).map(|w| clean(w)).unwrap_or_default();
        match unit_value(&unit_part).filter(|u| *u < 10) {
            Some(unit) => {
                minutes = tens + unit;
                extra_words = 1;
            }
            None => minutes = tens,
        }
    } else {
        minutes = unit_value(&minute_word).filter(|m| *m < 60)?;
    }
    index += 1 + extra_words;

    let prev_cue = start > 0
        && matches!(
            clean(words[start - 1]).as_str(),
            "at" | "by" | "until" | "around"
        );
    let mut suffix_words = 0usize;
    let mut meridiem = None;
    match words.get(index).map(|w| clean(w)).as_deref() {
        Some("pm") | Some("p m") => {
            meridiem = Some("PM");
            suffix_words = if words.get(index).map(|w| clean(w)).as_deref() == Some("p m") {
                2
            } else {
                1
            };
        }
        Some("am") | Some("a m") => {
            meridiem = Some("AM");
            suffix_words = if words.get(index).map(|w| clean(w)).as_deref() == Some("a m") {
                2
            } else {
                1
            };
        }
        _ => {}
    }
    if meridiem.is_none() && !prev_cue {
        return None;
    }

    let rendered_minutes = if oh_prefix && minutes < 10 {
        format!("0{minutes}")
    } else {
        format!("{minutes:02}")
    };

    let formatted = match meridiem {
        Some("PM") => {
            let hour12 = if hour == 12 { 12 } else { hour % 12 };
            format!(
                "{}:{rendered_minutes} PM",
                if hour12 == 0 { 12 } else { hour12 }
            )
        }
        Some("AM") => {
            let hour12 = if hour == 12 { 0 } else { hour };
            format!(
                "{}:{rendered_minutes} AM",
                if hour12 == 0 { 12 } else { hour12 }
            )
        }
        _ => format!("{hour}:{rendered_minutes}"),
    };
    Some((formatted, index - start + suffix_words))
}

/// Token-level normalization: numbers, decimals, percentages, currency,
/// units, and times.
fn normalize_numerics(text: &str) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut out: Vec<String> = Vec::with_capacity(words.len());
    let mut index = 0usize;

    while index < words.len() {
        if let Some((formatted, consumed)) = try_parse_time(&words, index) {
            out.push(formatted);
            index += consumed;
            continue;
        }

        let sign = matches!(number_clean(words[index]).as_str(), "minus" | "negative");
        let number_start = index + usize::from(sign);
        if number_start >= words.len() {
            out.push(words[index].to_string());
            break;
        }
        let integer = parse_spoken_integer(&words, number_start);
        let leading_fraction = (number_clean(words[number_start]) == "point")
            .then(|| parse_decimal_digits(&words, number_start))
            .flatten();

        if let Some(number) = integer.or_else(|| {
            leading_fraction.as_ref().map(|(_, used)| SpokenNumber {
                value: 0,
                had_large_scale: false,
                used: *used,
            })
        }) {
            if ambiguous_additive_context(&words, number_start) {
                out.push(words[index].to_string());
                index += 1;
                continue;
            }

            let mut used = number.used + usize::from(sign);
            let after_integer = number_start + number.used;
            let mut fraction = leading_fraction.map(|(digits, _)| digits);
            if fraction.is_none()
                && number_clean(words.get(after_integer).copied().unwrap_or("")) == "point"
            {
                if let Some((digits, fraction_used)) = parse_decimal_digits(&words, after_integer) {
                    fraction = Some(digits);
                    used += fraction_used;
                }
            }

            let scale_index = index + used;
            let compact_scale = fraction.as_ref().and_then(|_| {
                let word = number_clean(words.get(scale_index).copied().unwrap_or(""));
                large_scale(&word).map(|_| word)
            });
            if compact_scale.is_some() {
                used += 1;
            }

            let follower_index = index + used;
            let sign_prefix = if sign { "-" } else { "" };
            let number_text = match (&fraction, compact_scale.as_deref()) {
                (Some(digits), Some(scale)) => {
                    format!("{sign_prefix}{}.{} {scale}", number.value, digits)
                }
                (Some(digits), None) => format!("{sign_prefix}{}.{}", number.value, digits),
                (None, _) => format!("{sign_prefix}{}", group_digits(number.value)),
            };

            if let Some((symbol, currency_words)) =
                crate::audio_toolkit::normalization::currency_symbol(&words, follower_index)
            {
                used += currency_words;
                let rendered = if symbol == "₹" && fraction.is_none() && compact_scale.is_none() {
                    format!("{sign_prefix}{symbol}{}", group_indian_digits(number.value))
                } else {
                    format!("{symbol}{number_text}")
                };
                out.push(with_consumed_suffix(rendered, &words, index, used));
                index += used;
                continue;
            }

            if let Some((unit, spacing, unit_words)) =
                crate::audio_toolkit::normalization::unit(&words, follower_index)
            {
                used += unit_words;
                let rendered = match spacing {
                    crate::audio_toolkit::normalization::UnitSpacing::Tight => {
                        format!("{number_text}{unit}")
                    }
                    crate::audio_toolkit::normalization::UnitSpacing::Space => {
                        format!("{number_text} {unit}")
                    }
                };
                out.push(with_consumed_suffix(rendered, &words, index, used));
                index += used;
                continue;
            }

            let percent_words = percent_width(&words, follower_index);
            if percent_words > 0 {
                used += percent_words;
                out.push(with_consumed_suffix(
                    format!("{number_text}%"),
                    &words,
                    index,
                    used,
                ));
                index += used;
                continue;
            }

            if follower_index + 1 < words.len() {
                let next = clean(words[follower_index]);
                let after = clean(words[follower_index + 1]);
                if let Some(unit) = viewport_unit(&next, &after) {
                    used += 2;
                    out.push(with_consumed_suffix(
                        format!("{number_text}{unit}"),
                        &words,
                        index,
                        used,
                    ));
                    index += used;
                    continue;
                }
            }

            let explicit_digits = clean(words[number_start])
                .replace(',', "")
                .chars()
                .all(|character| character.is_ascii_digit());
            if fraction.is_some()
                || number.had_large_scale
                || explicit_digits && number.value >= 1_000
            {
                out.push(with_consumed_suffix(number_text, &words, index, used));
                index += used;
                continue;
            }
        }

        out.push(words[index].to_string());
        index += 1;
    }

    out.join(" ")
}

/// Normalize only spoken numeric values, currencies, percentages, units, and
/// times. This deliberately does not alter prose casing, punctuation, or
/// structure; S1-mini owns those concerns in the final transcript pipeline.
pub fn normalize_values(text: &str) -> String {
    normalize_numerics(text)
}

fn ambiguous_additive_context(words: &[&str], start: usize) -> bool {
    if start < 2 || number_clean(words[start - 1]) != "and" {
        return false;
    }
    parse_under_hundred(words, start - 2).is_some() && number_clean(words[start - 2]) != "hundred"
}

fn percent_width(words: &[&str], start: usize) -> usize {
    match (
        clean(words.get(start).copied().unwrap_or("")),
        clean(words.get(start + 1).copied().unwrap_or("")),
    ) {
        (word, _) if word == "percent" || word == "percentage" => 1,
        (first, second) if first == "per" && second == "cent" => 2,
        _ => 0,
    }
}

fn with_consumed_suffix(mut rendered: String, words: &[&str], start: usize, used: usize) -> String {
    let Some(last) = words.get(start + used - 1) else {
        return rendered;
    };
    let suffix_start = last
        .char_indices()
        .rev()
        .find(|(_, character)| character.is_alphanumeric())
        .map(|(index, character)| index + character.len_utf8())
        .unwrap_or(0);
    rendered.push_str(&last[suffix_start..]);
    rendered
}

// ---------------------------------------------------------------------------
// Structure: coordinate commas, ordinal lists, inline code
// ---------------------------------------------------------------------------

/// Formal mode: a clause using the same coordinating conjunction twice is an
/// enumeration — "A and B and C" becomes "A, B and C" with separators before
/// every non-final conjunction.
fn coordinate_commas(sentence: &str, style: PunctuationStyle) -> String {
    if matches!(style, PunctuationStyle::Informal) {
        return sentence.to_string();
    }
    for conjunction in ["and", "or"] {
        let tokens: Vec<&str> = sentence.split_whitespace().collect();
        let total = tokens
            .iter()
            .filter(|t| t.trim_matches(',').eq_ignore_ascii_case(conjunction))
            .count();
        if total < 2 {
            continue;
        }
        let mut rebuilt: Vec<String> = Vec::with_capacity(tokens.len());
        let mut seen = 0usize;
        for token in tokens {
            if token.trim_matches(',').eq_ignore_ascii_case(conjunction) {
                seen += 1;
                if seen < total
                    && rebuilt.last().is_some_and(|prev: &String| {
                        prev.chars()
                            .next_back()
                            .is_some_and(|c| c.is_alphanumeric())
                    })
                {
                    if let Some(previous) = rebuilt.last_mut() {
                        previous.push(',');
                    }
                }
            }
            rebuilt.push(token.to_string());
        }
        return rebuilt.join(" ");
    }
    sentence.to_string()
}

/// Capitalizes the first alphabetic character unless the token already carries
/// its own casing (camelCase/PascalCase/snake_case survive verbatim).
fn recapitalize(text: &str) -> String {
    for (index, ch) in text.char_indices() {
        if !ch.is_alphabetic() {
            continue;
        }
        let rest_has_upper = text[index + ch.len_utf8()..]
            .chars()
            .take_while(|c| c.is_alphanumeric())
            .any(|c| c.is_uppercase());
        let has_underscore = text[index..].contains('_');
        if rest_has_upper || has_underscore {
            return text.to_string();
        }
        let mut out = text.to_string();
        out.replace_range(index..index + ch.len_utf8(), &ch.to_uppercase().to_string());
        return out;
    }
    text.to_string()
}

const ORDINAL_CUES: &[&str] = &["first", "second", "third", "fourth", "fifth", "finally"];

/// Explicitly ordinal procedural speech becomes a numbered Markdown list.
/// Ordinal words are absorbed into their step numbers per the spec example:
/// "First fix the header." → "1. Fix the header."
fn numbered_list(sentences: &[&str]) -> Option<String> {
    let ordinal_positions: Vec<Option<usize>> = sentences
        .iter()
        .map(|sentence| {
            sentence
                .split_whitespace()
                .next()
                .map(clean)
                .and_then(|word| ORDINAL_CUES.iter().position(|cue| *cue == word))
        })
        .collect();

    let numbered = ordinal_positions.iter().filter(|p| p.is_some()).count();
    if numbered < 2 {
        return None;
    }

    let mut lines: Vec<String> = Vec::new();
    let mut step = 0usize;
    for (sentence, position) in sentences.iter().zip(&ordinal_positions) {
        match position {
            Some(_) => {
                step += 1;
                let body = sentence
                    .char_indices()
                    .find(|(_, c)| c.is_whitespace())
                    .map(|(index, _)| &sentence[index..])
                    .unwrap_or("")
                    .trim();
                lines.push(format!("{}. {}", step, recapitalize(body)));
            }
            None if step > 0 => {
                if let Some(last) = lines.last_mut() {
                    last.push(' ');
                    last.push_str(sentence);
                }
            }
            None => lines.push((*sentence).to_string()),
        }
    }
    Some(lines.join("\n"))
}

/// True for whitespace tokens that are literal technical artifacts: file
/// paths with extensions, hex colors, SCREAMING_SNAKE identifiers, and
/// hyphenated lowercase utility classes carrying a digit or variant.
fn is_technical_token(token: &str) -> bool {
    if token.len() < 4 || token.contains(' ') {
        return false;
    }

    if token.contains('/')
        && !token.ends_with('/')
        && token.rsplit('.').next().is_some_and(|ext| {
            (2..=6).contains(&ext.len()) && ext.chars().all(|c| c.is_ascii_alphanumeric())
        })
        && token
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "/.-_~".contains(c))
    {
        return true;
    }

    if token.starts_with('#')
        && token.len() <= 9
        && token[1..].chars().all(|c| c.is_ascii_hexdigit())
    {
        return true;
    }

    if token.contains('_')
        && token
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
    {
        return true;
    }

    if token.contains('-') && token.contains(':')
        || token.contains('-') && token.chars().any(|c| c.is_ascii_digit())
    {
        return token
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "-:.[]/".contains(c));
    }

    false
}

fn wrap_technical_tokens(text: &str) -> String {
    text.split('\n')
        .map(|line| {
            line.split_whitespace()
                .map(|token| {
                    let core = token.trim_matches(|c: char| ",.;:!?)(".contains(c));
                    if is_technical_token(core) {
                        let prefix_len = token.find(core).unwrap_or(0);
                        let prefix = &token[..prefix_len];
                        let suffix = &token[prefix_len + core.len()..];
                        format!("{prefix}`{core}`{suffix}")
                    } else {
                        token.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Formats normalized transcript text: numerics, sentence punctuation,
/// structural lists, then inline-code wrapping. Always on; deterministic.
pub fn format(text: &str, style: PunctuationStyle) -> String {
    if text.trim().is_empty() {
        return text.to_string();
    }

    let paragraphs: Vec<&str> = text.split("\n\n").collect();
    let formatted: Vec<String> = paragraphs
        .iter()
        .map(|paragraph| format_single(paragraph, style))
        .collect();
    formatted.join("\n\n")
}

fn format_single(text: &str, style: PunctuationStyle) -> String {
    let contracted = crate::audio_toolkit::normalization::apply_safe_contractions(text);
    let deshouted = de_shout(&contracted);
    let numerics = normalize_numerics(&deshouted);

    let commaed: String = numerics
        .split(". ")
        .map(|sentence| coordinate_commas(sentence, style))
        .collect::<Vec<_>>()
        .join(". ");

    let punctuated = crate::audio_toolkit::punctuation::punctuate(&commaed, style);
    // A leading orphan terminal (ASR sometimes opens with ". ") is noise.
    let punctuated = punctuated
        .trim_start_matches(['.', ' ', '!'])
        .trim_start()
        .to_string();
    let sentences: Vec<&str> = split_sentences(&punctuated);

    let structured = numbered_list(&sentences).unwrap_or_else(|| sentences.join(" "));
    wrap_technical_tokens(&structured)
}

/// Common function words ASR shouts in caps; they never stay uppercase.
const DESHOUT_WORDS: &[&str] = &[
    "and", "or", "but", "the", "a", "an", "of", "to", "in", "on", "with", "for", "from", "at",
    "by", "is", "are", "be", "as", "it", "its", "into", "add", "all", "put", "use", "when", "then",
    "keep", "make", "set", "give", "filter", "fetch", "check", "return", "handle", "before",
    "after", "without", "using", "also", "round", "center", "white", "black",
];

/// Rewrites SHOUTED words: known technical acronyms keep their canonical
/// casing via the lexicon; everything else that is fully uppercase and not a
/// catalog term gets lowercased ("AND I'm" → "and I'm").
fn de_shout(text: &str) -> String {
    text.split_whitespace()
        .map(|word| {
            let core = word.trim_matches(|c: char| !c.is_alphanumeric());
            let is_shouted = core.len() > 1
                && core.chars().all(|c| !c.is_lowercase())
                && core.chars().any(|c| c.is_alphabetic());
            if !is_shouted {
                return word.to_string();
            }
            let lowered = core.to_lowercase();
            // Function words de-shout unconditionally — SQL keyword catalogs
            // legitimately contain "AND", but shouted prose never means it.
            if DESHOUT_WORDS.contains(&lowered.as_str()) {
                word.replacen(core, &lowered, 1)
            } else {
                word.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn split_sentences(text: &str) -> Vec<&str> {
    let mut sentences = Vec::new();
    let mut start = 0usize;
    let bytes = text.as_bytes();
    for index in 0..bytes.len() {
        if matches!(bytes[index], b'.' | b'!' | b'?') {
            let next = text[index + 1..].chars().next();
            if next.is_none_or(|c| c.is_whitespace()) {
                let end = (index + 1).min(text.len());
                sentences.push(text[start..end].trim());
                start = end;
            }
        }
    }
    if start < text.len() {
        sentences.push(text[start..].trim());
    }
    sentences
        .into_iter()
        .filter(|sentence| !sentence.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Golden tests: real dictation utterances that failed in production.
    fn full_pipeline(text: &str, style: PunctuationStyle) -> String {
        let corrected = crate::audio_toolkit::tech_lexicon::apply(text);
        let corrected = crate::audio_toolkit::styling::apply(&corrected);
        let corrected = crate::audio_toolkit::programming_syntax::apply(&corrected);
        format(&corrected, style)
    }

    #[test]
    fn golden_tech_stack_paragraph() {
        let out = full_pipeline(
            "I'm using next year's React TypeScript Tailwind CSS TanStack Query on the front end. The backend is Node.js tRPC Prisma PostgreSQL Redis AND I'm also using Docker GitHub Action ZORD shed scene PLAYWRIGHT AND vercel",
            PunctuationStyle::Informal,
        );
        assert!(out.contains("Next.js"), "got: {out}");
        assert!(!out.contains("next year"), "got: {out}");
        assert!(out.contains("Zod"), "got: {out}");
        assert!(out.contains("shadcn/ui"), "got: {out}");
        assert!(out.contains("GitHub Actions"), "got: {out}");
        assert!(out.contains("Playwright"), "got: {out}");
        assert!(!out.contains("PLAYWRIGHT"), "got: {out}");
        assert!(out.contains("Vercel"), "got: {out}");
        assert!(out.contains("React TypeScript"), "got: {out}");
        assert!(out.contains(" and Vercel."), "got: {out}");
    }

    #[test]
    fn golden_tailwind_paragraph() {
        let out = full_pipeline(
            "Use bg stone 700 border neutral 200 rounded 2xl p 6 shadow-md text white on the card. Add hover bg stone 800 and transition colors. Keep everything center and give it a gap of sex.",
            PunctuationStyle::Formal,
        );
        assert!(out.contains("bg-stone-700"), "got: {out}");
        assert!(out.contains("border-neutral-200"), "got: {out}");
        assert!(out.contains("rounded-2xl"), "got: {out}");
        assert!(out.contains("p-6"), "got: {out}");
        assert!(out.contains("hover:bg-stone-800"), "got: {out}");
        assert!(out.contains("gap-6"), "got: {out}");
    }

    #[test]
    fn paragraphs_are_preserved_without_invented_numbering() {
        let out = format(
            "First topic about React.\n\nSecond topic about Docker.",
            PunctuationStyle::Informal,
        );
        assert_eq!(
            out,
            "First topic about React.\n\nSecond topic about Docker."
        );
    }

    #[test]
    fn golden_leading_dot_stripped() {
        let out = full_pipeline(".I'm using React", PunctuationStyle::Informal);
        assert!(!out.starts_with('.'), "got: {out}");
        assert!(out.starts_with("I'm"), "got: {out}");
    }

    #[test]
    fn formats_percentages_currency_and_units() {
        assert_eq!(
            normalize_numerics("it takes two hundred milliseconds"),
            "it takes 200ms"
        );
        assert_eq!(normalize_numerics("twenty percent more"), "20% more");
        assert_eq!(
            normalize_numerics("costs two thousand dollars"),
            "costs $2,000"
        );
        assert_eq!(
            normalize_numerics("add sixteen pixels of padding"),
            "add 16px of padding"
        );
        assert_eq!(normalize_numerics("set it ninety degrees"), "set it 90deg");
    }

    #[test]
    fn formats_decimals_and_large_numbers() {
        assert_eq!(normalize_numerics("one point five rem"), "1.5rem");
        assert_eq!(
            normalize_numerics("we made two hundred thousand dollars"),
            "we made $200,000"
        );
        assert_eq!(
            normalize_numerics("about one million users"),
            "about 1,000,000 users"
        );
        assert_eq!(
            normalize_numerics("the project costs one point five million dollars"),
            "the project costs $1.5 million"
        );
    }

    #[test]
    fn leaves_short_plain_numbers_and_prose_alone() {
        assert_eq!(normalize_numerics("give me five"), "give me five");
        assert_eq!(normalize_numerics("hello world"), "hello world");
        assert_eq!(normalize_numerics("one and done"), "one and done");
        assert_eq!(
            normalize_numerics("two and three dollars"),
            "two and three dollars"
        );
        assert_eq!(normalize_numerics("one two three"), "one two three");
    }

    #[test]
    fn formats_extended_numeric_grammar_without_losing_punctuation() {
        assert_eq!(
            normalize_numerics("two hundreed thousand dollars,"),
            "$200,000,"
        );
        assert_eq!(normalize_numerics("negative twenty five percent"), "-25%");
        assert_eq!(normalize_numerics("point five per cent"), "0.5%");
        assert_eq!(normalize_numerics("one lakh rupees"), "₹1,00,000");
        assert_eq!(normalize_numerics("five gigabytes"), "5 GB");
        assert_eq!(
            normalize_numerics("one trillion dollars"),
            "$1,000,000,000,000"
        );
    }

    #[test]
    fn ten_minute_transcript_formats_within_the_interactive_budget() {
        let paragraph = "please check two hundreed thousand dollars and twenty five percent then preserve src/backend/payment/payment.ts without unrelated changes ";
        let input = paragraph.repeat(100);
        let started = std::time::Instant::now();
        let output = format(&input, PunctuationStyle::Formal);
        assert!(started.elapsed() < std::time::Duration::from_millis(500));
        assert!(output.contains("$200,000"));
        assert!(output.contains("25%"));
        assert!(output.contains("`src/backend/payment/payment.ts`"));
    }

    #[test]
    fn parses_clock_times_with_context() {
        assert_eq!(
            normalize_numerics("meet at three thirty pm"),
            "meet at 3:30 PM"
        );
        // Without am/pm or a time cue, nothing becomes a clock.
        assert_eq!(normalize_numerics("meet at three thirty"), "meet at 3:30");
        assert_eq!(normalize_numerics("give me five"), "give me five");
    }

    #[test]
    fn builds_numbered_lists_from_ordinals() {
        assert_eq!(
            format(
                "first fix the header. second fix the card. third remove the footer.",
                PunctuationStyle::Informal
            ),
            "1. Fix the header.\n2. Fix the card.\n3. Remove the footer."
        );
    }

    #[test]
    fn wraps_technical_token_shapes_in_inline_code() {
        assert_eq!(
            format(
                "make the button bg-stone-600 text white",
                PunctuationStyle::Informal
            ),
            "Make the button `bg-stone-600` text white."
        );
        assert!(format(
            "the path is src/components/button.tsx",
            PunctuationStyle::Informal
        )
        .contains("`src/components/button.tsx`"));
    }

    #[test]
    fn ordinary_prose_is_never_wrapped_or_bloated() {
        assert_eq!(
            format("make it well known", PunctuationStyle::Informal),
            "Make it well known."
        );
    }

    #[test]
    fn empty_input_passthrough() {
        assert_eq!(format("", PunctuationStyle::Formal), "");
    }
}
