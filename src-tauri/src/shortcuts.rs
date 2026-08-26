//! User-defined dictation shortcuts.
//!
//! A shortcut is a user-named snippet ("design prompt", "work email") whose
//! content is expanded inline when the dictated transcript references it.
//! Deterministic and local: quoted references always expand
//! (`using my "design prompt"`); unquoted full-name mentions expand on word
//! boundaries (`tell him i changed my work email`). Everything else — links,
//! phone numbers, emails inside the content — is passed through verbatim.
//!
//! Expansion format follows the speaker's phrasing shape:
//! - short single-line content  -> inline replacement (`hduggal@superflow.ai`)
//! - long / multiline content   -> markdown block (`\n\n# design prompt\n…`)
//!
//! Runs after smart file references, before any optional AI stage. Cost is a
//! handful of case-insensitive scans over the transcript: microseconds for
//! realistic shortcut counts, far under the 100ms enrichment budget.

use crate::settings::Shortcut;
use log::debug;
use std::time::Instant;

/// Content at or below this length (single-line) expands inline.
const INLINE_MAX_CHARS: usize = 60;

pub(crate) fn expand_shortcuts(shortcuts: &[Shortcut], text: &str) -> Option<String> {
    if shortcuts.is_empty() || text.trim().is_empty() {
        return None;
    }
    let start = Instant::now();
    let lower = text.to_lowercase();

    // Longest names first so "design prompt v2" wins over "design prompt".
    let mut ordered: Vec<&Shortcut> = shortcuts.iter().collect();
    ordered.sort_by_key(|s| std::cmp::Reverse(s.name.chars().count()));

    let mut out = text.to_string();
    let mut replaced = false;

    for shortcut in ordered {
        let name = shortcut.name.trim();
        if name.is_empty() {
            continue;
        }
        let Some(span) = find_reference(&lower, &out, name) else {
            continue;
        };
        let replacement = replacement_for(shortcut);
        out.replace_range(span, &replacement);
        replaced = true;
        debug!(
            "shortcuts: expanded '{}' ({} chars)",
            name,
            shortcut.content.len()
        );
        // Re-lowercase only when we keep scanning (cheap; transcripts are short).
        return finish(shortcuts, out, replaced, start);
    }
    finish(shortcuts, out, replaced, start)
}

fn finish(shortcuts: &[Shortcut], out: String, replaced: bool, start: Instant) -> Option<String> {
    if !replaced {
        return None;
    }
    debug!("shortcuts: total expansion took {:?}", start.elapsed());
    let _ = shortcuts;
    Some(out)
}

/// Locate the first reference to `name` inside the current working text.
/// Returns the byte span to replace. Handles straight + curly quotes around
/// the name, otherwise requires whole-word occurrence (case-insensitive via
/// pre-lowered haystack rebuilt lazily by caller passing `&lower` of `out`).
fn find_reference(
    lower_haystack_of_original: &str,
    current: &str,
    name: &str,
) -> Option<std::ops::Range<usize>> {
    let _ = lower_haystack_of_original;
    let hay = current.to_lowercase();
    let needle = name.to_lowercase();

    // 1. Quoted forms win: "name", 'name', "name", ‘name’, „name"
    for q in ['"', '\'', '\u{201c}', '\u{2018}', '\u{201e}'] {
        let close = match q {
            '\u{201c}' => '\u{201d}',
            '\u{2018}' => '\u{2019}',
            '\u{201e}' => '\u{201c}',
            other => other,
        };
        let mut from = 0usize;
        while let Some(rel) = hay[from..].find(q) {
            let open_abs = from + rel;
            let rest_from = open_abs + q.len_utf8();
            if let Some(close_rel) = hay[rest_from..].find(close) {
                let close_abs = rest_from + close_rel;
                let inner = hay[rest_from..close_abs].trim();
                if inner == needle {
                    // Extend through trailing punctuation of the closing quote.
                    let mut end = close_abs + close.len_utf8();
                    let bytes = current.as_bytes();
                    while end < bytes.len()
                        && (bytes[end] == b','
                            || bytes[end] == b'.'
                            || bytes[end] == b';'
                            || bytes[end] == b':'
                            || bytes[end] == b'!')
                    {
                        end += 1;
                    }
                    return Some(open_abs..end);
                }
                from = close_abs + close.len_utf8();
            } else {
                break;
            }
        }
    }

    // 2. Whole-word unquoted mention.
    let mut search_from = 0usize;
    while let Some(rel) = hay[search_from..].find(&needle) {
        let start = search_from + rel;
        let end = start + needle.len();
        let before_ok = current[..start]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_alphanumeric());
        let after_ok = current[end..]
            .chars()
            .next()
            .is_none_or(|c| !c.is_alphanumeric());
        if before_ok && after_ok {
            return Some(start..end);
        }
        search_from = end.max(search_from + 1);
    }
    None
}

fn replacement_for(shortcut: &Shortcut) -> String {
    let content = shortcut.content.trim_end();
    let is_multiline = content.contains('\n');
    let is_long = content.chars().count() > INLINE_MAX_CHARS;
    if is_multiline || is_long {
        format!("\n\n# {}\n{}\n", shortcut.name.trim(), content)
    } else {
        content.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sc(name: &str, content: &str) -> Shortcut {
        Shortcut {
            id: format!("sc_{name}"),
            name: name.to_string(),
            content: content.to_string(),
        }
    }

    #[test]
    fn quoted_markdown_reference_expands_as_block() {
        let s = vec![sc(
            "design prompt",
            "# Design system\n- stone neutrals\n- no shadows",
        )];
        let out =
            expand_shortcuts(&s, "please fix the ui ux using my \"design prompt\" today").unwrap();
        assert!(
            out.contains("using my \n\n# design prompt\n# Design system"),
            "{out}"
        );
        assert!(out.contains("- stone neutrals"), "{out}");
        assert!(out.ends_with("today") || out.contains("today"), "{out}");
    }

    #[test]
    fn short_email_content_expands_inline() {
        let s = vec![sc("work email", "hduggal@superflow.ai")];
        let out = expand_shortcuts(
            &s,
            "draft a mail to alfred tell him i changed my work email",
        )
        .unwrap();
        assert_eq!(
            out,
            "draft a mail to alfred tell him i changed my hduggal@superflow.ai"
        );
    }

    #[test]
    fn curly_quotes_and_trailing_punctuation_are_consumed() {
        let s = vec![sc("phone", "+1 555 010 9999")];
        let out = expand_shortcuts(&s, "\u{201c}phone\u{201d}, call this").unwrap();
        assert!(out.starts_with("+1 555 010 9999 call this"), "{out}");
    }

    #[test]
    fn longest_name_wins_over_prefix() {
        let s = vec![
            sc("prompt", "SHORT"),
            sc("design prompt", "LONG CONTENT LINE"),
        ];
        let out = expand_shortcuts(&s, "apply my design prompt here").unwrap();
        assert!(out.contains("LONG CONTENT LINE"), "{out}");
        assert!(!out.contains("SHORT"), "{out}");
    }

    #[test]
    fn partial_word_never_matches() {
        let s = vec![sc("mail", "x@y.z")];
        assert_eq!(
            expand_shortcuts(&s, "open the gmail app"),
            None,
            "'gmail' contains 'mail' but must not expand"
        );
    }

    #[test]
    fn absent_name_is_silent() {
        let s = vec![sc("design prompt", "# DS")];
        assert_eq!(expand_shortcuts(&s, "fix the login bug"), None);
    }

    #[test]
    fn links_and_phone_numbers_survive_verbatim() {
        let content = "https://figma.com/file/AbCdEf?node=1%2F2 phone +91 98765 43210";
        let s = vec![sc("design link", content)];
        let out = expand_shortcuts(&s, "start from my \"design link\" please").unwrap();
        assert!(out.contains(content), "{out}");
    }

    #[test]
    fn perf_fifty_shortcuts_under_budget() {
        let s: Vec<Shortcut> = (0..50)
            .map(|i| sc(&format!("shortcut number {i}"), "some content"))
            .chain(std::iter::once(sc("target name", "VALUE")))
            .collect();
        let t = Instant::now();
        let out = expand_shortcuts(&s, "please use my target name right now");
        assert!(
            t.elapsed().as_millis() < 100,
            "{}ms",
            t.elapsed().as_millis()
        );
        assert!(out.unwrap().contains("VALUE"));
    }
}
