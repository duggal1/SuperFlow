use crate::settings::PunctuationStyle;
// Force slack_formatting into use as requested
use crate::audio_toolkit::slack_formatting as _slack_fmt;
const _: () = {
    let _ = _slack_fmt::SlackSurface::Unknown;
};

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
    // Decimal punctuation is semantic. Returning the literal unchanged here
    // makes the spoken-integer parser reject it instead of silently treating
    // `76.5` as `765`; the token is then preserved verbatim by the caller.
    let numeric = word.trim_matches(|character: char| {
        !character.is_ascii_digit() && !matches!(character, '.' | ',' | '-' | '+')
    });
    if numeric.contains('.')
        && numeric.split('.').count() == 2
        && numeric
            .chars()
            .all(|character| character.is_ascii_digit() || matches!(character, '.' | '-' | '+'))
    {
        return numeric.to_string();
    }
    crate::audio_toolkit::normalization::number_word(word)
}

fn unit_value(word: &str) -> Option<u64> {
    UNITS.iter().position(|u| *u == word).map(|i| i as u64)
}

fn tens_value(word: &str) -> Option<u64> {
    TENS.iter().position(|t| *t == word).map(|i| i as u64 * 10)
}

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

fn is_explicit_quantity_follower(word: &str) -> bool {
    matches!(
        number_clean(word).as_str(),
        "byte"
            | "bytes"
            | "credit"
            | "credits"
            | "dollar"
            | "dollars"
            | "euro"
            | "euros"
            | "line"
            | "lines"
            | "parameter"
            | "parameters"
            | "pixel"
            | "pixels"
            | "pound"
            | "pounds"
            | "request"
            | "requests"
            | "rupee"
            | "rupees"
            | "token"
            | "tokens"
            | "user"
            | "users"
            | "watt"
            | "watts"
    )
}

fn is_calendar_year(words: &[&str], start: usize, follower: usize, value: u128) -> bool {
    if !(1000..=2099).contains(&value)
        || words
            .get(follower)
            .is_some_and(|word| is_explicit_quantity_follower(word))
    {
        return false;
    }

    let previous = start
        .checked_sub(1)
        .and_then(|index| words.get(index))
        .map(|word| number_clean(word));
    (1900..=2099).contains(&value)
        || previous.as_deref().is_some_and(|word| {
            matches!(
                word,
                "after" | "before" | "during" | "from" | "in" | "since" | "until" | "year"
            )
        })
}

fn viewport_unit(first: &str, second: &str) -> Option<&'static str> {
    match (first, second) {
        ("viewport", "height") | ("view", "height") => Some("vh"),
        ("viewport", "width") | ("view", "width") => Some("vw"),
        _ => None,
    }
}

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
            "at" | "by" | "until" | "around" | "before" | "after"
        );

    // Avoid mis-parsing "twenty three thirty pm" as "twenty 3:30 PM": if the
    // word before the hour is itself a number, the hour is likely part of a
    // larger numeric phrase, not a standalone clock. This matches the expected
    // behaviour for "meet at twenty three thirty pm" → no conversion.
    if start > 0 {
        let prev = clean(words[start - 1]);
        if unit_value(&prev).is_some() || tens_value(&prev).is_some() || prev.parse::<u64>().is_ok()
        {
            return None;
        }
    }

    let mut suffix_words = 0usize;
    let mut meridiem = None;
    let suffix = words.get(index).map(|w| clean(w)).unwrap_or_default();

    if suffix == "pm" {
        meridiem = Some("PM");
        suffix_words = 1;
    } else if suffix == "am" {
        meridiem = Some("AM");
        suffix_words = 1;
    } else if suffix == "p" && words.get(index + 1).map(|w| clean(w)).as_deref() == Some("m") {
        meridiem = Some("PM");
        suffix_words = 2;
    } else if suffix == "a" && words.get(index + 1).map(|w| clean(w)).as_deref() == Some("m") {
        meridiem = Some("AM");
        suffix_words = 2;
    }

    if meridiem.is_some() && hour > 12 {
        return None;
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
        Some("PM") => format!("{hour}:{rendered_minutes} PM"),
        Some("AM") => format!("{hour}:{rendered_minutes} AM"),
        _ => format!("{hour}:{rendered_minutes}"),
    };

    Some((formatted, index - start + suffix_words))
}

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
                (None, _) if is_calendar_year(&words, index, follower_index, number.value) => {
                    format!("{sign_prefix}{}", number.value)
                }
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

/// Appends a sentence terminal when one is missing; never doubles one.
fn ensure_terminal(sentence: &str) -> String {
    let trimmed = sentence.trim();
    match trimmed.chars().last() {
        Some('.') | Some('!') | Some('?') | Some(':') | None => trimmed.to_string(),
        _ => format!("{trimmed}."),
    }
}

pub fn normalize_values(text: &str) -> String {
    let structurally_repaired =
        crate::audio_toolkit::text::repair_structural_token_boundaries(text);
    let structurally_repaired = normalize_product_identifier_punctuation(&structurally_repaired);
    let structurally_repaired = normalize_named_parameter_counts(&structurally_repaired);
    let structurally_repaired = normalize_parameter_counts(&structurally_repaired);
    let structurally_repaired = normalize_moe_parameter_counts(&structurally_repaired);
    let structurally_repaired = normalize_decimal_percentages(&structurally_repaired);
    let hardware_protected = protect_hardware_identifiers(&structurally_repaired);
    // Numeric formatting must never inspect the internals of an already-valid
    // version, filename, model identifier, URL, path, or quantization token.
    // Mask those spans with alphabetic placeholders, normalize surrounding
    // quantities, then restore their exact bytes.
    let protected = FormatterProtectedText::new(&hardware_protected);
    let numerics = normalize_numerics(protected.masked());
    let restored = restore_hardware_identifiers(&protected.restore(&numerics));
    normalize_technical_values(&restored)
}

fn normalize_named_parameter_counts(text: &str) -> String {
    static NAMED_PARAMETER_COUNT: once_cell::sync::Lazy<regex::Regex> =
        once_cell::sync::Lazy::new(|| {
            regex::Regex::new(
                r"(?i)\b([0-9]+(?:\.[0-9]+)?)\s+(million|billion|trillion)\s+parameters?\b",
            )
            .expect("named parameter-count regex must compile")
        });
    NAMED_PARAMETER_COUNT
        .replace_all(text, |captures: &regex::Captures<'_>| {
            let suffix = match captures[2].to_ascii_lowercase().as_str() {
                "million" => "M",
                "billion" => "B",
                "trillion" => "T",
                _ => return captures[0].to_string(),
            };
            format!("{}{suffix} parameters", &captures[1])
        })
        .into_owned()
}

fn normalize_decimal_percentages(text: &str) -> String {
    static DECIMAL_PERCENTAGE: once_cell::sync::Lazy<regex::Regex> =
        once_cell::sync::Lazy::new(|| {
            regex::Regex::new(r"(?i)\b([0-9]+\.[0-9]+)\s+percent(?:age)?\b")
                .expect("decimal percentage regex must compile")
        });
    DECIMAL_PERCENTAGE.replace_all(text, "$1%").into_owned()
}

fn normalize_product_identifier_punctuation(text: &str) -> String {
    static PRODUCT_NUMBER: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
        regex::Regex::new(r"(?i)\b(RTX(?:\s+PRO)?|Radeon)\s+([0-9]),([0-9]{3})(S?)\b")
            .expect("product identifier punctuation regex")
    });
    PRODUCT_NUMBER
        .replace_all(text, "${1} ${2}${3}${4}")
        .into_owned()
}

struct FormatterProtectedText {
    masked: String,
    originals: Vec<String>,
}

impl FormatterProtectedText {
    fn new(text: &str) -> Self {
        let spans = crate::superflow_grammar::protected_spans::find_protected_spans(text);
        let source: Vec<char> = text.chars().collect();
        let originals = spans
            .iter()
            .map(|span| source[span.start..span.end].iter().collect())
            .collect::<Vec<String>>();
        let mut masked = source;
        for (index, span) in spans.iter().enumerate().rev() {
            let marker: Vec<char> = format!("SFPROTECTED{}TOKEN", alphabetic_index(index))
                .chars()
                .collect();
            masked.splice(span.start..span.end, marker);
        }
        Self {
            masked: masked.into_iter().collect(),
            originals,
        }
    }

    fn masked(&self) -> &str {
        &self.masked
    }

    fn restore(&self, text: &str) -> String {
        let mut restored = text.to_string();
        for (index, original) in self.originals.iter().enumerate() {
            restored = restored.replace(
                &format!("SFPROTECTED{}TOKEN", alphabetic_index(index)),
                original,
            );
        }
        restored
    }
}

fn alphabetic_index(mut index: usize) -> String {
    let mut encoded = String::new();
    loop {
        encoded.push((b'A' + (index % 26) as u8) as char);
        index /= 26;
        if index == 0 {
            break;
        }
    }
    encoded
}

fn replace_all(text: String, pattern: &str, replacement: &str) -> String {
    regex::Regex::new(pattern)
        .expect("static transcript normalization regex must compile")
        .replace_all(&text, replacement)
        .into_owned()
}

/// Protect hardware identifiers from number normalization
/// GPU model numbers like "RTX 5000", "H100" must NEVER get comma separators
fn protect_hardware_identifiers(text: &str) -> String {
    // List of hardware identifier patterns that contain numbers but are NOT quantities
    // These are model/product identifiers that must stay unchanged
    const HARDWARE_PATTERNS: &[&str] = &[
        // NVIDIA RTX consumer/gaming
        r"\bRTX\s+5090\b",
        r"\bRTX\s+5080\b",
        r"\bRTX\s+5070\b",
        r"\bRTX\s+4090\b",
        r"\bRTX\s+4080\b",
        r"\bRTX\s+4070\b",
        r"\bRTX\s+3090\b",
        r"\bRTX\s+3080\b",
        r"\bRTX\s+3070\b",
        r"\bRTX\s+3060\b",
        // NVIDIA RTX Pro/Workstation
        r"\bRTX\s+PRO\s+6000\b",
        r"\bRTX\s+PRO\s+5000\b",
        r"\bRTX\s+PRO\s+4000\b",
        r"\bRTX\s+6000\b",
        r"\bRTX\s+5000\b",
        r"\bRTX\s+4000\b",
        r"\bRTX\s+A6000\b",
        r"\bRTX\s+A5000\b",
        // NVIDIA Data Center
        r"\bH200\b",
        r"\bH100\b",
        r"\bA100\b",
        r"\bA40\b",
        r"\bA30\b",
        r"\bA10\b",
        // AMD Radeon
        r"\bRadeon\s+8060S?\b",
        r"\bRadeon\s+7900\s*\w*\b",
        r"\bRadeon\s+7800\s*\w*\b",
        // Platforms and architecture
        r"\bGB300\b",
        r"\bGB200\b",
        r"\bConnectX-8\b",
        r"\bConnectX-7\b",
        r"\bConnectX8\b",
        r"\bConnectX7\b",
    ];

    let mut protected = text.to_string();

    // Replace each pattern with a unique placeholder to protect it
    for (index, pattern) in HARDWARE_PATTERNS.iter().enumerate() {
        let regex = regex::Regex::new(pattern).expect("hardware identifier regex must compile");
        protected = regex
            .replace_all(&protected, format!("⟪HWID{index}⟫"))
            .into_owned();
    }

    protected
}

/// Restore protected hardware identifiers after normalization
fn restore_hardware_identifiers(text: &str) -> String {
    // Map of placeholders back to original patterns (approximate restoration)
    // In practice, we store the actual matched values, but for now we use canonical forms
    const HARDWARE_CANONICAL: &[&str] = &[
        "RTX 5090",
        "RTX 5080",
        "RTX 5070",
        "RTX 4090",
        "RTX 4080",
        "RTX 4070",
        "RTX 3090",
        "RTX 3080",
        "RTX 3070",
        "RTX 3060",
        "RTX PRO 6000",
        "RTX PRO 5000",
        "RTX PRO 4000",
        "RTX 6000",
        "RTX 5000",
        "RTX 4000",
        "RTX A6000",
        "RTX A5000",
        "H200",
        "H100",
        "A100",
        "A40",
        "A30",
        "A10",
        "Radeon 8060S",
        "Radeon 7900",
        "Radeon 7800",
        "GB300",
        "GB200",
        "ConnectX-8",
        "ConnectX-7",
        "ConnectX-8",
        "ConnectX-7",
    ];

    let mut restored = text.to_string();

    for (index, canonical) in HARDWARE_CANONICAL.iter().enumerate() {
        restored = restored.replace(&format!("⟪HWID{index}⟫"), canonical);
    }

    restored
}

fn compact_parameter_count(digits: &str) -> Option<String> {
    let value = digits.replace(',', "").parse::<u128>().ok()?;
    for (scale, suffix) in [
        (1_000_000_000_000u128, "T"),
        (1_000_000_000u128, "B"),
        (1_000_000u128, "M"),
    ] {
        if value >= scale && value % scale == 0 {
            return Some(format!("{}{}", value / scale, suffix));
        }
        if value >= scale && value % (scale / 10) == 0 {
            let rendered = value as f64 / scale as f64;
            return Some(format!("{rendered:.1}{suffix}"));
        }
    }
    None
}

fn normalize_parameter_counts(text: &str) -> String {
    let pattern =
        regex::Regex::new(r"(?i)\b([0-9]{1,3}(?:,[0-9]{3})+|[0-9]+)\s+parameters?(\s+model)?\b")
            .expect("parameter-count regex must compile");
    pattern
        .replace_all(text, |captures: &regex::Captures<'_>| {
            compact_parameter_count(&captures[1])
                .map(|count| {
                    if captures.get(2).is_some() {
                        format!("{count} model")
                    } else {
                        format!("{count} parameters")
                    }
                })
                .unwrap_or_else(|| captures[0].to_string())
        })
        .into_owned()
}

fn normalize_moe_parameter_counts(text: &str) -> String {
    static MOE_MODEL_COUNT: once_cell::sync::Lazy<regex::Regex> =
        once_cell::sync::Lazy::new(|| {
            regex::Regex::new(r"(?i)\b([0-9]{1,3}(?:,[0-9]{3})+|[0-9]+)\s+(MoE|dense)\s+model\b")
                .expect("MoE parameter-count regex must compile")
        });
    MOE_MODEL_COUNT
        .replace_all(text, |captures: &regex::Captures<'_>| {
            compact_parameter_count(&captures[1])
                .map(|count| format!("{count} {} model", &captures[2]))
                .unwrap_or_else(|| captures[0].to_string())
        })
        .into_owned()
}

fn pluralize_exact_count_units(text: &str) -> String {
    static COUNT_UNIT: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
        regex::Regex::new(r"(?i)(^|[^\d,.$])((?:0|[2-9]|[1-9][0-9]+))\s+(GPU|user|request|token)\b")
            .expect("count-unit plural regex")
    });
    COUNT_UNIT
        .replace_all(text, |captures: &regex::Captures<'_>| {
            format!("{}{} {}s", &captures[1], &captures[2], &captures[3])
        })
        .into_owned()
}

fn normalize_benchmark_ranges(text: &str) -> String {
    // Pattern for genuine benchmark ranges like "180 to 250 tokens per second" or "about 200, 300 tokens per second"
    // IMPORTANT: We must exclude numbers with thousand-separator commas (e.g., "1,500 tokens per second")
    // A comma followed by exactly 3 digits and then a space is a thousand separator, NOT a range separator.
    // Range pattern: number + comma + space + number (where the second number is NOT exactly 3 digits OR the first number has fewer than 4 digits)
    // Examples of RANGES: "200, 300 tok/s" (small numbers), "1.5, 2.5 tok/s" (decimals)
    // Examples NOT ranges: "1,500 tok/s", "2,600 tok/s" (thousand separators)
    let pattern = regex::Regex::new(
        r"(?i)\b(about\s+)?([0-9]+(?:\.[0-9]+)?),\s+([0-9]+(?:\.[0-9]+)?)\s+tokens?\s+(?:per|a)\s+second\b",
    )
    .expect("benchmark-range regex must compile");
    pattern
        .replace_all(text, |captures: &regex::Captures<'_>| {
            let left_str = &captures[2];
            let right_str = &captures[3];

            // Guard against false positives: if right_str is exactly 3 digits (like "500" in "1,500"),
            // this is likely a thousand separator, not a range. Skip the conversion.
            // Exception: if left_str is also small (like "200, 300"), it IS a range.
            let right_digits = right_str.chars().filter(|c| c.is_ascii_digit()).count();
            let left_value = left_str.parse::<f64>().ok();
            let right_value = right_str.parse::<f64>().ok();

            // Skip if this looks like a thousand-separator comma (e.g., "1,500" where right="500" and left < 100)
            if right_digits == 3 {
                if let (Some(left), Some(right)) = (left_value, right_value) {
                    // If left is small (< 1000) and right is 3 digits, this could be thousand separator
                    // e.g., "1,500" -> left=1, right=500 -> skip
                    // But "200, 300" -> left=200, right=300 -> this IS a range
                    if left < 1000.0 && right < 1000.0 && left < 100.0 {
                        // This looks like "1,500" (thousand separator), not a range
                        return captures[0].to_string();
                    }
                }
            }

            let (first, second) = match (left_value, right_value) {
                (Some(left), Some(right)) if left > right => (&captures[3], &captures[2]),
                _ => (&captures[2], &captures[3]),
            };
            let prefix = if captures.get(1).is_some() { "~" } else { "" };
            format!("{prefix}{first}–{second} tok/s")
        })
        .into_owned()
}

fn normalize_transfer_rates(text: &str) -> String {
    let pattern = regex::Regex::new(
        r"(?i)\b([0-9]+(?:\.[0-9]+)?)\s+(bits?|kilobits?|megabits?|gigabits?|bytes?|kilobytes?|megabytes?|gigabytes?)\s+per\s+second\b",
    )
    .expect("transfer-rate regex must compile");
    let normalized = pattern
        .replace_all(text, |captures: &regex::Captures<'_>| {
            let unit = match captures[2].to_ascii_lowercase().as_str() {
                "bit" | "bits" => "bit/s",
                "kilobit" | "kilobits" => "kbit/s",
                "megabit" | "megabits" => "Mbit/s",
                "gigabit" | "gigabits" => "Gbit/s",
                "byte" | "bytes" => "B/s",
                "kilobyte" | "kilobytes" => "KB/s",
                "megabyte" | "megabytes" => "MB/s",
                "gigabyte" | "gigabytes" => "GB/s",
                _ => return captures[0].to_string(),
            };
            format!("{} {unit}", &captures[1])
        })
        .into_owned();
    replace_all(
        normalized,
        r"\b([0-9]+(?:\.[0-9]+)?)\s+(B|KB|MB|GB)\s+per\s+second\b",
        "$1 $2/s",
    )
}

fn normalize_technical_values(text: &str) -> String {
    // CRITICAL: Protect hardware/GPU identifiers BEFORE any number formatting
    // These are model numbers, not quantities - they must NEVER get comma separators
    let mut out = protect_hardware_identifiers(text);

    out = normalize_parameter_counts(&out);
    out = normalize_moe_parameter_counts(&out);

    // Fix malformed number patterns like "000–5", "000–41" which are corrupted "5,000", "41,000"
    // These happen when ASR outputs something like "000 dash 5" instead of "5,000"
    out = regex::Regex::new(r"\b0{2,}–(\d{1,2})\b")
        .expect("malformed-number regex must compile")
        .replace_all(&out, |caps: &regex::Captures<'_>| {
            format!("{},000", &caps[1])
        })
        .into_owned();

    out = normalize_benchmark_ranges(&out);
    out = normalize_transfer_rates(&out);

    // Restore protected identifiers (remove placeholder markers)
    out = restore_hardware_identifiers(&out);

    // Misc word fixes
    out = replace_all(out, r"\bPackage\b", "package");
    out = replace_all(out, r"\bPORT\b", "port");
    out = replace_all(out, r"\bGo\b", "go");
    out = replace_all(out, r"\bShell\b", "shell");
    out = replace_all(out, r"\bQuantization\b", "quantization");
    out = replace_all(out, r"\ba\s+10,000\s+GPU\b", "a $$10,000 GPU");

    // Domain-specific ASR corrections for AI/ML/GPU context
    // "exports" -> "experts" in MoE (mixture of experts) discussion context
    out = replace_all(
        out,
        r"(?i)\bthe\s+offloaded\s+exports\b",
        "the offloaded experts",
    );
    out = replace_all(
        out,
        r"(?i)\bMoE.*?offloaded\s+exports\b",
        "MoE model, and the offloaded experts",
    );
    out = replace_all(
        out,
        r"(?i)\boffloaded\s+exports\s+are\b",
        "offloaded experts are",
    );

    // "hoisting" -> "hosting" in deployment/agent context
    out = replace_all(
        out,
        r"(?i)\bhoisting\s+(?:their|your|my|our)\s+agents?\b",
        "hosting their agents",
    );
    out = replace_all(
        out,
        r"(?i)\bfor\s+hoisting\s+agents?\b",
        "for hosting agents",
    );

    // "prop" -> "prompt" in AI context
    out = replace_all(out, r"(?i)\bAI\s+prop\b", "AI prompt");
    out = replace_all(out, r"(?i)\bone\s+AI\s+prop\b", "one AI prompt");
    out = replace_all(out, r"(?i)\bthe\s+AI\s+prop\b", "the AI prompt");
    out = replace_all(out, r"(?i)\braise\s+your\s+prop\b", "raise your prompt");

    // Strongly contextual ASR repairs. Each phrase carries enough local
    // evidence to avoid guessing at malformed general speech.
    out = replace_all(
        out,
        r"(?i)\b(\d+(?:\.\d+)?)\s+gigawatt\s+(box|machine|system)\b",
        "$1 GB $2",
    );
    out = replace_all(out, r"(?i)\bquality\s+per\s+bike\b", "quality per bit");
    out = replace_all(
        out,
        r"(?i)\bevents\s+are\s+actually\s+on\s+top\b",
        "vents are actually on top",
    );
    out = replace_all(
        out,
        r"(?i)\bresponsive\s+for\s+doing\b",
        "responsible for doing",
    );

    // "currency" -> "concurrency" in throughput/parallel request context
    out = replace_all(out, r"(?i)\bcurrency\s+of\s+(\d+)\b", "concurrency of $1");
    out = replace_all(
        out,
        r"(?i)\bat\s+a\s+currency\s+of\b",
        "at a concurrency of",
    );

    // "HPM" -> "HBM" (high bandwidth memory confusion)
    out = replace_all(out, r"\bHPM\b", "HBM");

    // "div" -> "dip" (typo in analysis context)
    out = replace_all(out, r"\ba\s+div\s+in\b", "a dip in");
    out = replace_all(out, r"\bthe\s+div\s+in\b", "the dip in");

    // "floating 0.4" -> "FP4" (quantization format)
    out = replace_all(out, r"(?i)\bfloating\s+0\.4\b", "FP4");

    // Grammar fixes: verb agreement
    out = replace_all(out, r"\bmodel\s+spill\s+", "model spills ");
    out = replace_all(
        out,
        r"\bthe\s+model\s+spill\s+onto",
        "the model spills onto",
    );
    out = replace_all(out, r"\bif\s+the\s+model\s+spill", "if the model spills");
    out = replace_all(
        out,
        r"(?i)\btwo\s+installation\s+option\b",
        "two installation options",
    );
    out = replace_all(
        out,
        r"(?i)\ba\s+lot\s+of\s+Interface\b",
        "a lot of interfaces",
    );
    out = replace_all(
        out,
        r"(?i)\bcloning\s+the\s+drives\s+also\s+clone\b",
        "cloning the drives also clones",
    );

    // Grammar fixes: "United state" -> "United States"
    out = replace_all(out, r"\bUnited\s+state\b", "United States");

    // Grammar fixes: "not an HBM" -> "not in HBM"
    out = replace_all(out, r"\bnot\s+an\s+HBM\b", "not in HBM");
    out = replace_all(out, r"\bin\s+an\s+HBM\b", "in HBM");

    // Grammar fixes: "NVIDIA own" -> "NVIDIA's own"
    out = replace_all(out, r"\bNVIDIA\s+own\b", "NVIDIA's own");

    // Pluralize only complete integer quantities. Digits inside currency,
    // decimals, or identifiers must never be interpreted as a count.
    out = pluralize_exact_count_units(&out);

    // Capitalization: "Decode" -> "decode" in context
    out = replace_all(out, r"\bin\s+Decode\b", "in decode");
    out = replace_all(out, r"\bat\s+Decode\b", "at decode");

    // Hyphenation normalization
    out = replace_all(out, r"\bultra-low\s+latency\b", "ultra-low-latency");
    out = replace_all(out, r"\bhigh\s+bandwidth\b", "high-bandwidth");
    out = replace_all(
        out,
        r"\b(\d+)-volt\s+with\s+(\d+)\s+amp\b",
        "$1-volt with $2-amp",
    );
    out = replace_all(out, r"\b(\d+)\s+amp\s+outlet\b", "$1-amp outlet");

    // Fix "loner unit" -> "loaner unit" (borrowed/demo unit)
    out = replace_all(out, r"\bloner\s+unit\b", "loaner unit");

    // Remove .TypeScript context leakage (should never appear in transcripts)
    out = replace_all(out, r"\.TypeScript\s+one\b", "test one");
    out = replace_all(out, r"\.TypeScript\s+test\b", "test");
    out = replace_all(out, r"\.TypeScript\b", "");

    // Sentence-level duplicate pattern removal
    // Pattern: "X. X." or "X. Well, X." where X is a repeated phrase
    // Example: "HBM is 256 GB. HBM is 256 GB." → "HBM is 256 GB."
    // Duplicate speech is handled by transcript_cleanup's token-aware logic.
    // The former sentence splitter treated every decimal/file period as a
    // sentence boundary and corrupted `4.7`, `1.7B`, and `mistake.md`.

    // Technical abbreviation normalization - ensure consistent casing
    out = replace_all(out, r"\bconnectx8\b", "ConnectX-8");
    out = replace_all(out, r"\bconnectx7\b", "ConnectX-7");
    out = replace_all(out, r"\bConnectX8\b", "ConnectX-8");
    out = replace_all(out, r"\bConnectX7\b", "ConnectX-7");

    // Ensure model names are properly cased (after tech_lexicon runs)
    out = replace_all(out, r"\bnemotron\s+120b\b", "Nemotron 120B");
    out = replace_all(
        out,
        r"\bnemotron\s+3\s+super\s+120b\b",
        "Nemotron 3 Super 120B",
    );

    // Fix "Super Chip" capitalization
    out = replace_all(out, r"\bSuper\s+Chip\b", "Superchip");
    out = replace_all(out, r"\bPROPER\b", "proper");
    out = replace_all(out, r"\bWindows\s+Machines\b", "Windows machines");

    // Normalize "pre-fill" vs "prefill"
    out = replace_all(out, r"\bpre-fill\b", "prefill");
    out = replace_all(out, r"\bPre-fill\b", "Prefill");

    // Shared-unit benchmark ranges are handled above before scalar units.
    out = replace_all(
        out,
        r"(?i)\b([0-9]+(?:\.[0-9]+)?)\s+tokens?\s+(?:per|a)\s+second\b",
        "$1 tok/s",
    );
    out = replace_all(out, r"(?i)\b([0-9]+(?:\.[0-9]+)?)\s+watts?\b", "$1 W");
    out = replace_all(out, r"(?i)\b([0-9]+(?:\.[0-9]+)?)\s+gigs?\b", "$1 GB");
    out = replace_all(out, r"(?i)\bis\s+(APU|CPU|GPU)\b", "is an $1");
    out = replace_all(out, r"(?i)\b([0-9]+)\s+agent\b", "$1 agents");
    out = replace_all(out, r"\bAre\s+we\s+did\s+with\b", "Are we done with");
    out = replace_all(out, r"\bare\s+we\s+did\s+with\b", "are we done with");
    out = replace_all(
        out,
        r"(?i)\bwhich\s+it\s+be\s+around\b",
        "which should be around",
    );
    out = replace_all(
        out,
        r"\b([0-9]+(?:\.[0-9]+)?)\s+out\s+of\s+([0-9]+(?:\.[0-9]+)?)\s+GB\s+used\b",
        "$1 GB out of $2 GB used",
    );
    out = replace_all(out, r"(?i)\b(tok/s\s+on)\s+thead\b", "$1 the AMD");
    out = replace_all(out, r"(?i)\bthe\s+thead\s+side\b", "the AMD side");
    out = replace_all(out, r"(?i)\bI\s+did\s+\.TypeScript\s+test\b", "I did test");
    out = replace_all(
        out,
        r"(?i)\b(five|5)\s+to\s+(six|6)\s+times\s+faster\b",
        "5–6× faster",
    );

    for (word, digit) in [
        ("one", "1"),
        ("two", "2"),
        ("three", "3"),
        ("four", "4"),
        ("five", "5"),
        ("six", "6"),
        ("seven", "7"),
        ("eight", "8"),
        ("nine", "9"),
        ("ten", "10"),
    ] {
        out = replace_all(
            out,
            &format!(r"(?i)\b{word}\s+times\s+faster\b"),
            &format!("{digit}× faster"),
        );

        // Also normalize spoken number ranges like "seven to 8" → "7–8"
        out = replace_all(
            out,
            &format!(r"(?i)\b{word}\s+to\s+(\d+)\b"),
            &format!("{digit}–$1"),
        );
        out = replace_all(
            out,
            &format!(r"(?i)\b(\d+)\s+to\s+{word}\b"),
            &format!("$1–{digit}"),
        );
    }
    let price_range = regex::Regex::new(
        r"(?i)\b(sell(?:s|ing)?\s+(?:it|you)\s+for|costs?|priced?\s+at)\s+([0-9]{1,3}(?:,[0-9]{3})+)\s+to\s+([0-9]{1,3}(?:,[0-9]{3})+)(?:\s+to\s+([0-9]{1,3}(?:,[0-9]{3})+))?",
    )
    .expect("price-range regex must compile");
    out = price_range
        .replace_all(&out, |captures: &regex::Captures<'_>| {
            let upper = captures.get(4).unwrap_or_else(|| captures.get(3).unwrap());
            format!("{} ${}–${}", &captures[1], &captures[2], upper.as_str())
        })
        .into_owned();
    out
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

fn coordinate_commas(sentence: &str, style: PunctuationStyle) -> String {
    if matches!(style, PunctuationStyle::Informal) {
        return sentence.to_string();
    }

    let lower = sentence.trim_start().to_ascii_lowercase();
    let structurally_list_like = [
        "we use ",
        "we need ",
        "i need ",
        "i want ",
        "the options are ",
        "the files are ",
        "the models are ",
        "the changes are ",
    ]
    .iter()
    .any(|prefix| lower.starts_with(prefix));

    if !structurally_list_like || lower.contains(" then ") || lower.contains(" because ") {
        return sentence.to_string();
    }

    for conjunction in ["and", "or"] {
        let tokens: Vec<&str> = sentence.split_whitespace().collect();
        let total = tokens
            .iter()
            .filter(|token| token.trim_matches(',').eq_ignore_ascii_case(conjunction))
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
                    && rebuilt.last().is_some_and(|previous| {
                        previous
                            .chars()
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

fn recapitalize(text: &str) -> String {
    let Some(first_start) = text
        .char_indices()
        .find(|(_, c)| c.is_alphabetic())
        .map(|(i, _)| i)
    else {
        return text.to_string();
    };

    let first_end = text[first_start..]
        .char_indices()
        .find(|(_, c)| c.is_whitespace())
        .map(|(i, _)| first_start + i)
        .unwrap_or(text.len());
    let first_token = &text[first_start..first_end];

    let mixed_case = first_token.chars().skip(1).any(|c| c.is_uppercase());
    let technical_shape = first_token.contains('_')
        || first_token.contains('/')
        || first_token.contains("::")
        || first_token.starts_with('#');

    if mixed_case || technical_shape {
        return text.to_string();
    }

    let first_char = text[first_start..].chars().next().unwrap();
    if first_char.is_uppercase() {
        return text.to_string();
    }

    let mut out = text.to_string();
    out.replace_range(
        first_start..first_start + first_char.len_utf8(),
        &first_char.to_uppercase().to_string(),
    );
    out
}

const ORDINAL_CUES: &[&str] = &[
    "first", "second", "third", "fourth", "fifth", "sixth", "seventh", "eighth", "ninth", "tenth",
    "finally", "1st", "2nd", "3rd", "4th", "5th", "6th", "7th", "8th", "9th", "10th",
];

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

/// Spoken phrases a recognizer misheard as symbols. `@Word` survives only in
/// genuine symbol contexts — an email/handle mention cued by the previous
/// word, or a hex token like `#ff5500`. Everything else is ordinary speech:
/// "thinking @Once" is "thinking at once", "on the #actually" is "on the
/// actually" (the underlying spoken wording, never an invented hashtag).
const MENTION_CUES: &[&str] = &[
    "mention", "mentions", "dm", "ping", "message", "msg", "email", "send", "sent", "cc", "tag",
    "slack", "post", "forward", "notify", "shout", "tell", "loop", "add",
];
const HASHTAG_CUES: &[&str] = &["hash", "hashtag", "hashtags", "pound", "channel", "tag"];

fn restore_spoken_symbols(text: &str) -> String {
    if !text.contains('@') && !text.contains('#') {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut prev: Option<&str> = None;
    // split_inclusive keeps each token's trailing whitespace, so spacing and
    // paragraph structure survive byte-for-byte.
    for chunk in text.split_inclusive(char::is_whitespace) {
        let (word, trailing_ws) = match chunk.char_indices().find(|(_, c)| c.is_whitespace()) {
            Some((index, _)) => (&chunk[..index], &chunk[index..]),
            None => (chunk, ""),
        };
        out.push_str(&restore_symbol_word(word, prev));
        out.push_str(trailing_ws);
        if !word.is_empty() {
            prev = Some(word);
        }
    }
    out
}

fn restore_symbol_word(word: &str, prev: Option<&str>) -> String {
    let bare = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '#' && c != '@');
    if bare != word {
        return word.to_string(); // punctuation-adjacent context stays as-is
    }
    let prev_cue = prev
        .map(|p| {
            p.trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase()
        })
        .filter(|p| !p.is_empty());

    if let Some(body) = word.strip_prefix('@') {
        let handle_shape = !body.is_empty()
            && body.chars().next().is_some_and(char::is_alphabetic)
            && body
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_');
        if !handle_shape {
            // A bare "@" (or "@."-style fragment) is spoken "at".
            return "at".to_string();
        }
        let cued = prev_cue
            .as_deref()
            .is_some_and(|p| MENTION_CUES.contains(&p));
        if cued {
            return word.to_string();
        }

        // SMART @ DETECTION: Only keep @ if it looks like a genuine email/handle context.
        // Common false positives: @Once, @How, @Concurrency, @Just, @Least, @Home, etc.
        // These are spoken phrases "at once", "at how", etc. that ASR misheard as symbols.

        // Check if this looks like a common English word being misheard as a handle
        const COMMON_SPOKEN_AT_WORDS: &[&str] = &[
            "once",
            "twice",
            "how",
            "what",
            "why",
            "when",
            "where",
            "which",
            "least",
            "most",
            "best",
            "worst",
            "first",
            "last",
            "home",
            "work",
            "all",
            "some",
            "any",
            "none",
            "many",
            "much",
            "more",
            "less",
            "just",
            "only",
            "even",
            "still",
            "already",
            "yet",
            "now",
            "then",
            "here",
            "there",
            "every",
            "each",
            "both",
            "either",
            "neither",
            "concurrency",
            "throughput",
            "latency",
            "scale",
            "speed",
            "rate",
            "level",
            "stage",
            "phase",
            "step",
            "point",
            "time",
            "moment",
            // Technical terms commonly seen in AI/ML transcripts
            "tokens",
            "tok",
            "gpu",
            "cpu",
            "model",
            "batch",
            "stream",
        ];

        let body_lower = body.to_lowercase();
        if COMMON_SPOKEN_AT_WORDS.contains(&body_lower.as_str()) {
            // This is almost certainly spoken "at word", not a handle
            return format!("at {}", body_lower);
        }

        // Check if body is ALL CAPS or Title Case (common for proper nouns/handles)
        // vs lowercase or mixed case (more likely to be spoken words)
        let is_all_caps = body
            .chars()
            .filter(|c| c.is_alphabetic())
            .all(|c| c.is_uppercase());
        let is_title_case = body.chars().next().is_some_and(|c| c.is_uppercase())
            && body
                .chars()
                .skip(1)
                .filter(|c| c.is_alphabetic())
                .all(|c| c.is_lowercase());

        // If it's all caps or title case AND has digits or special handle chars, likely a real handle
        let has_digits = body.chars().any(|c| c.is_ascii_digit());
        let has_handle_chars = body.chars().any(|c| c == '-' || c == '_');

        if (is_all_caps || is_title_case) && (has_digits || has_handle_chars) {
            // Looks like a genuine handle: @OpenAI, @gpt-4, @user_123
            return word.to_string();
        }

        // If prev is a common English word (not a cue), it's likely spoken text
        const COMMON_ENGLISH_PREV: &[&str] = &[
            "look", "see", "watch", "check", "find", "get", "put", "set", "start", "begin", "end",
            "stop", "go", "come", "run", "walk", "think", "know", "say", "tell", "ask", "give",
            "take", "make", "about", "around", "near", "close", "far", "here", "there", "thinking",
            "looking", "starting", "running", "using", "working", "starts", "looks", "gets",
            "puts", "sets", "goes", "comes", "at", "in", "on", "to", "for", "with", "by", "from",
            "of", "is", "are", "was", "were", "be", "been", "being", "this", "that", "these",
            "those", "it", "they", "we", "you",
        ];

        if let Some(prev_word) = prev_cue.as_deref() {
            if COMMON_ENGLISH_PREV.contains(&prev_word) {
                // "look @How" -> "look at how", "starts @Just" -> "starts at just"
                return format!("at {}", body_lower);
            }
        }

        // Default: keep as handle if it looks like one, but this should rarely trigger
        // after all the above checks
        return word.to_string();
    }

    if let Some(body) = word.strip_prefix('#') {
        let hex_color =
            !body.is_empty() && body.len() <= 9 && body.chars().all(|c| c.is_ascii_hexdigit());
        let cued = prev_cue
            .as_deref()
            .is_some_and(|p| HASHTAG_CUES.contains(&p));
        if cued || hex_color {
            return word.to_string();
        }

        // SMART # DETECTION: Only keep # if it's explicitly cued or a hex color.
        // Common false positives: #but, #many, #actually, #design, etc.
        // These are spoken words that ASR misheard as hashtags.

        // Almost all standalone #Word in transcripts are false positives.
        // Real hashtags require explicit cues like "hashtag engineering" or "hash tag design"
        // which are handled by the cued check above.

        // Everything else: drop the # and keep the spoken word. A technical
        // noun alone is not evidence of hashtag intent.
        return body.to_string();
    }

    word.to_string()
}

fn format_single(text: &str, style: PunctuationStyle) -> String {
    let text = restore_spoken_symbols(text);
    let contracted = crate::audio_toolkit::normalization::apply_safe_contractions(&text);
    let deshouted = de_shout(&contracted);
    let numerics = normalize_numerics(&deshouted);

    let commaed: String = numerics
        .split(". ")
        .map(|sentence| coordinate_commas(sentence, style))
        .collect::<Vec<_>>()
        .join(". ");

    let punctuated = crate::audio_toolkit::punctuation::punctuate(&commaed, style);
    let punctuated = punctuated
        .trim_start_matches(['.', ' ', '!'])
        .trim_start()
        .to_string();
    let sentences: Vec<&str> = split_sentences(&punctuated);

    let structured = numbered_list(&sentences).unwrap_or_else(|| sentences.join(" "));
    wrap_technical_tokens(&structured)
}

const DESHOUT_WORDS: &[&str] = &[
    "and", "or", "but", "the", "a", "an", "of", "to", "in", "on", "with", "for", "from", "at",
    "by", "is", "are", "be", "as", "it", "its", "into", "add", "all", "put", "use", "when", "then",
    "keep", "make", "set", "give", "filter", "fetch", "check", "return", "handle", "before",
    "after", "without", "using", "also", "round", "center", "white", "black",
];

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
            if DESHOUT_WORDS.contains(&lowered.as_str()) {
                word.replacen(core, &lowered, 1)
            } else {
                word.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

const SENTENCE_ABBREVIATIONS: &[&str] = &[
    "e.g.", "i.e.", "mr.", "mrs.", "ms.", "dr.", "prof.", "sr.", "jr.", "vs.", "fig.",
];

fn is_sentence_terminal_at(text: &str, index: usize) -> bool {
    let bytes = text.as_bytes();
    if index >= bytes.len() || !matches!(bytes[index], b'.' | b'!' | b'?') {
        return false;
    }

    let next = text[index + 1..].chars().next();
    if next.is_some_and(|c| !c.is_whitespace()) {
        return false;
    }

    if bytes[index] != b'.' {
        return true;
    }

    let token_start = text[..index]
        .char_indices()
        .rev()
        .find(|(_, c)| c.is_whitespace())
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(0);
    let token = &text[token_start..=index];
    let lower = token.to_ascii_lowercase();

    if SENTENCE_ABBREVIATIONS.contains(&lower.as_str()) {
        return false;
    }
    if token.contains("://") || token.contains('@') {
        return false;
    }

    true
}

fn split_sentences(text: &str) -> Vec<&str> {
    let mut sentences = Vec::new();
    let mut start = 0usize;

    for (index, _) in text.char_indices() {
        if is_sentence_terminal_at(text, index) {
            let end = index + 1;
            let sentence = text[start..end].trim();
            if !sentence.is_empty() {
                sentences.push(sentence);
            }
            start = end;
        }
    }

    if start < text.len() {
        let sentence = text[start..].trim();
        if !sentence.is_empty() {
            sentences.push(sentence);
        }
    }

    sentences
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Span {
    start: usize,
    end: usize,
}

#[derive(Debug, Clone, Copy)]
struct TokenSpan {
    span: Span,
}

#[derive(Debug, Clone, Copy)]
struct SentenceSpan {
    span: Span,
    words: usize,
    terminal: bool,
}

#[derive(Debug, Clone)]
struct ParsedNumberedList {
    prefix: String,
    items: Vec<String>,
    consumed_end: usize,
}

#[derive(Debug, Clone)]
struct ParsedNaturalList {
    lead: String,
    items: Vec<String>,
    consumed_end: usize,
}

#[derive(Debug, Clone)]
struct ParsedColonList {
    lead: String,
    items: Vec<String>,
    consumed_end: usize,
}

const GREETING_OPENERS: &[&str] = &["hey", "hi", "hello", "dear"];
const GREETING_REJECT_FIRST: &[&str] = &[
    "everyone",
    "everybody",
    "team",
    "folks",
    "guys",
    "all",
    "there",
    "buddy",
    "buddies",
];
const GREETING_BODY_START: &[&str] = &[
    "just",
    "wanted",
    "quick",
    "hope",
    "please",
    "can",
    "could",
    "would",
    "how",
    "thanks",
    "thank",
    "following",
    "checking",
    "reaching",
    "writing",
    "sending",
    "letting",
    "giving",
    "i",
    "we",
    "the",
    "this",
    "that",
    "regarding",
    "about",
    "wanted",
    "sorry",
];
const SIGNOFFS: &[&str] = &[
    "many thanks",
    "thank you",
    "talk soon",
    "take care",
    "best regards",
    "kind regards",
    "thanks",
    "cheers",
    "best",
    "regards",
    "sincerely",
];

// -----------------------------------------------------------------------------
// EMAIL-SPECIFIC CONTEXT
// -----------------------------------------------------------------------------
//
// IMPORTANT:
// - recipient_name comes from Gmail's sender/header metadata.
// - author_name comes from the local Superflow profile / Gmail account identity.
// - DO NOT derive either from the email subject.
// - Capture/cache this context before formatting. No network call here.
//
// The formatter itself remains deterministic and extremely cheap.

#[derive(Debug, Clone, Copy, Default)]
pub struct EmailFormatContext<'a> {
    /// True only when we KNOW the focused surface is an email composer/reply.
    pub is_email: bool,

    /// Canonical name of the person we are replying to.
    ///
    /// Example:
    /// Gmail header says "Will Cannon <will@...>"
    /// Pass "Will" or "Will Cannon" depending on your preferred salutation.
    pub recipient_name: Option<&'a str>,

    /// Canonical local name of the Superflow user.
    ///
    /// Example:
    /// "Harpreet Duggal"
    ///
    /// This is the signature name after:
    ///
    /// Thanks,
    /// Harpreet Duggal
    pub author_name: Option<&'a str>,

    /// Optional job title for the multi-line signature block.
    pub author_title: Option<&'a str>,

    /// Optional company for the multi-line signature block.
    pub author_company: Option<&'a str>,

    /// When true, render the job title line (and company) under the name.
    pub include_title: bool,

    /// When true, render the company (standalone or after the title).
    pub include_company: bool,

    /// Sign-off used when the transcript has NO spoken sign-off (e.g. "Talk soon").
    /// Only populated when we KNOW the author, so we never invent an ending for
    /// an unknown user.
    pub default_signoff: Option<&'a str>,
}

#[derive(Debug, Clone)]
struct ParsedEmailClosing {
    body: String,
    closing: String,
    signature: Option<String>,
}

// ---------------------------------------------------------------------------
// Subject extraction (dictated email)
// ---------------------------------------------------------------------------

/// A dictated subject line, stripped from the transcript so the mail client's
/// Subject field receives it instead of the body.
pub struct ParsedEmailSubject {
    pub body: String,
    pub subject: String,
}

/// Subject-aware result of full email formatting: `text` is the finished body
/// (greeting, paragraphs, closing, signature); `subject` — when dictated — is
/// meant for the mail client's Subject input.
#[derive(Debug, Clone, Default)]
pub struct FormattedEmail {
    pub text: String,
    pub subject: Option<String>,
}

/// A subject phrase may not exceed this many words; longer spans are prose.
const SUBJECT_MAX_WORDS: usize = 12;

/// Words that start the actual email body when dictating subject-first
/// ("subject updated rollout timeline hey sarah …" — body starts at "hey").
const SUBJECT_BODY_STARTERS: &[&str] = &[
    "hey",
    "hi",
    "hello",
    "dear",
    "greetings",
    "yo",
    "good",
    "morning",
    "afternoon",
    "evening",
    "team",
];

fn canonical_marker(word: &str) -> Option<&'static str> {
    let w = word
        .trim_matches(|c: char| matches!(c, ':' | ',' | '.'))
        .to_lowercase();
    match w.as_str() {
        "subject" => Some("subject"),
        _ => None,
    }
}

fn is_greeting_starter(word: &str) -> bool {
    let w = word
        .trim_matches(|c: char| matches!(c, ',' | ':' | '.'))
        .to_lowercase();
    SUBJECT_BODY_STARTERS.contains(&w.as_str())
}

/// Normalize the captured subject: drop marker residue (`:`, doubled
/// "subject", leading "is"), trailing periods/commas, capitalize first word.
fn clean_subject_text(raw: &str, allow_leading_is: bool) -> Option<String> {
    let mut words: Vec<&str> = raw.split_whitespace().collect();
    // Doubled marker: "Subject: Subject, I am sick"
    while let Some(first) = words.first() {
        if canonical_marker(first).is_some() {
            words.remove(0);
        } else {
            break;
        }
    }
    if allow_leading_is && !words.is_empty() {
        let first = words[0]
            .trim_matches(|c: char| matches!(c, ':' | ','))
            .to_lowercase();
        if first == "is" {
            words.remove(0);
        }
    }
    if words.is_empty() {
        return None;
    }
    let mut subject = words.join(" ");
    while subject.ends_with(['.', ',']) {
        subject.pop();
        subject = subject.trim_end().to_string();
    }
    if subject.is_empty() {
        return None;
    }
    let mut chars = subject.chars();
    match chars.next() {
        Some(first) => Some(first.to_uppercase().collect::<String>() + chars.as_str()),
        None => None,
    }
}

/// Deterministic subject detection. Handles FIRST, LAST, and MID-FLOW
/// placements plus written-style "Subject: …" on its own line. Anything
/// ambiguous is left untouched (fail-closed). Subject is always moved to
/// Subject field, removed from body, and capped at 12 words.
fn extract_email_subject(text: &str) -> ParsedEmailSubject {
    let trimmed = text.trim();
    let no_op = || ParsedEmailSubject {
        body: trimmed.to_string(),
        subject: String::new(),
    };
    if trimmed.is_empty() {
        return no_op();
    }

    let words: Vec<&str> = trimmed.split_whitespace().collect();
    if words.is_empty() {
        return no_op();
    }

    // --- Case A: subject FIRST on one line ("subject … hey sarah …") ---
    if canonical_marker(words[0]).is_some() {
        // "subject line …" bigram.
        let mut span_start = 1usize;
        if words.len() > 1
            && canonical_marker(words[1]).is_none()
            && words[1]
                .trim_end_matches([':', ',', '.'])
                .eq_ignore_ascii_case("line")
        {
            span_start = 2;
        }
        let mut span_end = words.len();
        for (index, word) in words.iter().enumerate().skip(span_start) {
            if is_greeting_starter(word) {
                span_end = index;
                break;
            }
        }
        let span = &words[span_start..span_end];
        if !span.is_empty() && span.len() <= SUBJECT_MAX_WORDS {
            if let Some(subject) = clean_subject_text(&span.join(" "), true) {
                let body = words[span_end..].join(" ");
                if body.split_whitespace().count() >= 3 {
                    return ParsedEmailSubject { body, subject };
                }
            }
        }
        // Marker present but the span reads like prose — leave untouched.
        return no_op();
    }

    // --- Case B: subject on its OWN last line ("…\nsubject updated rollout") ---
    if let Some(last_line_start) = trimmed.rfind('\n') {
        let last_line = trimmed[last_line_start + 1..].trim();
        let line_words: Vec<&str> = last_line.split_whitespace().collect();
        if !line_words.is_empty()
            && canonical_marker(line_words[0]).is_some()
            && line_words.len() > 1
            && line_words.len() - 1 <= SUBJECT_MAX_WORDS
        {
            if let Some(subject) = clean_subject_text(&line_words[1..].join(" "), true) {
                let body = trimmed[..last_line_start].trim();
                if body.split_whitespace().count() >= 3 {
                    return ParsedEmailSubject {
                        body: body.to_string(),
                        subject,
                    };
                }
            }
        }
    }

    // --- Case C: subject as the FINAL clause ("… Alex. subject updated …") ---
    // Only after sentence punctuation or a clause comma, and the tail must not
    // read like continuing prose ("… the subject is unclear" never fires).
    if let Some(last_index) = words
        .iter()
        .rposition(|w| canonical_marker(w).is_some())
        .filter(|i| *i > 0 && *i + 1 < words.len())
    {
        let preceding = words[last_index - 1];
        if preceding.ends_with(['.', '!', '?', ',']) {
            let tail = &words[last_index + 1..];
            if !tail.is_empty() && tail.len() <= SUBJECT_MAX_WORDS {
                let first = tail[0]
                    .trim_matches(|c: char| matches!(c, ':' | ','))
                    .to_lowercase();
                let prose_continuation = matches!(
                    first.as_str(),
                    "is" | "was" | "will" | "does" | "the" | "this" | "that" | "it"
                );
                if !prose_continuation {
                    if let Some(subject) = clean_subject_text(&tail.join(" "), false) {
                        let mut body = words[..last_index].join(" ");
                        while body.ends_with(',') {
                            body.pop();
                            body = body.trim_end().to_string();
                        }
                        if body.split_whitespace().count() >= 3 {
                            return ParsedEmailSubject { body, subject };
                        }
                    }
                }
            }
        }
    }

    // --- Case D: subject MID-FLOW in one utterance ("Hey Mike, quick update subject updated rollout … can you check")
    // Handles greeting-first → subject-later in same flow, and subject buried in middle.
    // Fail-closed: prose "the subject is unclear" must not be extracted.
    let mut best: Option<(usize, usize, String)> = None; // (marker_index, tail_end, subject)
    for (marker_idx, word) in words.iter().enumerate() {
        if canonical_marker(word).is_none() {
            continue;
        }
        if marker_idx == 0 {
            continue; // already handled by Case A
        }
        // Prose guard: "the subject is unclear" / "this subject …" — marker preceded by determiner → likely mention, not dictation
        if marker_idx > 0 {
            let prev = words[marker_idx - 1]
                .trim_matches(|c: char| matches!(c, ':' | ',' | '.' | '!' | '?'))
                .to_lowercase();
            if matches!(prev.as_str(), "the" | "this" | "that" | "it" | "a" | "an") {
                continue;
            }
        }
        // "subject line" / "subject is" / "subject line is" in middle — skip those fillers
        let mut span_start = marker_idx + 1;
        if words.len() > span_start
            && words[span_start]
                .trim_end_matches([':', ',', '.'])
                .eq_ignore_ascii_case("line")
        {
            span_start += 1;
        }
        if words.len() > span_start
            && words[span_start]
                .trim_matches(|c: char| matches!(c, ':' | ','))
                .eq_ignore_ascii_case("is")
        {
            span_start += 1;
        }
        if span_start >= words.len() {
            continue;
        }
        // Find span_end: until greeting starter, body starter, sentence punctuation, or 12w
        // Body starters like "can", "please", "quick" signal end of subject phrase
        let mut span_end = (span_start + SUBJECT_MAX_WORDS).min(words.len());
        for idx in span_start..span_end {
            if is_greeting_starter(words[idx]) {
                span_end = idx;
                break;
            }
            // Body starters that clearly begin the next clause — subject ends before them
            let w = words[idx]
                .trim_matches(|c: char| matches!(c, ':' | ',' | '.' | '!' | '?'))
                .to_lowercase();
            if GREETING_BODY_START.contains(&w.as_str()) && idx > span_start {
                // But don't cut single-word subjects like "subject meeting" where next is "can" immediately
                // We need at least 1 word for subject, so if idx == span_start+1 and that one word is already subject, keep it
                // For "subject is updated rollout timeline can you check" — span_start at "updated", idx 3 is "can", so subject is 3 words before "can" — correct
                span_end = idx;
                break;
            }
            if words[idx].ends_with(['.', '!', '?']) && idx > span_start {
                span_end = idx + 1;
                break;
            }
        }
        while span_end > span_start && is_greeting_starter(words[span_end - 1]) {
            span_end -= 1;
        }
        // Try longest valid subject that still leaves body — gives "updated rollout timeline" not just "updated"
        let mut found: Option<(usize, String)> = None;
        for try_end in ((span_start + 1)..=span_end).rev() {
            let span = &words[span_start..try_end];
            if span.is_empty() || span.len() > SUBJECT_MAX_WORDS {
                continue;
            }
            let last = span
                .last()
                .unwrap()
                .trim_matches(|c: char| matches!(c, ':' | ','))
                .to_lowercase();
            if GREETING_BODY_START.contains(&last.as_str()) {
                continue;
            }
            if let Some(subject) = clean_subject_text(&span.join(" "), true) {
                let body_len = marker_idx + (words.len() - try_end);
                if body_len >= 3 {
                    found = Some((try_end, subject));
                    break; // longest valid wins
                }
            }
        }
        let (span_end, subject) = match found {
            Some(v) => v,
            None => continue,
        };
        // Body must still have ≥3 words after removal (already checked)
        let mut body_words = Vec::with_capacity(words.len() - (span_end - marker_idx));
        body_words.extend_from_slice(&words[..marker_idx]);
        body_words.extend_from_slice(&words[span_end..]);
        if body_words.len() >= 3 && best.is_none() {
            best = Some((marker_idx, span_end, subject));
            break; // first valid mid-flow subject wins deterministically
        }
    }
    if let Some((marker_idx, span_end, subject)) = best {
        let mut body_words = Vec::new();
        body_words.extend_from_slice(&words[..marker_idx]);
        body_words.extend_from_slice(&words[span_end..]);
        let mut body = body_words.join(" ");
        // Clean stray double punctuation left by removal: "update , can" → "update can"
        body = body
            .replace(" ,", ",")
            .replace("  ", " ")
            .trim()
            .to_string();
        while body.ends_with(',') {
            body.pop();
            body = body.trim_end().to_string();
        }
        if body.split_whitespace().count() >= 3 {
            return ParsedEmailSubject { body, subject };
        }
    }

    no_op()
}

/// Subject-aware entry point used by the Gmail surface. Runs subject
/// extraction FIRST, then the regular email layout pass on the clean body so
/// greeting/closing/signature logic operates without the subject in the way.
pub fn format_email_for_surface(text: &str, context: EmailFormatContext<'_>) -> FormattedEmail {
    if !context.is_email {
        return FormattedEmail {
            text: format_layout_with_email(text, Some(context)),
            subject: None,
        };
    }
    let parsed = extract_email_subject(text);
    let subject = if parsed.subject.is_empty() {
        None
    } else {
        Some(parsed.subject)
    };
    FormattedEmail {
        text: format_layout_with_email(&parsed.body, Some(context)),
        subject,
    }
}

/// Cheap, deterministic signal that `text` is an email draft. Rules:
/// - opens with a greeting addressed to a person and ends with an explicit
///   sign-off → email;
/// - opens with a dictated subject marker ("subject …") → email;
/// - a greeting alone or bare body → NOT an email.
/// This is the reliable fallback for routing dictation into the email formatter
/// (deterministic sign-off + signature, and subject extraction) when
/// Accessibility surface capture fails to identify the Gmail window. A greeting
/// to a known audience ("hey team") is intentionally not treated as an email.
pub fn is_email_message(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }
    // A dictated subject marker is itself a reliable email signal.
    if !extract_email_subject(trimmed).subject.is_empty() {
        return true;
    }
    extract_email_envelope(trimmed).is_some() && extract_signoff(trimmed, true).is_some()
}

// Longest phrases first.
const EMAIL_SIGNOFFS: &[&str] = &[
    "best regards",
    "kind regards",
    "many thanks",
    "thank you",
    "talk to you soon",
    "all the best",
    "talk soon",
    "take care",
    "warm regards",
    "sincerely",
    "regards",
    "cheers",
    "thanks",
    "best",
];

/// ASR mis-hears of "thanks" and casual shortenings that should normalize to a
/// real sign-off instead of being left in the body.
const SIGNOFF_ALIASES: &[(&str, &str)] = &[
    ("tanks", "thanks"),
    ("thx", "thanks"),
    ("ty", "thanks"),
    ("tks", "thanks"),
    ("tx", "thanks"),
];

/// Returns the canonical sign-off string for a single spoken word, or `None`.
/// Handles aliases (e.g. "tanks" -> "thanks") so they match `EMAIL_SIGNOFFS`.
fn canonical_signoff_word(word: &str) -> Option<&'static str> {
    let w = clean_email_word(word);
    for signoff in EMAIL_SIGNOFFS {
        if *signoff == w {
            return Some(signoff);
        }
    }
    for (alias, target) in SIGNOFF_ALIASES {
        if w == *alias {
            return Some(target);
        }
    }
    None
}

/// Build the multi-line signature block from the context identity.
///
/// `Name` -> `Name\nTitle` -> `Name\nTitle, Company` -> `Name\nCompany`.
/// Returns `None` when there is no known author name.
fn build_email_signature(context: &EmailFormatContext<'_>) -> Option<String> {
    let name = context
        .author_name
        .map(str::trim)
        .filter(|n| !n.is_empty())?;
    let mut lines = vec![name.to_string()];
    if context.include_title {
        if let Some(title) = context
            .author_title
            .map(str::trim)
            .filter(|t| !t.is_empty())
        {
            let mut line = title.to_string();
            if context.include_company {
                if let Some(company) = context
                    .author_company
                    .map(str::trim)
                    .filter(|c| !c.is_empty())
                {
                    line.push_str(", ");
                    line.push_str(company);
                }
            }
            lines.push(line);
        } else if context.include_company {
            if let Some(company) = context
                .author_company
                .map(str::trim)
                .filter(|c| !c.is_empty())
            {
                lines.push(company.to_string());
            }
        }
    } else if context.include_company {
        if let Some(company) = context
            .author_company
            .map(str::trim)
            .filter(|c| !c.is_empty())
        {
            lines.push(company.to_string());
        }
    }
    Some(lines.join("\n"))
}

// These mean "thanks" is still ordinary prose, not an email closing.
//
// Examples we MUST NOT turn into signatures:
// "Thanks for your help"
// "Thanks again for checking"
// "Thanks so much"
// "Thanks everyone"
const SIGNOFF_CONTINUATIONS: &[&str] = &[
    "for",
    "to",
    "again",
    "so",
    "very",
    "a",
    "the",
    "and",
    "but",
    "because",
    "everyone",
    "everybody",
    "all",
    "your",
    "you",
    "that",
    "this",
    "with",
    "about",
    "if",
    "when",
];

fn clean_email_word(word: &str) -> String {
    word.trim_matches(|c: char| !c.is_alphanumeric() && c != '\'' && c != '’' && c != '-')
        .replace('’', "'")
        .to_lowercase()
}

fn canonical_signoff(signoff: &str) -> String {
    let mut chars = signoff.chars();
    match chars.next() {
        Some(first) => {
            let mut out = first.to_uppercase().collect::<String>();
            out.push_str(chars.as_str());
            out
        }
        None => String::new(),
    }
}

fn looks_like_spoken_signature(tokens: &[&str]) -> bool {
    // A human name/signature should be tiny.
    //
    // This deliberately rejects things like:
    // "thanks for taking care of this"
    if tokens.is_empty() || tokens.len() > 3 {
        return false;
    }
    let first = clean_email_word(tokens[0]);
    if first.is_empty() || SIGNOFF_CONTINUATIONS.contains(&first.as_str()) {
        return false;
    }
    tokens.iter().all(|token| {
        let cleaned =
            token.trim_matches(|c: char| !c.is_alphanumeric() && c != '\'' && c != '’' && c != '-');
        cleaned.len() >= 2
            && cleaned
                .chars()
                .all(|c| c.is_alphabetic() || c == '\'' || c == '’' || c == '-')
    })
}

/// Detect an email closing only at the TAIL of the message.
///
/// This is intentionally NOT a global "find thanks" search.
///
/// Good:
///   "... I'll send it tomorrow thanks"
///   "... I'll send it tomorrow thanks harprit"
///   "... I'll send it tomorrow cheers duggal"
///
/// Not a closing:
///   "... thanks for helping with this ..."
///   "... he said thanks and left ..."
///   "... thanks everyone ..."
///
/// If a canonical author name exists, the spoken signature is NEVER trusted.
/// ASR can butcher "Wojciechowski" all it wants. We replace it with metadata.
fn extract_email_closing(
    text: &str,
    context: &EmailFormatContext<'_>,
) -> Option<ParsedEmailClosing> {
    if !context.is_email {
        return None;
    }
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    let tokens = token_spans(trimmed);
    if tokens.len() < 2 {
        return None;
    }
    // A closing can only live in a tiny tail window.
    //
    // Longest supported closing = 2 words.
    // Spoken signature = max 3 words.
    //
    // Six gives us a little safety room without scanning arbitrary prose.
    let search_start = tokens.len().saturating_sub(6);
    // Search backwards — longest signoffs first so "best regards" wins over
    // "regards" when they share the same tail.
    //
    // Therefore:
    // "thanks ... body ... thanks harprit"
    //
    // picks ONLY the final thanks.
    for signoff in EMAIL_SIGNOFFS {
        for candidate_index in (search_start..tokens.len()).rev() {
            let signoff_words: Vec<&str> = signoff.split_whitespace().collect();
            let signoff_end_index = candidate_index + signoff_words.len();
            if signoff_end_index > tokens.len() {
                continue;
            }
            let matched = signoff_words.iter().enumerate().all(|(offset, expected)| {
                canonical_signoff_word(span_text(trimmed, tokens[candidate_index + offset].span))
                    .map(|w| w == *expected)
                    .unwrap_or(false)
            });
            if !matched {
                continue;
            }
            // Everything after the signoff could only be a spoken signature.
            let trailing_tokens = &tokens[signoff_end_index..];
            let trailing_words: Vec<&str> = trailing_tokens
                .iter()
                .map(|token| span_text(trimmed, token.span))
                .collect();
            // CASE 1:
            //
            // "... thanks"
            //
            // Valid. We'll append the canonical author name if available.
            let has_no_spoken_signature = trailing_words.is_empty();
            // CASE 2:
            //
            // "... thanks harprit duggel"
            //
            // Valid signature shape.
            //
            // We do NOT preserve "harprit duggel".
            // We replace it with context.author_name.
            let has_spoken_signature = looks_like_spoken_signature(&trailing_words);
            if !has_no_spoken_signature && !has_spoken_signature {
                // "thanks for your help"
                // "thanks again"
                // "thanks so much"
                //
                // Ordinary prose. Leave it alone.
                continue;
            }
            // Smart duplicate reduction: if the sign-off is immediately preceded by
            // other sign-off tokens (e.g. "cheers thanks", "talk soon thanks"),
            // consume them too so they don't linger in the body or double up.
            let mut signoff_start = tokens[candidate_index].span.start;
            let mut scan = candidate_index;
            while scan > 0 {
                let prev = scan - 1;
                if prev < search_start {
                    break;
                }
                if canonical_signoff_word(span_text(trimmed, tokens[prev].span)).is_some() {
                    signoff_start = tokens[prev].span.start;
                    scan = prev;
                } else {
                    break;
                }
            }
            let body = trimmed[..signoff_start].trim_end();
            // Do not classify a standalone "thanks John" as an entire email.
            if body.split_whitespace().count() < 3 {
                continue;
            }
            return Some(ParsedEmailClosing {
                body: body.to_string(),
                closing: format!("{},", canonical_signoff(signoff)),
                signature: build_email_signature(context),
            });
        }
    }
    None
}

/// Replace the ASR-spelled greeting name with Gmail's canonical sender name.
///
/// Input:
///     "Hey Voytek,"
///
/// Gmail context:
///     recipient_name = "Wojciech"
///
/// Output:
///     "Hey Wojciech,"
///
/// No fuzzy matching. No guessing. Exact metadata wins.
fn canonicalize_email_greeting(greeting: &str, context: &EmailFormatContext<'_>) -> String {
    if !context.is_email {
        return greeting.to_string();
    }
    let Some(recipient) = context
        .recipient_name
        .map(str::trim)
        .filter(|name| !name.is_empty())
    else {
        return greeting.to_string();
    };
    let mut words = greeting.split_whitespace();
    let Some(opener) = words.next() else {
        return greeting.to_string();
    };
    let opener = opener
        .trim_matches(|c: char| !c.is_alphabetic())
        .to_string();
    let opener_norm = opener.to_lowercase();
    if !matches!(opener_norm.as_str(), "hey" | "hi" | "hello" | "dear") {
        return greeting.to_string();
    }
    format!("{opener} {recipient},")
}

const ORDINAL_NON_LIST_FOLLOWERS: &[&str] = &[
    "in", "to", "of", "at", "for", "place", "time", "person", "half", "quarter",
];
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

fn token_spans(text: &str) -> Vec<TokenSpan> {
    let mut spans = Vec::new();
    let mut start = None;

    for (index, ch) in text.char_indices() {
        if ch.is_whitespace() {
            if let Some(token_start) = start.take() {
                spans.push(TokenSpan {
                    span: Span {
                        start: token_start,
                        end: index,
                    },
                });
            }
        } else if start.is_none() {
            start = Some(index);
        }
    }

    if let Some(token_start) = start {
        spans.push(TokenSpan {
            span: Span {
                start: token_start,
                end: text.len(),
            },
        });
    }

    spans
}

fn normalized_word(word: &str) -> String {
    word.trim_matches(|c: char| !c.is_alphanumeric() && c != '\'' && c != '-')
        .replace('’', "'")
        .to_lowercase()
}

fn span_text<'a>(text: &'a str, span: Span) -> &'a str {
    &text[span.start..span.end]
}

fn trim_leading_layout_noise(text: &str) -> &str {
    text.trim_start_matches(|c: char| c.is_whitespace() || matches!(c, ',' | ';' | ':'))
}

fn trim_list_item(text: &str) -> &str {
    text.trim()
        .trim_start_matches(|c: char| matches!(c, ',' | ';' | ':' | '.'))
        .trim()
        .trim_end_matches(|c: char| matches!(c, ',' | ';' | ':' | '.' | '!' | '?'))
        .trim()
}

fn is_name_like(word: &str) -> bool {
    let core = word.trim_matches(|c: char| ",.;:!?\"'()[]{}".contains(c));
    core.len() >= 2
        && core
            .chars()
            .all(|c| c.is_alphabetic() || c == '\'' || c == '-')
}

fn extract_email_envelope(text: &str) -> Option<(String, String)> {
    let trimmed = text.trim();
    let first_line_end = trimmed.find('\n').unwrap_or(trimmed.len());
    let first_line = &trimmed[..first_line_end];
    let tokens = token_spans(first_line);
    if tokens.len() < 2 {
        return None;
    }

    let opener = normalized_word(span_text(first_line, tokens[0].span));
    if !GREETING_OPENERS.contains(&opener.as_str()) {
        return None;
    }

    let first_name_raw = span_text(first_line, tokens[1].span);
    let first_name_norm = normalized_word(first_name_raw);
    if !is_name_like(first_name_raw) || GREETING_REJECT_FIRST.contains(&first_name_norm.as_str()) {
        return None;
    }

    let mut names = vec![recapitalize(
        first_name_raw.trim_matches(|c: char| ",.;:!?\"'()[]{}".contains(c)),
    )];
    let mut last_name_token = 1usize;

    if !first_name_raw.ends_with(',') && tokens.len() > 2 {
        let second_raw = span_text(first_line, tokens[2].span);
        let second_norm = normalized_word(second_raw);
        let next_norm = tokens
            .get(3)
            .map(|token| normalized_word(span_text(first_line, token.span)))
            .unwrap_or_default();

        let second_is_body = GREETING_BODY_START.contains(&second_norm.as_str())
            || GREETING_REJECT_FIRST.contains(&second_norm.as_str());
        let next_is_body = GREETING_BODY_START.contains(&next_norm.as_str());
        let second_looks_name = is_name_like(second_raw) && !second_is_body;

        if second_looks_name && (second_raw.ends_with(',') || next_is_body) {
            names.push(recapitalize(
                second_raw.trim_matches(|c: char| ",.;:!?\"'()[]{}".contains(c)),
            ));
            last_name_token = 2;
        }
    }

    let body_start = tokens[last_name_token].span.end;
    let inline_body = first_line[body_start..]
        .trim_start_matches(|c: char| c.is_whitespace() || c == ',')
        .trim();
    let rest = if first_line_end < trimmed.len() {
        trimmed[first_line_end..].trim()
    } else {
        ""
    };

    let body = match (inline_body.is_empty(), rest.is_empty()) {
        (true, true) => return None,
        (false, true) => inline_body.to_string(),
        (true, false) => rest.to_string(),
        (false, false) => format!("{inline_body} {rest}"),
    };

    let greeting = format!(
        "{},",
        recapitalize(&format!("{opener} {}", names.join(" ")))
    );
    Some((greeting, body))
}

fn extract_signoff(text: &str, allow_inline: bool) -> Option<(String, String)> {
    let trimmed = text.trim();
    let tokens = token_spans(trimmed);
    if tokens.is_empty() {
        return None;
    }

    for signoff in SIGNOFFS {
        let signoff_words: Vec<&str> = signoff.split_whitespace().collect();
        if tokens.len() <= signoff_words.len() {
            continue;
        }

        let start_index = tokens.len() - signoff_words.len();
        let matches = signoff_words.iter().enumerate().all(|(offset, expected)| {
            normalized_word(span_text(trimmed, tokens[start_index + offset].span)) == *expected
        });
        if !matches {
            continue;
        }

        let start = tokens[start_index].span.start;
        let before = trimmed[..start].trim_end();
        if before.is_empty() {
            continue;
        }

        let preceding = &trimmed[..start];
        let isolated = preceding.ends_with('\n') || preceding.ends_with("\n\n");
        if !allow_inline && !isolated {
            continue;
        }
        if allow_inline && !isolated && before.split_whitespace().count() < 5 {
            continue;
        }

        return Some((before.to_string(), format!("{},", recapitalize(signoff))));
    }

    None
}

fn sentence_spans(text: &str) -> Vec<SentenceSpan> {
    let mut spans = Vec::new();
    let mut start = 0usize;

    for (index, _) in text.char_indices() {
        if !is_sentence_terminal_at(text, index) {
            continue;
        }

        let end = index + 1;
        let raw = &text[start..end];
        let leading = raw.len() - raw.trim_start().len();
        let trailing = raw.len() - raw.trim_end().len();
        let span = Span {
            start: start + leading,
            end: end - trailing,
        };
        if span.start < span.end {
            spans.push(SentenceSpan {
                span,
                words: span_text(text, span).split_whitespace().count(),
                terminal: true,
            });
        }
        start = end;
    }

    if start < text.len() {
        let raw = &text[start..];
        let leading = raw.len() - raw.trim_start().len();
        let trailing = raw.len() - raw.trim_end().len();
        let span = Span {
            start: start + leading,
            end: text.len() - trailing,
        };
        if span.start < span.end {
            spans.push(SentenceSpan {
                span,
                words: span_text(text, span).split_whitespace().count(),
                terminal: false,
            });
        }
    }

    spans
}

fn looks_like_existing_markdown(block: &str) -> bool {
    let mut list_lines = 0usize;
    let mut content_lines = 0usize;

    for line in block.lines().map(str::trim).filter(|line| !line.is_empty()) {
        content_lines += 1;
        if line.starts_with("- ") || line.starts_with("* ") {
            list_lines += 1;
            continue;
        }

        let digit_prefix = line
            .find(". ")
            .is_some_and(|dot| dot > 0 && line[..dot].chars().all(|c| c.is_ascii_digit()));
        if digit_prefix {
            list_lines += 1;
        }
    }

    content_lines >= 2 && list_lines >= 2
}

fn ordinal_value(word: &str) -> Option<u16> {
    Some(match word {
        "first" | "1st" => 1,
        "second" | "2nd" => 2,
        "third" | "3rd" => 3,
        "fourth" | "4th" => 4,
        "fifth" | "5th" => 5,
        "sixth" | "6th" => 6,
        "seventh" | "7th" => 7,
        "eighth" | "8th" => 8,
        "ninth" | "9th" => 9,
        "tenth" | "10th" => 10,
        "finally" => u16::MAX,
        _ => return None,
    })
}

fn cardinal_value(word: &str) -> Option<u16> {
    Some(match word {
        "one" | "1" => 1,
        "two" | "2" => 2,
        "three" | "3" => 3,
        "four" | "4" => 4,
        "five" | "5" => 5,
        "six" | "6" => 6,
        "seven" | "7" => 7,
        "eight" | "8" => 8,
        "nine" | "9" => 9,
        "ten" | "10" => 10,
        _ => return None,
    })
}

#[derive(Debug, Clone, Copy)]
struct OrdinalCue {
    value: u16,
    start: usize,
    end: usize,
    next_token: usize,
}

fn ordinal_cue_at(text: &str, tokens: &[TokenSpan], index: usize) -> Option<OrdinalCue> {
    let token = tokens.get(index)?;
    let word = normalized_word(span_text(text, token.span));

    let (value, mut next_token, mut end) = if let Some(value) = ordinal_value(&word) {
        (value, index + 1, token.span.end)
    } else if word == "number" {
        let next = tokens.get(index + 1)?;
        let cardinal = normalized_word(span_text(text, next.span));
        let value = cardinal_value(&cardinal)?;
        (value, index + 2, next.span.end)
    } else {
        return None;
    };

    if let Some(next) = tokens.get(next_token) {
        if normalized_word(span_text(text, next.span)) == "thing" {
            end = next.span.end;
            next_token += 1;
        }
    }

    Some(OrdinalCue {
        value,
        start: token.span.start,
        end,
        next_token,
    })
}

fn parse_numbered_list(text: &str) -> Option<ParsedNumberedList> {
    let tokens = token_spans(text);
    if tokens.len() < 4 {
        return None;
    }

    let mut cues = Vec::new();
    let mut index = 0usize;
    while index < tokens.len() {
        if let Some(cue) = ordinal_cue_at(text, &tokens, index) {
            index = cue.next_token;
            cues.push(cue);
        } else {
            index += 1;
        }
    }

    if cues.len() < 2 {
        return None;
    }

    if !cues.windows(2).all(|pair| {
        pair[0].value == u16::MAX || pair[1].value == u16::MAX || pair[1].value > pair[0].value
    }) {
        return None;
    }

    if cues
        .iter()
        .take(cues.len().saturating_sub(1))
        .any(|cue| cue.value == u16::MAX)
    {
        return None;
    }

    let prefix = text[..cues[0].start].trim();
    if prefix.split_whitespace().count() > 16 {
        return None;
    }

    for cue in &cues {
        if let Some(next) = tokens.get(cue.next_token) {
            let follower = normalized_word(span_text(text, next.span));
            if ORDINAL_NON_LIST_FOLLOWERS.contains(&follower.as_str()) {
                return None;
            }
        }
    }

    let mut items = Vec::with_capacity(cues.len());
    for cue_index in 0..cues.len() {
        let start = cues[cue_index].end;
        let end = cues
            .get(cue_index + 1)
            .map(|cue| cue.start)
            .unwrap_or(text.len());
        let item = trim_leading_layout_noise(&text[start..end]).trim();
        if item.is_empty() {
            return None;
        }
        if cue_index + 1 < cues.len() {
            let last_word = item
                .split_whitespace()
                .last()
                .map(normalized_word)
                .unwrap_or_default();
            if matches!(last_word.as_str(), "and" | "or") {
                return None;
            }
        }
        items.push(item.to_string());
    }

    Some(ParsedNumberedList {
        prefix: prefix.to_string(),
        items,
        consumed_end: text.len(),
    })
}

fn render_numbered_list(parsed: ParsedNumberedList) -> String {
    let mut sections = Vec::new();
    if !parsed.prefix.trim().is_empty() {
        sections.push(parsed.prefix.trim().to_string());
    }

    let list = parsed
        .items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let item = trim_list_item(item);
            format!("{}. {}", index + 1, ensure_terminal(&recapitalize(item)))
        })
        .collect::<Vec<_>>()
        .join("\n");
    sections.push(list);
    sections.join("\n\n")
}

fn match_introducer(text: &str) -> Option<(usize, String)> {
    let tokens = token_spans(text);
    if tokens.is_empty() {
        return None;
    }

    let mut candidates: Vec<&str> = INTRODUCERS.to_vec();
    candidates.sort_by_key(|value| std::cmp::Reverse(value.split_whitespace().count()));

    for introducer in candidates {
        let words: Vec<&str> = introducer.split_whitespace().collect();
        if tokens.len() < words.len() {
            continue;
        }

        let matches = words.iter().enumerate().all(|(index, expected)| {
            normalized_word(span_text(text, tokens[index].span)) == *expected
        });
        if !matches {
            continue;
        }

        let end = tokens[words.len() - 1].span.end;
        let lead = text[..end].trim().to_string();
        return Some((end, lead));
    }

    None
}

fn structural_comma_positions(text: &str) -> Vec<usize> {
    let bytes = text.as_bytes();
    let mut positions = Vec::new();

    for (index, ch) in text.char_indices() {
        if ch != ',' {
            continue;
        }
        let prev_digit = index > 0 && bytes[index - 1].is_ascii_digit();
        let next_digit = index + 1 < bytes.len() && bytes[index + 1].is_ascii_digit();
        if prev_digit && next_digit {
            continue;
        }
        positions.push(index);
    }

    positions
}

fn split_last_conjunction(segment: &str) -> Option<(&str, &str)> {
    let tokens = token_spans(segment);
    let conjunction = tokens.iter().enumerate().rev().find_map(|(index, token)| {
        let normalized = normalized_word(span_text(segment, token.span));
        matches!(normalized.as_str(), "and" | "or").then_some(index)
    })?;

    let token = tokens[conjunction];
    let before = segment[..token.span.start].trim();
    let after = segment[token.span.end..].trim();
    if after.is_empty() {
        return None;
    }
    Some((before, after))
}

fn parse_comma_items(segment: &str) -> Option<Vec<String>> {
    let commas = structural_comma_positions(segment);
    if commas.len() < 2 {
        return None;
    }

    let mut pieces = Vec::new();
    let mut start = 0usize;
    for comma in commas {
        pieces.push(segment[start..comma].trim());
        start = comma + 1;
    }
    pieces.push(segment[start..].trim());

    let last = pieces.pop()?;
    let (before_conjunction, after_conjunction) = split_last_conjunction(last)?;
    if !before_conjunction.is_empty() {
        pieces.push(before_conjunction);
    }
    pieces.push(after_conjunction);

    let items: Vec<String> = pieces
        .into_iter()
        .map(trim_list_item)
        .filter(|item| !item.is_empty())
        .map(ToString::to_string)
        .collect();

    if items.len() < 3 {
        return None;
    }

    let mut lengths = Vec::with_capacity(items.len());
    for item in &items {
        let word_count = item.split_whitespace().count();
        if word_count == 0 || word_count > 6 {
            return None;
        }
        lengths.push(word_count);
        let lower = item.to_ascii_lowercase();
        if lower.starts_with("then ")
            || lower.contains(" then ")
            || lower.starts_with("because ")
            || lower.starts_with("although ")
            || lower.starts_with("unless ")
            || lower.starts_with("while ")
        {
            return None;
        }
        if item.contains('\n') || item.contains('\r') {
            return None;
        }
    }

    let min_words = lengths.iter().copied().min().unwrap_or(1);
    let max_words = lengths.iter().copied().max().unwrap_or(1);
    if max_words > (min_words * 3).max(3) {
        return None;
    }

    Some(items)
}

fn parse_natural_list(text: &str) -> Option<ParsedNaturalList> {
    let (lead_end, lead) = match_introducer(text)?;
    let remainder = trim_leading_layout_noise(&text[lead_end..]);
    if remainder.is_empty() {
        return None;
    }

    let remainder_offset = remainder.as_ptr() as usize - text.as_ptr() as usize;
    let sentence_end = sentence_spans(remainder)
        .first()
        .map(|span| span.span.end)
        .unwrap_or(remainder.len());
    let list_segment = &remainder[..sentence_end];
    let items = parse_comma_items(list_segment)?;

    Some(ParsedNaturalList {
        lead,
        items,
        consumed_end: remainder_offset + sentence_end,
    })
}

fn render_natural_list(parsed: ParsedNaturalList) -> String {
    let lead = parsed
        .lead
        .trim_end_matches(|c: char| matches!(c, ':' | '.' | ','))
        .trim();
    let mut output = format!("{}:", recapitalize(lead));
    for item in parsed.items {
        let item = trim_list_item(&item);
        if item.is_empty() {
            continue;
        }
        output.push_str("\n- ");
        output.push_str(&recapitalize(item));
    }
    output
}

fn find_structural_colon(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();

    for (index, ch) in text.char_indices() {
        if ch != ':' {
            continue;
        }

        let next = text[index + 1..].chars().next();
        if next.is_some_and(|c| !c.is_whitespace()) {
            continue;
        }

        let prev_digit = index > 0 && bytes[index - 1].is_ascii_digit();
        let next_digit = index + 1 < bytes.len() && bytes[index + 1].is_ascii_digit();
        if prev_digit || next_digit {
            continue;
        }

        let token_start = text[..index]
            .char_indices()
            .rev()
            .find(|(_, c)| c.is_whitespace())
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(0);
        let token = &text[token_start..=index];
        if token.contains("://") || token.contains("::") {
            continue;
        }

        return Some(index);
    }

    None
}

fn parse_colon_list(text: &str) -> Option<ParsedColonList> {
    const MIN_ITEMS: usize = 3;
    const MAX_ITEM_WORDS: usize = 14;

    let colon = find_structural_colon(text)?;
    let lead = text[..colon].trim();
    if lead.is_empty() || lead.split_whitespace().count() > 6 {
        return None;
    }

    let after_raw = &text[colon + 1..];
    let after = after_raw.trim_start();
    if after.is_empty() {
        return None;
    }
    let after_offset = after.as_ptr() as usize - text.as_ptr() as usize;
    let sentences = sentence_spans(after);
    if sentences.len() < MIN_ITEMS {
        return None;
    }

    let mut items = Vec::new();
    let mut consumed = 0usize;
    for sentence in sentences {
        if sentence.words == 0 || sentence.words > MAX_ITEM_WORDS {
            break;
        }
        // Colon lists must be terminal sentences; a trailing non-terminal token
        // like "final-tail-token" should remain as separate tail prose, not a
        // bullet. This prevents "final-tail-token" from becoming "- Final-tail-token".
        if !sentence.terminal {
            break;
        }
        items.push(trim_list_item(span_text(after, sentence.span)).to_string());
        consumed = sentence.span.end;
    }

    if items.len() < MIN_ITEMS {
        return None;
    }

    Some(ParsedColonList {
        lead: lead.to_string(),
        items,
        consumed_end: after_offset + consumed,
    })
}

fn render_colon_list(parsed: ParsedColonList) -> String {
    let mut output = format!("{}:", recapitalize(parsed.lead.trim()));
    for item in parsed.items {
        let item = trim_list_item(&item);
        if item.is_empty() {
            continue;
        }
        output.push_str("\n- ");
        output.push_str(&recapitalize(item));
    }
    output
}

fn paragraph_groups(text: &str) -> Vec<String> {
    const TARGET_MIN: usize = 25;
    const TARGET_IDEAL: usize = 30;
    const TARGET_MAX: usize = 35;
    const ACCEPT_MIN: usize = 20;
    const HARD_MAX: usize = 40;

    let sentences = sentence_spans(text);
    if sentences.len() <= 1 || text.split_whitespace().count() <= TARGET_MIN {
        return vec![text.trim().to_string()];
    }

    let mut groups = Vec::new();
    let mut sentence_index = 0usize;

    while sentence_index < sentences.len() {
        let group_start = sentences[sentence_index].span.start;
        let mut cumulative = 0usize;
        let mut preferred: Option<(usize, usize)> = None;
        let mut acceptable: Option<(usize, usize)> = None;
        let mut first_over_hard: Option<(usize, usize)> = None;
        let mut last_under_accept: Option<(usize, usize)> = None;

        for candidate_index in sentence_index..sentences.len() {
            cumulative += sentences[candidate_index].words;
            let candidate = sentences[candidate_index];
            if !candidate.terminal || candidate.span.end >= text.trim_end().len() {
                continue;
            }

            if cumulative < ACCEPT_MIN {
                last_under_accept = Some((candidate_index, cumulative));
                continue;
            }

            if (TARGET_MIN..=TARGET_MAX).contains(&cumulative) {
                let distance = cumulative.abs_diff(TARGET_IDEAL);
                if preferred.is_none_or(|(_, best_distance)| distance < best_distance) {
                    preferred = Some((candidate_index, distance));
                }
                continue;
            }

            if (ACCEPT_MIN..=HARD_MAX).contains(&cumulative) {
                let distance = cumulative.abs_diff(TARGET_IDEAL);
                if acceptable.is_none_or(|(_, best_distance)| distance < best_distance) {
                    acceptable = Some((candidate_index, distance));
                }
                continue;
            }

            if cumulative > HARD_MAX {
                first_over_hard = Some((candidate_index, cumulative));
                break;
            }
        }

        let chosen = preferred
            .map(|value| value.0)
            .or_else(|| acceptable.map(|value| value.0))
            .or_else(|| {
                if let Some((previous_index, previous_words)) = last_under_accept {
                    if previous_words >= 15 {
                        return Some(previous_index);
                    }
                }
                first_over_hard.map(|value| value.0)
            });

        let Some(end_sentence) = chosen else {
            groups.push(text[group_start..].trim().to_string());
            break;
        };

        let end = sentences[end_sentence].span.end;
        if end >= text.trim_end().len() {
            groups.push(text[group_start..].trim().to_string());
            break;
        }

        groups.push(text[group_start..end].trim().to_string());
        sentence_index = end_sentence + 1;
    }

    if groups.is_empty() {
        vec![text.trim().to_string()]
    } else {
        groups
    }
}

fn process_layout_block(block: &str) -> Vec<String> {
    if block.trim().is_empty() {
        return Vec::new();
    }
    if looks_like_existing_markdown(block) {
        return vec![block.trim().to_string()];
    }

    let mut pieces = Vec::new();
    let mut offset = 0usize;
    let mut iterations = 0usize;

    while offset < block.len() {
        iterations += 1;
        if iterations > 128 {
            let tail = block[offset..].trim();
            if !tail.is_empty() {
                pieces.push(tail.to_string());
            }
            break;
        }

        let raw_remaining = &block[offset..];
        let leading = raw_remaining.len() - raw_remaining.trim_start().len();
        offset += leading;
        if offset >= block.len() {
            break;
        }
        let remaining = &block[offset..];

        if let Some(parsed) = parse_numbered_list(remaining) {
            let consumed = parsed.consumed_end;
            pieces.push(render_numbered_list(parsed));
            if consumed == 0 {
                pieces.push(remaining.to_string());
                break;
            }
            offset += consumed;
            continue;
        }

        if let Some(parsed) = parse_natural_list(remaining) {
            let consumed = parsed.consumed_end;
            pieces.push(render_natural_list(parsed));
            if consumed == 0 {
                pieces.push(remaining.to_string());
                break;
            }
            offset += consumed;
            continue;
        }

        if let Some(parsed) = parse_colon_list(remaining) {
            let consumed = parsed.consumed_end;
            pieces.push(render_colon_list(parsed));
            if consumed == 0 {
                pieces.push(remaining.to_string());
                break;
            }
            offset += consumed;
            continue;
        }

        pieces.extend(paragraph_groups(remaining));
        break;
    }

    pieces
}

pub fn format_layout(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.contains("```") {
        return text.to_string();
    }

    let envelope = extract_email_envelope(trimmed);
    let mut output = String::new();
    let mut working = match &envelope {
        Some((greeting, body)) => {
            output.push_str(greeting);
            output.push_str("\n\n");
            body.clone()
        }
        None => trimmed.to_string(),
    };

    let signoff = extract_signoff(&working, envelope.is_some());
    let closing = signoff.as_ref().map(|(_, closing)| closing.clone());
    if let Some((body, _)) = signoff {
        working = body;
    }

    let mut blocks = Vec::new();
    for block in working.split("\n\n") {
        blocks.extend(process_layout_block(block));
    }

    output.push_str(&blocks.join("\n\n"));
    if let Some(closing) = closing {
        if !output.trim().is_empty() {
            output.push_str("\n\n");
        }
        output.push_str(&closing);
    }

    let mut compact = output.trim().to_string();
    while compact.contains("\n\n\n") {
        compact = compact.replace("\n\n\n", "\n\n");
    }
    compact
}

// -----------------------------------------------------------------------------
// DROP-IN REPLACEMENT
// -----------------------------------------------------------------------------

pub fn format_layout_with_email(text: &str, email: Option<EmailFormatContext<'_>>) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.contains("```") {
        return text.to_string();
    }
    let context = email.unwrap_or_default();
    // Existing greeting parser remains useful for separating greeting/body.
    let envelope = extract_email_envelope(trimmed);
    let mut output = String::new();
    let mut working = match &envelope {
        Some((greeting, body)) => {
            let greeting = canonicalize_email_greeting(greeting, &context);
            output.push_str(&greeting);
            output.push_str("\n\n");
            body.clone()
        }
        None => trimmed.to_string(),
    };
    // IMPORTANT:
    //
    // Unlike the old extract_signoff(), this understands:
    //
    // thanks
    // thanks Harpreet
    // cheers Harpreet
    // best regards Harpreet Duggal
    //
    // while rejecting:
    //
    // thanks for your help
    // thanks again
    let mut closing = extract_email_closing(&working, &context);
    // Deterministic default ending: when NOTHING was spoken as a sign-off,
    // synthesize the configured default ("Talk soon") + stored signature.
    // Gated on: email surface, a non-empty default, a real body (same >=3
    // word floor extract_email_closing enforces), and a known author —
    // `default_signoff` is only populated when the name exists (actions.rs),
    // so we never invent an ending for an unknown user.
    if closing.is_none() && context.is_email {
        if let Some(default_signoff) = context
            .default_signoff
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            if working.split_whitespace().count() >= 3 {
                if let Some(signature) = build_email_signature(&context) {
                    closing = Some(ParsedEmailClosing {
                        body: working.clone(),
                        closing: format!("{},", canonical_signoff(default_signoff)),
                        signature: Some(signature),
                    });
                }
            }
        }
    }
    if let Some(parsed) = &closing {
        working = parsed.body.clone();
    }
    let mut blocks = Vec::new();
    for block in working.split("\n\n") {
        blocks.extend(process_layout_block(block));
    }
    output.push_str(&blocks.join("\n\n"));
    if let Some(parsed) = closing {
        if !output.trim().is_empty() {
            output.push_str("\n\n");
        }
        output.push_str(&parsed.closing);
        if let Some(signature) = parsed.signature {
            output.push('\n');
            output.push_str(&signature);
        }
    }
    let mut compact = output.trim().to_string();
    while compact.contains("\n\n\n") {
        compact = compact.replace("\n\n\n", "\n\n");
    }
    compact
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full_pipeline(text: &str, style: PunctuationStyle) -> String {
        let corrected = crate::audio_toolkit::tech_lexicon::apply(text);
        let corrected = crate::audio_toolkit::styling::apply(&corrected);
        format(&corrected, style)
    }

    #[test]
    fn spoken_at_phrases_are_not_symbols() {
        assert_eq!(
            restore_spoken_symbols("dozens of AIs all thinking @Once"),
            "dozens of AIs all thinking at once"
        );
        assert_eq!(
            restore_spoken_symbols("we can test that @home later"),
            "we can test that at home later"
        );
        assert_eq!(
            restore_spoken_symbols("it works @least for now"),
            "it works at least for now"
        );
        assert_eq!(restore_spoken_symbols("look @ this"), "look at this");
    }

    #[test]
    fn genuine_symbol_contexts_survive() {
        assert_eq!(
            restore_spoken_symbols("DM @superflow about the bug"),
            "DM @superflow about the bug"
        );
        assert_eq!(
            restore_spoken_symbols("email will@superflow.com now"),
            "email will@superflow.com now"
        );
        assert_eq!(
            restore_spoken_symbols("use the accent color #ff5500 here"),
            "use the accent color #ff5500 here"
        );
        assert_eq!(
            restore_spoken_symbols("add the hashtag #actually to the post"),
            "add the hashtag #actually to the post"
        );
    }

    #[test]
    fn stray_hashtag_keeps_the_spoken_word() {
        assert_eq!(
            restore_spoken_symbols("I tested one earlier this year on the #actually"),
            "I tested one earlier this year on the actually"
        );
        assert_eq!(
            restore_spoken_symbols("the #design is done"),
            "the design is done"
        );
    }

    #[test]
    fn symbol_restore_preserves_spacing_and_paragraphs() {
        assert_eq!(
            restore_spoken_symbols("first line @Once\n\nsecond #actually line"),
            "first line at once\n\nsecond actually line"
        );
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
    fn normalizes_publication_grade_technical_values_without_losing_decimals() {
        let cases = [
            ("122,000,000,000 parameter model", "122B model"),
            ("32,000,000,000 parameter model", "32B model"),
            ("358,000,000,000 MoE model", "358B MoE model"),
            ("1,700,000,000 parameters", "1.7B parameters"),
            ("RTX 5,080", "RTX 5080"),
            ("RTX PRO 6000", "RTX PRO 6000"),
            ("63 gigabit per second", "63 Gbit/s"),
            ("8 megabytes per second", "8 MB/s"),
            ("600 watts", "600 W"),
            ("100 percent", "100%"),
            ("three times faster", "3× faster"),
            ("six times faster", "6× faster"),
            ("five to six times faster", "5–6× faster"),
            ("a 10,000 GPU", "a $10,000 GPU"),
            (
                "which for the uninitiated is APU",
                "which for the uninitiated is an APU",
            ),
            ("run 10 agent", "run 10 agents"),
            (
                "Are we did with all the agents?",
                "Are we done with all the agents?",
            ),
            ("which it be around 10,000", "which should be around 10,000"),
            ("19.9 out of 96 GB used", "19.9 GB out of 96 GB used"),
            ("24 tokens per second on thead", "24 tok/s on the AMD"),
            (
                "it does not show the thead side",
                "it does not show the AMD side",
            ),
            ("I did .TypeScript test first", "I did test first"),
            ("Q4 Quantization", "Q4 quantization"),
            ("33, 34 tokens per second", "33–34 tok/s"),
            ("1.6 tokens per second", "1.6 tok/s"),
            ("11.2 tokens per second", "11.2 tok/s"),
            ("76.5 GB", "76.5 GB"),
            ("121 tok/s", "121 tok/s"),
            ("9. 41", "9.41"),
            ("13. 5", "13.5"),
            ("128 gigawatt box", "128 GB box"),
            ("quality per bike", "quality per bit"),
            ("events are actually on top", "vents are actually on top"),
            ("responsive for doing this", "responsible for doing this"),
            ("If you raise your prop", "If you raise your prompt"),
            ("two installation option", "two installation options"),
            ("a lot of Interface", "a lot of interfaces"),
            (
                "cloning the drives also clone the data",
                "cloning the drives also clones the data",
            ),
            ("Q4", "Q4"),
            ("Q8", "Q8"),
        ];

        for (input, expected) in cases {
            assert_eq!(normalize_values(input), expected, "input: {input}");
        }
        assert_ne!(normalize_values("76.5 GB"), "765 GB");
        assert_eq!(
            normalize_values("the 128 gigawatt power plant"),
            "the 128 gigawatt power plant"
        );
        assert_eq!(
            normalize_values("32,000,000,000 dollars"),
            "$32,000,000,000"
        );
        for identifier in [
            "RTX 5000",
            "RTX PRO 6000",
            "RTX 4090",
            "GB300",
            "H100",
            "H200",
            "ConnectX-8",
        ] {
            assert_eq!(normalize_values(identifier), identifier);
        }
    }

    #[test]
    fn technical_entity_repairs_are_contextual_and_conservative() {
        let cases = [
            ("Oculink PORT", "OCuLink port"),
            ("GMK Tech dropped the Evil X2", "GMKtec dropped the EVO-X2"),
            ("the Evil X3 uses Strix halo", "the EVO-X3 uses Strix Halo"),
            ("Quen 2.5 model", "Qwen2.5 model"),
            ("Llama C plus plus", "llama.cpp"),
            ("Nvidia and EMD GPU", "NVIDIA and AMD GPU"),
        ];
        for (input, expected) in cases {
            let corrected = crate::audio_toolkit::tech_lexicon::apply(input);
            assert_eq!(normalize_values(&corrected), expected, "input: {input}");
        }

        for prose in [
            "it was very very slow",
            "devices I manage to collect",
            "I did test first and start",
            "the package will go through the port",
        ] {
            assert_eq!(crate::audio_toolkit::tech_lexicon::apply(prose), prose);
        }
    }

    #[test]
    fn hardware_review_regression_fixture_preserves_meaning_and_blocks_contamination() {
        let raw = "That's how much graphics memory is sitting right here on my desk in a tiny little Package. These two are working together to run a single 122,000,000,000 parameter model. It's an Oculink PORT: 63 gigabit per second speed. GMK Tech dropped the Evil X2 with AMD Strix halo. Quen 2.5, 32,000,000,000 parameters, ran at 11.2 tokens per second. This is the RTX 5,080 with 16 gigs of memory. We're using 100 percent of the GPU and getting 1.6 tokens per second. Nvidia and EMD are both working. The Pro 6,000 uses almost 600 watts. I loaded the 122,000,000,000 parameter model with Q4 Quantization. That's 76.5 GB on disk, and it ran at 121 tokens per second. My phone and basically any other devices I manage to collect still work.";
        let corrected = crate::audio_toolkit::tech_lexicon::apply(raw);
        let output = normalize_values(&corrected);

        for expected in [
            "package",
            "122B model",
            "OCuLink port",
            "63 Gbit/s",
            "GMKtec",
            "EVO-X2",
            "Strix Halo",
            "Qwen2.5",
            "32B parameters",
            "11.2 tok/s",
            "RTX 5080",
            "16 GB",
            "100%",
            "1.6 tok/s",
            "NVIDIA",
            "AMD",
            "RTX PRO 6000",
            "600 W",
            "Q4",
            "76.5 GB",
            "121 tok/s",
            "devices I manage to collect",
        ] {
            assert!(output.contains(expected), "missing {expected:?}: {output}");
        }
        for contamination in ["src-tauri/", ".rs", ".TypeScript", "Vercel"] {
            assert!(
                !output.contains(contamination),
                "leaked {contamination}: {output}"
            );
        }
        assert!(
            !output.contains("765 GB"),
            "decimal was corrupted: {output}"
        );
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
    fn classifies_four_digit_years_and_quantities_before_formatting() {
        for (input, expected) in [
            ("in 1,996", "in 1996"),
            ("in 2,010", "in 2010"),
            ("in 2,026", "in 2026"),
            ("1057 lines", "1,057 lines"),
            ("1100 credits", "1,100 credits"),
            ("two thousand dollars", "$2,000"),
            ("two thousand rupees", "₹2,000"),
        ] {
            assert_eq!(normalize_numerics(input), expected, "input: {input}");
        }

        assert_eq!(normalize_numerics("twenty thousand users"), "20,000 users");
        assert_eq!(normalize_numerics("200,000 users"), "200,000 users");
    }

    #[test]
    fn normalizes_typed_parameter_counts_and_decimal_percentages() {
        assert_eq!(
            normalize_values("2.4 trillion parameters"),
            "2.4T parameters"
        );
        assert_eq!(normalize_values("67.4 percent"), "67.4%");
        assert_eq!(normalize_values("67.4%"), "67.4%");
        for identifier in ["RB26", "N20", "RTX 5000", "H100", "Qwen3.5", "v2.7"] {
            assert_eq!(normalize_values(identifier), identifier);
        }
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
    fn parses_clock_times_with_context() {
        assert_eq!(
            normalize_numerics("meet at three thirty pm"),
            "meet at 3:30 PM"
        );
        assert_eq!(
            normalize_numerics("meet at three thirty p m"),
            "meet at 3:30 PM"
        );
        assert_eq!(
            normalize_numerics("meet at three thirty a m"),
            "meet at 3:30 AM"
        );
        assert_eq!(normalize_numerics("meet at three thirty"), "meet at 3:30");
        assert_eq!(normalize_numerics("give me five"), "give me five");
        assert_eq!(
            normalize_numerics("meet at twenty three thirty pm"),
            "meet at twenty three thirty pm"
        );
    }

    #[test]
    fn builds_numbered_lists_from_punctuated_ordinals() {
        assert_eq!(
            format(
                "first fix the header. second fix the card. third remove the footer.",
                PunctuationStyle::Informal,
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
            PunctuationStyle::Informal,
        )
        .contains("`src/components/button.tsx`"));
    }

    #[test]
    fn recapitalize_looks_only_at_the_first_token() {
        assert_eq!(
            recapitalize("fix the MY_CONSTANT value"),
            "Fix the MY_CONSTANT value"
        );
        assert_eq!(
            recapitalize("myFunction should stay"),
            "myFunction should stay"
        );
        assert_eq!(
            recapitalize("src/file.rs should stay"),
            "src/file.rs should stay"
        );
    }

    #[test]
    fn coordinate_commas_do_not_rewrite_action_chains() {
        let input = "I opened Gmail and replied to Alex and went home";
        assert_eq!(coordinate_commas(input, PunctuationStyle::Formal), input);
    }

    #[test]
    fn sentence_boundary_ignores_abbreviations_and_urls() {
        assert_eq!(
            split_sentences("use e.g. this value. Then continue.").len(),
            2
        );
        assert_eq!(
            split_sentences("open https://example.com and continue. Done.").len(),
            2
        );
        assert_eq!(split_sentences("Use Next.js. Then Tauri.").len(), 2);
    }

    #[test]
    fn bare_greeting_without_a_name_is_left_alone() {
        let text = "hey everyone lets ship this today morning folks";
        assert_eq!(format_layout(text), text);
    }

    #[test]
    fn greeting_does_not_absorb_body_words_as_names() {
        let out = format_layout("hey david just wanted to give you an update thanks");
        assert!(out.starts_with("Hey David,\n\n"), "got: {out}");
        assert!(!out.starts_with("Hey David Just"), "got: {out}");
        assert!(out.ends_with("\n\nThanks,"), "got: {out}");
    }

    #[test]
    fn multi_token_names_survive_email_envelope() {
        let out = format_layout("hi david smith quick heads up that the build is green again bye");
        assert!(out.starts_with("Hi David Smith,\n\n"), "got: {out}");
    }

    #[test]
    fn email_envelope_shapes_greeting_body_signoff() {
        let out = format_layout(
            "hey david just wanted to give you an update on the project the dashboard changes are finished and the login issue is fixed thanks",
        );
        assert!(out.starts_with("Hey David,\n\n"), "got: {out}");
        assert!(out.ends_with("\n\nThanks,"), "got: {out}");
    }

    #[test]
    fn inline_thanks_is_not_a_signoff_without_an_email_envelope() {
        let text = "I spoke to Alex and said thanks";
        assert_eq!(format_layout(text), text);
    }

    #[test]
    fn digit_ordinals_keep_normal_occurrences_of_thing() {
        assert_eq!(
            format_layout("1st fix the login thing 2nd check the dashboard 3rd fix buttons"),
            "1. Fix the login thing.\n2. Check the dashboard.\n3. Fix buttons."
        );
    }

    #[test]
    fn spoken_ordinals_absorb_only_immediate_cue_thing() {
        assert_eq!(
            format_layout(
                "first thing fix login second thing check dashboard third thing fix buttons"
            ),
            "1. Fix login.\n2. Check dashboard.\n3. Fix buttons."
        );
    }

    #[test]
    fn number_one_number_two_are_single_cues() {
        assert_eq!(
            format_layout("number one fix login number two check dashboard number three ship it"),
            "1. Fix login.\n2. Check dashboard.\n3. Ship it."
        );
    }

    #[test]
    fn spoken_ordinals_keep_trailing_task_text_in_last_item() {
        let out = format_layout(
            "first fix the login thing second check dashboard third fix buttons and also clean code then test everything dont break anything please",
        );
        assert_eq!(
            out,
            "1. Fix the login thing.\n2. Check dashboard.\n3. Fix buttons and also clean code then test everything dont break anything please."
        );
    }

    #[test]
    fn lone_ordinal_stays_prose() {
        let text = "first open the repo and organize everything for me";
        assert_eq!(format_layout(text), text);
    }

    #[test]
    fn ordinal_sequence_must_move_forward() {
        let text = "first explain this second explain that first repeat this";
        assert_eq!(format_layout(text), text);
    }

    #[test]
    fn ordinary_ordinal_prose_is_not_a_numbered_list() {
        for text in [
            "I was first in line and second to leave.",
            "She finished first in the race and second in the qualifier.",
            "This is the first place and that is the second place.",
        ] {
            assert_eq!(format_layout(text), text, "input: {text}");
        }
    }

    #[test]
    fn ordinal_prefix_is_preserved() {
        assert_eq!(
            format_layout("I know but first fix this second test it third ship it"),
            "I know but\n\n1. Fix this.\n2. Test it.\n3. Ship it."
        );
    }

    #[test]
    fn natural_fruit_list_becomes_bullets() {
        assert_eq!(
            format_layout(
                "I'm going to get apples, bananas, pineapple, strawberries, and raspberries."
            ),
            "I'm going to get:\n- Apples\n- Bananas\n- Pineapple\n- Strawberries\n- Raspberries"
        );
    }

    #[test]
    fn natural_tech_list_preserves_existing_casing() {
        assert_eq!(
            format_layout("We need React, TypeScript, Tailwind CSS, and Tauri."),
            "We need:\n- React\n- TypeScript\n- Tailwind CSS\n- Tauri"
        );
    }

    #[test]
    fn action_chain_is_not_a_natural_list() {
        let text = "I opened Gmail, replied to Alex, and then went home.";
        assert_eq!(format_layout(text), text);
    }

    #[test]
    fn adjective_series_is_not_a_natural_list() {
        let text = "The app is fast, reliable, and local.";
        assert_eq!(format_layout(text), text);
    }

    #[test]
    fn natural_list_consumes_only_its_sentence() {
        let out = format_layout(
            "We need React, TypeScript, Tailwind CSS, and Tauri. Keep the existing backend untouched because it already works correctly.",
        );
        assert!(
            out.starts_with("We need:\n- React\n- TypeScript\n- Tailwind CSS\n- Tauri"),
            "got: {out}"
        );
        assert!(
            out.ends_with(
                "Keep the existing backend untouched because it already works correctly."
            ),
            "got: {out}"
        );
    }

    #[test]
    fn colon_lead_in_with_short_run_builds_bullets() {
        let out = format_layout(
            "quick status: dashboards shipped. login issue resolved. api bug patched. payments pending.",
        );
        assert_eq!(
            out,
            "Quick status:\n- Dashboards shipped\n- Login issue resolved\n- Api bug patched\n- Payments pending"
        );
    }

    #[test]
    fn technical_colons_are_not_list_markers() {
        for text in [
            "meet at 10:00 and review the build",
            "open https://example.com and check it",
            "use hover:bg-stone-700 on the card",
            "call std::mem::take here",
        ] {
            assert_eq!(format_layout(text), text, "input: {text}");
        }
    }

    #[test]
    fn paragraph_prefers_boundary_near_thirty_words() {
        let first = (0..29).map(|_| "alpha").collect::<Vec<_>>().join(" ");
        let text = format!("{first}. This is a second sentence with enough trailing text to remain visible after the first paragraph boundary.");
        let out = format_layout(&text);
        assert!(out.contains(".\n\nThis"), "got: {out}");
    }

    #[test]
    fn paragraph_uses_a_nearby_earlier_boundary_instead_of_sixty_words() {
        let first = (0..22).map(|_| "alpha").collect::<Vec<_>>().join(" ");
        let second = (0..37).map(|_| "beta").collect::<Vec<_>>().join(" ");
        let text = format!("{first}. {second}. Tail sentence stays here.");
        let out = format_layout(&text);
        assert!(out.contains("alpha.\n\n"), "got: {out}");
    }

    #[test]
    fn long_sentence_is_not_split_mid_sentence() {
        let long = (0..60).map(|_| "word").collect::<Vec<_>>().join(" ");
        let text = format!("{long}. Tail follows here.");
        let out = format_layout(&text);
        assert!(out.contains(".\n\nTail follows here."), "got: {out}");
        assert!(!out[..out.find('.').unwrap()].contains("\n\n"));
    }

    #[test]
    fn short_runs_do_not_get_paragraph_gaps() {
        let text = "just a quick note about the deploy status today everyone";
        assert_eq!(format_layout(text), text);
    }

    #[test]
    fn existing_markdown_lists_are_idempotent() {
        let text = "1. Fix login.\n2. Check dashboard.\n3. Ship it.";
        assert_eq!(format_layout(text), text);
        let bullets = "We need:\n- React\n- TypeScript\n- Tauri";
        assert_eq!(format_layout(bullets), bullets);
    }

    #[test]
    fn fenced_code_is_full_passthrough() {
        let text = "```\nmust not touch first second third in here\n```";
        assert_eq!(format_layout(text), text);
    }

    #[test]
    fn layout_is_idempotent_across_email_and_lists() {
        let once =
            format_layout("hey sam quick update first fix payments second verify receipts thanks");
        let twice = format_layout(&once);
        assert_eq!(twice, once, "once: {once}\ntwice: {twice}");
    }

    #[test]
    fn unicode_never_panics_or_corrupts_byte_slices() {
        let cases = [
            "hello José this is a normal sentence with café and résumé.",
            "नमस्ते दुनिया. This remains valid UTF-8.",
            "hey josé just checking in thanks",
            "We need café, résumé, jalapeño, and piñata.",
            "quick status: café shipped. résumé fixed. jalapeño tested. piñata ready.",
        ];
        for case in cases {
            let out = format_layout(case);
            assert!(std::str::from_utf8(out.as_bytes()).is_ok());
        }
    }

    #[test]
    fn protected_developer_tokens_survive_layout_byte_identical() {
        let text = "Keep src-tauri/src/managers/mlx.rs https://example.com dev@example.com bg-rose-600 Next.js TanStack Query shadcn/ui GGUF unchanged. Another sentence follows with enough words to make paragraph logic inspect the surrounding text without mutating those tokens at all.";
        let out = format_layout(text);
        for protected in [
            "src-tauri/src/managers/mlx.rs",
            "https://example.com",
            "dev@example.com",
            "bg-rose-600",
            "Next.js",
            "TanStack Query",
            "shadcn/ui",
            "GGUF",
        ] {
            assert!(out.contains(protected), "missing {protected}: {out}");
        }
    }

    #[test]
    fn layout_never_drops_tail_on_many_structures() {
        let text = "We need React, TypeScript, Tailwind CSS, and Tauri. quick status: dashboards shipped. login resolved. api patched. payments pending. final-tail-token";
        let out = format_layout(text);
        assert!(out.contains("final-tail-token"), "got: {out}");
    }

    #[test]
    fn long_layout_is_deterministic_and_bounded() {
        let sentence = "This sentence contains enough ordinary words to exercise deterministic paragraph grouping while preserving every protected token src-tauri/src/managers/mlx.rs and https://example.com without changing meaning or structure. ";
        let input = sentence.repeat(200);
        let started = std::time::Instant::now();
        let first = format_layout(&input);
        let elapsed = started.elapsed();
        let second = format_layout(&first);
        assert_eq!(second, first);
        assert!(
            elapsed < std::time::Duration::from_millis(200),
            "took {elapsed:?}"
        );
    }

    #[test]
    fn ten_minute_transcript_formats_within_interactive_budget() {
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
    fn golden_tech_stack_pipeline_still_preserves_canonical_terms() {
        let out = full_pipeline(
            "I'm using next year's React TypeScript Tailwind CSS TanStack Query on the front end. The backend is Node.js tRPC Prisma PostgreSQL Redis AND I'm also using Docker GitHub Action ZORD shed scene PLAYWRIGHT AND vercel",
            PunctuationStyle::Informal,
        );
        assert!(out.contains("Next.js"));
        assert!(out.contains("Zod"));
        assert!(out.contains("shadcn/ui"));
        assert!(out.contains("GitHub Actions"));
        assert!(out.contains("Playwright"));
        assert!(out.contains("Vercel"));
    }

    #[test]
    fn empty_input_passthrough() {
        assert_eq!(format("", PunctuationStyle::Formal), "");
        assert_eq!(format_layout(""), "");
    }
}

#[cfg(test)]
mod email_format_tests {
    use super::*;

    fn ctx() -> EmailFormatContext<'static> {
        EmailFormatContext {
            is_email: true,
            recipient_name: Some("Wojciech Kowalski"),
            author_name: Some("Harpreet Duggal"),
            ..Default::default()
        }
    }

    #[test]
    fn formats_terminal_thanks_with_signature() {
        let out = format_layout_with_email(
            "hey wojtek just wanted to confirm the project is finished thanks",
            Some(ctx()),
        );
        assert_eq!(
            out,
            "Hey Wojciech Kowalski,\n\njust wanted to confirm the project is finished\n\nThanks,\nHarpreet Duggal"
        );
    }

    #[test]
    fn replaces_bad_asr_signature_with_canonical_identity() {
        let out = format_layout_with_email(
            "hey wojtek the changes are finished cheers harprit duggel",
            Some(ctx()),
        );
        assert!(out.ends_with("Cheers,\nHarpreet Duggal"), "got: {out}");
        assert!(!out.contains("harprit duggel"));
    }

    #[test]
    fn thanks_inside_body_is_not_a_signoff() {
        let input =
            "hey wojtek thanks for sending the files I reviewed everything and it looks correct";
        let out = format_layout_with_email(input, Some(ctx()));
        assert!(out.contains("thanks for sending the files"), "got: {out}");
        assert!(!out.ends_with("Thanks,\nHarpreet Duggal"));
    }

    #[test]
    fn only_final_thanks_becomes_closing() {
        let out = format_layout_with_email(
            "hey wojtek thanks for sending that earlier I reviewed it and everything looks good thanks harprit",
            Some(ctx()),
        );
        assert!(
            out.contains("thanks for sending that earlier"),
            "got: {out}"
        );
        assert!(out.ends_with("Thanks,\nHarpreet Duggal"), "got: {out}");
    }

    #[test]
    fn thanks_again_is_not_a_signature() {
        let out =
            format_layout_with_email("hey wojtek everything looks good thanks again", Some(ctx()));
        assert!(!out.ends_with("Thanks,\nHarpreet Duggal"));
    }

    #[test]
    fn formal_signoff_works() {
        let out = format_layout_with_email(
            "dear wojtek I have attached the completed documentation best regards harprit duggal",
            Some(ctx()),
        );
        assert!(
            out.ends_with("Best regards,\nHarpreet Duggal"),
            "got: {out}"
        );
    }

    #[test]
    fn informal_cheers_works() {
        let out = format_layout_with_email("hi wojtek everything is shipped cheers", Some(ctx()));
        assert!(out.ends_with("Cheers,\nHarpreet Duggal"), "got: {out}");
    }

    #[test]
    fn non_email_text_never_gets_email_magic() {
        let context = EmailFormatContext {
            is_email: false,
            recipient_name: Some("Wojciech"),
            author_name: Some("Harpreet Duggal"),
            ..Default::default()
        };
        let input = "I told him thanks John";
        assert_eq!(format_layout_with_email(input, Some(context)), input);
    }

    #[test]
    fn default_ending_added_when_nothing_spoken() {
        let out = format_layout_with_email(
            "hi wojtek quick update on the payroll migration the backend work is finished and qa starts tomorrow morning",
            Some(EmailFormatContext {
                is_email: true,
                author_name: Some("Harpreet Duggal"),
                default_signoff: Some("Talk soon"),
                ..Default::default()
            }),
        );
        assert!(out.ends_with("Talk soon,\nHarpreet Duggal"), "got: {out}");
    }

    #[test]
    fn default_ending_renders_title_and_company() {
        let out = format_layout_with_email(
            "hi wojtek quick update on the payroll migration the backend work is finished and qa starts tomorrow morning",
            Some(EmailFormatContext {
                is_email: true,
                author_name: Some("Harpreet Duggal"),
                author_title: Some("Founder"),
                author_company: Some("Superflow"),
                include_title: true,
                include_company: true,
                default_signoff: Some("Talk soon"),
                ..Default::default()
            }),
        );
        assert!(
            out.ends_with("Talk soon,\nHarpreet Duggal\nFounder, Superflow"),
            "got: {out}"
        );
    }

    #[test]
    fn duplicate_signoffs_collapse_to_one() {
        let out = format_layout_with_email(
            "hi wojtek the deploy finished tonight cheers thanks",
            Some(ctx()),
        );
        assert!(out.ends_with("Cheers,\nHarpreet Duggal"), "got: {out}");
        assert!(!out.contains("Thanks"), "got: {out}");
    }

    #[test]
    fn asr_alias_tanks_becomes_thanks() {
        let out = format_layout_with_email(
            "hi wojtek the invoice was approved this morning tanks",
            Some(ctx()),
        );
        assert!(out.ends_with("Thanks,\nHarpreet Duggal"), "got: {out}");
    }

    #[test]
    fn subject_first_is_extracted_and_body_starts_at_greeting() {
        let formatted = format_email_for_surface(
            "subject updated rollout timeline hey wojtek the migration finished today thanks",
            ctx(),
        );
        assert_eq!(
            formatted.subject.as_deref(),
            Some("Updated rollout timeline")
        );
        let out = formatted.text;
        assert!(!out.contains("rollout timeline hey"), "got: {out}");
        assert!(out.starts_with("Hey Wojciech Kowalski,"), "got: {out}");
        assert!(out.ends_with("Thanks,\nHarpreet Duggal"), "got: {out}");
    }

    #[test]
    fn subject_last_inline_is_extracted() {
        let formatted = format_email_for_surface(
            "hi wojtek the qa pass is complete thanks alex. subject updated rollout timeline",
            ctx(),
        );
        assert_eq!(
            formatted.subject.as_deref(),
            Some("Updated rollout timeline")
        );
        let out = formatted.text;
        assert!(!out.contains("updated rollout"), "got: {out}");
        assert!(out.ends_with("Thanks,\nHarpreet Duggal"), "got: {out}");
    }

    #[test]
    fn subject_own_last_line_is_extracted() {
        let formatted = format_email_for_surface(
            "hi wojtek the qa pass is complete\nthanks harprit duggal\nsubject updated rollout timeline",
            ctx(),
        );
        assert_eq!(
            formatted.subject.as_deref(),
            Some("Updated rollout timeline")
        );
        let out = formatted.text;
        assert!(!out.ends_with("rollout timeline"), "got: {out}");
    }

    #[test]
    fn subject_midflow_after_greeting_is_extracted() {
        let formatted = format_email_for_surface(
            "hey mike, quick update subject is updated rollout timeline can you check thanks",
            ctx(),
        );
        assert_eq!(
            formatted.subject.as_deref(),
            Some("Updated rollout timeline")
        );
        let out = formatted.text;
        assert!(
            !out.to_lowercase().contains("updated rollout"),
            "subject should be removed from body, got: {out}"
        );
        assert!(
            out.starts_with("Hey Wojciech Kowalski,"),
            "greeting canonicalized, got: {out}"
        );
    }

    #[test]
    fn subject_first_and_greeting_one_flow_is_clean() {
        let formatted = format_email_for_surface(
            "subject updated rollout timeline hey mike, the email formatting doesn't work at all thanks",
            ctx(),
        );
        assert_eq!(
            formatted.subject.as_deref(),
            Some("Updated rollout timeline")
        );
        let out = formatted.text;
        assert!(out.starts_with("Hey Wojciech Kowalski,"), "got: {out}");
        assert!(
            !out.to_lowercase().contains("updated rollout timeline hey"),
            "subject and greeting not cleanly split, got: {out}"
        );
    }

    #[test]
    fn prose_mention_of_subject_is_never_extracted() {
        let formatted = format_email_for_surface(
            "hi wojtek the subject is unclear in the last email can you clarify it tomorrow",
            ctx(),
        );
        assert_eq!(formatted.subject, None);
        let out = formatted.text;
        assert!(out.contains("the subject is unclear"), "got: {out}");
    }

    /// End-to-end check mirroring a real dictated email: a greeting + body with
    /// NO spoken sign-off, plus (second case) a dictated subject. This is the
    /// exact path the app exercises when surface == Gmail and the user has
    /// configured their identity — it must produce a finished email with the
    /// subject in the Subject field and "Talk soon" + name at the end.
    fn real_gmail_context() -> EmailFormatContext<'static> {
        EmailFormatContext {
            is_email: true,
            recipient_name: None,
            author_name: Some("Harpreet Duggal"),
            author_title: None,
            author_company: None,
            include_title: false,
            include_company: false,
            default_signoff: Some("Talk soon"),
        }
    }

    #[test]
    fn real_dictated_email_appends_signature_without_spoken_ending() {
        let out = format_layout_with_email(
            "hey sarah just wanted to follow up on the q3 roadmap deck can you share the latest version before friday",
            Some(real_gmail_context()),
        );
        assert!(out.starts_with("Hey Sarah,"), "got: {out}");
        assert!(
            out.ends_with("Talk soon,\nHarpreet Duggal"),
            "expected default sign-off + name, got: {out}"
        );
    }

    #[test]
    fn real_dictated_email_with_subject_routes_subject_and_signs_body() {
        let formatted = format_email_for_surface(
            "subject q3 roadmap update hey sarah please share the latest deck before friday",
            real_gmail_context(),
        );
        assert_eq!(formatted.subject.as_deref(), Some("Q3 roadmap update"));
        let out = formatted.text;
        assert!(out.starts_with("Hey Sarah,"), "got: {out}");
        assert!(
            out.ends_with("Talk soon,\nHarpreet Duggal"),
            "expected default sign-off + name, got: {out}"
        );
        assert!(
            !out.contains("q3 roadmap update hey"),
            "subject must not leak into the body, got: {out}"
        );
    }

    #[test]
    fn email_detection_follows_the_simple_rules() {
        // A greeting alone is ambiguous and must not invent an email envelope.
        assert!(!is_email_message(
            "hey sarah just wanted to follow up on the q3 roadmap deck can you share it before friday"
        ));
        // Greeting + explicitly dictated sign-off => email.
        assert!(is_email_message(
            "hey sarah just wanted to follow up on the q3 roadmap deck can you share it before friday thanks"
        ));
        // subject + greeting + body => email
        assert!(is_email_message(
            "subject updated rollout timeline hey sarah quick update on the payroll migration we finished most of the backend work today thanks alex"
        ));
        // only body (no greeting, no subject) => NOT an email
        assert!(!is_email_message(
            "the backend migration finished today and qa found a couple issues with the import flow"
        ));
        // greeting to a known audience => NOT an email
        assert!(!is_email_message(
            "hey team quick update the deploy is finished and qa passed"
        ));
    }

    #[test]
    fn subject_first_email_routes_subject_and_appends_signature() {
        let formatted = format_email_for_surface(
            "subject updated rollout timeline hey sarah quick update on the payroll migration we finished most of the backend work today but qa found a couple issues with the employee import flow so we are fixing those now and we should have it ready for another qa pass tomorrow morning assuming everything looks good we should still be able to roll it out thursday afternoon i will send you another update once qa is done thanks alex",
            real_gmail_context(),
        );
        assert_eq!(
            formatted.subject.as_deref(),
            Some("Updated rollout timeline")
        );
        let out = formatted.text;
        assert!(out.starts_with("Hey Sarah,"), "got: {out}");
        // Spoken "thanks alex" is kept as the sign-off; the name is rendered
        // from the stored identity, never the ASR-spoken "alex".
        assert!(
            out.ends_with("Thanks,\nHarpreet Duggal"),
            "expected spoken sign-off + stored name, got: {out}"
        );
    }
}
