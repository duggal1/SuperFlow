//! Smart file references (Phase 2).
//!
//! When dictating into a terminal or code editor, spoken file names are
//! resolved against the active project: "hero doot tsx" becomes
//! `components/landing-page/hero.tsx`. Entirely local — frontmost-app
//! detection comes from the [`crate::context`] engine, the project root is
//! derived from the terminal's shell cwd (or the editor's last workspace),
//! and matching is exact filename lookup over an on-disk index.
//! Every failure path returns the original transcript untouched.
//!
//! Only explicit spoken forms (`hero.tsx`, `hero dot tsx`, spelled letters)
//! are eligible. Duplicate basenames resolve only when git activity singles
//! one out; vague phrases never introduce a filename or extension.

use crate::context::types::{ContextSnapshot, Surface};
use log::debug;
use once_cell::sync::Lazy;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// File extensions recognized after a spoken "dot"/direct suffix.
const EXTENSIONS: &[&str] = &[
    "ts", "tsx", "js", "jsx", "mjs", "cjs", "py", "rs", "go", "rb", "java", "kt", "kts", "swift",
    "m", "mm", "c", "h", "cpp", "cc", "cxx", "hpp", "cs", "php", "css", "scss", "sass", "less",
    "html", "htm", "json", "md", "mdx", "txt", "yml", "yaml", "toml", "sql", "sh", "bash", "zsh",
    "fish", "vue", "svelte", "astro", "prisma", "graphql", "gql", "env", "xml", "ini", "cfg",
    "conf",
];

/// Directories never indexed (dependency/output noise).
const IGNORED_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    "out",
    ".next",
    "vendor",
    "venv",
    ".venv",
    "__pycache__",
    "coverage",
    ".cache",
    ".turbo",
    ".output",
    ".svelte-kit",
    "Pods",
    "DerivedData",
    ".idea",
    ".gradle",
];

const MAX_INDEX_FILES: usize = 30_000;
const MAX_WALK_DEPTH: usize = 14;
const INDEX_TTL: Duration = Duration::from_secs(30);

/// How long a `git status --porcelain` snapshot stays trusted. Git state is
/// an activity *hint*, never a gate; staleness only weakens boosts.
const GIT_TTL: Duration = Duration::from_secs(15);

/// Terminal apps whose foreground shell's cwd is the active project.
const TERMINAL_BUNDLE_IDS: &[&str] = &[
    "com.apple.Terminal",
    "com.mitchellh.ghostty",
    "com.googlecode.iterm2",
    "dev.warp.Warp-Stable",
    "net.kovidgoyal.kitty",
    "io.alacritty",
    "com.github.wez.wezterm",
    "org.wezterm",
];

/// Editor bundle prefixes whose recent-workspace storage we can read.
const EDITOR_STORAGE: &[(&str, &str)] = &[
    (
        "com.microsoft.VSCode",
        "Library/Application Support/Code/User/globalStorage/storage.json",
    ),
    (
        "com.microsoft.VSCodeInsiders",
        "Library/Application Support/Code - Insiders/User/globalStorage/storage.json",
    ),
    (
        "com.todesktop.230313mzl4w4u92",
        "Library/Application Support/Cursor/User/globalStorage/storage.json",
    ),
    (
        "com.vscodium.codium",
        "Library/Application Support/VSCodium/User/globalStorage/storage.json",
    ),
];

pub(crate) fn project_root_for_snapshot(snapshot: &ContextSnapshot) -> Option<PathBuf> {
    if !matches!(snapshot.surface, Surface::Terminal | Surface::Editor) {
        return None;
    }
    project_root(snapshot.bundle_id.as_deref()?)
}

// -----------------------------------------------------------------
// Project root resolution
// -----------------------------------------------------------------

fn project_root(bundle_id: &str) -> Option<PathBuf> {
    if TERMINAL_BUNDLE_IDS.contains(&bundle_id) {
        terminal_project_root()
    } else {
        editor_project_root(bundle_id)
    }
}

#[cfg(target_os = "macos")]
fn terminal_project_root() -> Option<PathBuf> {
    // Level-2 anchor: an installed shell integration publishes the live PWD.
    // It is pane/window-accurate in a way the process-tree BFS below cannot
    // be, so when fresh it wins outright; otherwise we fall back to BFS.
    if let Some(root) = hook_project_root() {
        return Some(root);
    }
    let procs = list_processes()?;
    // Terminal app processes: comm is the executable name (max 16 chars).
    let terminal_names: &[&str] = &[
        "terminal",
        "ghostty",
        "iterm2",
        "warp",
        "kitty",
        "alacritty",
        "wezterm",
    ];
    let mut queue: VecDeque<i32> = procs
        .iter()
        .filter(|p| {
            p.comm.starts_with("Terminal")
                || terminal_names
                    .iter()
                    .any(|n| p.comm.eq_ignore_ascii_case(n))
        })
        .map(|p| p.pid)
        .collect();
    if queue.is_empty() {
        return None;
    }

    // BFS descendants of the terminal app looking for the newest shell.
    let shell_names: &[&str] = &["zsh", "bash", "fish", "sh", "pwsh", "nu"];
    let children: HashMap<i32, Vec<&ProcInfo>> = {
        let mut map: HashMap<i32, Vec<&ProcInfo>> = HashMap::new();
        for p in &procs {
            map.entry(p.ppid).or_default().push(p);
        }
        map
    };

    let mut best: Option<(&ProcInfo, f64)> = None;
    let mut visited: HashSet<i32> = HashSet::new();
    while let Some(pid) = queue.pop_front() {
        if !visited.insert(pid) {
            continue;
        }
        if let Some(kids) = children.get(&pid) {
            for kid in kids {
                if shell_names.iter().any(|n| kid.comm.eq_ignore_ascii_case(n)) {
                    let start = kid.start_secs();
                    if best.is_none_or(|(_, b)| start > b) {
                        best = Some((kid, start));
                    }
                }
                queue.push_back(kid.pid);
            }
        }
    }

    let shell = best?.0;
    process_cwd(shell.pid).filter(|p| p.is_dir())
}

#[cfg(not(target_os = "macos"))]
fn terminal_project_root() -> Option<PathBuf> {
    hook_project_root()
}

/// Newest working directory published by the optional SuperFlow shell
/// integration (`scripts/superflow-shell-hook.zsh|.bash`). The hook writes
/// `$PWD` into `$TMPDIR/superflow/cwd` atomically on every prompt; we accept
/// it only while fresh and only when it still points at a real directory.
const HOOK_MAX_AGE: Duration = Duration::from_secs(600);

fn hook_project_root() -> Option<PathBuf> {
    let base = std::env::var("TMPDIR").unwrap_or_else(|_| "/tmp".to_string());
    hook_project_root_in(&PathBuf::from(base))
}

fn hook_project_root_in(base: &Path) -> Option<PathBuf> {
    let marker = base.join("superflow").join("cwd");
    let modified = std::fs::metadata(&marker).ok()?.modified().ok()?;
    let age = modified.elapsed().ok()?;
    if age > HOOK_MAX_AGE {
        return None;
    }
    let cwd = std::fs::read_to_string(&marker).ok()?;
    let cwd = PathBuf::from(cwd.trim());
    cwd.is_dir().then_some(cwd)
}

struct ProcInfo {
    pid: i32,
    ppid: i32,
    comm: String,
    /// Position in the `ps` listing; higher = newer process. Good enough to
    /// pick the foreground shell without parsing start times.
    order: usize,
}

impl ProcInfo {
    fn start_secs(&self) -> f64 {
        self.order as f64
    }
}

/// Snapshot of all processes via `ps` — no extra dependency, no privileges.
fn list_processes() -> Option<Vec<ProcInfo>> {
    let out = std::process::Command::new("ps")
        .args(["-axo", "pid=,ppid=,comm="])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut procs = Vec::new();
    for (order, line) in text.lines().enumerate() {
        let mut parts = line.trim_start().splitn(3, char::is_whitespace);
        let pid = parts.next()?.parse::<i32>().ok()?;
        let ppid = parts.next()?.parse::<i32>().ok()?;
        let comm_raw = parts.next()?.trim();
        // `comm` may be a full path; the executable name is what matches.
        let comm = comm_raw.rsplit('/').next().unwrap_or(comm_raw).to_string();
        procs.push(ProcInfo {
            pid,
            ppid,
            comm,
            order,
        });
    }
    Some(procs)
}

/// Working directory of a process via lsof (same-user processes need no
/// privileges). Parses the `n<path>` line of `lsof -a -p PID -d cwd -Fn`.
fn process_cwd(pid: i32) -> Option<PathBuf> {
    let out = std::process::Command::new("lsof")
        .args(["-a", "-p", &pid.to_string(), "-d", "cwd", "-Fn"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|line| line.strip_prefix('n').filter(|p| !p.is_empty()))
        .map(PathBuf::from)
}

/// Last active workspace folder from the editor's storage.json.
fn editor_project_root(bundle_id: &str) -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let (_, rel) = EDITOR_STORAGE
        .iter()
        .find(|(id, _)| bundle_id.starts_with(id))?;
    let path = PathBuf::from(home).join(rel);
    let raw = std::fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&raw).ok()?;

    let folder_uri = value
        .pointer("/windowsState/lastActiveWindow/folder")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| {
            // Fall back to the most recent entry in history.
            value
                .pointer("/history/recentlyOpenedPathsList")
                .and_then(|v| v.as_array())
                .and_then(|list| list.first())
                .and_then(|e| e.get("folderUri"))
                .and_then(|v| v.as_str())
                .map(str::to_string)
        })?;

    uri_to_path(&folder_uri).filter(|p| p.is_dir())
}

/// `file:///Users/x/My%20Project` → `/Users/x/My Project`.
fn uri_to_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    // Strip a leading authority (usually empty).
    let path = if let Some(stripped) = rest.strip_prefix("localhost/") {
        format!("/{stripped}")
    } else {
        rest.to_string()
    };
    Some(PathBuf::from(percent_decode(&path)))
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if let (Some(h), Some(l)) = (
                bytes.get(i + 1).and_then(|b| (*b as char).to_digit(16)),
                bytes.get(i + 2).and_then(|b| (*b as char).to_digit(16)),
            ) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

// -----------------------------------------------------------------
// Project file index
// -----------------------------------------------------------------

#[derive(Clone)]
pub(crate) struct FileEntry {
    /// Path relative to the project root, `/`-separated.
    rel: String,
    name_lower: String,
}

type CachedIndex = HashMap<PathBuf, (Instant, Vec<FileEntry>)>;
static INDEX_CACHE: Lazy<Mutex<CachedIndex>> = Lazy::new(|| Mutex::new(HashMap::new()));

/// Cached project file index (path-metadata entries only — never contents).
fn project_index(root: &Path) -> Option<Vec<FileEntry>> {
    {
        let cache = INDEX_CACHE.lock().ok()?;
        if let Some((at, entries)) = cache.get(root) {
            if at.elapsed() < INDEX_TTL {
                return Some(entries.clone());
            }
        }
    }

    let mut entries = Vec::new();
    walk(root, root, 0, &mut entries);
    debug!(
        "file_refs: indexed {} files under {}",
        entries.len(),
        root.display()
    );

    if let Ok(mut cache) = INDEX_CACHE.lock() {
        if cache.len() > 8 {
            cache.clear(); // simple eviction; roots per session are few
        }
        cache.insert(root.to_path_buf(), (Instant::now(), entries.clone()));
    }
    Some(entries)
}

fn walk(root: &Path, dir: &Path, depth: usize, out: &mut Vec<FileEntry>) {
    if depth > MAX_WALK_DEPTH || out.len() >= MAX_INDEX_FILES {
        return;
    }
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in read.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if file_type.is_dir() {
            if !IGNORED_DIRS.contains(&name.as_ref()) && !name.starts_with('.') {
                walk(root, &entry.path(), depth + 1, out);
            }
        } else if file_type.is_file() {
            if is_sensitive_file_name(&name) {
                continue;
            }
            let rel = entry
                .path()
                .strip_prefix(root)
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_else(|_| name.to_string());
            let name_lower = name.to_lowercase();
            out.push(FileEntry { rel, name_lower });
        }
    }
}

fn is_sensitive_file_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.starts_with('.')
        || lower.starts_with("id_rsa")
        || lower.starts_with("id_ed25519")
        || lower.contains("credential")
        || lower.contains("keychain")
        || lower.ends_with(".pem")
        || lower.ends_with(".key")
        || lower.ends_with(".p12")
}

// -----------------------------------------------------------------
// Git activity signal
// -----------------------------------------------------------------

type GitCache = HashMap<PathBuf, (Instant, Arc<HashSet<String>>)>;
static GIT_MODIFIED_CACHE: Lazy<Mutex<GitCache>> = Lazy::new(|| Mutex::new(HashMap::new()));

/// Paths currently modified in the working tree (`git status --porcelain`),
/// relative to `root`, `/`-separated. Empty set on any failure or non-repo:
/// the signal is a boost only, never a gate.
fn git_modified(root: &Path) -> Arc<HashSet<String>> {
    if let Ok(cache) = GIT_MODIFIED_CACHE.lock() {
        if let Some((at, set)) = cache.get(root) {
            if at.elapsed() < GIT_TTL {
                return Arc::clone(set);
            }
        }
    }

    let mut set = HashSet::new();
    if let Ok(out) = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["status", "--porcelain"])
        .output()
    {
        if out.status.success() {
            for line in String::from_utf8_lossy(&out.stdout).lines() {
                // Porcelain v1: "XY <path>"; renames carry "old -> new".
                let Some(path) = line.get(3..) else {
                    continue;
                };
                let path = path.rsplit_once(" -> ").map_or(path, |(_, new)| new);
                set.insert(path.trim_matches('"').replace('\\', "/"));
            }
        }
    }

    let set = Arc::new(set);
    if let Ok(mut cache) = GIT_MODIFIED_CACHE.lock() {
        if cache.len() > 8 {
            cache.clear(); // roots per session are few
        }
        cache.insert(root.to_path_buf(), (Instant::now(), Arc::clone(&set)));
    }
    set
}

// -----------------------------------------------------------------
// Spoken reference matching
// -----------------------------------------------------------------

fn clean_token(word: &str) -> String {
    word.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

fn is_ext(token: &str) -> Option<&'static str> {
    EXTENSIONS.iter().find(|e| **e == token).copied()
}

fn is_dot_word(token: &str) -> bool {
    // "doot"/"dots" are common ASR renderings of a spoken "dot".
    matches!(token, "dot" | "doot" | "dots" | "period")
}

fn best_match(index: &[FileEntry], filename: &str, root: &Path) -> Option<String> {
    let target = filename.to_lowercase();
    let mut hits = index.iter().filter(|e| e.name_lower == target);
    let first = hits.next()?;
    if hits.next().is_none() {
        return Some(first.rel.clone());
    }
    // Duplicate basenames are never guessed. Only working-tree activity may
    // break the tie: exactly one git-modified twin is decisive evidence.
    let git = git_modified(root);
    let modified: Vec<&FileEntry> = index
        .iter()
        .filter(|e| e.name_lower == target && git.contains(&e.rel))
        .collect();
    if let [one] = modified.as_slice() {
        return Some(one.rel.clone());
    }
    debug!("file_refs: ambiguous basename '{filename}' left unresolved");
    None
}

/// Resolve explicit spoken file references inside `text` against `root`'s
/// project. Vague phrases are deliberately not inferred: only a filename and
/// extension the speaker actually said may introduce a project path.
pub fn resolve_references(root: &Path, text: &str) -> Option<String> {
    let index = project_index(root)?;
    let words: Vec<&str> = text.split_whitespace().collect();
    let cleaned: Vec<String> = words.iter().map(|w| clean_token(w)).collect();
    let mut out: Vec<String> = Vec::with_capacity(words.len());
    let mut replaced = false;
    let mut i = 0;

    while i < words.len() {
        if let Some((span, rel)) = detect_reference(&words, &cleaned, i, &index, root) {
            out.push(rel.to_string());
            i += span;
            replaced = true;
        } else {
            out.push(words[i].to_string());
            i += 1;
        }
    }

    replaced.then(|| out.join(" "))
}

/// Try to match a spoken file reference starting exactly at position `start`.
type Detected = (usize, String);

fn detect_reference(
    words: &[&str],
    cleaned: &[String],
    start: usize,
    index: &[FileEntry],
    root: &Path,
) -> Option<Detected> {
    // Inline form: "hero.tsx" transcribed as one token.
    let inline = &cleaned[start];
    if let Some((stem, ext)) = inline.split_once('.') {
        if !stem.is_empty() && is_ext(ext).is_some() {
            if let Some(rel) = best_match(index, &format!("{stem}.{ext}"), root) {
                return Some((1, rel));
            }
        }
    }

    let max_stem = 3.min(words.len() - start - 1);
    for stem_words in 1..=max_stem {
        let stem_start = start;
        let after_stem = stem_start + stem_words;
        if after_stem >= words.len() {
            break;
        }

        // Stem candidate: single words join directly, multi-word stems use
        // dots ("next config" → next.config).
        let stem_parts: Vec<&str> = cleaned[stem_start..after_stem]
            .iter()
            .map(String::as_str)
            .collect();
        if stem_parts.iter().any(|p| p.is_empty()) {
            break;
        }
        let stem_base = if stem_parts.len() == 1 {
            stem_parts[0].to_string()
        } else {
            stem_parts.join(".")
        };
        if stem_base.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            continue; // avoid hijacking plain numbers followed by words
        }

        let mut cursor = after_stem;
        let has_dot_word = is_dot_word(cleaned[cursor].as_str()) || words[cursor].contains('.');
        if has_dot_word {
            cursor += 1;
            if cursor >= words.len() {
                break;
            }
        }

        // Extension: either one multi-letter token or spelled-out letters.
        let ext_candidates: Vec<(usize, String)> = {
            let mut v = Vec::new();
            let tok = &cleaned[cursor];
            if !tok.is_empty() {
                v.push((1, tok.clone()));
            }
            let mut letters = String::new();
            let mut consumed = 0usize;
            while cursor + consumed < words.len()
                && consumed < 4
                && cleaned[cursor + consumed].chars().count() == 1
            {
                letters.push_str(&cleaned[cursor + consumed]);
                consumed += 1;
                v.push((consumed, letters.clone()));
            }
            v
        };

        for (ext_span, ext) in ext_candidates {
            if let Some(valid_ext) = is_ext(&ext) {
                let filename = format!("{stem_base}.{valid_ext}");
                if let Some(rel) = best_match(index, &filename, root) {
                    let span = (cursor - start) + ext_span;
                    return Some((span, rel));
                }
            }
        }

        if has_dot_word && cursor >= words.len() {
            break;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Process-unique scratch dir. Nanoseconds alone are insufficient:
    /// parallel tests reading the clock inside the same microsecond used to
    /// collide and share a fixture.
    fn unique_temp_dir(label: &str) -> PathBuf {
        static SEQ: AtomicU64 = AtomicU64::new(0);
        std::env::temp_dir().join(format!(
            "superflow_{label}_{}_{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn temp_project() -> PathBuf {
        let dir = unique_temp_dir("file_refs");
        std::fs::create_dir_all(dir.join("components/landing-page")).unwrap();
        std::fs::create_dir_all(dir.join("node_modules/somepkg")).unwrap();
        std::fs::write(dir.join("components/landing-page/hero.tsx"), "").unwrap();
        std::fs::write(dir.join("components/landing-page/Hero.tsx.bak"), "").unwrap();
        std::fs::write(dir.join("next.config.ts"), "").unwrap();
        std::fs::write(dir.join("main.py"), "").unwrap();
        std::fs::write(dir.join("node_modules/somepkg/index.ts"), "").unwrap();
        dir
    }

    fn resolve_in(dir: &Path, text: &str) -> Option<String> {
        // Exercises the real entry point; every test uses a unique temp dir,
        // so the per-root index cache can never leak state between tests.
        resolve_references(dir, text)
    }

    /// Marks `modified` as working-tree-dirty so activity signals fire.
    /// Commits everything first, then touches the targets — porcelain then
    /// reports a deterministic ` M <path>` per entry.
    fn with_git_dirty(dir: &Path, modified: &[&str]) {
        let have_git = std::process::Command::new("git")
            .arg("--version")
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false);
        if !have_git {
            return; // environment without git: boosts vanish, tests stay honest
        }
        let git = |args: &[&str]| {
            std::process::Command::new("git")
                .args(["-c", "core.fsmonitor=false"])
                .args(args)
                .current_dir(dir)
                .output()
        };
        let _ = git(&[
            "-c",
            "user.email=superflow@test",
            "-c",
            "user.name=superflow",
            "init",
        ]);
        let _ = git(&["add", "-A", "-f"]);
        let _ = git(&[
            "-c",
            "user.email=superflow@test",
            "-c",
            "user.name=superflow",
            "commit",
            "-m",
            "fixture",
        ]);
        for path in modified {
            let _ = std::fs::write(dir.join(path), "dirty");
        }
    }

    #[test]
    fn spoken_dot_form_resolves() {
        let dir = temp_project();
        let result = resolve_in(&dir, "edit the hero dot tsx file");
        assert_eq!(
            result.as_deref(),
            Some("edit the components/landing-page/hero.tsx file")
        );
    }

    #[test]
    fn spelled_letters_resolves() {
        let dir = temp_project();
        let result = resolve_in(&dir, "open hero doot t s x now");
        assert!(
            result.as_deref().unwrap_or_default().contains("hero.tsx"),
            "{result:?}"
        );
    }

    #[test]
    fn direct_suffix_resolves() {
        let dir = temp_project();
        let result = resolve_in(&dir, "run main py");
        assert_eq!(result.as_deref(), Some("run main.py"));
    }

    #[test]
    fn multiword_stem_uses_dots() {
        let dir = temp_project();
        let result = resolve_in(&dir, "check next config dot ts");
        assert_eq!(result.as_deref(), Some("check next.config.ts"));
    }

    #[test]
    fn ignores_node_modules_and_unknown_names() {
        let dir = temp_project();
        let text = "this has no file refs at all";
        assert!(resolve_in(&dir, text).is_none());
        // Unknown stem stays untouched.
        assert!(resolve_in(&dir, "open ghost dot tsx").is_none());
    }

    #[test]
    fn duplicate_basenames_are_not_guessed() {
        let dir = temp_project();
        std::fs::create_dir_all(dir.join("a/b")).unwrap();
        std::fs::write(dir.join("a/b/hero.tsx"), "").unwrap();
        // A basename alone is insufficient evidence when more than one file matches.
        let result = resolve_in(&dir, "open hero dot tsx");
        assert_eq!(result, None);
    }

    #[test]
    fn git_modified_twin_breaks_basename_tie() {
        let dir = temp_project();
        std::fs::create_dir_all(dir.join("a/b")).unwrap();
        std::fs::write(dir.join("a/b/hero.tsx"), "").unwrap();
        with_git_dirty(&dir, &["components/landing-page/hero.tsx"]);
        let result = resolve_in(&dir, "open hero dot tsx");
        assert_eq!(
            result.as_deref(),
            Some("open components/landing-page/hero.tsx"),
            "exactly one git-modified twin is decisive"
        );
    }

    #[test]
    fn vague_reference_stays_untouched_without_activity_evidence() {
        let dir = unique_temp_dir("vague_plain");
        std::fs::create_dir_all(dir.join("src/server/payments")).unwrap();
        std::fs::create_dir_all(dir.join("src/frontend/checkout")).unwrap();
        std::fs::write(dir.join("src/server/payments/payment-service.ts"), "").unwrap();
        std::fs::write(dir.join("src/frontend/checkout/Payment.tsx"), "").unwrap();
        // No git state: two plausible stems, no decisive evidence → refuse.
        assert_eq!(resolve_in(&dir, "fix the backend payment file"), None);
    }

    #[test]
    fn vague_reference_never_invents_git_active_server_file() {
        let dir = unique_temp_dir("vague_git");
        std::fs::create_dir_all(dir.join("src/server/payments")).unwrap();
        std::fs::create_dir_all(dir.join("src/frontend/checkout")).unwrap();
        std::fs::write(dir.join("src/server/payments/payment-service.ts"), "").unwrap();
        std::fs::write(dir.join("src/frontend/checkout/Payment.tsx"), "").unwrap();
        with_git_dirty(&dir, &["src/server/payments/payment-service.ts"]);
        assert_eq!(resolve_in(&dir, "fix the backend payment file"), None);
    }

    #[test]
    fn component_without_explicit_extension_stays_untouched() {
        let dir = temp_project();
        let result = resolve_in(&dir, "update the hero component");
        assert_eq!(result, None);
    }

    #[test]
    fn vague_tie_with_equal_activity_refuses() {
        let dir = unique_temp_dir("vague_tie");
        std::fs::create_dir_all(dir.join("src/server/payments")).unwrap();
        std::fs::create_dir_all(dir.join("src/lib")).unwrap();
        std::fs::write(dir.join("src/server/payments/payment-service.ts"), "").unwrap();
        std::fs::write(dir.join("src/lib/payment-utils.ts"), "").unwrap();
        with_git_dirty(
            &dir,
            &[
                "src/server/payments/payment-service.ts",
                "src/lib/payment-utils.ts",
            ],
        );
        // Both dirty → margin collapses below VAGUE_MIN_MARGIN → refuse.
        assert_eq!(resolve_in(&dir, "fix the backend payment file"), None);
    }

    #[test]
    fn shell_hook_marker_resolves_while_fresh_and_expires() {
        let base = tempfile::TempDir::new().unwrap();
        let proj = base.path().join("proj");
        std::fs::create_dir_all(&proj).unwrap();
        let marker_dir = base.path().join("superflow");
        std::fs::create_dir_all(&marker_dir).unwrap();
        let marker = marker_dir.join("cwd");
        std::fs::write(&marker, proj.to_str().unwrap()).unwrap();

        assert_eq!(hook_project_root_in(base.path()), Some(proj.clone()));

        let file = std::fs::File::options().write(true).open(&marker).unwrap();
        let stale = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
        file.set_times(std::fs::FileTimes::new().set_modified(stale))
            .unwrap();
        drop(file);
        assert_eq!(hook_project_root_in(base.path()), None);

        // A marker whose target vanished must never be returned either.
        std::fs::write(&marker, base.path().join("gone").to_str().unwrap()).unwrap();
        assert_eq!(hook_project_root_in(base.path()), None);
    }

    #[test]
    fn uri_decoding_works() {
        assert_eq!(
            uri_to_path("file:///Users/x/My%20Project"),
            Some(PathBuf::from("/Users/x/My Project"))
        );
        assert_eq!(uri_to_path("https://example.com"), None);
    }
}
