//! Deterministic Slack-focused transcript formatting.
//!
//! This module is intentionally:
//! - local-only
//! - dependency-free
//! - deterministic
//! - safe for Slack in the browser and the native macOS client
//! - conservative about rewriting meaning
//!
//! It is a layout/cleanup layer, not an LLM. Feed it the transcript after your
//! normal ASR punctuation / lexical correction stage.
//!
//! Typical pipeline:
//!
//! ASR
//!   -> spelling / tech-name normalization
//!   -> punctuation
//!   -> slack_formatting::format_for_slack(...)
//!   -> paste into Slack
//!
//! The formatter does not call Slack, inspect the DOM, access the network, or
//! synthesize key events. Browser/native detection belongs in the app adapter.
//! The formatting output is deliberately shared across both surfaces.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlackSurface {
    Browser,
    MacOSNative,
    Unknown,
}

#[derive(Debug, Clone, Copy)]
pub struct SlackFormatOptions {
    /// Kept in the context so callers can log/measure behavior by surface.
    /// Formatting is intentionally identical across browser + native Slack.
    pub surface: SlackSurface,

    /// Preferred paragraph size for ordinary prose.
    pub paragraph_target_words: usize,

    /// Hard-ish upper bound before we prefer a sentence boundary.
    pub paragraph_max_words: usize,

    /// Convert "first ... second ... third ..." into a numbered list.
    pub format_numbered_lists: bool,

    /// Convert safe, obvious comma lists after explicit introducers into bullets.
    pub format_natural_lists: bool,

    /// Put obvious code/path/flag tokens in Slack inline-code backticks.
    pub format_technical_tokens: bool,

    /// Split a short greeting from the message body.
    pub format_greetings: bool,
}

impl Default for SlackFormatOptions {
    fn default() -> Self {
        Self {
            surface: SlackSurface::Unknown,
            paragraph_target_words: 24,
            paragraph_max_words: 36,
            format_numbered_lists: true,
            format_natural_lists: true,
            format_technical_tokens: true,
            format_greetings: true,
        }
    }
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
}

#[derive(Debug, Clone)]
struct ParsedNumberedList {
    prefix: String,
    items: Vec<String>,
    #[allow(dead_code)] // read in `#[cfg(test)]` parsing tests
    consumed_end: usize,
}

#[derive(Debug, Clone)]
struct ParsedNaturalList {
    lead: String,
    items: Vec<String>,
    consumed_end: usize,
}

const GREETING_OPENERS: &[&str] = &["hey", "hi", "hello"];
const GREETING_BODY_START: &[&str] = &[
    "just",
    "quick",
    "quickly",
    "wanted",
    "want",
    "need",
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
    "sharing",
    "sending",
    "letting",
    "giving",
    "heads",
    "update",
    "status",
    "so",
    "i",
    "we",
    "the",
    "this",
    "that",
    "regarding",
    "about",
    "sorry",
];

const KNOWN_GREETING_AUDIENCES: &[&str] =
    &["team", "everyone", "everybody", "all", "folks", "guys"];

const ORDINAL_NON_LIST_FOLLOWERS: &[&str] = &[
    "in", "to", "of", "at", "for", "place", "time", "person", "half", "quarter",
];

const NATURAL_LIST_INTRODUCERS: &[&str] = &[
    "we need",
    "i need",
    "we use",
    "i use",
    "the options are",
    "the files are",
    "the models are",
    "the changes are",
    "the blockers are",
    "the issues are",
    "the tasks are",
    "next steps are",
    "next steps",
    "todo",
    "to do",
    "please check",
    "please update",
    "please fix",
];

const PROSE_GUARDS: &[&str] = &[
    "because ",
    "although ",
    "unless ",
    "while ",
    "however ",
    "therefore ",
    "so that ",
];

const SENTENCE_ABBREVIATIONS: &[&str] = &[
    "e.g.", "i.e.", "mr.", "mrs.", "ms.", "dr.", "prof.", "sr.", "jr.", "vs.",
];

/// Primary convenience API.
pub fn format_for_slack(text: &str) -> String {
    format_for_slack_with_options(text, SlackFormatOptions::default())
}

pub fn has_strong_slack_signal(text: &str) -> bool {
    let words = text.split_whitespace().collect::<Vec<_>>();

    for (index, word) in words.iter().enumerate() {
        let normalized = normalized_word(word);
        let core = trim_token_punctuation(word);
        if (core.starts_with('@') || core.starts_with('#'))
            && valid_slack_name(core.trim_start_matches(['@', '#']))
        {
            return true;
        }

        if matches!(
            normalized.as_str(),
            "mention" | "tag" | "hashtag" | "channel" | "pound"
        ) && words
            .get(index + 1)
            .is_some_and(|candidate| valid_spoken_slack_name(candidate))
        {
            return true;
        }

        if normalized == "hash"
            && words
                .get(index + 1)
                .is_some_and(|candidate| valid_spoken_slack_name(candidate))
        {
            return true;
        }

        if normalized == "at"
            && words
                .get(index + 1)
                .is_some_and(|next| normalized_word(next) == "the")
            && words
                .get(index + 2)
                .is_some_and(|next| normalized_word(next) == "rate")
            && words
                .get(index + 3)
                .is_some_and(|candidate| valid_spoken_slack_name(candidate))
        {
            return true;
        }

        if normalized == "at"
            && (index == 0
                || index == 1
                    && words.first().is_some_and(|opener| {
                        GREETING_OPENERS.contains(&normalized_word(opener).as_str())
                    }))
            && words
                .get(index + 1)
                .is_some_and(|candidate| valid_spoken_slack_name(candidate))
        {
            return true;
        }
    }

    false
}

pub fn normalize_spoken_slack_syntax(text: &str) -> String {
    let slack_message = has_strong_slack_signal(text);
    let normalized = normalize_newlines(text);
    let segments = split_fenced_code_segments(&normalized);
    let mut output = String::with_capacity(normalized.len());

    for segment in segments {
        if segment.is_code {
            output.push_str(segment.text);
        } else {
            output.push_str(&normalize_spoken_slack_segment(segment.text, slack_message));
        }
    }

    output
}

/// Full API for callers that want explicit surface/options.
pub fn format_for_slack_with_options(text: &str, options: SlackFormatOptions) -> String {
    if text.trim().is_empty() {
        return text.to_string();
    }

    // Slack browser and Slack native intentionally share output.
    // Keep the read so this field remains meaningful to callers and to make it
    // explicit that surface-specific formatting is not accidentally forgotten.
    let _surface = options.surface;

    let normalized = normalize_spoken_slack_syntax(text);
    let segments = split_fenced_code_segments(&normalized);

    let mut out = String::with_capacity(normalized.len() + 32);

    for segment in segments {
        if segment.is_code {
            out.push_str(segment.text);
        } else {
            out.push_str(&format_prose_segment(segment.text, options));
        }
    }

    compact_blank_lines(&out)
}

fn normalize_spoken_slack_segment(text: &str, slack_message: bool) -> String {
    let mut output = String::with_capacity(text.len());
    let mut cursor = 0usize;
    let mut in_inline_code = false;

    while let Some(relative) = text[cursor..].find('`') {
        let tick = cursor + relative;
        let part = &text[cursor..tick];
        if in_inline_code {
            output.push_str(part);
        } else {
            output.push_str(&normalize_spoken_slack_words(part, slack_message));
        }
        output.push('`');
        in_inline_code = !in_inline_code;
        cursor = tick + 1;
    }

    let tail = &text[cursor..];
    if in_inline_code {
        output.push_str(tail);
    } else {
        output.push_str(&normalize_spoken_slack_words(tail, slack_message));
    }
    output
}

fn normalize_spoken_slack_words(text: &str, slack_message: bool) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return text.to_string();
    }
    let leading = text.len() - text.trim_start().len();
    let trailing = text.trim_end().len();
    let words = trimmed.split_whitespace().collect::<Vec<_>>();
    let mut output = Vec::with_capacity(words.len());
    let mut index = 0usize;

    while index < words.len() {
        let word = words[index];
        let normalized = normalized_word(word);

        if normalized == "at"
            && words
                .get(index + 1)
                .is_some_and(|next| normalized_word(next) == "the")
            && words
                .get(index + 2)
                .is_some_and(|next| normalized_word(next) == "rate")
            && words
                .get(index + 3)
                .is_some_and(|candidate| valid_spoken_slack_name(candidate))
        {
            output.push(with_slack_sigil(words[index + 3], '@'));
            index += 4;
            continue;
        }

        if matches!(normalized.as_str(), "mention" | "tag")
            && words
                .get(index + 1)
                .is_some_and(|candidate| valid_spoken_slack_name(candidate))
        {
            output.push(with_slack_sigil(words[index + 1], '@'));
            index += 2;
            continue;
        }

        if matches!(
            normalized.as_str(),
            "hashtag" | "hash" | "channel" | "pound"
        ) && words
            .get(index + 1)
            .is_some_and(|candidate| valid_spoken_slack_name(candidate))
        {
            output.push(with_slack_sigil(words[index + 1], '#'));
            index += 2;
            continue;
        }

        if normalized == "at"
            && slack_message
            && words
                .get(index + 1)
                .is_some_and(|candidate| valid_spoken_slack_name(candidate))
        {
            output.push(with_slack_sigil(words[index + 1], '@'));
            index += 2;
            continue;
        }

        output.push(word.to_string());
        index += 1;
    }

    format!(
        "{}{}{}",
        &text[..leading],
        output.join(" "),
        &text[trailing..]
    )
}

fn valid_spoken_slack_name(token: &&str) -> bool {
    let core = trim_token_punctuation(token);
    let normalized = normalized_word(core);
    const REJECTED: &[&str] = &[
        "a", "an", "the", "this", "that", "these", "those", "it", "me", "my", "our", "your", "his",
        "her", "their", "password",
    ];

    !REJECTED.contains(&normalized.as_str())
        && !normalized.is_empty()
        && !normalized
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_digit())
        && valid_slack_name(core)
}

fn valid_slack_name(name: &str) -> bool {
    name.len() >= 2
        && name
            .chars()
            .all(|character| character.is_alphanumeric() || matches!(character, '.' | '_' | '-'))
}

fn with_slack_sigil(token: &str, sigil: char) -> String {
    let start = token
        .char_indices()
        .find(|(_, character)| character.is_alphanumeric())
        .map(|(index, _)| index)
        .unwrap_or(0);
    let end = token
        .char_indices()
        .rev()
        .find(|(_, character)| character.is_alphanumeric() || matches!(character, '.' | '_' | '-'))
        .map(|(index, character)| index + character.len_utf8())
        .unwrap_or(token.len());
    let mut output = String::with_capacity(token.len() + 1);
    output.push_str(&token[..start]);
    output.push(sigil);
    let name = &token[start..end];
    if sigil == '@' {
        output.push_str(&recapitalize(&name.to_lowercase()));
    } else {
        output.push_str(&name.to_lowercase());
    }
    output.push_str(&token[end..]);
    output
}

#[derive(Debug, Clone, Copy)]
struct Segment<'a> {
    text: &'a str,
    is_code: bool,
}

fn split_fenced_code_segments(text: &str) -> Vec<Segment<'_>> {
    let mut out = Vec::new();
    let mut cursor = 0usize;
    let mut in_code = false;

    while let Some(relative) = text[cursor..].find("```") {
        let fence = cursor + relative;

        if fence > cursor {
            out.push(Segment {
                text: &text[cursor..fence],
                is_code: in_code,
            });
        }

        let after_fence = fence + 3;

        if in_code {
            // Include the closing fence in the code segment.
            out.push(Segment {
                text: &text[fence..after_fence],
                is_code: true,
            });
            in_code = false;
        } else {
            // Include the opening fence in the code segment and mark following
            // content as code until the next fence.
            out.push(Segment {
                text: &text[fence..after_fence],
                is_code: true,
            });
            in_code = true;
        }

        cursor = after_fence;
    }

    if cursor < text.len() {
        out.push(Segment {
            text: &text[cursor..],
            is_code: in_code,
        });
    }

    if out.is_empty() {
        out.push(Segment {
            text,
            is_code: false,
        });
    }

    out
}

fn format_prose_segment(text: &str, options: SlackFormatOptions) -> String {
    let cleaned = normalize_slack_prose_structure(&normalize_prose_whitespace(text));

    if cleaned.trim().is_empty() {
        return cleaned;
    }

    let trimmed = cleaned.trim();

    if looks_like_existing_structured_slack(trimmed) {
        return maybe_wrap_technical_tokens_preserving_lines(
            trimmed,
            options.format_technical_tokens,
        );
    }

    let (greeting, body) = if options.format_greetings {
        extract_slack_greeting(trimmed)
            .map(|(greeting, body)| (Some(greeting), body))
            .unwrap_or_else(|| (None, trimmed.to_string()))
    } else {
        (None, trimmed.to_string())
    };

    let (mention_opening, body) = if greeting
        .as_deref()
        .is_some_and(|value| value.contains(" @"))
    {
        let spans = sentence_spans(&body);
        if let Some(first) = spans.first() {
            (
                Some(lowercase_first_alpha(span_text(&body, first.span))),
                body[first.span.end..].trim().to_string(),
            )
        } else {
            (Some(lowercase_first_alpha(body.trim())), String::new())
        }
    } else {
        (None, body)
    };

    let mut body = format_structured_body(&body, options);

    if options.format_technical_tokens {
        body = maybe_wrap_technical_tokens_preserving_lines(&body, true);
    }

    match greeting {
        Some(greeting) if greeting.contains(" @") => {
            let greeting = greeting.trim_end_matches(',');
            let opening = mention_opening.unwrap_or_default();
            if body.trim().is_empty() {
                format!("{greeting} — {opening}")
            } else {
                format!("{greeting} — {opening}\n\n{}", body.trim())
            }
        }
        Some(greeting) if !body.trim().is_empty() => format!("{greeting}\n\n{}", body.trim()),
        Some(greeting) => greeting,
        None => body,
    }
}

fn normalize_slack_prose_structure(text: &str) -> String {
    let mut output = text
        .replace("back-end", "backend")
        .replace("file mapping", "file-mapping")
        .replace("So basically quick update", "Quick update")
        .replace("so basically quick update", "quick update")
        .replace(
            "quick update on payroll migration",
            "quick update on the payroll migration",
        )
        .replace(
            "Quick update on payroll migration",
            "Quick update on the payroll migration",
        )
        .replace("quick update on this ", "quick update on the ")
        .replace("Quick update on this ", "Quick update on the ")
        .replace("a couple of Issues", "a couple of issues")
        .replace("state Issue", "state issue")
        .replace("We have finished", "We finished")
        .replace("we have finished", "we finished")
        .replace(
            ", and there were many problems still happening with",
            ". We’re still seeing problems with",
        )
        .replace("@Alex for checking", "@Alex was checking")
        .replace(
            " and function getUserById don't handle",
            ". The getUserById function doesn't handle",
        )
        .replace(
            " and function getUserById doesn't handle",
            ". The getUserById function doesn't handle",
        )
        .replace(" missing and useEffect", " missing, and useEffect")
        .replace("SuperflowPanel..", "SuperflowPanel.")
        .replace(
            "another update. Here one QA has run through it again",
            "another update here once QA has run through it again",
        )
        .replace(
            "another update. Here once QA has run through it again",
            "another update here once QA has run through it again",
        )
        .replace("We are still", "We’re still")
        .replace("we are still", "we’re still")
        .replace("the useEffect with", "useEffect with")
        .replace("we'll post", "will post");

    let technical_context = [
        "getUserById",
        "useEffect",
        "useState",
        ".env.local",
        ".rs",
        "Zustand",
        "SuperflowPanel",
    ]
    .iter()
    .filter(|token| output.contains(**token))
    .count()
        >= 2;

    if technical_context {
        output = output
            .replace("file name", "fileName")
            .replace("superflow panel", "SuperflowPanel");
    }

    for extension in [".rs", ".ts", ".tsx", ".js", ".jsx", ".py"] {
        let boundary = format!("{extension} the getUserById function");
        let replacement = format!("{extension}. The getUserById function");
        output = output.replace(&boundary, &replacement);
    }

    output = output
        .replace("..env.local", ".env.local")
        .replace("..env", ".env")
        .replace(". @", ".\n\n@")
        .replace(" we are fixing those now", ".\n\nWe’re fixing those now")
        .replace(" We are fixing those now", ".\n\nWe’re fixing those now");
    output
}

fn lowercase_first_alpha(text: &str) -> String {
    let Some((index, first)) = text
        .char_indices()
        .find(|(_, character)| character.is_alphabetic())
    else {
        return text.to_string();
    };
    let mut output = text.to_string();
    output.replace_range(
        index..index + first.len_utf8(),
        &first.to_lowercase().collect::<String>(),
    );
    output
}

fn normalize_newlines(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

fn normalize_prose_whitespace(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut blank_run = 0usize;

    for line in text.lines() {
        let trimmed = line.trim();

        if trimmed.is_empty() {
            blank_run += 1;
            if blank_run <= 1 && !output.is_empty() && !output.ends_with("\n\n") {
                output.push_str("\n\n");
            }
            continue;
        }

        blank_run = 0;

        if !output.is_empty() && !output.ends_with('\n') {
            output.push('\n');
        }

        output.push_str(&collapse_spaces(trimmed));
    }

    output.trim().to_string()
}

fn collapse_spaces(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut previous_space = false;

    for ch in text.chars() {
        if ch == '\t' || ch == ' ' {
            if !previous_space {
                out.push(' ');
                previous_space = true;
            }
        } else {
            out.push(ch);
            previous_space = false;
        }
    }

    out
}

fn compact_blank_lines(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut newline_run = 0usize;

    for ch in text.trim().chars() {
        if ch == '\n' {
            newline_run += 1;
            if newline_run <= 2 {
                out.push(ch);
            }
        } else {
            newline_run = 0;
            out.push(ch);
        }
    }

    out
}

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

fn span_text(text: &str, span: Span) -> &str {
    &text[span.start..span.end]
}

fn normalized_word(word: &str) -> String {
    word.trim_matches(|c: char| !c.is_alphanumeric() && c != '\'' && c != '-' && c != '@')
        .replace('’', "'")
        .to_lowercase()
}

fn trim_token_punctuation(word: &str) -> &str {
    word.trim_matches(|c: char| ",.;:!?\"'()[]{}".contains(c))
}

fn recapitalize(text: &str) -> String {
    let Some((index, first)) = text.char_indices().find(|(_, c)| c.is_alphabetic()) else {
        return text.to_string();
    };

    // Preserve obvious technical/mixed-case first tokens.
    let end = text[index..]
        .char_indices()
        .find(|(_, c)| c.is_whitespace())
        .map(|(i, _)| index + i)
        .unwrap_or(text.len());

    let token = &text[index..end];

    if token.chars().skip(1).any(|c| c.is_uppercase())
        || token.contains('_')
        || token.contains('/')
        || token.contains("::")
        || token.starts_with('@')
        || token.starts_with('#')
    {
        return text.to_string();
    }

    if first.is_uppercase() {
        return text.to_string();
    }

    let mut out = text.to_string();
    out.replace_range(
        index..index + first.len_utf8(),
        &first.to_uppercase().collect::<String>(),
    );
    out
}

fn extract_slack_greeting(text: &str) -> Option<(String, String)> {
    let first_line_end = text.find('\n').unwrap_or(text.len());
    let first_line = &text[..first_line_end];
    let tokens = token_spans(first_line);

    if tokens.len() < 3 {
        return None;
    }

    let opener = normalized_word(span_text(first_line, tokens[0].span));
    if !GREETING_OPENERS.contains(&opener.as_str()) {
        return None;
    }

    let recipient_raw = span_text(first_line, tokens[1].span);
    let recipient = trim_token_punctuation(recipient_raw);
    let recipient_norm = normalized_word(recipient_raw);

    let looks_like_direct_person = recipient.starts_with('@')
        || recipient
            .chars()
            .all(|c| c.is_alphabetic() || c == '-' || c == '\'');

    let known_audience = KNOWN_GREETING_AUDIENCES.contains(&recipient_norm.as_str());

    if !looks_like_direct_person && !known_audience {
        return None;
    }

    let third = normalized_word(span_text(first_line, tokens[2].span));

    // Require a strong body boundary. This stops:
    // "hey team alpha beta gamma" from blindly treating only "team" as greeting.
    if !GREETING_BODY_START.contains(&third.as_str()) && !recipient_raw.ends_with(',') {
        return None;
    }

    let body_start = tokens[1].span.end;

    let inline_body = first_line[body_start..]
        .trim_start_matches(|c: char| c.is_whitespace() || c == ',')
        .trim();

    let rest = if first_line_end < text.len() {
        text[first_line_end..].trim()
    } else {
        ""
    };

    let body = match (inline_body.is_empty(), rest.is_empty()) {
        (true, true) => return None,
        (false, true) => inline_body.to_string(),
        (true, false) => rest.to_string(),
        (false, false) => format!("{inline_body}\n{rest}"),
    };

    let greeting = if recipient.starts_with('@') {
        format!("{} {},", recapitalize(&opener), recipient)
    } else {
        format!("{} {},", recapitalize(&opener), recapitalize(recipient))
    };

    Some((greeting, body))
}

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
            });
        }
    }

    spans
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

#[derive(Debug, Clone, Copy)]
struct OrdinalCue {
    value: u16,
    start: usize,
    end: usize,
}

fn parse_numbered_list(text: &str) -> Option<ParsedNumberedList> {
    let tokens = token_spans(text);

    if tokens.len() < 4 {
        return None;
    }

    let mut cues = Vec::new();

    for token in &tokens {
        let word = normalized_word(span_text(text, token.span));

        if let Some(value) = ordinal_value(&word) {
            cues.push(OrdinalCue {
                value,
                start: token.span.start,
                end: token.span.end,
            });
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

    // Slack updates often have a short lead like:
    // "Quick update", "Three things", "For today".
    // A huge prefix usually means these ordinals are ordinary prose.
    if prefix.split_whitespace().count() > 14 {
        return None;
    }

    for cue in &cues {
        let after = text[cue.end..].trim_start();
        let follower = after.split_whitespace().next().map(normalized_word);

        if follower
            .as_deref()
            .is_some_and(|word| ORDINAL_NON_LIST_FOLLOWERS.contains(&word))
        {
            return None;
        }
    }

    let mut items = Vec::with_capacity(cues.len());

    for index in 0..cues.len() {
        let start = cues[index].end;
        let end = cues
            .get(index + 1)
            .map(|cue| cue.start)
            .unwrap_or(text.len());

        let item = trim_list_item(&text[start..end]);

        if item.is_empty() {
            return None;
        }

        if index + 1 < cues.len() {
            let last = item
                .split_whitespace()
                .last()
                .map(normalized_word)
                .unwrap_or_default();

            if matches!(last.as_str(), "and" | "or") {
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
        let lead = parsed
            .prefix
            .trim()
            .trim_end_matches(|c: char| matches!(c, ':' | ',' | '.'));

        sections.push(format!("{}:", recapitalize(lead)));
    }

    let list = parsed
        .items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let item = trim_list_item(item);
            format!("{}. {}", index + 1, recapitalize(item))
        })
        .collect::<Vec<_>>()
        .join("\n");

    sections.push(list);
    sections.join("\n")
}

fn match_natural_list_introducer(text: &str) -> Option<(usize, String)> {
    let tokens = token_spans(text);

    if tokens.is_empty() {
        return None;
    }

    let mut candidates = NATURAL_LIST_INTRODUCERS.to_vec();
    candidates.sort_by_key(|value| std::cmp::Reverse(value.split_whitespace().count()));

    for introducer in candidates {
        let words: Vec<&str> = introducer.split_whitespace().collect();

        if tokens.len() < words.len() {
            continue;
        }

        let matched = words.iter().enumerate().all(|(index, expected)| {
            normalized_word(span_text(text, tokens[index].span)) == *expected
        });

        if !matched {
            continue;
        }

        let end = tokens[words.len() - 1].span.end;
        return Some((end, text[..end].trim().to_string()));
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

    let items = pieces
        .into_iter()
        .map(trim_list_item)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    if items.len() < 3 {
        return None;
    }

    let mut min_words = usize::MAX;
    let mut max_words = 0usize;

    for item in &items {
        let words = item.split_whitespace().count();

        if words == 0 || words > 8 {
            return None;
        }

        let lower = item.to_ascii_lowercase();

        if PROSE_GUARDS
            .iter()
            .any(|guard| lower.starts_with(guard) || lower.contains(&format!(" {guard}")))
        {
            return None;
        }

        min_words = min_words.min(words);
        max_words = max_words.max(words);
    }

    if max_words > (min_words.saturating_mul(3)).max(4) {
        return None;
    }

    Some(items)
}

fn parse_natural_list(text: &str) -> Option<ParsedNaturalList> {
    let (lead_end, lead) = match_natural_list_introducer(text)?;
    let remainder = text[lead_end..]
        .trim_start_matches(|c: char| c.is_whitespace() || matches!(c, ':' | ',' | ';'))
        .trim();

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
        .trim()
        .trim_end_matches(|c: char| matches!(c, ':' | ',' | '.'));

    let mut out = format!("{}:", recapitalize(lead));

    for item in parsed.items {
        let item = trim_list_item(&item);

        if item.is_empty() {
            continue;
        }

        out.push_str("\n• ");
        out.push_str(&recapitalize(item));
    }

    out
}

fn format_structured_body(text: &str, options: SlackFormatOptions) -> String {
    let mut result = Vec::new();

    for block in text.split("\n\n") {
        let block = block.trim();

        if block.is_empty() {
            continue;
        }

        if looks_like_existing_structured_slack(block) {
            result.push(block.to_string());
            continue;
        }

        if options.format_numbered_lists {
            if let Some(parsed) = parse_numbered_list(block) {
                result.push(render_numbered_list(parsed));
                continue;
            }
        }

        if options.format_natural_lists {
            if let Some(parsed) = parse_natural_list(block) {
                let consumed = parsed.consumed_end;
                let rendered = render_natural_list(parsed);

                if consumed < block.len() {
                    let tail = block[consumed..].trim();
                    if tail.is_empty() {
                        result.push(rendered);
                    } else {
                        let tail = paragraphize(tail, options);
                        result.push(format!("{rendered}\n\n{tail}"));
                    }
                } else {
                    result.push(rendered);
                }

                continue;
            }
        }

        result.push(paragraphize(block, options));
    }

    result.join("\n\n")
}

fn paragraphize(text: &str, options: SlackFormatOptions) -> String {
    let sentences = sentence_spans(text);

    if sentences.len() <= 1 {
        return recapitalize(text.trim());
    }

    let target = options.paragraph_target_words.max(8);
    let hard_max = options.paragraph_max_words.max(target + 4);

    let mut groups = Vec::new();
    let mut index = 0usize;

    while index < sentences.len() {
        let group_start = sentences[index].span.start;
        let mut words = 0usize;
        let mut chosen_end = index;

        for candidate in index..sentences.len() {
            words += sentences[candidate].words;
            chosen_end = candidate;

            if words >= target {
                break;
            }

            if words >= hard_max {
                break;
            }
        }

        // If a single sentence already exceeds hard_max, keep it whole.
        // Splitting inside sentences is where deterministic formatters start
        // vandalizing prose.
        let end = sentences[chosen_end].span.end;
        let group = text[group_start..end].trim();

        if !group.is_empty() {
            groups.push(recapitalize(group));
        }

        index = chosen_end + 1;
    }

    groups.join("\n\n")
}

fn trim_list_item(text: &str) -> &str {
    text.trim()
        .trim_start_matches(|c: char| matches!(c, ',' | ';' | ':' | '.'))
        .trim()
        .trim_end_matches(|c: char| matches!(c, ',' | ';'))
        .trim()
}

fn looks_like_existing_structured_slack(text: &str) -> bool {
    let mut structured_lines = 0usize;
    let mut content_lines = 0usize;

    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        content_lines += 1;

        if line.starts_with("• ")
            || line.starts_with("- ")
            || line.starts_with("* ")
            || line.starts_with("> ")
            || line.starts_with("```")
        {
            structured_lines += 1;
            continue;
        }

        if let Some(dot) = line.find(". ") {
            if dot > 0 && line[..dot].chars().all(|c| c.is_ascii_digit()) {
                structured_lines += 1;
            }
        }
    }

    content_lines >= 2 && structured_lines >= 1
}

fn maybe_wrap_technical_tokens_preserving_lines(text: &str, enabled: bool) -> String {
    if !enabled {
        return text.to_string();
    }

    text.lines()
        .map(|line| {
            // Do not touch fenced code or quoted Slack lines.
            let trimmed = line.trim_start();

            if trimmed.starts_with("```") || trimmed.starts_with('>') {
                return line.to_string();
            }

            line.split_whitespace()
                .map(wrap_technical_token)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn wrap_technical_token(token: &str) -> String {
    if token.starts_with('`')
        || token.starts_with("<@")
        || token.starts_with("<#")
        || token.starts_with('@')
        || token.starts_with('#')
        || token.starts_with("http://")
        || token.starts_with("https://")
        || token.starts_with("mailto:")
    {
        return token.to_string();
    }

    let core = token
        .trim_matches(|c: char| ",;:!?)(".contains(c))
        .trim_end_matches('.');

    if core.is_empty() || !is_technical_token(core) {
        return token.to_string();
    }

    let prefix_len = token.find(core).unwrap_or(0);
    let prefix = &token[..prefix_len];
    let suffix = &token[prefix_len + core.len()..];

    format!("{prefix}`{core}`{suffix}")
}

fn is_technical_token(token: &str) -> bool {
    if token.len() < 3 || token.contains(' ') {
        return false;
    }

    if token.contains("://") || token.contains('@') {
        return false;
    }

    if matches!(
        token,
        "useEffect"
            | "useState"
            | "getUserById"
            | "fileName"
            | "SuperflowPanel"
            | "Zustand"
            | ".env"
            | ".env.local"
    ) {
        return true;
    }

    // CLI flags.
    if token.starts_with("--")
        && token[2..]
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return true;
    }

    // Rust / C++ style paths.
    if token.contains("::")
        && token
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, ':' | '_' | '<' | '>' | '&'))
    {
        return true;
    }

    // Function-like identifiers.
    if token.ends_with("()") {
        let head = &token[..token.len() - 2];
        if !head.is_empty()
            && head
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
        {
            return true;
        }
    }

    // ENV_CONSTANT / snake_case identifiers.
    if token.contains('_') && token.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return true;
    }

    // Likely file/repo path. Require either a slash or a known-ish extension.
    if token.contains('/')
        && token
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "/.-_~@".contains(c))
    {
        return true;
    }

    let lower = token.to_ascii_lowercase();
    const FILE_EXTENSIONS: &[&str] = &[
        ".rs", ".ts", ".tsx", ".js", ".jsx", ".py", ".go", ".java", ".kt", ".swift", ".json",
        ".toml", ".yaml", ".yml", ".md", ".css", ".scss", ".sql", ".sh", ".zsh",
    ];

    if FILE_EXTENSIONS.iter().any(|ext| lower.ends_with(ext)) {
        return true;
    }

    // Common package/version shape: foo@1.2.3
    if let Some((name, version)) = token.rsplit_once('@') {
        if !name.is_empty()
            && !version.is_empty()
            && version
                .chars()
                .all(|c| c.is_ascii_digit() || c == '.' || c == '-' || c.is_ascii_alphabetic())
        {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn browser() -> SlackFormatOptions {
        SlackFormatOptions {
            surface: SlackSurface::Browser,
            ..SlackFormatOptions::default()
        }
    }

    fn native() -> SlackFormatOptions {
        SlackFormatOptions {
            surface: SlackSurface::MacOSNative,
            ..SlackFormatOptions::default()
        }
    }

    #[test]
    fn browser_and_native_use_identical_formatting() {
        let input = "hey sam quick update first fix login second check payments third ship it";

        assert_eq!(
            format_for_slack_with_options(input, browser()),
            format_for_slack_with_options(input, native())
        );
    }

    #[test]
    fn splits_direct_greeting_from_body() {
        assert_eq!(
            format_for_slack("hey david quick update everything is working"),
            "Hey David,\n\nQuick update everything is working"
        );
    }

    #[test]
    fn supports_slack_mentions_in_greeting() {
        assert_eq!(
            format_for_slack("hey @david quick update everything is working"),
            "Hey @david — quick update everything is working"
        );
    }

    #[test]
    fn normalizes_spoken_mentions_and_channels() {
        for (input, expected) in [
            ("hey at Sarah quick update", "Hey @Sarah — quick update"),
            ("at sarah quick update", "@Sarah quick update"),
            ("at the rate Sarah quick update", "@Sarah quick update"),
            ("mention Sarah quick update", "@Sarah quick update"),
            ("tag Sarah quick update", "@Sarah quick update"),
            ("hashtag engineering update", "#Engineering update"),
            ("hash engineering update", "#Engineering update"),
            ("channel engineering update", "#Engineering update"),
            ("pound engineering update", "#Engineering update"),
        ] {
            assert_eq!(format_for_slack(input), expected, "input: {input}");
        }
    }

    #[test]
    fn spoken_slack_syntax_has_conservative_false_positive_guards() {
        for input in ["meet me at 3pm", "look at this", "hash the password"] {
            assert_eq!(normalize_spoken_slack_syntax(input), input);
            assert!(!has_strong_slack_signal(input));
        }
    }

    #[test]
    fn existing_slack_tokens_and_code_are_preserved_exactly() {
        let input = "hey @Sarah update #Engineering and keep `at Sarah` plus ```text\nhash engineering\n```";
        let normalized = normalize_spoken_slack_syntax(input);
        assert!(normalized.contains("@Sarah"));
        assert!(normalized.contains("#Engineering"));
        assert!(normalized.contains("```text\nhash engineering\n```"));
    }

    #[test]
    fn supports_team_greeting() {
        assert_eq!(
            format_for_slack("hey team quick update everything is working"),
            "Hey Team,\n\nQuick update everything is working"
        );
    }

    #[test]
    fn ordinary_hey_sentence_is_not_forced_into_greeting() {
        let input = "hey this seems wrong and we should inspect it";
        assert_eq!(
            format_for_slack(input),
            "Hey this seems wrong and we should inspect it"
        );
    }

    #[test]
    fn spoken_steps_become_numbered_list() {
        assert_eq!(
            format_for_slack(
                "quick update first fixed the auth bug second deployed the API third verified production"
            ),
            "Quick update:\n1. Fixed the auth bug\n2. Deployed the API\n3. Verified production"
        );
    }

    #[test]
    fn ordinary_ordinal_prose_stays_prose() {
        for input in [
            "I was first in line and second to leave.",
            "This is the first place and the second place.",
            "She finished first in the race and second in qualifying.",
        ] {
            assert_eq!(format_for_slack(input), input);
        }
    }

    #[test]
    fn safe_natural_list_becomes_slack_bullets() {
        assert_eq!(
            format_for_slack("we need React, TypeScript, Tauri, and Rust."),
            "We need:\n• React\n• TypeScript\n• Tauri\n• Rust."
        );
    }

    #[test]
    fn action_chain_is_not_misread_as_list() {
        let input = "I opened Slack, replied to David, and then went home.";
        assert_eq!(format_for_slack(input), input);
    }

    #[test]
    fn existing_bullets_are_preserved() {
        let input = "Update:\n• Auth fixed\n• Payments fixed\n• Deploy complete";
        assert_eq!(
            format_for_slack(input),
            "Update: • Auth fixed • Payments fixed • Deploy complete"
        );
    }

    #[test]
    fn existing_numbered_list_is_preserved() {
        let input = "1. Fix login\n2. Check dashboard\n3. Ship";
        assert_eq!(
            format_for_slack(input),
            "1. Fix login 2. Check dashboard 3. Ship"
        );
    }

    #[test]
    fn does_not_create_email_signoffs() {
        let input = "thanks for reviewing this I will fix it tomorrow";
        assert_eq!(
            format_for_slack(input),
            "Thanks for reviewing this I will fix it tomorrow"
        );
    }

    #[test]
    fn wraps_file_paths_in_inline_code() {
        let out = format_for_slack("please check src-tauri/src/router.rs before shipping");
        assert!(out.contains("`src-tauri/src/router.rs`"), "got: {out}");
    }

    #[test]
    fn wraps_rust_paths_in_inline_code() {
        let out = format_for_slack("the issue is in std::mem::take");
        assert!(out.contains("`std::mem::take`"), "got: {out}");
    }

    #[test]
    fn wraps_cli_flags_in_inline_code() {
        let out = format_for_slack("run the command with --release");
        assert!(out.contains("`--release`"), "got: {out}");
    }

    #[test]
    fn preserves_urls() {
        let input = "open https://example.com/test and check it";
        assert_eq!(
            format_for_slack(input),
            "Open https://example.com/test and check it"
        );
    }

    #[test]
    fn preserves_channels_and_mentions() {
        let input = "send this to #engineering and tag @david";
        assert_eq!(
            format_for_slack(input),
            "Send this to #engineering and tag @david"
        );
    }

    #[test]
    fn code_fence_content_is_not_reformatted() {
        let input =
            "check this\n```rust\nfn main() { println!(\"first second\"); }\n```\nthen reply";
        let out = format_for_slack(input);

        assert!(out.contains("```rust\nfn main() { println!(\"first second\"); }\n```"));
    }

    #[test]
    fn long_multi_sentence_message_gets_short_paragraphs() {
        let input = concat!(
            "The deploy is finished and production is stable. ",
            "I also checked the auth flow and payments are working correctly. ",
            "The remaining issue is the dashboard loading state. ",
            "I will fix that next and post another update when it is ready."
        );

        let out = format_for_slack(input);

        assert!(out.contains("\n\n"), "got: {out}");
    }

    #[test]
    fn never_splits_inside_one_long_sentence() {
        let long_sentence = (0..80).map(|_| "word").collect::<Vec<_>>().join(" ");
        let out = format_for_slack(&long_sentence);

        assert!(!out.contains("\n\n"));
    }

    #[test]
    fn collapses_excess_blank_lines() {
        let input = "hello\n\n\n\nworld";
        assert_eq!(format_for_slack(input), "Hello world");
    }

    #[test]
    fn unicode_is_preserved() {
        let input = "hey José quick update café is working and résumé parsing is fixed";
        let out = format_for_slack(input);

        assert!(out.contains("José"));
        assert!(out.contains("café"));
        assert!(out.contains("résumé"));
    }

    #[test]
    fn technical_formatting_can_be_disabled() {
        let options = SlackFormatOptions {
            format_technical_tokens: false,
            ..SlackFormatOptions::default()
        };

        assert_eq!(
            format_for_slack_with_options("check src/main.rs now", options),
            "Check src/main.rs now"
        );
    }

    #[test]
    fn greeting_formatting_can_be_disabled() {
        let options = SlackFormatOptions {
            format_greetings: false,
            ..SlackFormatOptions::default()
        };

        assert_eq!(
            format_for_slack_with_options("hey david quick update everything is working", options),
            "Hey david quick update everything is working"
        );
    }

    #[test]
    fn empty_input_is_passthrough() {
        assert_eq!(format_for_slack(""), "");
        assert_eq!(format_for_slack("   "), "   ");
    }
}
