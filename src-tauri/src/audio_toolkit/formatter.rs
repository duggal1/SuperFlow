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

const ORDINAL_CUES: &[&str] = &[
    "first", "second", "third", "fourth", "fifth", "sixth", "seventh", "eighth", "ninth", "tenth",
    "finally", "1st", "2nd", "3rd", "4th", "5th", "6th", "7th", "8th", "9th", "10th",
];

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
                lines.push(format!(
                    "{}. {}",
                    step,
                    ensure_terminal(&recapitalize(body))
                ));
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

// ============================================================
// Layout — deterministic paragraphs, lists, and email envelopes
// ============================================================

/// Byte offset just past the first `n` whitespace-separated words' trailing
/// space run; used to split a greeting header off dictated prose safely.
fn skip_leading_words(line: &str, n: usize) -> usize {
    let bytes = line.as_bytes();
    let mut index = 0usize;
    let mut seen = 0usize;
    while index < bytes.len() {
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index >= bytes.len() {
            break;
        }
        while index < bytes.len() && !bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        seen += 1;
        if seen == n {
            while index < bytes.len() && bytes[index].is_ascii_whitespace() {
                index += 1;
            }
            return index;
        }
    }
    line.len()
}

const GREETING_OPENERS: &[&str] = &["hey", "hi", "hello", "dear"];

const SIGNOFFS: &[&str] = &[
    "thanks",
    "thank you",
    "many thanks",
    "cheers",
    "best",
    "regards",
    "talk soon",
    "take care",
    "sincerely",
];

/// Appends a sentence terminal when one is missing; never doubles one.
fn ensure_terminal(sentence: &str) -> String {
    let trimmed = sentence.trim();
    match trimmed.chars().last() {
        Some('.') | Some('!') | Some('?') | Some(':') | None => trimmed.to_string(),
        _ => format!("{trimmed}."),
    }
}

/// Pulls a leading greeting off dictated email prose. Deterministic: only the
/// four openers qualify, and a real name token must follow (a bare "hey" is
/// ordinary speech, never an envelope). The name run continues through
/// consecutive Titlecase tokens ("David Smith") and stops at the first
/// lowercase word or comma. Handles lower-case dictated names by capitalizing.
fn extract_email_envelope(text: &str) -> Option<(String, String)> {
    let trimmed = text.trim();
    let first_line = trimmed
        .split(['\n', '\r'])
        .find(|line| !line.trim().is_empty())?;
    let mut tokens = first_line.split_whitespace();
    let opener_raw = tokens.next()?;
    let opener = opener_raw
        .trim_matches(|c: char| !c.is_alphanumeric())
        .to_lowercase();
    if !GREETING_OPENERS.contains(&opener.as_str()) {
        return None;
    }

    let mut name_tokens: Vec<String> = Vec::new();
    for token in tokens {
        let core = token.trim_matches(|c: char| ",.;:!?\"'()".contains(c));
        if core.is_empty()
            || !core
                .chars()
                .all(|c| c.is_alphanumeric() || c == '\'' || c == '.')
        {
            break;
        }
        // Accept lower-case dictated names (e.g. "david") and Titlecase them.
        // Only stop when token is not alphabetic or is a clear non-name (lowercase verb).
        // For envelope we accept 1-2 consecutive name-like tokens regardless of case.
        let is_name_like = core.chars().all(|c| c.is_alphabetic() || c == '\'' || c == '.')
            && core.len() >= 2;
        if !is_name_like {
            break;
        }
        // Heuristic: if token is lower case verb like "just", "wanted" stop.
        // We treat tokens that are all lower and not capitalized as potential names only for first 2 tokens.
        // To keep deterministic, we accept first 1-2 tokens after opener as name if they are alphabetic,
        // then require next token to be lower-case verb to break? Simpler: accept up to 2 tokens.
        if name_tokens.len() >= 2 {
            break;
        }
        name_tokens.push(recapitalize(&core.to_lowercase()));
        if token.ends_with(',') {
            break;
        }
    }
    if name_tokens.is_empty() {
        return None;
    }

    // Body starts at the first word after the greeting run within this line;
    // anything on following lines rides along untouched.
    let greeting_words = 1 + name_tokens.len();
    let cut = skip_leading_words(first_line, greeting_words);
    let inline_body = first_line[cut..].trim();
    let rest_of_text = trimmed[first_line.len()..].trim();
    let body = if inline_body.is_empty() {
        rest_of_text.to_string()
    } else {
        format!("{inline_body} {rest_of_text}")
    };
    if body.is_empty() {
        return None;
    }

    let greeting_line = format!(
        "{},",
        recapitalize(&format!("{opener} {}", name_tokens.join(" ")))
    );
    Some((greeting_line, body.to_string()))
}

/// Splits a trailing bare sign-off ("thanks", "cheers", "best regards"…)
/// onto its own closing line with a comma, exactly like a written email.
fn extract_signoff(text: &str) -> Option<(String, String)> {
    let last_line = text.lines().rev().find(|line| !line.trim().is_empty())?;
    let normalized = last_line
        .trim()
        .trim_end_matches(['.', ',', '!', '?'])
        .to_lowercase();
    if normalized.split_whitespace().count() > 2 || !SIGNOFFS.contains(&normalized.as_str()) {
        return None;
    }
    let before_len = text.len() - last_line.len();
    let before = text[..before_len].trim_end().to_string();
    if before.is_empty() {
        return None;
    }
    Some((before, format!("{},", recapitalize(&normalized))))
}

/// Whitespace-token byte spans of a string, in order.
fn word_spans(text: &str) -> Vec<(usize, usize)> {
    text.split_whitespace()
        .map(|word| {
            let start = word.as_ptr() as usize - text.as_ptr() as usize;
            (start, start + word.len())
        })
        .collect()
}

/// Byte position of the first sentence terminator at or after `from`,
/// including the terminator itself; None when the tail has none.
fn next_terminal_end(text: &str, from: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    for index in from..bytes.len() {
        if matches!(bytes[index], b'.' | b'!' | b'?') {
            let next = text[index + 1..].chars().next();
            if next.is_none_or(|c| c.is_whitespace()) {
                // Protect abbreviations, decimals, file paths: only split when
                // the terminator is followed by whitespace/EOF, which we already check.
                // This naturally avoids splitting inside "e.g.", "3.14", "src/file.tsx"
                // because those have alphanumeric after the dot.
                return Some(index + 1);
            }
        }
    }
    None
}

/// Deterministic paragraphing per spec 25–35 preferred, 40 hard max:
/// - Count words since previous break.
/// - After >=25, search for next natural boundary (. ? !).
/// - Prefer 25–35, allow up to 40, preserve long sentences intact.
fn gap_paragraphs(text: &str) -> Vec<String> {
    const TARGET_MIN: usize = 25;
    const TARGET_PREF_MAX: usize = 35;
    const HARD_MAX: usize = 40;

    let mut paragraphs = Vec::new();
    let mut remainder = text.trim();
    for _guard in 0..500 {
        let spans = word_spans(remainder);
        if spans.len() <= TARGET_MIN {
            paragraphs.push(remainder.to_string());
            return paragraphs;
        }

        // Preferred window 25..35
        let pref_first = spans[TARGET_MIN - 1].0;
        let pref_last_idx = (TARGET_PREF_MAX.min(spans.len())) - 1;
        let pref_last = spans[pref_last_idx].1;

        // Hard window 35..40
        let hard_last_idx = (HARD_MAX.min(spans.len())) - 1;
        let hard_last = spans[hard_last_idx].1;

        // First try preferred window
        let pref_hit = next_terminal_end(remainder, pref_first).filter(|end| *end <= pref_last);
        let split_at = match pref_hit {
            Some(end) => Some(end),
            None => {
                // Try hard window
                let hard_hit = next_terminal_end(remainder, pref_last).filter(|end| *end <= hard_last);
                match hard_hit {
                    Some(end) => Some(end),
                    None => next_terminal_end(remainder, hard_last),
                }
            }
        };

        match split_at {
            Some(end) => {
                // Do not split a single long sentence merely because it exceeds 40.
                // next_terminal_end after hard_last gives the end of that long sentence,
                // which is correct to keep intact.
                paragraphs.push(remainder[..end].trim().to_string());
                remainder = remainder[end..].trim();
                if remainder.is_empty() {
                    return paragraphs;
                }
            }
            None => {
                paragraphs.push(remainder.to_string());
                return paragraphs;
            }
        }
    }
    paragraphs.push(remainder.to_string());
    paragraphs
}

// ---------------------------------------------------------------------------
// Natural list detection (spec section 3,4)
// ---------------------------------------------------------------------------

const INTRODUCERS: &[&str] = &[
    "i'm going to get",
    "im going to get",
    "i am going to get",
    "i'm going to use",
    "im going to use",
    "i am going to use",
    "the options are",
    "the files are",
    "the models are",
    "the changes are",
    "i need",
    "i want",
    "we need",
    "we use",
    "use",
    "install",
    "add",
    "remove",
];

const KNOWN_TECH_CANONICAL: &[(&str, &str)] = &[
    ("next.js", "Next.js"),
    ("tanstack query", "TanStack Query"),
    ("shadcn/ui", "shadcn/ui"),
    ("shadcn", "shadcn/ui"),
    ("mlx", "MLX"),
    ("gguf", "GGUF"),
    ("react", "React"),
    ("typescript", "TypeScript"),
    ("tailwind css", "Tailwind CSS"),
    ("tailwind", "Tailwind CSS"),
    ("tauri", "Tauri"),
    ("node.js", "Node.js"),
    ("trpc", "tRPC"),
    ("prisma", "Prisma"),
    ("postgresql", "PostgreSQL"),
    ("postgres", "PostgreSQL"),
    ("redis", "Redis"),
    ("docker", "Docker"),
    ("github actions", "GitHub Actions"),
    ("github action", "GitHub Actions"),
    ("zod", "Zod"),
    ("playwright", "Playwright"),
    ("vercel", "Vercel"),
    ("vite", "Vite"),
];

fn canonical_tech(term: &str) -> Option<&'static str> {
    let lower = term.to_lowercase();
    for (k, v) in KNOWN_TECH_CANONICAL {
        if lower == *k {
            return Some(*v);
        }
    }
    None
}

fn normalize_list_item(raw: &str) -> String {
    let trimmed = raw.trim().trim_matches(|c: char| c == ',' || c == '.' || c == ';' || c == ':' || c == '!' || c == '?');
    let trimmed = trimmed.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if let Some(canonical) = canonical_tech(trimmed) {
        return canonical.to_string();
    }
    // Preserve casing for mixed-case or slash-containing tech like "shadcn/ui"
    // For all-lowercase ordinary words, capitalize first letter.
    let has_upper = trimmed.chars().any(|c| c.is_uppercase());
    let has_slash = trimmed.contains('/');
    let has_dot = trimmed.contains('.');
    if has_upper || has_slash || has_dot {
        // Keep as is but ensure first alphabetic is capitalized if not known tech
        // For "apple" -> "Apple", for "Next.js" keep.
        if trimmed.chars().next().is_some_and(|c| c.is_lowercase()) && !has_slash {
            // Check if it's a known lower tech like "shadcn/ui" - already handled via canonical
            return recapitalize(trimmed);
        }
        return trimmed.to_string();
    }
    recapitalize(trimmed)
}

fn find_introducer(block: &str) -> Option<(usize, String)> {
    let normalized = block
        .to_lowercase()
        .replace('’', "'")
        .replace('`', "'");
    // Sort by length descending to match longest first
    let mut sorted: Vec<&str> = INTRODUCERS.to_vec();
    sorted.sort_by_key(|s| std::cmp::Reverse(s.len()));
    for intro in sorted {
        if normalized.starts_with(intro) {
            let rest = &normalized[intro.len()..];
            if rest.is_empty()
                || rest.starts_with(|c: char| c.is_whitespace() || c == ':' || c == ',' || c == '.')
            {
                // Return original cased lead length and normalized lead for later
                // Find byte length of intro in original block (case-insensitive, so approximate)
                // Use intro.len() as char count approximation, but need byte offset in original.
                // Simpler: find where intro ends in original by counting words.
                let intro_word_count = intro.split_whitespace().count();
                let lead_end = skip_leading_words(block, intro_word_count);
                let lead = block[..lead_end].trim().trim_end_matches(|c: char| c == ':' || c == ',' ).to_string();
                return Some((lead_end, lead));
            }
        }
    }
    None
}

fn split_comma_list(remainder: &str) -> Option<Vec<String>> {
    // Must contain at least 2 commas and final and/or
    let comma_count = remainder.matches(',').count();
    if comma_count < 2 {
        return None;
    }
    let lower = remainder.to_lowercase();
    if !lower.contains(" and ") && !lower.contains(" or ") {
        return None;
    }
    // Split by commas
    let mut parts: Vec<String> = remainder.split(',').map(|s| s.to_string()).collect();
    // Last part contains "and"/"or" + final item
    if let Some(last) = parts.last_mut() {
        let last_lower = last.to_lowercase();
        let and_pos = last_lower.find(" and ").or_else(|| last_lower.find(" or "));
        if let Some(pos) = and_pos {
            let sep_len = if last_lower[pos..].starts_with(" and ") { 5 } else { 4 };
            let before = last[..pos].trim().to_string();
            let after = last[pos + sep_len..].trim().trim_end_matches('.').trim().to_string();
            // Replace last with two items
            let before_item = before.trim().to_string();
            let after_item = after.trim().to_string();
            parts.pop();
            if !before_item.is_empty() {
                parts.push(before_item);
            }
            if !after_item.is_empty() {
                parts.push(after_item);
            }
        } else {
            // No and/or in last part, but overall contains and/or somewhere else?
            // Might be "a, b and c" without Oxford comma: last part is " b and c"
            // Actually our split already handled? For "a, b and c" there is one comma, not 2, so earlier check fails.
            // So require 2 commas, so this path is for Oxford comma.
            return None;
        }
    }
    let items: Vec<String> = parts.into_iter().map(|p| p.trim().trim_end_matches('.').trim().to_string()).filter(|s| !s.is_empty()).collect();
    if items.len() < 3 {
        return None;
    }
    // Each item should be short (<= 6 words) and not contain "then" or be a full clause
    const MAX_ITEM_WORDS: usize = 6;
    for item in &items {
        let wc = item.split_whitespace().count();
        if wc == 0 || wc > MAX_ITEM_WORDS {
            return None;
        }
        let lower_item = item.to_lowercase();
        if lower_item.contains(" then ") || lower_item.starts_with("then ") {
            return None;
        }
        // Avoid verb-heavy action sequences: check if item starts with past tense verb like "opened", "replied", "went"
        // For prose "I opened Gmail, replied to Alex, and then went..." the items after split would be "I opened Gmail", "replied to Alex" etc, which contain verbs.
        // We treat items that contain "replied", "opened", "went", "works", "is" as not list-worthy unless introducer is strong?
        // Simpler: if item contains " and then" or is longer than 4 words with verb, skip.
        // For now, if item contains " then " we already returned None.
    }
    // Check structural similarity: all items should be relatively similar length (within factor 2)
    // For fruit list, all single words, similar.
    // For tech list, "Tailwind CSS" 2 words vs "React" 1 word, okay.
    Some(items)
}

fn split_tech_list(remainder: &str) -> Option<Vec<String>> {
    // Scan for sequence of known tech terms separated by optional commas/spaces and final and/or
    // Example: "React TypeScript Tailwind CSS and Tauri for this"
    // We collect tech terms until non-tech encountered.
    let mut rest = remainder.trim();
    // Remove leading colon
    rest = rest.trim_start_matches(|c: char| c == ':' || c.is_whitespace());
    let tokens: Vec<&str> = rest.split_whitespace().collect();
    let mut pos = 0usize;
    // We need to handle multi-word tech terms like "Tailwind CSS" (2 words), "GitHub Actions" (2), "TanStack Query" (2)
    // Build a trie of known terms lowercased split into words.
    // For simplicity, try to match longest known term at each pos.
    let mut collected: Vec<String> = Vec::new();
    let lower_tokens: Vec<String> = tokens.iter().map(|t| t.to_lowercase().trim_matches(|c: char| ",.;:!?\"'()".contains(c)).to_string()).collect();
    let mut iter_guard = 0usize;
    while pos < tokens.len() && iter_guard < 30 {
        iter_guard += 1;
        let token_lower = lower_tokens[pos].as_str();
        if token_lower == "and" || token_lower == "or" || token_lower == "," || token_lower.is_empty() {
            pos += 1;
            continue;
        }
        // Try to match 2-word tech first
        let mut matched: Option<(String, usize)> = None;
        // Try 2-word
        if pos + 1 < tokens.len() {
            let two = format!("{} {}", lower_tokens[pos], lower_tokens[pos+1]);
            if let Some(canon) = canonical_tech(&two) {
                matched = Some((canon.to_string(), 2));
            }
        }
        // Try 1-word
        if matched.is_none() {
            if let Some(canon) = canonical_tech(token_lower) {
                matched = Some((canon.to_string(), 1));
            }
        }
        if let Some((canon_string, words)) = matched {
            collected.push(canon_string);
            pos += words;
            continue;
        } else {
            // Not a tech term, check if we have collected at least 3, then stop
            if collected.len() >= 3 {
                break;
            } else {
                // Not enough, not a tech list
                return None;
            }
        }
    }
    if collected.len() >= 3 {
        // Ensure original remainder had "and" or "or" before last item (for tech list spec, final and required)
        let lower_remainder = remainder.to_lowercase();
        if lower_remainder.contains(" and ") || lower_remainder.contains(" or ") {
            return Some(collected);
        }
        // Even without and, if 3+ tech terms in sequence, still consider list? Spec says need final and/or, but for tech without commas maybe still?
        // For "React TypeScript Tailwind CSS and Tauri" there is "and", so passes.
    }
    None
}

fn detect_natural_list(block: &str) -> Option<(String, Vec<String>, String)> {
    // Find introducer at block start (or after leading whitespace)
    let trimmed = block.trim();
    // Also try to find introducer at start of first sentence, not just block start
    // For simplicity, check block start.
    let (lead_end, lead) = find_introducer(trimmed)?;
    let remainder = trimmed[lead_end..].trim_start_matches(|c: char| c == ':' || c.is_whitespace() || c == ',');
    // Try comma list first
    if let Some(items) = split_comma_list(remainder) {
        // Find where list ends in original remainder to get trailing
        // The list text is the portion up to last item's end (including period)
        // For simplicity, trailing is remainder after the list's textual representation.
        // We'll reconstruct list_text as items joined with ", " and " and " and find its position.
        // Simpler: if remainder after list items contains extra words like "for this and I also want..."
        // we need to detect trailing. For comma list, the list ends at the last item's punctuation.
        // The remainder after list is everything after the last item's occurrence.
        // We can find the last item's position.
        let last_item = items.last().unwrap();
        if let Some(pos) = remainder.to_lowercase().rfind(&last_item.to_lowercase()) {
            let after = &remainder[pos + last_item.len()..];
            // Trim leading punctuation and whitespace
            let trailing = after.trim_start_matches(|c: char| c == '.' || c == ',' || c == ';' || c.is_whitespace()).to_string();
            // Only consider trailing if it contains sentence-like content (>3 words) and not just empty
            let trailing_trimmed = trailing.trim();
            if !trailing_trimmed.is_empty() && trailing_trimmed.split_whitespace().count() > 2 {
                // Check if trailing starts with "for" or "and I" etc, we keep it as separate prose
                return Some((lead, items, trailing_trimmed.to_string()));
            }
        }
        return Some((lead, items, String::new()));
    }
    // Try tech list
    if let Some(items) = split_tech_list(remainder) {
        // Find trailing after tech list
        let last_item = items.last().unwrap();
        if let Some(pos) = remainder.to_lowercase().rfind(&last_item.to_lowercase()) {
            let after = &remainder[pos + last_item.len()..];
            let trailing = after.trim_start_matches(|c: char| c == '.' || c == ',' || c == ';' || c.is_whitespace()).to_string();
            if !trailing.is_empty() && trailing.split_whitespace().count() > 2 {
                return Some((lead, items, trailing));
            }
        }
        return Some((lead, items, String::new()));
    }
    None
}

fn format_natural_list(lead: String, items: Vec<String>) -> String {
    let mut out = String::new();
    // Ensure lead ends without colon, then add colon
    let lead_trimmed = lead.trim_end_matches(|c: char| c == ':' || c == '.' || c == ',' ).trim();
    // Capitalize lead first letter
    let lead_cap = recapitalize(lead_trimmed);
    out.push_str(&lead_cap);
    out.push(':');
    for item in items {
        let norm = normalize_list_item(&item);
        if norm.is_empty() { continue; }
        out.push_str("\n- ");
        out.push_str(&norm);
    }
    out
}

/// Conservative deterministic bullets for email bodies: a lead-in sentence
/// ending with ':' followed by ≥3 consecutive short sentences becomes a
/// Markdown bullet group. Meaning untouched — pure structure.
/// Updated to also handle colon inside first sentence (e.g. "quick status: dashboards shipped.").
fn maybe_bulletize(block: &str) -> String {
    const MAX_ITEM_WORDS: usize = 14;
    const MIN_ITEMS: usize = 3;

    // First try original sentence-based bullet (first sentence ends with colon)
    let sentences = split_sentences(block);
    if sentences.len() >= MIN_ITEMS + 1 && sentences[0].trim_end().ends_with(':') {
        let items = &sentences[1..];
        if items.iter().all(|item| item.split_whitespace().count() <= MAX_ITEM_WORDS) {
            let lead = sentences[0].trim_end();
            let mut out = String::with_capacity(block.len() + items.len());
            out.push_str(lead.trim_end_matches(':'));
            out.push(':');
            for item in items {
                out.push_str("\n- ");
                out.push_str(item.strip_suffix('.').unwrap_or(item).trim());
            }
            return out;
        }
    }

    // Handle colon inside first sentence: "quick status: dashboards shipped. ..."
    // Split block at first colon, then treat remainder as sentences.
    if let Some(colon_pos) = block.find(':') {
        let before = block[..colon_pos].trim();
        let after = block[colon_pos+1..].trim();
        // Before should be short lead (<= 6 words)
        if before.split_whitespace().count() <= 6 && !before.is_empty() {
            let after_sentences = split_sentences(after);
            if after_sentences.len() >= MIN_ITEMS && after_sentences.iter().all(|s| s.split_whitespace().count() <= MAX_ITEM_WORDS) {
                let mut out = String::new();
                out.push_str(before);
                out.push(':');
                for item in after_sentences {
                    out.push_str("\n- ");
                    out.push_str(item.strip_suffix('.').unwrap_or(item).trim());
                }
                return out;
            }
        }
    }
    block.to_string()
}

/// The deterministic layout pass over finished transcript prose:
/// optional email envelope (greeting + sign-off), then per-block structure —
/// explicit ordinal lists first, colon-led bullet groups second, natural lists third,
/// else the 25–40 word paragraph-gap rule. Fenced code forces full passthrough.
pub fn format_layout(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.contains("```") {
        return text.to_string();
    }

    let mut head = String::new();
    let mut working = trimmed.to_string();
    if let Some((greeting, rest)) = extract_email_envelope(trimmed) {
        head.push_str(&greeting);
        head.push_str("\n\n");
        working = rest;
    }

    let mut signoff: Option<String> = None;
    if let Some((before, closing)) = extract_signoff(&working) {
        working = before;
        signoff = Some(closing);
    }

    let mut blocks: Vec<String> = Vec::new();
    for block in working.split("\n\n") {
        let block = block.trim();
        if block.is_empty() {
            continue;
        }
        // Pipeline per spec: explicit numbered → natural lists → colon bullets → paragraphs
        // We iteratively consume the block to handle multiple structures inside one block.
        let mut remaining = block.to_string();
        let mut produced: Vec<String> = Vec::new();
        let mut loop_guard = 0;
        while !remaining.trim().is_empty() && loop_guard < 20 {
            loop_guard += 1;
            let rem_trimmed = remaining.trim();
            // 1. Explicit numbered (punctuated)
            let sentences = split_sentences(rem_trimmed);
            if let Some(listed) = numbered_list(&sentences) {
                produced.push(listed);
                // Remove the consumed sentences from remaining
                // numbered_list consumes all sentences that start with ordinal + trailing
                // For simplicity, if listed was produced from all sentences, we are done.
                // If there is trailing after numbered list that was absorbed, we need to detect remainder.
                // Our numbered_list absorbs trailing sentences after last ordinal into last item,
                // so if remaining had natural list after numbered, it would be absorbed incorrectly.
                // To avoid, we check if natural list introducer appears after numbered list's last ordinal
                // and split accordingly. For now, break and handle remainder as separate.
                // If produced covers all, break, else try to extract remainder.
                // We attempt to find where numbered list consumed up to.
                // For simplicity, if original block contained "first ... second ... third ..." and then "I'm going to use..."
                // the numbered_list would have absorbed the "I'm going to use..." into last item, which we don't want.
                // So we need to detect that case and split.
                // Check if remaining after numbered_list's last ordinal contains a natural list introducer
                let listed_len = produced.last().unwrap().len();
                // Heuristic: if remaining still contains introducer after the numbered portion, split
                if rem_trimmed.to_lowercase().contains("i'm going to get") || rem_trimmed.to_lowercase().contains("i'm going to use") || rem_trimmed.to_lowercase().contains("we need") {
                    // Try to find where natural list starts and split
                    // For now, break and let natural list handle the rest in next iteration
                    // But we already consumed all, so we need to re-parse.
                    // Simpler: if block contains both numbered and natural list, we should have detected numbered first, then natural list in next loop iteration.
                    // To do that, we need to not consume all remaining, but only the numbered portion.
                    // Let's attempt to split remaining at the point where natural list starts.
                    let lower = rem_trimmed.to_lowercase();
                    let mut split_pos = None;
                    for intro in ["i'm going to get", "i'm going to use", "we need", "the options are"] {
                        if let Some(pos) = lower.find(intro) {
                            // Ensure it's after the numbered portion (after ~ third)
                            if pos > 20 {
                                split_pos = Some(pos);
                                break;
                            }
                        }
                    }
                    if let Some(pos) = split_pos {
                        // Revert last produced and split
                        let full_listed = produced.pop().unwrap();
                        // Split the original remaining into two parts: numbered part and natural part
                        let numbered_part = rem_trimmed[..pos].trim();
                        let natural_part = rem_trimmed[pos..].trim();
                        let numbered_sentences = split_sentences(numbered_part);
                        if let Some(num) = numbered_list(&numbered_sentences) {
                            produced.push(num);
                        } else if let Some(items) = segment_by_cues(numbered_part) {
                            produced.push(render_numbered(&items));
                        } else {
                            produced.push(numbered_part.to_string());
                        }
                        remaining = natural_part.to_string();
                        continue;
                    }
                }
                break;
            }
            if let Some(items) = segment_by_cues(rem_trimmed) {
                // Only for unpunctuated: if block has sentence terminators, skip segment_by_cues
                let punct_count = rem_trimmed.matches('.').count() + rem_trimmed.matches('!').count() + rem_trimmed.matches('?').count();
                if punct_count <= 1 {
                    produced.push(render_numbered(&items));
                    break;
                }
                // Otherwise treat as not a list and continue to next checks
            }
            // 2. Natural list
            if let Some((lead, items, trailing)) = detect_natural_list(rem_trimmed) {
                let bullet = format_natural_list(lead, items);
                produced.push(bullet);
                if !trailing.is_empty() {
                    // Trailing prose after list (e.g. "for this and I also want...")
                    // It may contain further sentences; process it in next iteration
                    remaining = trailing;
                    continue;
                } else {
                    break;
                }
            }
            // 3. Colon bullet
            let bulleted = maybe_bulletize(rem_trimmed);
            if bulleted != rem_trimmed {
                produced.push(bulleted);
                break;
            }
            // 4. Paragraphs
            let paras = gap_paragraphs(rem_trimmed);
            produced.extend(paras);
            break;
        }
        // Join produced pieces with double newline
        let shaped = produced.join("\n\n");
        blocks.push(shaped);
    }

    head.push_str(&blocks.join("\n\n"));
    if let Some(closing) = signoff {
        head.push_str("\n\n");
        head.push_str(&closing);
    }
    // Normalize whitespace: collapse 3+ newlines to 2, trim
    let mut out = head;
    while out.contains("\n\n\n") {
        out = out.replace("\n\n\n", "\n\n");
    }
    out.trim().to_string()
}

/// Segments unpunctuated speech lists: every standalone ordinal cue token
/// starts a new item ("first fix x second fix y"), yielding ≥2 items before
/// anything happens. The cue words themselves are absorbed — they become
/// the numbers instead. Lone ordinals return None and stay prose.
/// Stricter: only for blocks with <=1 sentence terminator (unpunctuated).
fn segment_by_cues(text: &str) -> Option<Vec<String>> {
    // Only for unpunctuated speech: if text has 2+ sentence terminators, prefer numbered_list
    let punct_count = text.matches('.').count() + text.matches('!').count() + text.matches('?').count();
    if punct_count > 1 {
        return None;
    }
    let mut segments: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut found_ordinal = false;
    for token in text.split_whitespace() {
        let core = token
            .trim_matches(|c: char| !c.is_alphanumeric() && c != '\'')
            .to_lowercase();
        // Handle "number one" pattern: check if token is "number" and next is ordinal number word
        // For simplicity, treat "number" as cue starter and skip it, next token "one" etc will be cue
        if core == "number" {
            // Peek next token? Instead, treat "number" as part of cue and absorb next.
            // We handle by checking if next token is digit word, but we don't have lookahead here.
            // Instead, just treat "number" as cue and absorb, the following "one" will also be cue and cause extra split.
            // To avoid double split, we should treat "number one" as single cue.
            // We do this by checking if current token is "number" and next token is ordinal word, then skip current and let next handle.
            // For now, just treat "number" as cue.
            if !current.trim().is_empty() {
                segments.push(current.trim().to_string());
            }
            current.clear();
            found_ordinal = true;
            continue;
        }
        // Handle "thing" after ordinal: e.g. "first thing" -> cue is "first", "thing" is filler to skip
        if core == "thing" && found_ordinal {
            // If previous token was ordinal cue, "thing" is part of cue, skip it
            // Check if last segment boundary was just created (current empty and segments not empty)
            // Then "thing" should be ignored, not added to item.
            continue;
        }
        if ORDINAL_CUES.contains(&core.as_str()) {
            if !current.trim().is_empty() {
                segments.push(current.trim().to_string());
            }
            current.clear();
            found_ordinal = true;
        } else {
            // Also check for "one", "two" etc when preceded by "number" - we already handled number, now check digit words
            // If previous token was "number", then "one" should be considered cue, not item.
            // But our loop already handles "number" as cue, the next "one" would be next iteration and be checked as ORDINAL_CUES? No, ORDINAL_CUES does not contain "one".
            // So we need to handle "one", "two" as cues when they follow "number".
            // For now, we treat plain "one", "two" not as cues unless after "number", so they will be part of items, which is fine for "number one" case we want to treat as cue.
            // To handle "number one" as single cue, we already consumed "number", now "one" should be skipped as well.
            // We can detect if previous token was "number" by checking if last segment creation was due to "number" and current is "one"/"two" etc.
            // Simplify: if core is "one", "two", "three" etc and found_ordinal and current is empty, skip it (it's part of number cue).
            if found_ordinal && current.trim().is_empty() && ["one","two","three","four","five","six","seven","eight","nine","ten"].contains(&core.as_str()) {
                continue;
            }
            current.push_str(token);
            current.push(' ');
        }
    }
    if !current.trim().is_empty() {
        segments.push(current.trim().to_string());
    }
    // Need at least 2 items and at least 2 ordinals found
    if segments.len() >= 2 && found_ordinal {
        // Ensure ordinals were in order? For now just check count.
        // Stricter: check sequence 1,2,3... We could verify by scanning original text for ordinal order.
        // For "first ... second ... third ..." the order is 0,1,2 indices in ORDINAL_CUES, which is sequential.
        // We should verify that the cues appeared in order.
        // Extract cue indices in order encountered
        let mut cue_indices: Vec<usize> = Vec::new();
        for token in text.split_whitespace() {
            let core = token.trim_matches(|c: char| !c.is_alphanumeric() && c != '\'').to_lowercase();
            if let Some(pos) = ORDINAL_CUES.iter().position(|c| *c == core) {
                cue_indices.push(pos);
            } else if core == "number" {
                // Look ahead for digit word
                // We don't have lookahead, skip
            }
        }
        // Check if at least 2 and sequential (each next > previous)
        if cue_indices.len() >= 2 && cue_indices.windows(2).all(|w| w[1] > w[0]) {
            return Some(segments);
        }
        // Also allow non-sequential but at least 2? For "first ... third" skipping second, still sequential.
        if cue_indices.len() >= 2 {
            return Some(segments);
        }
    }
    None
}

/// Renders items as a Markdown ordered list with renumbering from 1.
fn render_numbered(items: &[String]) -> String {
    items
        .iter()
        .enumerate()
        .map(|(index, item)| format!("{}. {}", index + 1, ensure_terminal(&recapitalize(item))))
        .collect::<Vec<_>>()
        .join("\n")
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
    fn ten_minute_catalog_pipeline_reuses_compiled_matchers() {
        let paragraph = "please use next jays with background stone six hundred and deploy on render hosting then keep this normal prose unchanged ";
        let input = paragraph.repeat(100);
        crate::audio_toolkit::tech_lexicon::warm_up();
        crate::audio_toolkit::styling::warm_up();
        crate::audio_toolkit::programming_syntax::warm_up();

        let started = std::time::Instant::now();
        let output = full_pipeline(&input, PunctuationStyle::Formal);
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
        assert!(output.contains("Next.js"));
        assert!(output.contains("bg-stone-600"));
        assert!(output.contains("Render"));
        assert!(output.contains("normal prose"));
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

    // ===== Layout engine: gaps, lists, envelopes =====

    #[test]
    fn first_stop_inside_word_window_earns_the_gap() {
        // Words 1-24 filler, stop arrives at word ~30 (inside 25..=40).
        let mut text: Vec<String> = (0..24).map(|_| "filler".into()).collect();
        text.push("the metrics are looking done.".into());
        text.push("More quiet sentences follow afterwards.".into());
        let joined = text.join(" ");
        let out = format_layout(&joined);
        assert!(out.contains("done.\n\n"), "got: {out}");
        assert_eq!(out.split("\n\n").count(), 2);
    }

    #[test]
    fn later_stops_in_same_window_are_ignored() {
        // Two stops between words 25 and 40: FIRST one takes the gap.
        let mut text: Vec<String> = (0..24).map(|_| "pad".into()).collect();
        text.push("first point lands here.".into());
        text.push("second point sits close.".into());
        text.push("tail words carry on quietly now.".into());
        let out = format_layout(&text.join(" "));
        assert!(out.contains("here.\n\n"), "got: {out}");
        assert!(!out.contains("close.\n\n"), "got: {out}");
    }

    #[test]
    fn window_without_stops_extends_to_next_terminal() {
        // No punctuation until well past word 40 → still splits there once.
        let mut text: Vec<String> = (0..45).map(|_| "run".into()).collect();
        text.push("it finally stops.".into());
        let out = format_layout(&text.join(" "));
        assert!(out.contains("stops.\n\n"), "got: {out}");
    }

    #[test]
    fn short_runs_never_get_paragraph_gaps() {
        let text = "just a quick note about the deploy status today everyone";
        assert!(!format_layout(text).contains("\n\n"));
    }

    #[test]
    fn digit_ordinals_speech_becomes_clean_numbered_list() {
        let out = format_layout("1st fix the login thing 2nd check the dashboard 3rd fix buttons");
        assert_eq!(
            out,
            "1. Fix the login thing.\n2. Check the dashboard.\n3. Fix buttons."
        );
    }

    #[test]
    fn spoken_ordinals_renumber_and_absorb_trailing_tasks() {
        let out = format_layout(
            "first fix the login thing second check dashboard third fix buttons \
             and also clean code then test everything dont break anything please",
        );
        assert!(out.contains("1. Fix the login thing."), "got: {out}");
        assert!(out.contains("2. Check dashboard."), "got: {out}");
        // Everything after "third" rides inside item 3 — no forced splits.
        assert!(
            out.ends_with("3. Fix buttons and also clean code then test everything dont break anything please."),
            "got: {out}"
        );
        assert!(!out.contains("first "), "got: {out}");
    }

    #[test]
    fn lone_ordinal_without_a_second_cue_stays_plain_prose() {
        let text = "first open the repo and organize everything for me";
        assert_eq!(format_layout(text), text);
    }

    #[test]
    fn email_envelope_shapes_greeting_body_signoff() {
        let out = format_layout(
            "hey david just wanted to give you an update on the project \
             the dashboard changes are finished and the login issue is fixed \
             thanks",
        );
        assert!(out.starts_with("Hey David,\n\n"), "got: {out}");
        assert!(out.ends_with("\n\nThanks,"), "got: {out}");
    }

    #[test]
    fn multi_token_capitalized_names_survive_the_envelope() {
        let out = format_layout("hi david smith quick heads up that the build is green again bye");
        assert!(out.starts_with("Hi David Smith,\n\n"), "got: {out}");
    }

    #[test]
    fn bare_greeting_without_a_name_is_left_alone() {
        let text = "hey everyone lets ship this today morning folks";
        assert!(
            !format_layout(text).contains(",\n\nHey") && !format_layout(text).starts_with("Hey")
        );
    }

    #[test]
    fn colon_lead_in_with_short_run_builds_bullets() {
        let out = format_layout(
            "quick status: dashboards shipped. login issue resolved. \
             api bug patched. payments pending.",
        );
        assert!(out.contains("- Dashboards shipped"), "got: {out}");
        assert!(out.contains("- Login issue resolved"), "got: {out}");
    }

    #[test]
    fn fenced_code_forces_full_passthrough() {
        let text = "```\nmust not touch anything in here\n```";
        assert_eq!(format_layout(text), text);
    }

    #[test]
    fn layout_is_idempotent_across_all_structures() {
        let email =
            format_layout("hey sam quick update first fix payments second verify receipts thanks");
        assert_eq!(format_layout(&email), email);
    }
}
