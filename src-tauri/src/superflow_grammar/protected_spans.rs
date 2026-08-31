use harper_core::Span;
use once_cell::sync::Lazy;
use regex::Regex;

/// Protected span types — first-class Superflow subsystem.
/// Any grammar lint whose span overlaps a protected span is dropped.
/// This gives 9.5-10/10 preservation without trusting dictionary alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtectedKind {
    Path,
    Filename,
    Url,
    Email,
    CamelCase,
    PascalCase,
    SnakeCase,
    KebabCase,
    Version,
    Command,
    Package,
    Mention,
    InlineCode,
}

#[derive(Debug, Clone)]
pub struct ProtectedText {
    masked: String,
    originals: Vec<String>,
}

impl ProtectedText {
    pub fn new(text: &str) -> Self {
        let spans = find_protected_spans(text);
        let source: Vec<char> = text.chars().collect();
        let originals = spans
            .iter()
            .map(|span| source[span.start..span.end].iter().collect())
            .collect::<Vec<String>>();
        let mut masked = source;
        for (index, span) in spans.iter().enumerate().rev() {
            let marker: Vec<char> = format!("\u{e000}{index}\u{e001}").chars().collect();
            masked.splice(span.start..span.end, marker);
        }
        Self {
            masked: masked.into_iter().collect(),
            originals,
        }
    }

    pub fn masked(&self) -> &str {
        &self.masked
    }

    pub fn restore(&self, text: &str) -> String {
        let mut restored = text.to_string();
        for (index, original) in self.originals.iter().enumerate() {
            restored = restored.replace(&format!("\u{e000}{index}\u{e001}"), original);
        }
        restored
    }
}

static REGEX_URL: Lazy<Regex> = Lazy::new(|| Regex::new(r"https?://\S+|www\.\S+").unwrap());
static REGEX_EMAIL: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b[\w.%-]+@[\w.-]+\.\w+\b").unwrap());
static REGEX_PATH: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b(?:[~./]?[\w.-]+/)+[\w.-]+(?:\.\w+)?\b").unwrap());
static REGEX_FILENAME: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b[\w.-]+\.(?:rs|ts|tsx|js|jsx|py|json|toml|md|css|html|rs|go|cpp|c|h)\b").unwrap()
});
static REGEX_CAMEL: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b[a-z]+[A-Z][a-zA-Z0-9]*\b").unwrap());
static REGEX_PASCAL: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b[A-Z][a-z]+(?:[A-Z][a-zA-Z0-9]*)+\b").unwrap());
static REGEX_SNAKE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b\w+_\w+\b").unwrap());
static REGEX_KEBAB_TECH: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b(?:[a-z0-9]+-){1,}[a-z0-9]+\b").unwrap());
static REGEX_VERSION: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b(?:v)?\d+\.\d+(?:\.\d+)?(?:[-+][\w.]+)?\b").unwrap());
static REGEX_INLINE_CODE: Lazy<Regex> = Lazy::new(|| Regex::new(r"`[^`]+`").unwrap());
static REGEX_PACKAGE: Lazy<Regex> = Lazy::new(|| {
    // harper-core, tauri-plugin-clipboard-manager, etc.
    Regex::new(r"\b[\w.-]+(?:-core|-plugin|-manager)(?:-[\w.-]+)*\b").unwrap()
});
static REGEX_MENTION: Lazy<Regex> = Lazy::new(|| Regex::new(r"[@#][\w.-]+\b").unwrap());

/// Convert byte indices (from regex on `&str`) to char indices (harper `Span<char>`).
fn byte_to_char_span(text: &str, byte_start: usize, byte_end: usize) -> Span<char> {
    let char_start = text[..byte_start].chars().count();
    let char_len = text[byte_start..byte_end].chars().count();
    Span::new(char_start, char_start + char_len)
}

/// Find all protected spans in `text`, merged and sorted.
/// Spans are in harper `Span<char>` (char indices, not bytes).
pub fn find_protected_spans(text: &str) -> Vec<Span<char>> {
    let mut spans: Vec<Span<char>> = Vec::new();
    let mut push_regex = |re: &Regex| {
        for m in re.find_iter(text) {
            spans.push(byte_to_char_span(text, m.start(), m.end()));
        }
    };

    // Order matters for overlap merging later — collect all then merge.
    push_regex(&REGEX_INLINE_CODE);
    push_regex(&REGEX_URL);
    push_regex(&REGEX_EMAIL);
    push_regex(&REGEX_PATH);
    push_regex(&REGEX_FILENAME);
    push_regex(&REGEX_PACKAGE);
    push_regex(&REGEX_MENTION);
    push_regex(&REGEX_VERSION);
    push_regex(&REGEX_PASCAL);
    push_regex(&REGEX_CAMEL);
    push_regex(&REGEX_SNAKE);
    // Kebab is last and most greedy — filter to tech-like kebab only if contains known tech token?
    // For preservation we are aggressive: any kebab with 2+ segments is protected.
    push_regex(&REGEX_KEBAB_TECH);

    if spans.is_empty() {
        return spans;
    }

    // Sort by start and merge overlapping/adjacent spans
    spans.sort_by_key(|s| s.start);
    let mut merged: Vec<Span<char>> = Vec::new();
    let mut cur = spans[0];
    for s in spans.into_iter().skip(1) {
        if s.start <= cur.end {
            // overlap or adjacent — extend
            cur.end = cur.end.max(s.end);
        } else {
            merged.push(cur);
            cur = s;
        }
    }
    merged.push(cur);
    merged
}

/// True if `lint_span` overlaps any protected span.
pub fn is_protected(lint_span: Span<char>, protected: &[Span<char>]) -> bool {
    protected.iter().any(|p| p.overlaps_with(lint_span))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protects_file_paths() {
        let text = "the file src-tauri/src/transcript/cleanup.rs contains function";
        let spans = find_protected_spans(text);
        assert!(!spans.is_empty());
        // cleanup.rs should be inside a protected span.
        let has = spans.iter().any(|s| {
            let substr: String = text.chars().skip(s.start).take(s.end - s.start).collect();
            substr.contains("cleanup.rs")
        });
        assert!(has, "spans: {:?} text: {}", spans, text);
    }

    #[test]
    fn protects_camel_and_snake() {
        let text = "getUserById fileName myVarName handlePaste";
        let spans = find_protected_spans(text);
        assert!(spans.len() >= 3);
    }

    #[test]
    fn protects_urls_and_emails() {
        let text = "see https://example.com and alex@example.com";
        let spans = find_protected_spans(text);
        assert!(spans.len() >= 2);
    }

    #[test]
    fn protects_kebab_packages() {
        let text = "harper-core tauri-plugin-clipboard-manager";
        let spans = find_protected_spans(text);
        assert!(spans.len() >= 2);
    }

    #[test]
    fn merges_overlapping() {
        let text = "src-tauri/src/transcript/cleanup.rs";
        let spans = find_protected_spans(text);
        // Should be merged into 1 span covering the whole path, not 2 fragments
        assert_eq!(spans.len(), 1);
    }

    #[test]
    fn masks_and_restores_exact_bytes() {
        let input = "use ZustandStore in src-tauri/src/foo.rs via /api/parse for foo@bar.com";
        let protected = ProtectedText::new(input);
        assert!(!protected.masked().contains("ZustandStore"));
        assert_eq!(protected.restore(protected.masked()), input);
    }
}
