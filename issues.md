# Code-File Intelligence, Hands-Free, and Clipboard — Root Causes & Implementation Plan

Scope: `src-tauri/src/file_refs.rs`, `src-tauri/src/code_context.rs`, `src-tauri/src/context/*`,
`src-tauri/src/actions.rs`, `src-tauri/src/managers/audio.rs`, `src-tauri/src/settings.rs`,
`src-tauri/src/shortcut/*`, `src-tauri/src/transcription_coordinator.rs`, `src-tauri/src/escape_cancel.rs`,
`src-tauri/src/clipboard.rs`, `src-tauri/src/paste_tx/*`, frontend `AdvancedSettings` + settings store.
No unrelated refactors. Fail closed everywhere.

## Issue 1 — File intelligence scans outside the active repository (CRITICAL)

**Current unsafe behavior** (`file_refs.rs`):

- `project_root_for_snapshot` (`96`) falls through: bundle root → `repo_root_from_cwd_if_project` →
  `newest_shell_project_root` (newest `zsh|bash|fish|sh|pwsh|nu` **system-wide** via `ps`+`lsof`, no
  terminal-app scoping, no uid check) → `repo_root_from_cwd` (walks `current_dir().ancestors()` to `/`,
  last resort returns **bare cwd even with no markers**, `276`).
- App cwd is SuperFlow's own launch dir, not the user's terminal — so a degraded snapshot indexes the
  wrong tree. In the worst case (`/` or `$HOME` as cwd) `walk` (`504`, 30k files / depth 14) enumerates
  another repo, `$HOME`, or `/`.
- `hook_project_root` (`306`) trusts `$TMPDIR/superflow/cwd` content (attacker-writable) with only a
  600s mtime + `is_dir` check: planted path → arbitrary-dir indexing (TOCTOU).
- `editor_project_root` (`383`) reads editor `storage.json` via `$HOME.join(rel)` and falls back to
  `history.recentlyOpenedPathsList[0]` (stale workspace from another session).
- `cached_cd_root` (`129`, 5s TTL, keyed on nothing) reuses a stale root across apps/snapshots.
- No canonicalization or containment: `resolve_references`, `project_index`/`walk`, and
  `code_context::collect_resolved_files`/`scan_symbols` join paths without proving they stay inside the
  root. No `..`/symlink/absolute-path rejection. `walk` follows symlinked dirs via `read_dir`+`file_type`
  without a symlink check.
- Tests hardcode `/Users/harshitduggal/workspace/SuperFLow-macos` and assert degraded snapshots
  **must** resolve a repo (`gmail_and_slack_never_get_project_root`, `aggressive_live_repo_end_to_end`),
  i.e. the suite currently locks in the unsafe fallback.

**Intended behavior**: intelligence runs only inside one canonical, verified active project root.
Every candidate path is canonicalized and proven `root`-contained before access. Unprovable → reject,
no broader search.

**Strategy**:

- New authoritative `src-tauri/src/code_intel.rs` guard: `get_code_intel_context(snapshot, enabled)
-> Option<AllowedContext { bundle_id, project_root (canonical), workdir (canonical) }>`. All
  downstream file ops require it; no context → zero scanning.
- Strict allowlist `SUPPORTED_DEV_APPS = com.apple.Terminal, com.mitchellh.ghostty,
com.microsoft.VSCode (prefix, covers Insiders), com.todesktop.230313mzl4w4u92 (Cursor)`. Unknown /
  missing / ambiguous bundle → deny. (Surface classification in `context/classify.rs` is untouched;
  the allowlist lives in the guard so iTerm2/Warp/kitty/Alacritty/WezTerm/browsers/ChatGPT/Slack/etc
  classify fine but never get file intelligence.)
- Terminal/Ghostty: accept hook cwd **only if** canonical + inside a marker-anchored repo root
  (`.git` | `Cargo.toml` | `package.json` walked **upward only**, never downward, never `$HOME`/`/`
  themselves unless they carry markers — they don't); else BFS newest shell **under the frontmost
  terminal's process subtree only**, validated the same way. Delete: system-wide newest-shell fallback,
  `repo_root_from_cwd*` fallbacks, bare-cwd return, global CD cache (or key it per bundle+pid — simpler:
  delete it; resolution is already ms-scale and index has its own TTL).
- VS Code/Cursor: only `windowsState.lastActiveWindow.folder` from `storage.json`; delete the
  `recentlyOpenedPathsList` stale fallback. Canonicalize, require `is_dir`, reject `$HOME`. Never merge
  multiple workspace folders; ambiguous → `None`.
- Gmail/Slack surfaces → `None` (keep). `Surface::Other` / missing bundle → `None` (deny; removes the
  degraded-snapshot CD fallback that tests currently assert).
- `resolve_within_root(root, rel)` + `is_path_within(root, abs)`: canonicalize root once;
  for each candidate, canonicalize (files) or lexical-normalize + component check (missing paths),
  reject `..` escapes, absolute-path injection, symlink escapes. Wire into `resolve_references`,
  `code_context::collect_resolved_files`/`scan_symbols`/`normalize_diag_rel` (absolute diags must
  suffix-match an anchored file **and** canonical-resolve inside root).
- `walk`: skip symlinks (`symlink_metadata` / `file_type().is_symlink()`), keep depth/file caps,
  extended `IGNORED_DIRS` (add `.parcel-cache`, `.vercel`), skip `.gitignore`-ignored paths via the
  existing `ignore` crate walker semantics (no new deps), skip files > 256 KiB and dotfiles/sensitive
  names (keep `is_sensitive_file_name`).

**Edge cases**: hook file missing/stale/planted; shell cwd = `$HOME`/`/`/`~/Documents`/`~/projects`
(bare dir, no markers → deny); nested repos (nearest upward marker wins); workspace folder deleted;
`storage.json` malformed; canonicalization failure → deny; case-sensitive vs insensitive macOS paths
(compare canonical strings, no lowercasing).

**Verification**: supported app + valid repo works; + `$HOME`/`/`/non-repo denied; unsupported app
denied even with valid-looking text; file in sibling repo never resolves; `../` escape rejected;
symlink-escape rejected; `node_modules`/`.git` ignored; master toggle off → zero FS access
(see §7); enabled-but-unknown-repo → denied.

## Issue 2 — No-repository fallback scanning

**Root cause**: same fallbacks as Issue 1 (`repo_root_from_cwd`, bare-cwd return, system-wide newest
shell, stale CD cache, history fallback). A filename match anywhere is treated as permission to infer a
repo (spec: "scan machine → infer project").

**Intended**: no valid root → feature fully disabled for that request, zero scanning.

**Strategy**: guard returns `None`; `capture_recording_context(resolve_project=false)` short-circuits
before any FS touch; `resolve_references`/`maybe_enhance` never called without `AllowedContext`.
Update the two tests that assert the opposite.

## Issue 3 — No strict application allowlist

**Root cause**: `project_root()` dispatches on `TERMINAL_BUNDLE_IDS` (8 terminals) else
`editor_project_root` for **any** bundle string; degraded snapshots ignore the app entirely.
Anything with a shell nearby gets intelligence.

**Strategy**: guard allowlist (§Issue 1). `allowed = bundle ∈ {Terminal, Ghostty, VSCode*, Cursor}`.
Everything else (ChatGPT, Chrome, Safari, Slack, Finder, Mail, Notion, Electron, unknown, missing)
→ deny before any FS work.

## Issue 4 — Terminal cwd outside a real project still gets intelligence

**Root cause**: hook/BFS cwd returned verbatim (`cwd.is_dir().then_some(cwd)`) without marker validation.

**Strategy**: `anchor_repo_root(cwd)`: canonicalize; if `cwd` is filesystem root or `$HOME` → deny
unless markers present (they never are); else walk **upward only** for `.git`/`Cargo.toml`/
`package.json`; none → deny. Never search downward/children, never reuse last repo.

## Issue 5 — VS Code/Cursor workspace handling

**Root cause**: `editor_project_root` merges `lastActiveWindow` with stale history; multi-root
workspaces collapse to one `folderUri`; no per-editor-file scoping.

**Strategy**: only `lastActiveWindow.folder`; drop history fallback; canonicalize + `is_dir` +
not-`$HOME`; single root only. Active-file scoping is unavailable to the backend (no editor extension
channel), so multi-root workspaces resolve to the last-active folder only — documented limitation,
fail closed otherwise.

## Issue 6 — Repository filtering gaps

**Root cause**: `IGNORED_DIRS` lacks `.parcel-cache`, `.vercel`; no `.gitignore` respect; no file-size
cap (huge/generated files parsed by `scan_symbols` up to 3000 lines regardless of bytes); `walk`
records dot-dirs only by name prefix (ok) but follows symlinks.

**Strategy**: extend ignore list, skip symlinks, honor `.gitignore` via `ignore` crate's `WalkBuilder`
(or manual `.gitignore` pattern load if walker swap is too invasive — prefer `WalkBuilder` with
`hidden(false→skip dotfiles manually to preserve behavior)`, `git_ignore(true)`, `max_depth`,
`follow_links(false)`), cap indexed/read files at 256 KiB, keep 30k/14 caps + 30s TTL.

## Issue 7 — Missing master toggle

**Root cause**: only `smart_file_references_enabled` (default true) gates capture; `AdvancedSettings`
doesn't render it; no single binary authorization gate.

**Strategy**: add `AppSettings.code_intelligence_enabled` (default true, serde default for old stores),
`change_code_intelligence_enabled_setting` command + specta registration, `bindings.ts` +
...[truncated 6154 chars]

## Release

Shipped as **v1.0.4** (bump 1.0.3 → 1.0.4 in `package.json`,
`src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`).
Release is cut by pushing tag `v1.0.4` (see `.github/workflows/release.yml` guards).

Verification at ship time:

- `cargo check --lib` (macOS): pass, zero warnings from touched code.
- `cargo fmt --check`: clean. `prettier --check` on touched frontend files: clean.
- Translation consistency: new `codeIntelligence` keys present in all 24 locales
  (the checker still reports pre-existing drift on unrelated `agents.*` keys).
- `cargo test --lib` / full `cargo build`: **not runnable in this environment** —
  pre-existing `tauri_build` script failure on the clean tree (silent exit 1;
  `binaries/audiocpp_cli` sidecar absent) plus a pre-existing broken
  `tests/transcript_normalization_comprehensive.rs` (`formatter` unresolved).
  CI (`test.yml`, release matrix) is the gate for the unit suite.
- `bun run lint` / `bunx tsc`: not runnable here (eslint + @types/node not
  installed); frontend changes mirror existing component patterns exactly.
