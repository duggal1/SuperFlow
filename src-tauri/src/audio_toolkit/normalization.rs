use once_cell::sync::Lazy;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Deserialize)]
struct AliasEntry {
    canonical: String,
    aliases: Vec<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UnitSpacing {
    Tight,
    Space,
}

#[derive(Debug, Deserialize)]
struct CurrencyEntry {
    symbol: String,
    aliases: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct UnitEntry {
    canonical: String,
    spacing: UnitSpacing,
    aliases: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct NormalizationCatalog {
    version: u32,
    number_aliases: Vec<AliasEntry>,
    currencies: Vec<CurrencyEntry>,
    units: Vec<UnitEntry>,
    safe_contractions: Vec<AliasEntry>,
}

#[derive(Debug)]
struct NormalizationIndex {
    number_aliases: HashMap<String, String>,
    currencies: HashMap<String, String>,
    units: HashMap<String, (String, UnitSpacing)>,
    contractions: HashMap<String, String>,
}

static INDEX: Lazy<NormalizationIndex> = Lazy::new(|| {
    let catalog: NormalizationCatalog =
        serde_json::from_str(include_str!("../catalog/normalization.json"))
            .expect("normalization.json must match its typed schema");
    assert_eq!(
        catalog.version, 3,
        "unsupported normalization catalog version"
    );

    let mut seen = HashSet::new();
    let mut number_aliases = HashMap::new();
    for entry in catalog.number_aliases {
        for alias in entry.aliases {
            insert_unique(&mut seen, &mut number_aliases, alias, &entry.canonical);
        }
    }

    let mut seen = HashSet::new();
    let mut currencies = HashMap::new();
    for entry in catalog.currencies {
        for alias in entry.aliases {
            insert_unique(&mut seen, &mut currencies, alias, &entry.symbol);
        }
    }

    let mut seen = HashSet::new();
    let mut units = HashMap::new();
    for entry in catalog.units {
        for alias in entry.aliases {
            let key = normalized_key(&alias);
            assert!(!key.is_empty(), "normalization aliases cannot be empty");
            assert!(
                seen.insert(key.clone()),
                "duplicate normalization alias: {alias}"
            );
            units.insert(key, (entry.canonical.clone(), entry.spacing));
        }
    }

    let mut seen = HashSet::new();
    let mut contractions = HashMap::new();
    for entry in catalog.safe_contractions {
        for alias in entry.aliases {
            insert_unique(&mut seen, &mut contractions, alias, &entry.canonical);
        }
    }

    NormalizationIndex {
        number_aliases,
        currencies,
        units,
        contractions,
    }
});

fn insert_unique(
    seen: &mut HashSet<String>,
    target: &mut HashMap<String, String>,
    alias: String,
    canonical: &str,
) {
    let key = normalized_key(&alias);
    assert!(!key.is_empty(), "normalization aliases cannot be empty");
    assert!(
        seen.insert(key.clone()),
        "duplicate normalization alias: {alias}"
    );
    target.insert(key, canonical.to_string());
}

fn normalized_key(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

pub fn number_word(word: &str) -> String {
    let key = normalized_key(word);
    INDEX.number_aliases.get(&key).cloned().unwrap_or(key)
}

pub fn currency_symbol(words: &[&str], start: usize) -> Option<(&'static str, usize)> {
    lookup_phrase(&INDEX.currencies, words, start)
}

pub fn unit(words: &[&str], start: usize) -> Option<(&'static str, UnitSpacing, usize)> {
    for width in (1..=2).rev() {
        if start + width > words.len() {
            continue;
        }
        let key = words[start..start + width]
            .iter()
            .map(|word| normalized_key(word))
            .collect::<String>();
        if let Some((canonical, spacing)) = INDEX.units.get(&key) {
            return Some((canonical.as_str(), *spacing, width));
        }
    }
    None
}

fn lookup_phrase(
    values: &'static HashMap<String, String>,
    words: &[&str],
    start: usize,
) -> Option<(&'static str, usize)> {
    for width in (1..=2).rev() {
        if start + width > words.len() {
            continue;
        }
        let key = words[start..start + width]
            .iter()
            .map(|word| normalized_key(word))
            .collect::<String>();
        if let Some(value) = values.get(&key) {
            return Some((value.as_str(), width));
        }
    }
    None
}

pub fn apply_safe_contractions(text: &str) -> String {
    text.split_whitespace()
        .map(|word| {
            let key = normalized_key(word);
            let Some(replacement) = INDEX.contractions.get(&key) else {
                return word.to_string();
            };
            replace_core(word, replacement)
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn replace_core(word: &str, replacement: &str) -> String {
    let start = word
        .char_indices()
        .find(|(_, character)| character.is_alphanumeric())
        .map(|(index, _)| index)
        .unwrap_or(0);
    let end = word
        .char_indices()
        .rev()
        .find(|(_, character)| character.is_alphanumeric())
        .map(|(index, character)| index + character.len_utf8())
        .unwrap_or(word.len());
    format!("{}{}{}", &word[..start], replacement, &word[end..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_valid_and_lookup_is_exact() {
        assert_eq!(number_word("hundreed"), "hundred");
        assert_eq!(number_word("ordinary"), "ordinary");
        assert_eq!(currency_symbol(&["indian", "rupees"], 0), Some(("₹", 2)));
        assert_eq!(unit(&["g", "b"], 0), Some(("GB", UnitSpacing::Space, 2)));
    }

    #[test]
    fn safe_contractions_preserve_punctuation() {
        assert_eq!(
            apply_safe_contractions("dont stop, youre done"),
            "don't stop, you're done"
        );
        assert_eq!(apply_safe_contractions("were ready"), "were ready");
        assert_eq!(apply_safe_contractions("its value"), "its value");
    }
}
