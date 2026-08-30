# Contributor Guide

Development workflow, change rules, and architectural invariants for `sp`.

## Table of Contents

- [Project Overview](#project-overview)
- [Architecture Summary](#architecture-summary)
- [Development Workflow](#development-workflow)
- [Change Rules](#change-rules)
- [Testing Strategy](#testing-strategy)
- [Module Dependencies](#module-dependencies)
- [Prompt Files Are Executable](#prompt-files-are-executable)
- [Common Tasks](#common-tasks)

---

## Project Overview

`sp` (Sapphire Agent Factory) is a **local control plane** for terminal-native coding agents. It orchestrates Codex, Claude, Qwen, and Forge CLIs through PTY sessions, normalizes output into a 16-state lifecycle, persists state to SQLite, and provides a tmux-based teamwork surface.

**Key design decisions:**
- Terminal-first, not web-based
- Local control plane, not a model runner
- Machine-readable protocol for all worker communication
- Deterministic watchdog loop, not event-driven callbacks
- SQLite persistence, not a hosted database

---

## Architecture Summary

```
┌─────────────────────────────────────────────────────────┐
│                       sp (CLI)                          │
├──────────┬──────────────┬──────────────┬────────────────┤
│  CLI     │ Orchestrator │  Protocol    │  Store         │
│  (clap)  │ (watchdog)   │  (parser)    │  (SQLite)      │
├──────────┼──────────────┼──────────────┼────────────────┤
│  Agent   │ Runtime      │  Adapter     │  Templates     │
│  Kinds   │ (PTY/tmux)   │  (per-agent) │  (prompts)     │
├──────────┴──────────────┴──────────────┴────────────────┤
│                    TUI (ratatui)                         │
│              (startup/fallback dashboard)                │
└─────────────────────────────────────────────────────────┘
```

### Core Modules

| Module | File | Responsibility | Lines |
|---|---|---|---|
| CLI | `src/cli.rs` | Argument parsing, `LaunchConfig`/`ResumeConfig` construction | ~350 |
| Orchestrator | `src/orchestrator/mod.rs` | Mission lifecycle, watchdog loop, introspection | ~5068 |
| Runtime | `src/runtime/mod.rs` | PTY management, prompt injection, startup automation | ~400 |
| Protocol | `src/protocol.rs` | `SAPPHIRE_STATUS/MAIL/ACK/LEASE` parser | ~150 |
| Store | `src/store/mod.rs` | SQLite persistence (9 tables) | ~600 |
| Adapter | `src/adapter.rs` | Per-agent state detection, prompt building | ~500 |
| Agent | `src/agent/mod.rs` | Launch specs for Qwen/Forge/Codex/Claude | ~200 |
| Templates | `src/templates.rs` | Compiled prompt sources via `include_str!` | ~100 |
| Model | `src/model.rs` | Shared domain types | ~400 |
| TUI | `src/tui/` | Ratatui dashboard (not yet wired into main) | ~800 |
| tmux | `src/tmux/` | tmux CLI wrapper, pane layout | ~300 |
| Mail | `src/mail/` | Inter-worker mail with threading and ack | ~300 |

---

## Development Workflow

### 1. Set Up

```bash
cargo build --release
cargo test
```

### 2. Make Changes

Follow the [Change Rules](#change-rules) below. This project is small and tightly coupled — prefer surgical changes.

### 3. Verify

```bash
# Format
cargo fmt

# Lint
cargo clippy

# Test
cargo test

# Build (release)
cargo build --release
```

### 4. Test with Dry Run

```bash
cargo run --bin sp -- codex 2 --repo . --mission "test the dry run" --dry-run
```

### 5. Test Live (When Ready)

```bash
cargo run --bin sp -- claude 1 --repo . --mission "small test mission" --tmux-session-name test-mission
```

### 6. Inspect Results

```bash
# Check mission status
cargo run --bin sp -- status

# Replay events
cargo run --bin sp -- sessions
cargo run --bin sp -- replay <mission-id>

# Worker-specific replay
cargo run --bin sp -- watch <mission-id> worker-01
```

---

## Change Rules

These rules are critical. Breaking them breaks the orchestration loop.

### Rule 1: Protocol Contract

**Do not break the contract between prompt text and runtime parsing.** If you change protocol wording or supported states, update **all** of:

- `src/protocol.rs` — Parser regex and directive extraction
- `src/model.rs` — `SessionState` enum and `from_directive()` 
- `src/orchestrator/mod.rs` — State handling paths
- `src/adapter.rs` — Heuristic keyword lists and state detection
- `src/templates.rs` — Prompt instructions that teach the protocol
- Any prompt markdown that teaches the protocol (`product-direction.md`, `supervisor-templates/prompt.md`, `Agent-to-agent communication-simulate-swe-team.md`)

The full trace for a protocol change is:
```
templates -> orchestrator -> runtime -> protocol -> adapter -> store
```

### Rule 2: Session State Changes Are Global

If you add a session state, update:

1. `SessionState::from_directive()` in `src/model.rs` — Parse from JSON directive
2. `is_terminal()` in `src/model.rs` — Terminal state logic
3. Watchdog escalation in `src/orchestrator/mod.rs` — State transition handling
4. Prompt instructions that enumerate valid states
5. Adapter heuristic keyword lists in `src/adapter.rs` (`detect_state_impl`)

Terminal states are: `Validated`, `Failed`, `Exited`.

### Rule 3: Schema Migration

If you change the persistence schema or stored payload shape, **keep old runs readable** or add a migration strategy. Current approach: inline schema creation with opportunistic legacy table rename (`{name}_legacy_v0`) on column mismatch.

### Rule 4: Agent Launch Safety

If you change `AgentKind` launch behavior, preserve startup automation for trust/setup prompts:

| Agent | Automation |
|---|---|
| Qwen | Auto-dismisses "Do you want to connect IDE" (sends "2") |
| Codex | Auto-accepts directory trust (sends "1"); initial `\n` after 400ms |
| Claude | Auto-accepts "Yes, I trust this folder" |
| Forge | Sets isolated `HOME`/`XDG_DATA_HOME`/`XDG_CONFIG_HOME` |

A broken launch spec means the entire factory stalls at boot.

### Rule 5: Keep Worker Packets Narrow

Worker packets define scope boundaries via Blake3 hash. This codebase assumes ownership boundaries are a safety mechanism, not just prompt decoration.

### Rule 6: Prefer Surgical Changes

This project is small and tightly coupled. Avoid broad refactors without understanding the full loop.

### Rule 7: Mission/Session Naming

Mission/session/task/message/summary storage names do not perfectly match across all modules. **Read model, store, and orchestrator together** before changing any schema-facing code.

### Rule 8: Submit Mode Matters

`SubmitMode` (LineFeed vs CarriageReturn) is per-agent and affects both `send_prompt()` and startup automation rule responses. Do not change without updating all agent specs.

---

## Testing Strategy

### Unit Tests

```bash
cargo test
```

Current test coverage:
- **Protocol parser** — Multi-directive parsing, partial-line buffering, ANSI sanitization
- **Store** — CRUD operations, legacy migration, snapshot loading
- **Adapter** — State detection heuristics, prompt building

### No Integration Tests Yet

There is no separate integration test harness around PTY orchestration. The `runtime/` directory at repo root is empty and unused (the actual runtime is in `src/runtime/mod.rs`).

### Manual Testing

The primary testing method is a live run with `--dry-run` for planning verification, then a small live mission (1-2 workers) for runtime verification.

---

## Module Dependencies

```
main.rs
├── cli.rs
├── orchestrator/mod.rs
│   ├── agent/mod.rs
│   ├── adapter.rs
│   ├── runtime/mod.rs
│   ├── protocol.rs
│   ├── store/mod.rs
│   ├── templates.rs
│   ├── model.rs
│   ├── tmux/
│   └── mail/
├── tui/
└── model.rs
```

### Dependency Notes

- `model.rs` is consumed by almost everything — it defines all domain types
- `protocol.rs` is standalone — only the directive parser
- `templates.rs` is standalone — only prompt compilation
- `adapter.rs` depends on `model.rs`, `protocol.rs`, `templates.rs`, `agent/mod.rs`
- `orchestrator/mod.rs` is the hub — imports almost everything
- `store/mod.rs` depends on `model.rs` only

---

## Prompt Files Are Executable

Prompt files are **not passive documentation**. They are compiled into the binary at build time via `include_str!` and directly control agent behavior.

| File | Role |
|---|---|
| `product-direction.md` | Product direction and design philosophy |
| `supervisor-templates/prompt.md` | Supervisor prompt template with mission context |
| `How-should-the-supervisor-be-built.md` | Supervisor design specification |
| `Agent-to-agent communication-simulate-swe-team.md` | Inter-worker communication rules |
| `agents.md-instructions.md` | AGENTS.md seeding instructions (embedded at build time) |

**Small wording changes can materially alter orchestration quality.** Treat prompt edits with the same care as code changes.

---

## Common Tasks

### Add a New Agent

1. Add variant to `AgentKind` enum in `src/agent/mod.rs`
2. Define launch spec (executable, args, env, submit mode, automation rules)
3. The `impl_standard_adapter!` macro generates the adapter automatically
4. Qwen alone uses `PromptStyle::Compact`; all others use `PromptStyle::Standard`

### Add a New Session State

1. Add variant to `SessionState` enum in `src/model.rs`
2. Update `from_directive()` parser
3. Update `is_terminal()` logic
4. Add heuristic keywords in `src/adapter.rs` (`detect_state_impl`)
5. Update watchdog escalation in `src/orchestrator/mod.rs`
6. Update prompt instructions

### Add a New Protocol Directive

1. Update regex in `src/protocol.rs` (`consume_directives`)
2. Add directive type to `src/model.rs` if needed
3. Add handler in `src/orchestrator/mod.rs`
4. Update prompt files to document the new directive
5. Add unit tests in `src/protocol.rs`

### Change the SQLite Schema

1. Add/remove columns in `src/store/mod.rs` schema creation
2. Update all CRUD operations
3. Test legacy migration (rename to `{name}_legacy_v0`)
4. Ensure old runs are still readable or add explicit migration

### Add a New Subcommand

1. Add variant to `Command` enum in `src/cli.rs`
2. Add corresponding variant to `CliAction` enum
3. Update `Cli::into_action()` dispatch
4. Add handler in `src/main.rs`
5. All subcommands are flat — no nested subcommand enums

---

## See Also

- [Quickstart Guide](./quickstart.md) — Setup and first run
- [CLI Command Reference](./commands.md) — All commands and flags
- [AGENTS.md](../AGENTS.md) — Full architecture and protocol specification
