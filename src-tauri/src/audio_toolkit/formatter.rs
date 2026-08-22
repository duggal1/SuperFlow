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

fn unit_value(word: &str) -> Option<u64> {
    UNITS.iter().position(|u| *u == word).map(|i| i as u64)
}

fn tens_value(word: &str) -> Option<u64> {
    TENS.iter().position(|t| *t == word).map(|i| i as u64 * 10)
}

/// A parsed spoken number. `scale` records the last large multiplier so a
/// spoken fraction can be rendered compactly ("one point five million").
#[derive(Debug)]
struct SpokenNumber {
    value: u64,
    base: u64,
    scale: Option<(&'static str, u64)>,
    used: usize,
}

fn parse_spoken_integer(words: &[&str], start: usize) -> Option<SpokenNumber> {
    let mut total: u64 = 0;
    let mut current: u64 = 0;
    let mut scale: Option<(&'static str, u64)> = None;
    let mut base: u64 = 0;
    let mut saw_digit = false;
    let mut used = 0usize;

    let mut index = start;
    while index < words.len() {
        let word = clean(words[index]);
        if word.is_empty() {
            break;
        }

        if let Some(value) = unit_value(&word).filter(|v| *v > 0) {
            current += value;
            saw_digit = true;
        } else if let Some(value) = tens_value(&word) {
            current += value;
            saw_digit = true;
        } else {
            let factor = match word.as_str() {
                "hundred" => Some((100u64, false)),
                "thousand" => Some((1_000, true)),
                "million" => Some((1_000_000, true)),
                "billion" => Some((1_000_000_000, true)),
                _ => None,
            };
            let Some((factor, is_big)) = factor else {
                break;
            };
            if !saw_digit {
                break;
            }
            if is_big {
                base = current.max(1);
                total += current.max(1) * factor;
                current = 0;
                scale = Some((
                    match word.as_str() {
                        "thousand" => "thousand",
                        "million" => "million",
                        _ => "billion",
                    },
                    factor,
                ));
            } else {
                current = current.max(1) * factor;
            }
            saw_digit = true;
        }
        index += 1;
        used += 1;

        // "and" continues the phrase ("one hundred and five").
        if clean(words.get(index).copied().unwrap_or("")) == "and" {
            index += 1;
            used += 1;
        }
    }

    if !saw_digit {
        return None;
    }
    Some(SpokenNumber {
        value: total + current,
        base,
        scale,
        used,
    })
}

/// Consumes spoken fraction digits starting at the word "point":
/// "point one four" → digits "14".
fn parse_decimal_digits(words: &[&str], start: usize) -> Option<(String, usize)> {
    if clean(words.get(start)?) != "point" {
        return None;
    }
    let mut digits = String::new();
    let mut used = 1usize;
    let mut index = start + 1;
    while index < words.len() {
        let word = clean(words[index]);
        if let Some(digit) = unit_value(&word) {
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

fn group_digits(value: u64) -> String {
    let raw = value.to_string();
    let bytes = raw.as_bytes();
    let mut grouped = String::new();
    for (index, byte) in bytes.iter().enumerate() {
        if index > 0 && (bytes.len() - index) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(*byte as char);
    }
    grouped
}

fn currency_symbol(word: &str) -> Option<&'static str> {
    Some(match word {
        "dollar" | "dollars" | "usd" => "$",
        "euro" | "euros" | "eur" => "€",
        "pound" | "pounds" | "gbp" => "£",
        "rupee" | "rupees" | "inr" => "₹",
        "yen" | "jpy" => "¥",
        "won" => "₩",
        _ => return None,
    })
}

fn technical_unit(word: &str) -> Option<&'static str> {
    Some(match word {
        "pixel" | "pixels" | "px" => "px",
        "rem" | "rems" => "rem",
        "millisecond" | "milliseconds" | "ms" => "ms",
        "second" | "seconds" => "s",
        "degree" | "degrees" | "deg" => "deg",
        _ => return None,
    })
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

        if let Some(number) = parse_spoken_integer(&words, index) {
            let after_number = index + number.used;
            let mut used = number.used;

            let mut fraction: Option<String> = None;
            if clean(words.get(after_number).copied().unwrap_or("")) == "point" {
                if let Some((digits, fraction_used)) = parse_decimal_digits(&words, after_number) {
                    fraction = Some(digits);
                    used += fraction_used;
                }
            }

            // A spoken magnitude may follow the fraction ("one point five
            // million"); fold it into a compact rendering.
            let mut scale_name: Option<&str> = number.scale.map(|(name, _)| name);
            let mut scale_consumed = 0usize;
            if fraction.is_some() {
                let after_fraction = clean(words.get(index + used).copied().unwrap_or(""));
                if matches!(after_fraction.as_str(), "thousand" | "million" | "billion") {
                    scale_name = Some(match after_fraction.as_str() {
                        "thousand" => "thousand",
                        "million" => "million",
                        _ => "billion",
                    });
                    scale_consumed = 1;
                }
            }

            let follower_index = index + used + scale_consumed;
            let follower = clean(words.get(follower_index).copied().unwrap_or(""));
            let number_text = match (&fraction, scale_name) {
                (Some(digits), Some(name)) => {
                    format!("{}.{} {}", number.base.max(1), digits, name)
                }
                (Some(digits), None) => format!("{}.{}", number.value, digits),
                (None, _) => group_digits(number.value),
            };
            used += scale_consumed;

            let pushed = if let Some(symbol) = currency_symbol(&follower) {
                out.push(format!("{symbol}{number_text}"));
                used += 1;
                true
            } else if let Some(unit) = technical_unit(&follower) {
                out.push(format!("{number_text}{unit}"));
                used += 1;
                true
            } else if follower.starts_with("percent") {
                out.push(format!("{number_text}%"));
                used += 1;
                true
            } else if follower_index + 1 < words.len() {
                let next = clean(words[follower_index]);
                let after = clean(words[follower_index + 1]);
                match viewport_unit(&next, &after) {
                    Some(unit) => {
                        out.push(format!("{number_text}{unit}"));
                        used += 2;
                        true
                    }
                    None => false,
                }
            } else {
                false
            };

            if pushed {
                index += used;
                continue;
            }

            // Standalone numbers stay as words unless multi-word or large.
            if used >= 2 || number.value >= 1_000 {
                out.push(number_text);
                index += used;
                continue;
            }
        }

        out.push(words[index].to_string());
        index += 1;
    }

    out.join(" ")
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
    let mut chars = text.char_indices();
    while let Some((index, ch)) = chars.next() {
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
                .map(|word| clean(word))
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

    let numerics = normalize_numerics(text);

    let commaed: String = numerics
        .split(". ")
        .map(|sentence| coordinate_commas(sentence, style))
        .collect::<Vec<_>>()
        .join(". ");

    let punctuated = crate::audio_toolkit::punctuation::punctuate(&commaed, style);
    let sentences: Vec<&str> = split_sentences(&punctuated);

    let structured = numbered_list(&sentences).unwrap_or_else(|| sentences.join(" "));
    wrap_technical_tokens(&structured)
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

#[cfg(test)]
mod debug_probe {
    use super::*;
    #[test]
    fn probe() {
        let words: Vec<&str> = "the project costs one point five million dollars"
            .split_whitespace()
            .collect();
        eprintln!(
            "P1 numerics: {:?}",
            normalize_numerics("the project costs one point five million dollars")
        );
        let n = parse_spoken_integer(&words, 3);
        eprintln!(
            "P2 int: {:?}",
            n.as_ref().map(|n| (n.value, n.base, n.used, n.scale))
        );
        eprintln!(
            "P3 time: {:?}",
            try_parse_time(
                &"meet at three thirty pm"
                    .split_whitespace()
                    .collect::<Vec<_>>(),
                2
            )
        );
    }
}
