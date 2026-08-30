# Phase 4 Documentation Validation Report

**Validator:** Validator-1 (Validation Engineer)
**Date:** 2026-04-04
**Scope:** README.md, AGENTS.md, TODO.md — accuracy vs. actual codebase state

---

## Executive Summary

Overall documentation accuracy is **high** (~90%+). Most architectural claims, code paths, and feature states match the actual source code. However, several discrepancies were found ranging from minor stale references to one contradictory claim between two docs.

---

## Findings

### 1. TUI Wiring Status — CONTRADICTION BETWEEN DOCS ⚠️ MEDIUM

| Document | Claim | Actual State | Verdict |
|---|---|---|---|
| **AGENTS.md** (§ Code Map, `src/tui/`) | "Not yet wired into `main.rs` — lives behind `#![allow(dead_code)]`" | TUI **IS** wired into `main.rs` (lines 34-69, 117-150). Uses `tui::run_enabled_for_launch()`, `tui::attach_for_repo()`, `tui::run_startup_dashboard_until_tmux()`, `tui::run_launch_dashboard()`. | **OUTDATED** |
| **README.md** (§ Architecture) | "TUI (ratatui) — startup/fallback dashboard shell" shown as core component | Accurate — the TUI is indeed the startup/fallback dashboard. | **ACCURATE** |
| **Actual code** | `src/tui/app.rs` and `src/tui/data.rs` have `#![allow(dead_code)]` | Some functions within TUI modules are unused, but the TUI as a whole **is** wired into main.rs. | Partially true |

**Fix needed:** Update AGENTS.md § Code Map (`src/tui/`) to say: "Ratatui/crossterm TUI dashboard shell, **wired into `main.rs` as the startup/fallback dashboard**. Individual modules (`app.rs`, `data.rs`) carry `#![allow(dead_code)]` for unused helper functions."

---

### 2. Orchestrator Line Count — MINOR STALE ⚠️ LOW

| Document | Claim | Actual State | Verdict |
|---|---|---|---|
| **AGENTS.md** (§ Code Map, `src/orchestrator/mod.rs`) | "~5068 lines" | **5186 lines** (`wc -l`) | **STALE** (+118 lines) |

**Fix needed:** Update to "~5186 lines" or "~5.2K lines".

---

### 3. Test Count — MINOR STALE ⚠️ LOW

| Document | Claim | Actual State | Verdict |
|---|---|---|---|
| **QWEN.md** (memory) | "77 tests pass" | **79 tests pass** (`cargo test` result) | **STALE** |
| **TODO.md** (§ 3.11) | "Tests pass (79/79)" | **79 tests pass** | **ACCURATE** |
| **AGENTS.md** (§ Current Gaps) | No specific test count claimed | N/A | N/A |

**Fix needed:** Update QWEN.md memory reference from 77 to 79.

---

### 4. Protocol Parser Implementation — README ACCURATE ✅

| Document | Claim | Actual State | Verdict |
|---|---|---|---|
| **README.md** (§ Sapphire Control Protocol) | "extracts JSON objects using a brace-depth counter (not regex) for robustness with nested/multiline JSON" | `extract_json_object_range()` in `src/protocol.rs` (line ~148) does brace-depth counting. `parse_next_directive()` finds `SAPPHIRE_` prefix via string search + regex for the kind, then brace-depth for JSON extraction. | **ACCURATE** |
| **AGENTS.md** (§ `src/protocol.rs`) | "The regex is `SAPPHIRE_(STATUS|MAIL|ACK|LEASE) (\{.*\})`" | **OUTDATED** — the current implementation uses `directive_prefix_regex()` for the prefix kind only, then `extract_json_object_range()` for brace-depth JSON extraction. The old regex-based JSON extraction was replaced. | **STALE** |

**Fix needed:** Update AGENTS.md to describe the current two-phase parsing: regex for `SAPPHIRE_(STATUS|MAIL|ACK|LEASE)` prefix identification, then brace-depth counter for JSON object extraction.

---

### 5. Security Audit Claims — ACCURATE ✅

The README security audit section accurately reflects the codebase:

| Area | Claim | Verification | Verdict |
|---|---|---|---|
| `shell_quote()` | "Standard POSIX quoting pattern" | Found in 3 locations (`runtime/mod.rs:744`, `tmux/mod.rs:399`, `orchestrator/mod.rs:4274`) — all use `format!("'{}'", text.replace('\'', "'\"'\"'"))` | **ACCURATE** |
| SQLite binding | "All queries use `params![]` macro" | Confirmed in `store/mod.rs` | **ACCURATE** |
| ANSI sanitization | `sanitize_output()` strips ANSI via regex | Found at `protocol.rs:98`, uses `ansi_escape_regex()` | **ACCURATE** |
| Buffer management | "24KB cap, trim-to-12KB" | `BufferManager::new(24_000, 12_000)` in `runtime/mod.rs:159` | **ACCURATE** |
| Protocol buffer | Not mentioned in README | `protocol.rs` has separate 128KB/64KB buffer for directive parsing | **GAP** (not a security concern, but worth noting) |

---

### 6. Phase 4 Status — TODO.md ACCURATE ✅

| Phase | Claim | Actual State | Verdict |
|---|---|---|---|
| 4.1 Live Watchdog Dashboard | 🚧 TODO | Watchdog loop exists in orchestrator but live dashboard polling/dashboard display is not implemented as a separate component | **ACCURATE** |
| 4.2 Structured Output Aggregation | 🚧 TODO | No `.sp/live-status.json` persistence found | **ACCURATE** |
| 4.3 Team Role Naming | ✅ Done | `role_type` and `display_name` fields exist in `WorkerPacket` (`model.rs:84-86`), 13 role templates loaded, `generate_display_name()` in `adapter.rs:1119` | **ACCURATE** |
| 4.4 Stuck/Error Detection | 🚧 TODO | Stall detection exists (escalation ladder) but specific "same output for 3+ consecutive polls" detection is not implemented | **ACCURATE** |
| 4.5 Live Logging | 🚧 TODO | No `.sp/watchdog-log.jsonl` found | **ACCURATE** |
| 4.2.1-4.2.4 Supervisor Behavioral Rules | ✅ Done | `consecutive_stall_failures` field exists (`orchestrator/mod.rs:78`), escalation ladder implemented, propulsion/solo artist rules in templates | **ACCURATE** |

---

### 7. CLI Actions Count — MINOR DISCREPANCY ⚠️ LOW

| Document | Claim | Actual State | Verdict |
|---|---|---|---|
| **AGENTS.md** (§ Post-Launch Introspection) | "6 CLI actions + run" | `CliAction` enum has 7 variants: `Run`, `Status`, `Sessions`, `Replay`, `Summary`, `Resume`, `Watch` | **ACCURATE** (6 post-launch + 1 run = 7 total) |
| **README.md** (§ CLI Commands) | Lists 7 commands | Matches `CliAction` enum | **ACCURATE** |

No discrepancy — just confirming the count is correct.

---

### 8. `src/internal/agents/templetes/` Typo — COSMETIC ⚠️ LOW

The directory `src/internal/agents/templetes/roles/job-roles/` contains a typo: "templetes" should be "templates". This is referenced in:
- `src/templates.rs` (via `include_str!`)
- All 13 role markdown files have `role_type: enterprise_team_role` in their frontmatter (not machine-key format as documented)

**Note:** The role template files use `role_type: enterprise_team_role` as a YAML frontmatter field, but the actual machine keys used in code are the filenames (e.g., `software-engineer.md` → `"software-engineer"`). The `role_type` field in the markdown frontmatter is not parsed — it's the filename that matters. This is consistent with how `PromptLibrary::load_role_templates()` works (iterating directory entries by filename).

---

### 9. 16-State Lifecycle — ACCURATE ✅

Both AGENTS.md and README.md list the same 16 states:
`Planned → Booting → NotStarted → Progressing → Blocked → Stalled → DoneClaimed → NeedsValidation → WeakOutput → WrongDirection → Contradictory → NeedsRetry → Validated → Failed → Exited`

Verified in `src/model.rs` — `SessionState` enum matches. Terminal states (`Validated`, `Failed`, `Exited`) match `SessionState::is_terminal()`.

---

## Summary of Required Fixes

| Priority | Document | Section | Issue | Fix |
|---|---|---|---|---|
| **MEDIUM** | AGENTS.md | Code Map `src/tui/` | Says "Not yet wired into main.rs" | Update to confirm TUI IS wired in |
| **MEDIUM** | AGENTS.md | Code Map `src/protocol.rs` | Says regex extracts JSON | Update to describe brace-depth parsing |
| **LOW** | AGENTS.md | Code Map `src/orchestrator/mod.rs` | "~5068 lines" | Update to "~5186 lines" |
| **LOW** | QWEN.md | Memory | "77 tests pass" | Update to "79 tests pass" |
| **LOW** | README.md | Security § Protocol Parser | Doesn't mention 128KB/64KB protocol buffer | Minor addition |
| **COSMETIC** | All docs | Path references | "templetes" directory typo | Note as cosmetic only — changing would require code updates |

---

## Validation Conclusion

**Overall accuracy: 90%+**

The documentation is substantially accurate and reflects the actual codebase state. The most significant issue is the contradictory TUI wiring status between AGENTS.md (says not wired) and README.md (says wired) — the truth is that the TUI **is wired** into `main.rs`. All Phase 4 completion markers in TODO.md are accurate. The escalation ladder, role naming, and supervisor behavioral rules are all implemented as documented.

**No critical inaccuracies found.** No misleading claims that would cause a contributor to make incorrect architectural decisions.
