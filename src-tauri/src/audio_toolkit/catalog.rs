//! Shared harvester for the speech-normalization catalogs.
//!
//! The catalogs mix several pair shapes ({spoken, canonical},
//! {canonical, spoken: [...]}, {speech, canonical}, {spoken, output},
//! {spoken, implementation}) across deeply nested sections. One recursive
//! walk collects every token-level mapping into the uniform
//! (canonical → spoken aliases) contract the fuzzy correction engine uses.
//!
//! Documentation-shaped sections (examples, tests, prose rules) are excluded
//! by key name — they describe sentence-level rewrites that do not belong in
//! a word-substitution pass.

use serde_json::Value;

#[derive(Debug, Clone)]
pub(crate) struct AliasPair {
    pub canonical: String,
    pub aliases: Vec<String>,
}

/// Sections whose entries are sentence-level examples, test fixtures, or
/// prose contracts rather than token substitutions.
const EXCLUDED_KEYS: &[&str] = &[
    "purpose",
    "architecture",
    "intent_model",
    "intent_preserving_examples",
    "quality_tests",
    "correction_logic",
    "structured_data_output_rules",
    "spreadsheet_output_rules",
    "developer_workflow_language",
    "final_processing_rules",
    "context_resolution",
    "do_not_change_user_vocabulary",
    "final_output_contract",
    "critical_examples",
];

fn pair_from_object(object: &serde_json::Map<String, Value>) -> Option<AliasPair> {
    let spoken_single = object
        .get("spoken")
        .or_else(|| object.get("speech"))
        .and_then(Value::as_str);
    let canonical = object
        .get("canonical")
        .or_else(|| object.get("implementation"))
        .or_else(|| object.get("output"))
        .and_then(Value::as_str);

    if let (Some(spoken), Some(canonical)) = (spoken_single, canonical) {
        if !spoken.trim().is_empty() && !canonical.trim().is_empty() {
            return Some(AliasPair {
                canonical: canonical.to_string(),
                aliases: vec![spoken.to_string()],
            });
        }
        return None;
    }

    // Inverted shapes: {canonical, spoken|input_patterns: [...]} and
    // {canonical, aliases: [...]}.
    if let Some(canonical) = object.get("canonical").and_then(Value::as_str) {
        for list_key in ["spoken", "aliases", "input_patterns"] {
            if let Some(spoken) = object.get(list_key).and_then(Value::as_array) {
                let aliases: Vec<String> = spoken
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect();
                if !canonical.trim().is_empty() && !aliases.is_empty() {
                    return Some(AliasPair {
                        canonical: canonical.to_string(),
                        aliases,
                    });
                }
            }
        }
    }

    None
}

/// Recursively collects every token-level mapping under `value`.
pub(crate) fn harvest(value: &Value, out: &mut Vec<AliasPair>) {
    match value {
        Value::Object(map) => {
            if let Some(pair) = pair_from_object(map) {
                out.push(pair);
                return;
            }
            for (key, child) in map {
                if EXCLUDED_KEYS.contains(&key.as_str()) {
                    continue;
                }
                harvest(child, out);
            }
        }
        Value::Array(items) => {
            for item in items {
                harvest(item, out);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn harvests_all_documented_shapes() {
        let doc = json!({
            "a": {"spoken": "dot slash", "canonical": "./"},
            "b": {"canonical": "bg", "spoken": ["b g", "bee gee"]},
            "c": {"speech": "bgstone 500", "canonical": "bg-stone-500"},
            "d": {"spoken": "hover bg stone six hundred", "output": "hover:bg-stone-600"},
            "e": {"spoken": "make the border full", "implementation": "rounded-full"},
            "quality_tests": {"input": "x", "expected": "y"},
            "nested": {"deep": {"spoken": "empty array", "canonical": "[]"}}
        });

        let mut pairs = Vec::new();
        harvest(&doc, &mut pairs);
        let canonicals: Vec<&str> = pairs.iter().map(|p| p.canonical.as_str()).collect();
        assert_eq!(
            canonicals,
            vec![
                "./",
                "bg",
                "bg-stone-500",
                "hover:bg-stone-600",
                "rounded-full",
                "[]"
            ]
        );
    }
}
