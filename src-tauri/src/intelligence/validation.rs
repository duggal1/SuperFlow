use std::collections::HashSet;

const MAX_OUTPUT_CHARS: usize = 16_000;

pub(super) fn accept_model_output(source: &str, candidate: &str) -> Option<String> {
    let output = candidate.trim();
    if output.is_empty()
        || output.len() > MAX_OUTPUT_CHARS
        || has_forbidden_wrapper(output)
        || has_repetition_loop(output)
    {
        return None;
    }

    let candidate_lower = output.to_lowercase();
    if critical_tokens(source)
        .iter()
        .any(|token| !candidate_lower.contains(token))
    {
        return None;
    }

    Some(output.to_string())
}

fn has_forbidden_wrapper(output: &str) -> bool {
    let lower = output.to_ascii_lowercase();
    lower.starts_with("here is")
        || lower.starts_with("here's")
        || lower.starts_with("certainly")
        || lower.starts_with("```markdown")
        || lower.contains("<think>")
}

fn critical_tokens(source: &str) -> HashSet<String> {
    source
        .split_whitespace()
        .filter_map(|raw| {
            let token = raw
                .trim_matches(|character: char| {
                    matches!(
                        character,
                        '`' | '\'' | '"' | ',' | ';' | ':' | '(' | ')' | '[' | ']' | '{' | '}'
                    )
                })
                .trim_end_matches(['.', '!', '?'])
                .to_lowercase();
            let is_negation = matches!(
                token.as_str(),
                "not" | "never" | "no" | "don't" | "doesn't" | "can't" | "without" | "avoid"
            );
            let is_number = token.chars().any(|character| character.is_ascii_digit());
            let is_code = token.contains('/')
                || token.contains('\\')
                || token.contains('_')
                || token.contains("::")
                || token.contains("()")
                || (token.contains('.') && token.chars().any(char::is_alphabetic));
            (token.len() >= 2 && (is_negation || is_number || is_code)).then_some(token)
        })
        .collect()
}

fn has_repetition_loop(output: &str) -> bool {
    let normalized: Vec<&str> = output.split_whitespace().collect();
    if normalized.len() < 24 {
        return false;
    }
    for width in 4..=12 {
        if normalized.len() < width * 3 {
            continue;
        }
        for start in 0..=normalized.len() - width * 3 {
            let first = &normalized[start..start + width];
            let second = &normalized[start + width..start + width * 2];
            let third = &normalized[start + width * 2..start + width * 3];
            if first == second && second == third {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_numbers_paths_and_negations() {
        let source = "Fix src/payment.ts, never return more than $200,000.";
        assert!(accept_model_output(
            source,
            "Fix `src/payment.ts`. Never return more than $200,000."
        )
        .is_some());
        assert!(accept_model_output(source, "Fix payment. Return the amount.").is_none());
    }

    #[test]
    fn rejects_wrappers_and_repetition_loops() {
        assert!(
            accept_model_output("write this", "Here is the polished text: write this").is_none()
        );
        let looped = "one two three four one two three four one two three four one two three four one two three four one two three four";
        assert!(accept_model_output("one", looped).is_none());
    }
}
