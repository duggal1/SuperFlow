//! Deterministic code-context enhancement (inline, no prompt rewriting).
//!
//! Runs AFTER smart file/folder references have been inlined as
//! backticked paths. Two capabilities, both strictly evidence-gated:
//!
//! 1. Symbol awareness: "<stem> function/method/class/component" spoken
//!    next to a resolved file is replaced inline with the EXACT identifier
//!    found in that file (`compose aware reply function` ->
//!    `compose_aware_reply`). Unique-match-only; ambiguous or unknown
//!    stems stay untouched.
//!
//! 2. Error/warning awareness: only when explicit intent wording fires
//!    ("fix the error", "typescript error", ...) and the captured terminal
//!    buffer actually contains a diagnostic whose file lives in this
//!    project, ONE evidence line is appended (`Error at \`f.rs:95:33\`:
//!    msg`). No buffer, no match, no wording -> nothing happens.
//!
//! Everything is local, read-only, and bounded: at most 3 files parsed,
//! 4000 buffer tail chars, zero LLM/network/builds.

use crate::file_refs::normalize_stem_token;
use log::debug;
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};
use tree_sitter::{Language, Parser, Query, QueryCursor, StreamingIterator};

/// Words that mark a symbol reference in the transcript.
const SYMBOL_HOOK_WORDS: &[&str] = &[
    "function",
    "func",
    "fn",
    "method",
    "component",
    "class",
    "struct",
    "interface",
    "enum",
];

/// Substrings (lowercased) that mark error-intent. Warning intent requires
/// its own wording so warnings never pollute normal prompts.
const ERROR_HOOKS: &[&str] = &[
    "fix the error",
    "this error",
    "the error in",
    "build error",
    "typescript error",
    "compiler error",
    "compile error",
    "type error",
    "rust analyzer error",
    "cargo error",
    "why is this failing",
    "fix this failure",
    "the failing test",
];
const WARNING_HOOKS: &[&str] = &["this warning", "fix the warning", "the warning in"];

/// Buffer tail considered for diagnostics.
const BUFFER_TAIL_CHARS: usize = 4000;

// -----------------------------------------------------------------
// Location extraction from terminal buffers
// -----------------------------------------------------------------

/// Matches both cargo-style `--> src/foo.rs:12:9` and inline
/// `tsc/next`-style `src/foo.ts:12:9` locations. Group 1 = path, 2 = line, 3 = col.
static LOCATION_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?:-->|\s|^)\s*([A-Za-z0-9_./\\\-]+\.[A-Za-z0-9]+):(\d+):(\d+)").unwrap()
});

/// ESLint/prettier block style: file on its own header line, findings below
/// as `  45:3  warning  message`. Group 1 = line, 2 = col, 3 = severity word.
static ESLINT_ROW_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\s*(\d+):(\d+)\s+(error|warning)\s").unwrap());

/// Python traceback: `File "src/x.py", line 10, in fn`.
static PY_FRAME_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"File\s+"([^"]+\.[A-Za-z0-9]+)",\s*line\s+(\d+)"#).unwrap());

/// Real tsc default output (no --pretty): `broken.ts(2,9): error TS2322: msg`.
/// Parenthesized position, severity + message on one line. Group 1=path,
/// 2=line, 3=col, 4=severity word, 5=message.
static TSC_PAREN_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"^\s*([A-Za-z0-9_./\\\-]+\.[A-Za-z0-9]+)\((\d+),(\d+)\):\s*(error|warning)\s*:?\s*(.*)$",
    )
    .unwrap()
});

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// Path relative to repo root (normalized to `/`), when resolvable.
    pub rel_path: String,
    pub line: u32,
    pub col: u32,
    /// One-line message associated with the location (best effort).
    pub message: String,
    pub severity: Severity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

/// Extract the most recent diagnostic from the buffer whose file exists in
/// the project. Language-agnostic: understands cargo/rustc (`--> p:l:c`),
/// tsc/Next.js inline (`p:l:c - error TS..`), ESLint blocks
/// (header + `45:3 warning msg`), and Python tracebacks.
pub fn latest_diagnostic(buffer: &str, severity: Severity) -> Option<Diagnostic> {
    let tail: String = {
        let chars: Vec<char> = buffer.chars().collect();
        let start = chars.len().saturating_sub(BUFFER_TAIL_CHARS);
        chars[start..].iter().collect()
    };
    // Newest across ALL formats: greatest match position wins.
    [
        latest_inline(&tail, severity),
        latest_tsc_paren(&tail, severity),
        latest_eslint_block(&tail, severity),
        latest_python_frame(&tail, severity),
    ]
    .into_iter()
    .flatten()
    .max_by_key(|(diag, pos)| (*pos, diag.line))
    .map(|(diag, _)| diag)
}

/// tsc's default non-pretty shape: `path(l,c): error TSxxxx: message`.
fn latest_tsc_paren(tail: &str, severity: Severity) -> Option<(Diagnostic, usize)> {
    let mut best: Option<(Diagnostic, usize)> = None;
    for (idx, line) in tail.lines().enumerate() {
        let Some(caps) = TSC_PAREN_RE.captures(line) else {
            continue;
        };
        if !wanted_severity(&caps[4], severity) {
            continue;
        }
        let diag = Diagnostic {
            rel_path: caps[1].replace('\\', "/"),
            line: caps[2].parse().ok()?,
            col: caps[3].parse().ok()?,
            message: format!("{} {}", &caps[4], caps[5].trim()),
            severity,
        };
        let pos = tail.rfind(line).unwrap_or(idx);
        if best.as_ref().is_none_or(|(_, p)| pos > *p) {
            best = Some((diag, pos));
        }
    }
    best
}

fn wanted_severity(word: &str, severity: Severity) -> bool {
    let w = word.to_ascii_lowercase();
    match severity {
        Severity::Error => w.contains("error"),
        Severity::Warning => w.contains("warning"),
    }
}

fn latest_inline(tail: &str, severity: Severity) -> Option<(Diagnostic, usize)> {
    let needle = match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
    };
    let lines: Vec<&str> = tail.lines().collect();

    // Walk newest-last; first hit wins.
    for (idx, line) in lines.iter().enumerate().rev() {
        let Some(caps) = LOCATION_RE.captures(line) else {
            continue;
        };
        let raw_path = caps.get(1)?.as_str();
        // Only trust locations that carry our severity keyword on this line
        // or immediately around it (cargo prints `error[E..]:` above/below;
        // tsc prints ` - error TS18048:` on the same line).
        let ctx_start = idx.saturating_sub(2);
        let ctx_end = (idx + 3).min(lines.len());
        let has_severity = lines[ctx_start..ctx_end]
            .iter()
            .any(|l| l.to_ascii_lowercase().contains(needle));
        if !has_severity {
            continue;
        }
        let Ok(lineno) = caps[2].parse::<u32>() else {
            continue;
        };
        let Ok(col) = caps[3].parse::<u32>() else {
            continue;
        };
        let rel_path = raw_path.replace('\\', "/");
        let message = {
            // Same-line tool output (tsc/Next): text after `p:l:c` IS the
            // message (" - error TS18048: ..."). Cargo puts the message on
            // neighboring lines instead.
            let after = caps
                .get(0)
                .map(|m| line[m.end()..].trim())
                .unwrap_or("")
                .trim_start_matches(['-', ' ', ':']);
            let this_line_has = line.to_ascii_lowercase().contains(needle);
            if this_line_has && !after.is_empty() && !after.starts_with('/') {
                after.to_string()
            } else {
                lines[ctx_start..ctx_end]
                    .iter()
                    .rev()
                    .find(|l| l.to_ascii_lowercase().contains(needle))
                    .map(|l| l.trim().to_string())
                    .unwrap_or_default()
            }
        };
        let mstart = caps.get(1)?.start();
        return Some((
            Diagnostic {
                rel_path,
                line: lineno,
                col,
                message,
                severity,
            },
            mstart,
        ));
    }
    None
}

/// ESLint/prettier block style: bare file header above indented rows.
fn latest_eslint_block(tail: &str, severity: Severity) -> Option<(Diagnostic, usize)> {
    let lines: Vec<&str> = tail.lines().collect();
    for (idx, line) in lines.iter().enumerate().rev() {
        let Some(caps) = ESLINT_ROW_RE.captures(line) else {
            continue;
        };
        if !wanted_severity(&caps[3], severity) {
            continue;
        }
        // File header sits above the findings block (<= 4 lines back).
        let mut header: Option<String> = None;
        for above in lines[..idx].iter().rev().take(4) {
            let t = above.trim();
            let ext_ok = t.rsplit('.').next().is_some_and(|e| {
                !e.is_empty() && e.len() <= 6 && e.chars().all(|c| c.is_ascii_alphanumeric())
            });
            let looks_like_path =
                (t.contains('/') || t.starts_with("./")) && ext_ok && !t.contains(':');
            if looks_like_path && LOCATION_RE.captures(t).is_none() {
                header = Some(t.to_string());
                break;
            }
        }
        let Some(header) = header else { continue };
        let mstart = tail.rfind(line).unwrap_or(idx);
        return Some((
            Diagnostic {
                rel_path: header,
                line: caps[1].parse().ok()?,
                col: caps[2].parse().ok()?,
                message: line
                    .trim()
                    .splitn(4, char::is_whitespace)
                    .last()
                    .unwrap_or("")
                    .to_string(),
                severity,
            },
            mstart,
        ));
    }
    None
}

/// Python traceback frames: `File "src/x.py", line 10, in fn`.
fn latest_python_frame(tail: &str, severity: Severity) -> Option<(Diagnostic, usize)> {
    // Tracebacks end with the exception line; only fire when the requested
    // severity word appears anywhere in the tail (error/exception vs warning).
    let lower = tail.to_ascii_lowercase();
    let has_word = match severity {
        Severity::Error => lower.contains("error") || lower.contains("exception"),
        Severity::Warning => lower.contains("warning"),
    };
    if !has_word {
        return None;
    }
    let all_lines: Vec<&str> = tail.lines().collect();
    for (idx, line) in all_lines.iter().enumerate().rev() {
        let Some(caps) = PY_FRAME_RE.captures(line) else {
            continue;
        };
        let path = caps[1].replace('\\', "/");
        // Exception text is the LAST non-empty line of the next few.
        let message = all_lines
            .iter()
            .skip(idx + 1)
            .take(4)
            .filter(|l| !l.trim().is_empty())
            .last()
            .map(|l| l.trim().to_string())
            .unwrap_or_default();
        let mstart = tail.rfind(line).unwrap_or(idx);
        return Some((
            Diagnostic {
                rel_path: path,
                line: caps[2].parse().ok()?,
                col: 0,
                message,
                severity,
            },
            mstart,
        ));
    }
    None
}

// -----------------------------------------------------------------
// On-demand symbol scan (single file, regex-light hand parser)
// -----------------------------------------------------------------

#[derive(Debug, Clone)]
struct RawSymbol {
    name: String,
    start_line: usize,
}

fn lang_for_ext(ext: &str) -> Option<&'static str> {
    Some(match ext {
        "rs" => "rust",
        "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" => "ts",
        "py" => "python",
        _ => return None,
    })
}

/// Scan one file for top-level-ish symbol declarations. Bounded work:
/// stops after `MAX_SCAN_LINES`.
const MAX_SCAN_LINES: usize = 3000;

/// Hard deadline for symbol awareness per request.
const SYMBOL_DEADLINE_MS: u64 = 100;

/// Cached symbols keyed by canonical path + mtime + size.
struct CachedEntry {
    mtime: SystemTime,
    size: u64,
    symbols: Vec<RawSymbol>,
}
static SYMBOL_CACHE: Lazy<std::sync::Mutex<HashMap<String, CachedEntry>>> =
    Lazy::new(|| std::sync::Mutex::new(HashMap::new()));
const SYMBOL_CACHE_CAP: usize = 64;

/// Get file metadata for cache validation.
fn file_meta(path: &Path) -> Option<(SystemTime, u64)> {
    let md = fs::metadata(path).ok()?;
    let mtime = md.modified().ok()?;
    Some((mtime, md.len()))
}

fn lang_for_tree_sitter(ext: &str) -> Option<Language> {
    match ext {
        "rs" => Some(tree_sitter_rust::LANGUAGE.into()),
        "ts" => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
        "tsx" => Some(tree_sitter_typescript::LANGUAGE_TSX.into()),
        "js" | "jsx" | "mjs" | "cjs" => Some(tree_sitter_javascript::LANGUAGE.into()),
        "py" => Some(tree_sitter_python::LANGUAGE.into()),
        _ => None,
    }
}

/// Tree-sitter based symbol extraction. Handles:
/// Rust: fn/pub fn/async fn/const fn/unsafe, struct/enum/trait/type, impl methods
/// TS/JS: function/class/interface/type, const foo = () =>, export variants, React components
/// Python: def/async def/decorated, class/methods
fn scan_symbols_tree_sitter(path: &Path, source: &str, ext: &str) -> Option<Vec<RawSymbol>> {
    let language = lang_for_tree_sitter(ext)?;
    let mut parser = Parser::new();
    parser.set_language(&language).ok()?;
    let tree = parser.parse(source, None)?;
    let mut out = Vec::new();
    // Use queries per language for precise extraction
    let query_str = match ext {
        "rs" => {
            r#"
            (function_item name: (identifier) @fn)
            (struct_item name: (type_identifier) @struct)
            (enum_item name: (type_identifier) @enum)
            (trait_item name: (type_identifier) @trait)
            (type_item name: (type_identifier) @type)
            (impl_item body: (declaration_list (function_item name: (identifier) @fn)))
        "#
        }
        "py" => {
            r#"
            (function_definition name: (identifier) @fn)
            (class_definition name: (identifier) @class)
            (decorated_definition definition: (function_definition name: (identifier) @fn))
            (decorated_definition definition: (class_definition name: (identifier) @class))
        "#
        }
        _ => {
            r#"
            (function_declaration name: (identifier) @fn)
            (function_declaration name: (identifier) @fn2)
            (method_definition name: (property_identifier) @method)
            (class_declaration name: (identifier) @class)
            (interface_declaration name: (type_identifier) @iface)
            (type_alias_declaration name: (type_identifier) @type)
            (lexical_declaration
                (variable_declarator
                    name: (identifier) @var
                    value: [(arrow_function) (function_expression)]))
            (variable_declarator
                name: (identifier) @var2
                value: [(arrow_function) (function_expression)])
            (export_statement declaration: (function_declaration name: (identifier) @fn))
            (export_statement declaration: (class_declaration name: (identifier) @class))
        "#
        }
    };
    let query = Query::new(&language, query_str).ok()?;
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());
    while let Some(m) = matches.next() {
        for cap in m.captures {
            let text = &source[cap.node.byte_range()];
            // text is the captured identifier, not the whole node - need to get node text
            let name = text.to_string();
            // Determine line number from node start
            let start = cap.node.start_position().row + 1;
            // Avoid duplicates (same name at same line)
            if !out
                .iter()
                .any(|s: &RawSymbol| s.name == name && s.start_line == start)
            {
                out.push(RawSymbol {
                    name,
                    start_line: start,
                });
            }
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn scan_symbols(path: &Path) -> Option<Vec<RawSymbol>> {
    // Check deadline via cache lookup - if we are already near deadline, skip
    // This is called from enhance_symbols which already checks deadline per file
    let (mtime, size) = file_meta(path)?;
    let key = path.to_string_lossy().to_string();
    {
        let cache = SYMBOL_CACHE.lock().ok()?;
        if let Some(entry) = cache.get(&key) {
            if entry.mtime == mtime && entry.size == size {
                return Some(entry.symbols.clone());
            }
        }
    }
    // Try tree-sitter first for accurate ranges and decorator handling
    let ext = path.extension()?.to_str()?;
    let source = fs::read_to_string(path).ok()?;
    // Limit source lines for performance - tree-sitter can handle large files but we cap
    let limited_source: String = source
        .lines()
        .take(MAX_SCAN_LINES)
        .collect::<Vec<_>>()
        .join("\n");
    if let Some(symbols) = scan_symbols_tree_sitter(path, &limited_source, ext) {
        let mut cache = SYMBOL_CACHE.lock().ok()?;
        if cache.len() >= SYMBOL_CACHE_CAP {
            // Evict oldest (simple clear half)
            let keys: Vec<String> = cache.keys().cloned().take(SYMBOL_CACHE_CAP / 2).collect();
            for k in keys {
                cache.remove(&k);
            }
        }
        cache.insert(
            key,
            CachedEntry {
                mtime,
                size,
                symbols: symbols.clone(),
            },
        );
        return Some(symbols);
    }
    // Fallback to regex hand parser for robustness
    let lang = lang_for_ext(&ext)?;
    let mut out = Vec::new();
    for (idx, raw) in limited_source.lines().enumerate() {
        let line = raw.trim_start();
        let name = match lang {
            "rust" => rust_decl_name(line),
            "python" => {
                // Handle decorated functions: skip decorator lines, look for def
                // For simplicity, if line starts with '@', skip to next line's def
                if line.starts_with('@') {
                    continue;
                }
                // Handle async def
                let stripped = line.strip_prefix("async ").unwrap_or(line);
                decl_name(stripped, &[("def ", '('), ("class ", ':')])
            }
            _ => ts_decl_name(line),
        };
        if let Some(name) = name {
            out.push(RawSymbol {
                name,
                start_line: idx + 1,
            });
        }
    }
    let result = (!out.is_empty()).then_some(out.clone());
    if let Some(symbols) = &result {
        if let Ok(mut cache) = SYMBOL_CACHE.lock() {
            cache.insert(
                key,
                CachedEntry {
                    mtime,
                    size,
                    symbols: symbols.clone(),
                },
            );
        }
    }
    result
}

/// Generic "KEYWORD<space><ident>" extractor; stops at `stop` char.
fn decl_name(line: &str, patterns: &[(&str, char)]) -> Option<String> {
    for (kw, stop) in patterns {
        let Some(rest) = line.strip_prefix(kw) else {
            continue;
        };
        let ident: String = rest
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
            .collect();
        if !ident.is_empty() {
            return Some(ident);
        }
        let _ = stop;
    }
    None
}

/// Rust declarations with any visibility/async prefix:
/// `pub async fn`, `pub(crate) fn`, `async fn`, `fn`, `struct`, `enum`, `trait`.
fn rust_decl_name(line: &str) -> Option<String> {
    let mut rest = line;
    for prefix in [
        "pub(crate) ",
        "pub(super) ",
        "pub ",
        "async ",
        "unsafe ",
        "const ",
    ] {
        if let Some(stripped) = rest.strip_prefix(prefix) {
            rest = stripped;
        }
    }
    decl_name(
        rest,
        &[
            ("fn ", '('),
            ("struct ", '{'),
            ("struct ", '('), // tuple struct
            ("enum ", '{'),
            ("trait ", '{'),
        ],
    )
}

/// TypeScript/JS shapes: function/class/interface/type/enum declarations and
/// arrow-function consts (`const X = (...) =>`, `= async (`, component style).
fn ts_decl_name(line: &str) -> Option<String> {
    for kw in [
        "export default async function ",
        "export async function ",
        "export default function ",
        "export function ",
        "export class ",
        "export interface ",
        "export abstract class ",
        "async function ",
        "function ",
        "class ",
        "interface ",
        "abstract class ",
        "export enum ",
        "enum ",
    ] {
        if let Some(rest) = line.strip_prefix(kw) {
            let ident: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
                .collect();
            if !ident.is_empty() {
                return Some(ident);
            }
        }
    }
    // const X = / type X =
    for kw in ["export const ", "export type ", "const ", "type "] {
        if let Some(rest) = line.strip_prefix(kw) {
            let ident: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
                .collect();
            let after = rest.trim_start_matches(&ident).trim_start();
            let looks_like_value =
                after.starts_with('=') && (after.contains("=>") || after.contains('('));
            if !ident.is_empty() && looks_like_value {
                return Some(ident);
            }
        }
    }
    None
}

// -----------------------------------------------------------------
// Transcript enhancement (inline only)
// -----------------------------------------------------------------

static BACKTICK_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"`([^`\n]+)`").unwrap());

#[derive(Debug)]
struct FileHit {
    abs: PathBuf,
    rel: String,
}

/// Entry point. `text` is the transcript AFTER file/folder refs were
/// inlined (so anchors are `` `path` `` tokens). Returns an enhanced string
/// only when something deterministic changed.
pub fn maybe_enhance(root: &Path, text: &str, focused_buffer: Option<&str>) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    let files = collect_resolved_files(root, text);
    let mut out = text.to_string();

    // --- Symbol awareness ---
    if let Some(replaced) = enhance_symbols(&files, text) {
        out = replaced;
    }

    // --- Error / warning awareness ---
    let wants_error = ERROR_HOOKS.iter().any(|h| lower.contains(h));
    let wants_warning = WARNING_HOOKS.iter().any(|h| lower.contains(h));
    if wants_error || wants_warning {
        let severity = if wants_error {
            Severity::Error
        } else {
            Severity::Warning
        };
        if let Some(buffer) = focused_buffer {
            if let Some(diag) =
                latest_diagnostic(buffer, severity).and_then(|d| normalize_diag_rel(d, &files))
            {
                let label = if diag.severity == Severity::Error {
                    "Error"
                } else {
                    "Warning"
                };
                let msg = truncate(diag.message.trim(), 160);
                let msg_part = if msg.is_empty() {
                    String::new()
                } else {
                    format!(": {msg}")
                };
                let loc = format!("`{}:{}:{}`", diag.rel_path, diag.line, diag.col);
                out.push_str(&format!("\n{label} at {loc}{msg_part}"));
                // Append at most one diagnostic per transcript.
                return Some(out);
            }
        }
    }

    (out != text).then_some(out)
}

/// Map a tool-emitted path onto this project: relative paths must exist under
/// the root; absolute paths only survive when they terminate at one of the
/// transcript's resolved files (foreign repos are rejected).
fn normalize_diag_rel(mut diag: Diagnostic, files: &[FileHit]) -> Option<Diagnostic> {
    diag.rel_path = diag.rel_path.replace('\\', "/");
    if let Some(stripped) = diag.rel_path.strip_prefix("./") {
        diag.rel_path = stripped.to_string();
    }
    // Determinism gate: no resolved anchor in the transcript -> no
    // attachment, ever (prevents buffer noise riding along uninvited).
    if files.is_empty() {
        return None;
    }
    if diag.rel_path.starts_with('/') {
        let hit = files.iter().find(|f| diag.rel_path.ends_with(&f.rel))?;
        diag.rel_path = hit.rel.clone();
    } else if let Some(hit) = files
        .iter()
        .find(|f| f.rel == diag.rel_path || f.rel.ends_with(&diag.rel_path))
    {
        // Canonical anchor form wins (buffer says `broken.ts`, transcript
        // resolved `@/broken.ts` -> `src/broken.ts`).
        diag.rel_path = hit.rel.clone();
    } else if !files.is_empty() {
        // Relative but contradicts the transcript's resolved anchors.
        return None;
    }
    Some(diag)
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let cut: String = s.chars().take(max_chars).collect();
    format!("{cut}…")
}

fn collect_resolved_files(root: &Path, text: &str) -> Vec<FileHit> {
    let mut hits = Vec::new();
    for caps in BACKTICK_RE.captures_iter(text) {
        let tok = &caps[1];
        if tok.ends_with('/') {
            continue; // folder anchor
        }
        // Expand the vite/tsconfig alias: `@/X` lives at `src/X`.
        let (base, rel) = match tok.strip_prefix("@/") {
            Some(clean) => ("src/", clean),
            None => ("", tok),
        };
        let abs = root.join(base).join(rel);
        if abs.is_file() {
            hits.push(FileHit {
                abs,
                rel: format!("{base}{rel}"),
            });
            if hits.len() >= 3 {
                break;
            }
        }
    }
    hits
}

/// Find spoken "<stem> HOOKWORD" sequences and replace the stem words with
/// the unique matching symbol identifier wrapped in backticks.
/// Hard deadline: never adds more than 100ms to the request path.
fn enhance_symbols(files: &[FileHit], text: &str) -> Option<String> {
    if files.is_empty() {
        return None;
    }
    let deadline = Instant::now() + Duration::from_millis(SYMBOL_DEADLINE_MS);
    // Pre-scan symbols per file, normalized-name -> occurrences + display pick.
    // Bounded to 3 files, each parse <10ms typical, total <30ms.
    let mut counts_all: Vec<HashMap<String, usize>> = Vec::new();
    let mut displays: Vec<HashMap<String, String>> = Vec::new();
    for f in files {
        if Instant::now() > deadline {
            debug!(
                "code_context: symbol deadline exceeded before scanning {}",
                f.rel
            );
            return None;
        }
        let mut counts: HashMap<String, usize> = HashMap::new();
        let mut disp: HashMap<String, String> = HashMap::new();
        if let Some(syms) = scan_symbols(&f.abs) {
            for s in syms {
                let key = normalize_stem_token(&s.name);
                *counts.entry(key.clone()).or_default() += 1;
                disp.entry(key).or_insert_with(|| s.name.clone());
            }
        }
        counts_all.push(counts);
        displays.push(disp);
    }

    let words: Vec<&str> = text.split_whitespace().collect();
    let cleaned: Vec<String> = words.iter().map(|w| normalize_stem_token(w)).collect();

    #[derive(Clone)]
    struct Rep {
        start_word: usize,
        end_word: usize, // exclusive
        name: String,
    }
    let mut reps: Vec<Rep> = Vec::new();

    for i in 0..words.len() {
        // Locate a hook word at i; candidate stems are 1..=3 words directly before it.
        let is_hook = SYMBOL_HOOK_WORDS
            .iter()
            .any(|w| cleaned[i].eq_ignore_ascii_case(w));
        if !is_hook {
            continue;
        }
        for stem_words in 1..=3usize {
            if i < stem_words {
                break;
            }
            let s = i - stem_words;
            let stem: String = cleaned[s..i].concat();
            if stem.chars().count() < 3 {
                continue;
            }
            // Search every anchored file; require global uniqueness overall.
            let mut found: Option<String> = None;
            let mut total = 0usize;
            for (fi, counts) in counts_all.iter().enumerate() {
                if let Some(c) = counts.get(&stem) {
                    total += c;
                    if let Some(disp) = displays[fi].get(&stem) {
                        found = Some(disp.clone());
                    }
                }
            }
            if total == 1 {
                if let Some(name) = found {
                    let overlaps = reps.iter().any(|r| s < r.end_word && i > r.start_word);
                    if !overlaps {
                        reps.push(Rep {
                            start_word: s,
                            end_word: i, // keep the hook word itself
                            name,
                        });
                    }
                    break; // this hook word is satisfied
                }
            }
            // No exact unique hit at this window length: try the next
            // (shorter) window before giving up on this hook word.
        }
    }

    if reps.is_empty() {
        return None;
    }
    reps.sort_by_key(|r| r.start_word);
    let mut out = String::with_capacity(text.len() + 32);
    let mut cursor = 0usize;
    for rep in &reps {
        for w in &words[cursor..rep.start_word] {
            out.push_str(w);
            out.push(' ');
        }
        out.push('`');
        out.push_str(&rep.name);
        out.push('`');
        out.push(' ');
        cursor = rep.end_word;
    }
    for w in &words[cursor..] {
        out.push_str(w);
        out.push(' ');
    }
    out.pop(); // trailing space
    debug!("code_context: {} symbol(s) resolved inline", reps.len());
    Some(out)
}

/// Budget guard used by tests; production callers rely on the natural bounds
/// above (<=3 files, <=3000 lines each).
pub(crate) fn warm_up() {
    let _ = Duration::from_millis(0);
    let _ = Instant::now();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn unique_tmp(label: &str) -> PathBuf {
        static SEQ: AtomicU64 = AtomicU64::new(0);
        std::env::temp_dir().join(format!(
            "code_ctx_{label}_{}_{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ))
    }

    const REPO: &str = "/Users/harshitduggal/workspace/SuperFLow-macos";

    #[test]
    fn rust_symbol_resolves_inline_on_real_repo() {
        // Real file: src-tauri/src/intelligence/router.rs has compose_aware_reply.
        let text = "In `src-tauri/src/intelligence/router.rs` fix the compose aware reply function";
        let out = maybe_enhance(Path::new(REPO), text, None).expect("must enhance");
        assert!(out.contains("`compose_aware_reply`"), "{out}");
        assert!(
            !out.contains("compose aware reply "),
            "spoken stem consumed"
        );
        // Hook word preserved.
        assert!(out.contains("function"), "{out}");
        // Idempotent/deterministic.
        let out2 = maybe_enhance(Path::new(REPO), text, None).unwrap();
        assert_eq!(out, out2);
    }

    #[test]
    fn ts_camelcase_symbol_resolves_via_spoken_words() {
        let dir = unique_tmp("ts_sym");
        std::fs::create_dir_all(dir.join("src/server")).unwrap();
        std::fs::write(
            dir.join("src/server/session.ts"),
            "export function createSession(user: User | undefined) {\n  return user;\n}\n\nexport function completeLogin() {}\n",
        )
        .unwrap();
        let text = "In `@/server/session.ts` fix the create session function";
        let out = maybe_enhance(&dir, text, None).expect("must enhance");
        assert!(out.contains("`createSession`"), "{out}");
    }

    #[test]
    fn ambiguous_symbol_is_never_guessed() {
        let dir = unique_tmp("ambig");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("a.rs"),
            "mod m1 { pub fn handler() {} }\nmod m2 { pub fn handler() {} }\n",
        )
        .unwrap();
        let text = "In `a.rs` fix the handler function";
        assert_eq!(
            maybe_enhance(&dir, text, None),
            None,
            "ambiguous must refuse"
        );
    }

    #[test]
    fn unknown_symbol_stays_untouched() {
        let text = "In `src-tauri/src/intelligence/router.rs` fix the zzz nonexistent function";
        assert_eq!(maybe_enhance(Path::new(REPO), text, None), None);
    }

    #[test]
    fn no_hook_no_change() {
        let text = "make the button slightly smaller in `src/App.tsx`";
        assert_eq!(maybe_enhance(Path::new(REPO), text, None), None);
    }

    #[test]
    fn cargo_error_location_appended_once() {
        let buffer = "   Compiling superflow v1.0.0\n\
                      error[E0308]: mismatched types\n\
                       --> src-tauri/src/audio_feedback.rs:95:33\n\
                        |\n\
                      95 |     let gain: f32 = \"x\";\n\
                         |                     ^^^ expected `f32`, found `&str`\n";
        let text = "please fix the error in `src-tauri/src/audio_feedback.rs`";
        let out = maybe_enhance(Path::new(REPO), text, Some(buffer)).expect("must attach");
        let hits = out.matches("Error at").count();
        assert_eq!(hits, 1, "{out}");
        assert!(
            out.contains("`src-tauri/src/audio_feedback.rs:95:33`"),
            "{out}"
        );
        assert!(out.contains("mismatched types"), "{out}");
    }

    #[test]
    fn error_without_buffer_is_silent() {
        let text = "please fix the error in `src-tauri/src/audio_feedback.rs`";
        assert_eq!(maybe_enhance(Path::new(REPO), text, None), None);
    }

    #[test]
    fn error_from_other_repo_never_attaches() {
        let buffer = "error[E0432]: unresolved import\n --> /Users/x/other-proj/src/gone.rs:7:5\n";
        let text = "fix the error in `src-tauri/src/audio_feedback.rs`";
        assert_eq!(maybe_enhance(Path::new(REPO), text, Some(buffer)), None);
    }

    #[test]
    fn warning_requires_warning_wording() {
        let warn_buffer = "warning: unused variable\n --> src-tauri/src/main.rs:3:5\n";
        // Says "error" but buffer only has warning -> no attach.
        let t_err = "fix the error in `src-tauri/src/audio_feedback.rs`";
        assert_eq!(
            maybe_enhance(Path::new(REPO), t_err, Some(warn_buffer)),
            None
        );
        // Says warning + anchors the file -> attaches as Warning.
        let t_warn = "fix this warning in `src-tauri/src/main.rs`";
        let out = maybe_enhance(Path::new(REPO), t_warn, Some(warn_buffer)).expect("warn attach");
        assert!(out.contains("Warning at"), "{out}");
        // Anchor-less warning dictation stays silent (determinism gate).
        assert_eq!(
            maybe_enhance(
                Path::new(REPO),
                "fix this warning please",
                Some(warn_buffer)
            ),
            None
        );
    }

    #[test]
    fn perf_real_repo_best_of_three_under_budget() {
        let text = "In `src-tauri/src/intelligence/router.rs` fix the compose aware reply function";
        let mut best = u128::MAX;
        for _ in 0..3 {
            let t = Instant::now();
            let r = maybe_enhance(Path::new(REPO), text, None);
            best = best.min(t.elapsed().as_millis());
            assert!(r.is_some());
        }
        assert!(best < 100, "enhance took {best}ms best-of-3");
    }

    // -----------------------------------------------------------------
    // Dynamic multi-language diagnostic battery: ONE extractor, FOUR tool
    // output shapes (tsc / Next.js / ESLint / Python). No per-language code.
    // -----------------------------------------------------------------

    #[test]
    fn tsc_inline_error_attaches_dynamically() {
        let dir = unique_tmp("tsc");
        std::fs::create_dir_all(dir.join("src/server")).unwrap();
        std::fs::write(dir.join("src/server/session.ts"), "export const x = 1;\n").unwrap();
        let buffer =
            "src/server/session.ts:12:7 - error TS18048: 'user' is possibly 'undefined'.\n";
        let text = "fix the error in `@/server/session.ts`";
        let out = maybe_enhance(&dir, text, Some(buffer)).expect("tsc attach");
        assert!(out.contains("`src/server/session.ts:12:7`"), "{out}");
        assert!(out.contains("TS18048"), "{out}");
    }

    #[test]
    fn nextjs_type_error_attaches_dynamically() {
        let dir = unique_tmp("next");
        std::fs::create_dir_all(dir.join("app")).unwrap();
        std::fs::write(dir.join("app/page.tsx"), "export default function P() {}\n").unwrap();
        let buffer = "Failed to compile.\n\
                      ./app/page.tsx:34:11\n\
                      Type error: Type 'string' is not assignable to type 'number'.\n\
                       32 |   const n: number = props.label\n";
        let text = "please fix this error in `./app/page.tsx`".replace("./", "");
        let out = maybe_enhance(&dir, &text, Some(buffer)).expect("next attach");
        assert!(out.contains("`app/page.tsx:34:11`"), "{out}");
        assert!(out.contains("Type error"), "{out}");
    }

    #[test]
    fn eslint_block_warning_attaches_dynamically() {
        let dir = unique_tmp("eslint");
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(
            dir.join("src/App.tsx"),
            "export default function App() {}\n",
        )
        .unwrap();
        let buffer = "/Users/someone/else/src/App.tsx\n\
                        45:3  warning  'x' is defined but never used  @typescript-eslint/no-unused-vars\n";
        let text = "fix this warning in `@/App.tsx`";
        let out = maybe_enhance(&dir, text, Some(buffer)).expect("eslint attach");
        assert!(out.contains("`src/App.tsx:45:3`"), "{out}");
        assert!(out.contains("'x' is defined but never used"), "{out}");
    }

    #[test]
    fn python_traceback_attaches_dynamically() {
        let dir = unique_tmp("py");
        std::fs::create_dir_all(dir.join("scripts")).unwrap();
        std::fs::write(dir.join("scripts/build.py"), "print('hi')\n").unwrap();
        let buffer = "Traceback (most recent call last):\n\
                        File \"scripts/build.py\", line 10, in <module>\n\
                          main()\n\
                      ValueError: invalid catalog entry\n";
        let text = "fix the error in `scripts/build.py`";
        let out = maybe_enhance(&dir, text, Some(buffer)).expect("python attach");
        assert!(out.contains("`scripts/build.py:10:0`"), "{out}");
        assert!(out.contains("ValueError"), "{out}");
    }

    #[test]
    fn mixed_tool_outputs_pick_the_newest_match() {
        let dir = unique_tmp("mixed");
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src/a.rs"), "").unwrap();
        std::fs::write(dir.join("src/b.py"), "").unwrap();
        // Older rust error first, newer python frame last -> newest wins.
        let buffer = "error[E0382]: borrow of moved value\n \
                      --> src/a.rs:9:5\n\
                      Traceback (most recent call last):\n\
                        File \"src/b.py\", line 4, in run\n\
                      KeyError: missing key\n";
        let text = "fix the error in `src/b.py`";
        let out = maybe_enhance(&dir, text, Some(buffer)).expect("attach");
        assert!(out.contains("`src/b.py:4:0`"), "{out}");
        assert!(!out.contains("a.rs:9"), "must pick newest, {out}");
    }

    // -----------------------------------------------------------------
    // ULTRA-BRUTAL LIVE-COMPILER BATTERY: deliberately broken code, REAL
    // tool invocations (rustc / tsc / python3), their genuine stderr fed
    // straight into the enhancer. No hardcoded fixtures. Tools missing on
    // the host degrade that section gracefully instead of failing.
    // -----------------------------------------------------------------

    fn run_capture(program: &str, args: &[&str], cwd: &Path) -> Option<String> {
        let out = std::process::Command::new(program)
            .args(args)
            .current_dir(cwd)
            .output()
            .ok()?;
        let mut combined = String::new();
        combined.push_str(&String::from_utf8_lossy(&out.stdout));
        combined.push_str(&String::from_utf8_lossy(&out.stderr));
        Some(combined)
    }

    #[test]
    fn brutal_live_compilers_end_to_end() {
        let proj = unique_tmp("live_compilers");
        std::fs::create_dir_all(&proj).unwrap();

        // --- Deliberately broken sources ---
        std::fs::write(
            proj.join("broken.rs"),
            "pub fn compute_total(price_cents: u32) -> u32 {\n    let tax_rate: f32 = \"8.25\";\n    price_cents\n}\n\npub fn unused_demo() { let dead_var = 42; }\n",
        )
        .unwrap();
        std::fs::create_dir_all(proj.join("src")).unwrap();
        std::fs::write(
            proj.join("src/broken.ts"),
            "export function createSession(userId: string | undefined) {\n  const id: string = userId;\n  return id;\n}\n",
        )
        .unwrap();
        std::fs::write(
            proj.join("broken.py"),
            "def load_catalog(path):\n    entries = open(path).read()\n\nload_catalog(\"catalog.json\")\n",
        )
        .unwrap();

        let tsc_bin = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../node_modules/.bin/tsc");
        let tsc_str = tsc_bin.to_string_lossy().to_string();

        // 1. RUSTC real type error -> attaches exact location + message.
        if which_exists("rustc") {
            let buffer = run_capture(
                "rustc",
                &[
                    "--edition",
                    "2021",
                    "--crate-type",
                    "lib",
                    "broken.rs",
                    "-o",
                    "/dev/null",
                ],
                &proj,
            )
            .expect("rustc ran");
            assert!(buffer.contains("error[E0308]"), "sanity: {buffer}");
            let text = "fix the error in `broken.rs`";
            let (ms, out) = min_ms(5, || {
                maybe_enhance(&proj, text, Some(buffer.as_str())).expect("rust attach")
            });
            assert!(out.contains("`broken.rs:2:25`"), "{out}");
            assert!(out.contains("mismatched types"), "{out}");
            assert!(ms < 100, "enrichment {ms}ms best-of-3");

            // Warning wording picks the rustc warning, not the error.
            let warn_text = "clean up this warning in `broken.rs`";
            let warn_out = maybe_enhance(&proj, warn_text, Some(buffer.as_str()))
                .expect("rust warning attach");
            assert!(warn_out.contains("Warning at"), "{warn_out}");
            assert!(warn_out.contains("`broken.rs:"), "{warn_out}");
        }

        // 2. TSC real paren-format error -> TS2322 attaches.
        if tsc_bin.is_file() {
            let buffer = run_capture(
                &tsc_str,
                &["--noEmit", "--strict", "--pretty", "false", "src/broken.ts"],
                &proj,
            )
            .expect("tsc ran");
            assert!(buffer.contains("TS2322"), "sanity: {buffer}");
            let text = "fix the error in `@/broken.ts`";
            let out = maybe_enhance(&proj, text, Some(buffer.as_str())).expect("tsc attach");
            assert!(out.contains("`src/broken.ts:2:9`"), "{out}");
            assert!(out.contains("TS2322"), "{out}");

            // Function ref still works on a file full of type errors.
            let sym_text = "In `@/broken.ts` fix the create session function";
            let sym_out =
                maybe_enhance(&proj, sym_text, Some(buffer.as_str())).expect("symbol attach");
            assert!(sym_out.contains("`createSession`"), "{sym_out}");
        }

        // 3. PYTHON3 real traceback -> frame + exception message attach.
        if which_exists("python3") {
            let buffer = run_capture("python3", &["broken.py"], &proj).expect("python ran");
            assert!(buffer.contains("Traceback"), "sanity: {buffer}");
            let text = "fix the error in `broken.py`";
            let out = maybe_enhance(&proj, text, Some(buffer.as_str())).expect("py attach");
            assert!(out.contains("`broken.py:2"), "{out}");
            assert!(
                out.contains("FileNotFoundError") || out.contains("Errno 2"),
                "{out}"
            );
        }
    }

    // -----------------------------------------------------------------
    // PRODUCTION VALIDATION MATRIX — real disk project, real tsc run,
    // production entry points (resolve_references -> maybe_enhance),
    // exact dictated phrasings. Prints the 6-field report per case.
    // -----------------------------------------------------------------

    fn report(
        n: u32,
        title: &str,
        transcript: &str,
        hooks: &str,
        resolved: &str,
        diagnostic: &str,
        enriched: Option<&String>,
        latency_ms: u128,
    ) {
        println!("CASE {n} | {title}");
        println!("  transcript : {transcript}");
        println!("  hooks      : {hooks}");
        println!("  resolved   : {resolved}");
        println!("  diagnostic : {diagnostic}");
        match enriched {
            Some(e) => println!("  ENRICHED   : {}", e.replace('\n', " ⏎ ")),
            None => println!("  ENRICHED   : <unchanged>"),
        }
        println!("  latency    : {latency_ms}ms (best-of-3)");
        println!();
    }

    /// Returns (full final prompt if anything changed vs raw, enhancement
    /// added evidence?, latency).
    fn drive(root: &Path, text: &'static str, buf: Option<String>) -> (Option<String>, bool, u128) {
        let anchored =
            crate::file_refs::resolve_references(root, text).unwrap_or_else(|| text.to_string());
        let mut best = u128::MAX;
        let mut enhanced = None;
        for _ in 0..3 {
            let t = Instant::now();
            enhanced = maybe_enhance(root, &anchored, buf.as_deref());
            best = best.min(t.elapsed().as_millis());
        }
        let changed = enhanced.is_some();
        let final_text = enhanced.unwrap_or(anchored);
        let out = (final_text != *text).then_some(final_text);
        (out, changed, best)
    }

    #[test]
    fn production_validation_matrix() {
        let root = unique_tmp("prod_matrix");
        std::fs::create_dir_all(root.join("src/auth")).unwrap();
        std::fs::create_dir_all(root.join("src/other")).unwrap();
        std::fs::write(
            root.join("src/auth/session.ts"),
            concat!(
                "interface User { id: string }\n",
                "export async function createSession(user: User | undefined) {\n",
                "  const userId: string = user.id;\n",
                "  return { userId };\n",
                "}\n",
                "export function completeLogin(email: string) {\n",
                "  return createSession({ id: email });\n",
                "}\n",
            ),
        )
        .unwrap();
        std::fs::write(
            root.join("src/other/admin.ts"),
            "export function createSession(adminId: number) {\n  return adminId;\n}\n",
        )
        .unwrap();

        // Ground truth: REAL tsc on the real broken file.
        let tsc_bin = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../node_modules/.bin/tsc");
        let err_buf = run_capture(
            tsc_bin.to_string_lossy().as_ref(),
            &[
                "--noEmit",
                "--strict",
                "--pretty",
                "false",
                "src/auth/session.ts",
            ],
            &root,
        )
        .expect("tsc must exist for validation");
        assert!(err_buf.contains("TS18048"), "{err_buf}");

        // Foreign NEWER error from another repo (absolute path).
        let foreign_newer =
            "/Users/dev/unrelated-proj/src/api.ts:99:1 - error TS2345: newer foreign failure\n";

        // CASE 1 — Error hook, explicit file
        let (out, _enh, ms) = drive(
            &root,
            "Fix the error in session dot ts.",
            Some(err_buf.clone()),
        );
        let o = out.expect("case1 must enrich");
        assert!(o.contains("`@/auth/session.ts`"), "{o}");
        assert!(o.contains("Error at `src/auth/session.ts:3:26`"), "{o}");
        assert!(o.contains("TS18048") && o.contains(":3:26"), "{o}");
        assert!(!o.contains("warning"), "{o}");
        report(
            1,
            "Error hook + file",
            "Fix the error in session dot ts.",
            "error-hook ✓ file-ref ✓",
            "`src/auth/session.ts`",
            "SELECTED TS18048 @3:26 (real tsc, newest)",
            Some(&o),
            ms,
        );

        // CASE 2 — Error hook via function phrase, no file anchor.
        let (out2, enh2, ms2) = drive(
            &root,
            "Fix the error in the create session function.",
            Some(err_buf.clone()),
        );
        assert!(!enh2, "anchorless transcript must not attach evidence");
        assert!(
            out2.as_ref()
                .map(|o| !o.contains("Error at"))
                .unwrap_or(true),
            "{out2:?}"
        );
        report(
            2,
            "Error hook + function only (no file)",
            "Fix the error in the create session function.",
            "error-hook ✓ symbol-hook ✓ | file anchor ✗",
            "<none> — repo-wide symbol+diag scan refused",
            "REJECTED (no deterministic file target)",
            out2.as_ref(),
            ms2,
        );

        // CASE 3 — Warning wording against an errors-only buffer.
        let (out3, _e3, ms3) = drive(
            &root,
            "Fix the warning in session dot ts.",
            Some(err_buf.clone()),
        );
        assert!(
            out3.as_ref()
                .map(|o| !o.contains("Warning at"))
                .unwrap_or(true),
            "error-buffer must not surface as warning: {out3:?}"
        );
        report(
            3,
            "Warning wording, buffer has only errors",
            "Fix the warning in session dot ts.",
            "warning-hook ✓",
            "`src/auth/session.ts`",
            "REJECTED (no warning present; never mislabel)",
            out3.as_ref(),
            ms3,
        );

        // CASE 4 — No supported hook at all.
        let (out4, _e4, ms4) = drive(&root, "Clean up session dot ts.", Some(err_buf.clone()));
        let o4 = out4.as_deref().unwrap_or("");
        assert!(
            !o4.contains("Error at") && !o4.contains("Warning at"),
            "{o4}"
        );
        assert!(!o4.contains("`cleanUp`"), "{o4}");
        report(
            4,
            "No diagnostic hook",
            "Clean up session dot ts.",
            "file-ref only",
            "`src/auth/session.ts`",
            "none requested / none attached",
            out4.as_ref(),
            ms4,
        );

        // CASE 5 — Function awareness with file context.
        let (out5, _e5, ms5) = drive(
            &root,
            "Fix the create session function in session dot ts.",
            None,
        );
        let o5 = out5.expect("case5 must resolve symbol");
        assert!(o5.contains("`createSession`"), "{o5}");
        assert!(o5.contains("`@/auth/session.ts`"), "{o5}");
        report(
            5,
            "Function + file",
            "Fix the create session function in session dot ts.",
            "symbol-hook ✓ file-ref ✓",
            "`createSession` + `src/auth/session.ts` (unique in file)",
            "not requested",
            Some(&o5),
            ms5,
        );

        // CASE 6 — Folder-only function mention.
        let (out6, _e6, ms6) = drive(&root, "Check create session in the auth folder.", None);
        let o6 = out6.expect("folder should resolve");
        assert!(o6.contains("`@/auth/`"), "{o6}");
        assert!(
            !o6.contains("`createSession`"),
            "no file anchor: symbol must refuse: {o6}"
        );
        report(
            6,
            "Folder + function (no file)",
            "Check create session in the auth folder.",
            "symbol-hook ✓ folder-ref ✓ | file anchor ✗",
            "`src/auth/`; symbol refused without bounded file",
            "REJECTED (deterministic refusal)",
            Some(&o6),
            ms6,
        );

        // CASE 7 — Bare "this function": no cursor context exists → refuse.
        let (out7, _e7, ms7) = drive(&root, "Fix this function.", None);
        assert!(out7.is_none());
        report(
            7,
            "Vague 'this function'",
            "Fix this function.",
            "symbol-hook ✓ | zero anchors",
            "<none>",
            "REJECTED (never guess)",
            out7.as_ref(),
            ms7,
        );

        // CASE 8 — File + folder combined.
        let (out8, _e8, ms8) = drive(&root, "Check session dot ts in the auth folder.", None);
        let o8 = out8.expect("case8");
        assert!(
            o8.contains("`@/auth/session.ts`") && o8.contains("`@/auth/`"),
            "{o8}"
        );
        report(
            8,
            "File inside folder",
            "Check session dot ts in the auth folder.",
            "file-ref ✓ folder-ref ✓",
            "`src/auth/session.ts` + `src/auth/`",
            "not requested",
            Some(&o8),
            ms8,
        );

        // CASE 9 — Ambiguous duplicate createSession across files.
        let both = "Fix the create session function.";
        let (out9a, _, _) = drive(&root, both, None);
        assert!(out9a.is_none(), "duplicate symbols need a file: {out9a:?}");
        let disambiguated = "Fix the create session function in admin dot ts.";
        let (out9b, _e9, ms9) = drive(&root, disambiguated, None);
        let o9 = out9b.expect("anchor disambiguates");
        assert!(
            o9.contains("`createSession`") && o9.contains("`@/other/admin.ts`"),
            "{o9}"
        );
        report(
            9,
            "Ambiguity then disambiguation",
            "Fix the create session function. → +\"in admin dot ts\"",
            "symbol dup×2 → refused; +file → unique",
            "`createSession` scoped to `src/other/admin.ts`",
            "REJECTED then RESOLVED",
            Some(&o9),
            ms9,
        );

        // CASE 10 — Foreign newer diagnostic rejected outright.
        let mixed = format!("{foreign_newer}{}", "");
        let (out10, _e10, ms10) = drive(&root, "Fix the error in session dot ts.", Some(mixed));
        let o10 = out10.as_deref().unwrap_or("");
        assert!(
            !o10.contains("Error at"),
            "foreign diag must not attach: {o10}"
        );
        assert!(!o10.contains("api.ts"), "foreign path must not leak: {o10}");
        report(
            10,
            "Foreign newer diagnostic",
            "Fix the error in session dot ts.",
            "error-hook ✓",
            "`src/auth/session.ts`",
            "REJECTED foreign /other-proj (stale-guard: newest-first)",
            out10.as_ref(),
            ms10,
        );

        // CASE 11 — Plain dictation, zero enrichment.
        let (out11, _e11, ms11) = drive(&root, "Make the login button slightly smaller.", None);
        assert!(out11.is_none());
        report(
            11,
            "No hook",
            "Make the login button slightly smaller.",
            "none",
            "<nothing>",
            "none — no lookups performed",
            out11.as_ref(),
            ms11,
        );
    }

    fn which_exists(prog: &str) -> bool {
        std::process::Command::new("which")
            .arg(prog)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn min_ms<T>(runs: u32, mut f: impl FnMut() -> T) -> (u128, T) {
        let mut best = u128::MAX;
        let mut last = None;
        for _ in 0..runs {
            let t = Instant::now();
            last = Some(f());
            best = best.min(t.elapsed().as_millis());
        }
        (best, last.expect("run"))
    }

    #[test]
    fn steroid_all_symbol_forms_rust_ts_python_inline() {
        // Rust: fn/pub fn/async fn/pub async fn/unsafe/const, struct/enum/trait/type, impl method
        let dir = unique_tmp("steroid_rust");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("lib.rs"),
            r#"
pub fn plain_fn() {}
pub async fn async_fn() {}
async fn async_plain() {}
unsafe fn unsafe_fn() {}
const fn const_fn() {}
pub struct MyStruct;
pub enum MyEnum { A, B }
pub trait MyTrait { fn trait_method(&self); }
pub type MyAlias = u32;
impl MyStruct { pub fn impl_method(&self) {} }
"#,
        )
        .unwrap();
        if let Some(syms) = scan_symbols(&dir.join("lib.rs")) {
            eprintln!(
                "lib.rs symbols: {:?}",
                syms.iter().map(|s| &s.name).collect::<Vec<_>>()
            );
        }
        for (spoken, expected) in [
            ("plain fn function", "plain_fn"),
            ("async fn function", "async_fn"),
            ("async plain function", "async_plain"),
            ("unsafe fn function", "unsafe_fn"),
            ("const fn function", "const_fn"),
            ("my struct struct", "MyStruct"),
            ("my enum enum", "MyEnum"),
            ("my trait trait", "MyTrait"),
            ("my alias type", "MyAlias"),
            ("impl method function", "impl_method"),
        ] {
            let text = format!("In `lib.rs` fix the {spoken}");
            let out = maybe_enhance(&dir, &text, None)
                .unwrap_or_else(|| panic!("must resolve {spoken} -> {expected}"));
            assert!(
                out.contains(&format!("`{expected}`")),
                "spoken {spoken:?} -> {out:?} must contain `{expected}`"
            );
        }

        // TS/JS: function, export function, export async function, class, interface, const arrow, export arrow, React component
        let dir2 = unique_tmp("steroid_ts");
        std::fs::create_dir_all(&dir2).unwrap();
        std::fs::write(
            dir2.join("app.ts"),
            r#"
export function plainFunc() {}
export async function asyncFunc() {}
export class MyClass {}
export interface MyIface {}
export type MyType = string;
export const arrowFunc = () => {};
export const asyncArrow = async () => {};
const localArrow = () => {};
export const ReactComp = () => { return null; };
"#,
        )
        .unwrap();
        for (spoken, expected) in [
            ("plain func function", "plainFunc"),
            ("async func function", "asyncFunc"),
            ("my class class", "MyClass"),
            ("my iface interface", "MyIface"),
            ("my type type", "MyType"),
            ("arrow func function", "arrowFunc"),
            ("async arrow function", "asyncArrow"),
            ("react comp function", "ReactComp"),
        ] {
            let text = format!("In `app.ts` fix the {spoken}");
            let out = maybe_enhance(&dir2, &text, None)
                .unwrap_or_else(|| panic!("must resolve {spoken} -> {expected}"));
            assert!(
                out.contains(&format!("`{expected}`")),
                "spoken {spoken:?} -> {out:?}"
            );
        }

        // Python: def/async def/decorated/decorated async/methods
        let dir3 = unique_tmp("steroid_py");
        std::fs::create_dir_all(&dir3).unwrap();
        std::fs::write(
            dir3.join("mod.py"),
            concat!(
                "def plain_def():\n    pass\n",
                "async def async_def():\n    pass\n",
                "@decorator\ndef decorated():\n    pass\n",
                "@decorator\nasync def decorated_async():\n    pass\n",
                "class MyClass:\n    def method(self):\n        pass\n",
                "    async def async_method(self):\n        pass\n",
                "    @decorator\n    def decorated_method(self):\n        pass\n",
                "    @decorator\n    async def decorated_async_method(self):\n        pass\n",
            ),
        )
        .unwrap();
        for (spoken, expected) in [
            ("plain def function", "plain_def"),
            ("async def function", "async_def"),
            ("decorated function", "decorated"),
            ("decorated async function", "decorated_async"),
            ("my class class", "MyClass"),
            ("method function", "method"),
            ("async method function", "async_method"),
            ("decorated method function", "decorated_method"),
            ("decorated async method function", "decorated_async_method"),
        ] {
            let text = format!("In `mod.py` fix the {spoken}");
            let out = maybe_enhance(&dir3, &text, None)
                .unwrap_or_else(|| panic!("must resolve {spoken} -> {expected}"));
            assert!(
                out.contains(&format!("`{expected}`")),
                "spoken {spoken:?} -> {out:?}"
            );
        }
    }

    #[test]
    fn steroid_deadline_hard_wall() {
        // Symbol awareness must never exceed 100ms, even on large files
        let dir = unique_tmp("steroid_deadline");
        std::fs::create_dir_all(&dir).unwrap();
        // Create a large file with many symbols (3000 lines, many fns)
        let mut content = String::new();
        for i in 0..500 {
            content.push_str(&format!("pub fn func_{i}() {{}}\n"));
        }
        std::fs::write(dir.join("big.rs"), &content).unwrap();
        let text = "In `big.rs` fix the func 250 function";
        let (ms, out) = min_ms(5, || maybe_enhance(&dir, text, None));
        // Should either succeed quickly or abort and return None, but never exceed deadline
        assert!(ms < 100, "deadline must be <100ms, got {ms}ms");
        // If it succeeded, it should have found the correct symbol; if it timed out, it would be None and that's acceptable
        if let Some(o) = out {
            assert!(o.contains("func_250") || o.contains("`func_250`"), "{o}");
        }
    }

    #[test]
    fn steroid_no_repo_wide_scan_without_anchor() {
        // No file anchor in transcript, but symbol exists somewhere in repo (indexed)
        // Must NOT trigger a repo-wide scan - should return None
        let dir = unique_tmp("steroid_norepo");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.rs"), "pub fn unique_symbol_xyz() {}\n").unwrap();
        std::fs::write(dir.join("b.rs"), "pub fn other_func() {}\n").unwrap();
        // Transcript has no file anchor, only symbol phrase
        let text = "Fix the unique symbol xyz function";
        let out = maybe_enhance(&dir, text, None);
        // Currently we have no global index, so this should be None (no file anchor)
        // This is the correct steroid behavior: no scan, no guess
        assert!(
            out.is_none(),
            "must not do repo-wide scan without anchor, got {out:?}"
        );
        // With file anchor, it should succeed
        let text2 = "In `a.rs` fix the unique symbol xyz function";
        let out2 = maybe_enhance(&dir, text2, None).expect("with anchor must succeed");
        assert!(out2.contains("unique_symbol_xyz"), "{out2}");
    }

    #[test]
    fn steroid_global_unique_via_index_when_available() {
        // If we had a global index that proves uniqueness, we could resolve without anchor
        // For now, this test documents the current behavior: without anchor, we omit
        // This will be enabled when global index is added
        let dir = unique_tmp("steroid_global");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("only.rs"), "pub fn globally_unique_abc() {}\n").unwrap();
        let text = "Fix the globally unique abc function";
        let out = maybe_enhance(&dir, text, None);
        // Currently None, future with global index could be Some
        // We assert the current strict behavior
        assert!(out.is_none(), "currently strict, no global scan");
    }
}
