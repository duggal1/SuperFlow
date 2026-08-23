use crate::context::types::ContextSnapshot;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const MAX_SNIPPETS: usize = 6;
const MAX_TOTAL_CHARS: usize = 8_000;
const MAX_FILE_BYTES: u64 = 256 * 1024;
const SEARCH_BUDGET: Duration = Duration::from_millis(150);

/// Repo metadata files whose *existence* is worth one prompt line. Contents
/// are never read — presence alone signals stack/tooling to the model.
const REPO_MANIFEST_FILES: &[&str] = &[
    "AGENTS.md",
    "CLAUDE.md",
    "package.json",
    "Cargo.toml",
    "pyproject.toml",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceSnippet {
    pub relative_path: String,
    pub line_start: usize,
    pub text: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspaceEvidence {
    pub root: Option<PathBuf>,
    pub resolved_paths: Vec<String>,
    pub snippets: Vec<EvidenceSnippet>,
    /// Present repo manifest filenames (existence-only probe).
    pub repo_manifest: Vec<String>,
}

pub fn collect(snapshot: &ContextSnapshot, transcript: &str) -> WorkspaceEvidence {
    let Some(root) = crate::file_refs::project_root_for_snapshot(snapshot) else {
        return WorkspaceEvidence::default();
    };
    let Ok(root) = root.canonicalize() else {
        return WorkspaceEvidence::default();
    };

    let terms = search_terms(transcript);
    let mut evidence = WorkspaceEvidence {
        root: Some(root.clone()),
        repo_manifest: repo_manifest(&root),
        ..WorkspaceEvidence::default()
    };
    let mut seen_paths = HashSet::new();
    let started = Instant::now();
    let mut used_chars = 0usize;

    for token in transcript.split_whitespace() {
        let candidate = token.trim_matches(|character: char| {
            matches!(
                character,
                '`' | '\'' | '"' | ',' | ';' | ':' | '(' | ')' | '[' | ']'
            )
        });
        if !candidate.contains('/') && !candidate.contains('.') {
            continue;
        }
        let path = root.join(candidate);
        if let Some(path) = safe_file(&root, &path) {
            let relative = relative(&root, &path);
            if seen_paths.insert(relative.clone()) {
                evidence.resolved_paths.push(relative);
            }
        }
    }

    // Reuse the cached project index maintained by `file_refs` instead of
    // re-walking the repository on every utterance. Only candidate files are
    // opened here, bounded by snippet/char/time budgets below.
    let Some(index) = crate::file_refs::project_index(&root) else {
        return evidence;
    };
    for entry in index {
        if evidence.snippets.len() >= MAX_SNIPPETS
            || used_chars >= MAX_TOTAL_CHARS
            || started.elapsed() >= SEARCH_BUDGET
        {
            break;
        }
        if !is_searchable_source(Path::new(&entry.rel)) {
            continue;
        }
        let path = root.join(&entry.rel);
        let Some(path) = safe_file(&root, &path) else {
            continue;
        };
        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        if content.contains('\0') {
            continue;
        }

        let relative_path = entry.rel.clone();
        let exact_path = evidence
            .resolved_paths
            .iter()
            .any(|resolved| resolved == &relative_path);
        let matching_line = content
            .lines()
            .position(|line| terms.iter().any(|term| line.contains(term)));
        let Some(line_index) = matching_line.or(exact_path.then_some(0)) else {
            continue;
        };
        let remaining = MAX_TOTAL_CHARS.saturating_sub(used_chars);
        let snippet = snippet_around(&content, line_index, remaining.min(1_400));
        if snippet.text.is_empty() {
            continue;
        }
        used_chars += snippet.text.len();
        evidence.snippets.push(EvidenceSnippet {
            relative_path,
            line_start: snippet.line_start,
            text: snippet.text,
        });
    }

    evidence
}

fn repo_manifest(root: &Path) -> Vec<String> {
    REPO_MANIFEST_FILES
        .iter()
        .filter(|name| root.join(name).is_file())
        .map(|name| (*name).to_string())
        .collect()
}

struct Snippet {
    line_start: usize,
    text: String,
}

fn snippet_around(content: &str, line_index: usize, max_chars: usize) -> Snippet {
    let lines: Vec<&str> = content.lines().collect();
    let start = line_index.saturating_sub(2);
    let end = (line_index + 5).min(lines.len());
    let mut text = lines[start..end].join("\n");
    if text.len() > max_chars {
        let boundary = text
            .char_indices()
            .take_while(|(index, _)| *index <= max_chars)
            .last()
            .map(|(index, character)| index + character.len_utf8())
            .unwrap_or(0);
        text.truncate(boundary);
    }
    Snippet {
        line_start: start + 1,
        text,
    }
}

fn safe_file(root: &Path, path: &Path) -> Option<PathBuf> {
    let metadata = fs::symlink_metadata(path).ok()?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > MAX_FILE_BYTES {
        return None;
    }
    if is_sensitive(path) {
        return None;
    }
    let canonical = path.canonicalize().ok()?;
    canonical.starts_with(root).then_some(canonical)
}

fn is_sensitive(path: &Path) -> bool {
    path.components().any(|component| {
        let name = component.as_os_str().to_string_lossy().to_ascii_lowercase();
        name == ".git"
            || name == "node_modules"
            || name == "target"
            || name == "dist"
            || name.starts_with(".env")
            || name.starts_with("id_rsa")
            || name.starts_with("id_ed25519")
            || name.contains("credential")
            || name.contains("keychain")
            || name.ends_with(".pem")
            || name.ends_with(".key")
            || name.ends_with(".p12")
    })
}

fn is_searchable_source(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some(
            "rs" | "ts"
                | "tsx"
                | "js"
                | "jsx"
                | "json"
                | "toml"
                | "md"
                | "css"
                | "scss"
                | "html"
                | "py"
                | "go"
                | "swift"
                | "java"
                | "kt"
                | "sql"
                | "yml"
                | "yaml"
                | "sh"
                | "zsh"
        )
    )
}

fn search_terms(transcript: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    transcript
        .split_whitespace()
        .filter_map(|word| {
            let term = word
                .trim_matches(|character: char| !character.is_alphanumeric() && character != '_')
                .trim_end_matches("()")
                .to_string();
            let is_identifier = term.len() >= 3
                && (term.contains('_')
                    || term.chars().skip(1).any(char::is_uppercase)
                    || word.ends_with("()"));
            (is_identifier && seen.insert(term.clone())).then_some(term)
        })
        .take(12)
        .collect()
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snippets_are_bounded_and_line_numbered() {
        let content = "one\ntwo\nfn calculateFinalPayment() {}\nfour\nfive\nsix\nseven";
        let snippet = snippet_around(content, 2, 200);
        assert_eq!(snippet.line_start, 1);
        assert!(snippet.text.contains("calculateFinalPayment"));
    }

    #[test]
    fn sensitive_paths_are_always_rejected() {
        assert!(is_sensitive(Path::new("src/.env.production")));
        assert!(is_sensitive(Path::new("keys/signing.pem")));
        assert!(!is_sensitive(Path::new("src/payment.ts")));
    }

    #[test]
    fn search_terms_only_keeps_identifier_evidence() {
        assert_eq!(
            search_terms("fix calculateFinalPayment() in payment_handler"),
            vec!["calculateFinalPayment", "payment_handler"]
        );
    }

    #[test]
    fn repo_manifest_lists_only_present_markers() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("AGENTS.md"), "").unwrap();
        std::fs::write(dir.path().join("package.json"), "{}").unwrap();
        let manifest = repo_manifest(dir.path());
        assert_eq!(manifest, vec!["AGENTS.md".to_string(), "package.json".to_string()]);
    }
}
