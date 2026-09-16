//! Centralized authorization guard for code-file intelligence.
//!
//! Single policy layer answering `get_code_intel_context(): AllowedContext | None`.
//! Every downstream file operation must hold a verified context; no context = no operation.
//!
//! Invariant: intelligence runs only when the user explicitly enabled it, the active app is
//! allowlisted, and exactly one canonical active repository root is verified. Anything else
//! yields zero filesystem intelligence (fail closed, no fallbacks).

use crate::context::types::{ContextSnapshot, Surface};
use std::path::{Component, Path, PathBuf};

/// Strict allowlist of developer environments permitted to use code-file intelligence.
/// Anything not listed here — browsers, chat apps, Finder, unknown, missing — is denied.
pub const SUPPORTED_APP_TERMINAL: &str = "com.apple.Terminal";
pub const SUPPORTED_APP_GHOSTTY: &str = "com.mitchellh.ghostty";
/// VS Code identifier prefix (covers `com.microsoft.VSCode` and Insiders builds).
pub const SUPPORTED_APP_VSCODE_PREFIX: &str = "com.microsoft.VSCode";
pub const SUPPORTED_APP_CURSOR: &str = "com.todesktop.230313mzl4w4u92";

/// Directories never indexed (dependency/output noise, caches, artifacts).
pub const IGNORED_DIRS: &[&str] = &[
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
    ".parcel-cache",
    ".vercel",
    ".turbo",
    ".output",
    ".svelte-kit",
    "Pods",
    "DerivedData",
    ".idea",
    ".gradle",
];

/// Files larger than this are never read for symbol/diagnostic context.
pub const MAX_READ_BYTES: u64 = 256 * 1024;

/// Marker files proving a directory (or its ancestor) is a real project repository.
const PROJECT_MARKERS: &[&str] = &[".git", "Cargo.toml", "package.json"];

/// Verified authorization context. All values are trusted: the filesystem/search layer
/// must not independently decide to scan elsewhere.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllowedContext {
    /// Canonical bundle id of the verified supported application.
    pub bundle_id: String,
    /// Canonical absolute repository/workspace root. All file access must stay inside it.
    pub project_root: PathBuf,
    /// Canonical absolute working directory the root was derived from (shell cwd or workspace).
    pub workdir: PathBuf,
}

/// Strict allowlist check. Unknown / missing / ambiguous application → false.
pub fn is_supported_app(bundle_id: Option<&str>) -> bool {
    match bundle_id {
        Some(id) => {
            id == SUPPORTED_APP_TERMINAL
                || id == SUPPORTED_APP_GHOSTTY
                || id == SUPPORTED_APP_CURSOR
                || id == SUPPORTED_APP_VSCODE_PREFIX
                || id.starts_with(SUPPORTED_APP_VSCODE_PREFIX)
        }
        None => false,
    }
}

fn is_terminal_app(bundle_id: &str) -> bool {
    bundle_id == SUPPORTED_APP_TERMINAL || bundle_id == SUPPORTED_APP_GHOSTTY
}

fn is_editor_app(bundle_id: &str) -> bool {
    bundle_id == SUPPORTED_APP_CURSOR || bundle_id.starts_with(SUPPORTED_APP_VSCODE_PREFIX)
}

/// Canonicalize a path, failing closed on any error.
pub fn canonicalize_strict(path: &Path) -> Option<PathBuf> {
    std::fs::canonicalize(path).ok()
}

/// True when `candidate` (canonical) is contained inside `root` (canonical).
pub fn is_within_root(root: &Path, candidate: &Path) -> bool {
    candidate.starts_with(root)
}

/// Lexically normalize a possibly-nonexistent path (resolves `.`/`..` without touching disk).
fn normalize_lexical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Resolve a repository-relative path against `root` (both canonical) and prove containment.
/// Rejects `..` traversal, absolute-path injection, and symlink escapes (via canonicalization
/// of the existing file or its nearest existing ancestor).
pub fn resolve_within_root(root: &Path, rel: &str) -> Option<PathBuf> {
    let rel_path = Path::new(rel);
    if rel_path.is_absolute() {
        return None;
    }
    if rel_path.components().any(|c| {
        matches!(
            c,
            Component::ParentDir | Component::Prefix(_) | Component::RootDir
        )
    }) {
        // `..` segments are rejected outright: legitimate index entries never contain them.
        return None;
    }
    let joined = root.join(rel_path);
    // Canonicalize the file itself when it exists (resolves symlinks → catches escapes).
    if let Ok(canonical) = std::fs::canonicalize(&joined) {
        return is_within_root(root, &canonical).then_some(canonical);
    }
    // For not-yet-existing paths, canonicalize the nearest existing ancestor and re-attach.
    let mut ancestor = joined.as_path();
    let mut tail: Vec<std::ffi::OsString> = Vec::new();
    loop {
        if ancestor.exists() {
            let canonical = std::fs::canonicalize(ancestor).ok()?;
            if !is_within_root(root, &canonical) {
                return None;
            }
            let mut rebuilt = canonical;
            for part in tail.iter().rev() {
                if part == ".." || part == "." {
                    return None;
                }
                rebuilt.push(part);
            }
            let normalized = normalize_lexical(&rebuilt);
            return is_within_root(root, &normalized).then_some(normalized);
        }
        match ancestor.file_name() {
            Some(name) => {
                tail.push(name.to_os_string());
                match ancestor.parent() {
                    Some(parent) => ancestor = parent,
                    None => return None,
                }
            }
            None => return None,
        }
    }
}

/// Verify an absolute candidate path resolves inside `root`. Canonicalizes when possible.
pub fn verify_absolute_within_root(root: &Path, candidate: &Path) -> Option<PathBuf> {
    let abs = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        return None;
    };
    if let Ok(canonical) = std::fs::canonicalize(&abs) {
        return is_within_root(root, &canonical).then_some(canonical);
    }
    let normalized = normalize_lexical(&abs);
    is_within_root(root, &normalized).then_some(normalized)
}

/// Home directory of the current user, if determinable.
fn home_dir() -> Option<PathBuf> {
    std::env::var("HOME").ok().map(PathBuf::from)
}

/// Reject filesystem roots and the bare home directory as project roots — they are never
/// valid repositories by themselves (markers cannot credibly appear there for this feature).
fn is_bare_home_or_fs_root(path: &Path) -> bool {
    if path.parent().is_none() {
        return true;
    }
    if let Some(home) = home_dir() {
        if path == home {
            return true;
        }
    }
    false
}

/// Walk upward from `cwd` to find the nearest project marker. Returns the canonical repo root.
/// Never searches downward, never falls back to cwd/HOME/filesystem root.
pub fn anchor_repo_root(cwd: &Path) -> Option<PathBuf> {
    let canonical_cwd = canonicalize_strict(cwd)?;
    if !canonical_cwd.is_dir() {
        return None;
    }
    for ancestor in canonical_cwd.ancestors() {
        let has_marker = PROJECT_MARKERS
            .iter()
            .any(|marker| ancestor.join(marker).exists());
        if has_marker {
            let canonical = canonicalize_strict(ancestor)?;
            if is_bare_home_or_fs_root(&canonical) {
                return None;
            }
            return Some(canonical);
        }
    }
    None
}

/// Validate a candidate workspace/project root: canonical, is a directory, not bare
/// HOME/filesystem root. Terminal-derived roots must additionally be marker-anchored by the
/// caller (`anchor_repo_root`); editor workspace folders are user-declared projects.
fn validate_workspace_root(candidate: &Path) -> Option<PathBuf> {
    let canonical = canonicalize_strict(candidate)?;
    if !canonical.is_dir() {
        return None;
    }
    if is_bare_home_or_fs_root(&canonical) {
        return None;
    }
    Some(canonical)
}

/// Authoritative guard. Returns a verified context or `None` (feature disabled for this request).
/// `code_intel_enabled` must already be `master && smart_file_references`; false short-circuits
/// before any filesystem touch.
pub fn get_code_intel_context(
    snapshot: &ContextSnapshot,
    code_intel_enabled: bool,
) -> Option<AllowedContext> {
    if !code_intel_enabled {
        return None;
    }
    // Gmail/Slack prose must never gain file rewriting.
    if matches!(snapshot.surface, Surface::Gmail | Surface::Slack) {
        return None;
    }
    let bundle = snapshot.bundle_id.as_deref()?;
    if !is_supported_app(Some(bundle)) {
        return None;
    }
    if is_terminal_app(bundle) {
        let cwd = crate::file_refs::terminal_workdir()?;
        let root = anchor_repo_root(&cwd)?;
        let workdir = canonicalize_strict(&cwd)?;
        if !is_within_root(&root, &workdir) {
            return None;
        }
        Some(AllowedContext {
            bundle_id: bundle.to_string(),
            project_root: root,
            workdir,
        })
    } else if is_editor_app(bundle) {
        let workspace = crate::file_refs::editor_workspace_root(bundle)?;
        let root = validate_workspace_root(&workspace)?;
        Some(AllowedContext {
            bundle_id: bundle.to_string(),
            project_root: root.clone(),
            workdir: root,
        })
    } else {
        None
    }
}

/// Whether a repository-relative path is excludable noise (build artifacts, caches, dotfiles).
pub fn is_excluded_rel(rel: &str) -> bool {
    let first = rel.split('/').next().unwrap_or("");
    if IGNORED_DIRS.contains(&first) {
        return true;
    }
    for segment in rel.split('/') {
        if segment.starts_with('.') {
            return true;
        }
        if IGNORED_DIRS.contains(&segment) {
            return true;
        }
    }
    let file_name = rel.rsplit('/').next().unwrap_or(rel);
    let lower = file_name.to_ascii_lowercase();
    lower.starts_with('.')
        || lower.starts_with("id_rsa")
        || lower.starts_with("id_ed25519")
        || lower.contains("credential")
        || lower.contains("keychain")
        || lower.ends_with(".pem")
        || lower.ends_with(".key")
        || lower.ends_with(".p12")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::types::Surface;

    fn snapshot_with(bundle: Option<&str>, surface: Surface) -> ContextSnapshot {
        ContextSnapshot {
            surface,
            app_name: "Test".into(),
            bundle_id: bundle.map(str::to_string),
            url: None,
            title: None,
            focused_text: None,
            captured_at_ms: 0,
        }
    }

    #[test]
    fn allowlist_permits_only_supported_dev_apps() {
        assert!(is_supported_app(Some("com.apple.Terminal")));
        assert!(is_supported_app(Some("com.mitchellh.ghostty")));
        assert!(is_supported_app(Some("com.microsoft.VSCode")));
        assert!(is_supported_app(Some("com.microsoft.VSCodeInsiders")));
        assert!(is_supported_app(Some("com.todesktop.230313mzl4w4u92")));
        assert!(!is_supported_app(Some("com.google.Chrome")));
        assert!(!is_supported_app(Some("com.apple.Safari")));
        assert!(!is_supported_app(Some("com.tinyspeck.slackmacgap")));
        assert!(!is_supported_app(Some("com.googlecode.iterm2")));
        assert!(!is_supported_app(Some("dev.warp.Warp-Stable")));
        assert!(!is_supported_app(Some("net.kovidgoyal.kitty")));
        assert!(!is_supported_app(Some("com.vscodium.codium")));
        assert!(!is_supported_app(None));
        assert!(!is_supported_app(Some("")));
    }

    #[test]
    fn disabled_master_toggle_denies_without_fs_touch() {
        let snap = snapshot_with(Some("com.apple.Terminal"), Surface::Terminal);
        assert_eq!(get_code_intel_context(&snap, false), None);
    }

    #[test]
    fn unsupported_app_denied_even_with_valid_surface() {
        let snap = snapshot_with(Some("com.google.Chrome"), Surface::Terminal);
        assert_eq!(get_code_intel_context(&snap, true), None);
        let snap = snapshot_with(None, Surface::Terminal);
        assert_eq!(get_code_intel_context(&snap, true), None);
    }

    #[test]
    fn gmail_and_slack_surfaces_denied() {
        for surface in [Surface::Gmail, Surface::Slack] {
            let snap = snapshot_with(Some("com.apple.Terminal"), surface);
            assert_eq!(get_code_intel_context(&snap, true), None);
        }
    }

    #[test]
    fn dotdot_and_absolute_paths_rejected() {
        let root = std::env::temp_dir().join(format!("code_intel_guard_{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let canonical = canonicalize_strict(&root).unwrap();
        assert!(resolve_within_root(&canonical, "../escape.rs").is_none());
        assert!(resolve_within_root(&canonical, "a/../../escape.rs").is_none());
        assert!(resolve_within_root(&canonical, "/etc/passwd").is_none());
        // Ordinary relative path inside root is accepted (normalized form).
        let ok = resolve_within_root(&canonical, "src/main.rs");
        assert!(ok.is_some());
        assert!(is_within_root(&canonical, &ok.unwrap()));
    }

    #[test]
    fn symlink_escape_rejected() {
        let base = std::env::temp_dir().join(format!("code_intel_symlink_{}", std::process::id()));
        let root = base.join("root");
        let outside = base.join("outside");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("secret.rs"), "x").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(outside.join("secret.rs"), root.join("link.rs")).unwrap();
        #[cfg(unix)]
        {
            let canonical = canonicalize_strict(&root).unwrap();
            assert!(resolve_within_root(&canonical, "link.rs").is_none());
        }
    }

    #[test]
    fn anchor_rejects_home_and_fs_root_without_markers() {
        assert_eq!(
            anchor_repo_root(Path::new("/definitely/not/a/project-xyz")),
            None
        );
        if let Some(home) = home_dir() {
            if !home.join(".git").exists()
                && !home.join("Cargo.toml").exists()
                && !home.join("package.json").exists()
            {
                assert_eq!(anchor_repo_root(&home), None);
            }
        }
        assert_eq!(anchor_repo_root(Path::new("/")), None);
    }

    #[test]
    fn anchor_finds_nearest_marker_upward() {
        let base = std::env::temp_dir().join(format!("code_intel_anchor_{}", std::process::id()));
        let nested = base.join("proj").join("a").join("b");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::create_dir_all(base.join("proj").join(".git")).unwrap();
        let root = anchor_repo_root(&nested).expect("must anchor");
        assert_eq!(
            root,
            canonicalize_strict(&base.join("proj")).unwrap(),
            "nearest upward marker wins"
        );
    }

    #[test]
    fn excluded_paths_cover_build_artifacts() {
        for rel in [
            "node_modules/react/index.js",
            ".git/objects/x",
            ".next/cache/y",
            ".turbo/z",
            "dist/bundle.js",
            "build/out.js",
            "out/x",
            "coverage/lcov.info",
            ".cache/x",
            ".parcel-cache/y",
            ".vercel/z",
            ".hidden",
            "src/.secret",
        ] {
            assert!(is_excluded_rel(rel), "{rel} must be excluded");
        }
        assert!(!is_excluded_rel("src/App.tsx"));
        assert!(!is_excluded_rel("src-tauri/src/main.rs"));
    }
}
