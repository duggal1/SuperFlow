# Quickstart Guide

Get `sp` (Sapphire Agent Factory) running in under 5 minutes.

## What is `sp`?

`sp` is a Rust CLI that orchestrates terminal-native coding agents (Codex, Claude, Qwen, Forge) as a **local control plane**. It launches one supervisor session plus multiple worker sessions, pushes prompts into PTYs, normalizes raw terminal output into Sapphire state, persists mission state into SQLite, and maintains a local control surface under `.sp/`.

It is **not** a model runner. It coordinates existing agent CLIs.

## Prerequisites

### Required

| Dependency | Version | Purpose |
|---|---|---|
| Rust | 1.94.1+ (stable) | Build and run the CLI |
| Coding agents | See below | Workers to orchest |

### Optional

| Dependency | Purpose |
|---|---|
| tmux | Teamwork surface — live pane grid with per-worker transcript tails |
| Ghostty (macOS) | Preferred terminal host for Ghostty-specific tab/window automation |

### Agent Requirements

At least one of these must be installed and available on your `$PATH`:

| Agent | Install | Notes |
|---|---|---|
| **Qwen Code** (`qwen`) | `npm install -g @anthropic/qwen-code` | Launches with `--screen-reader`; relies on user's existing approval config |
| **Codex** (`codex`) | `npm install -g @openai/codex` | Pinned to `gpt-5.4-mini` with low reasoning effort |
| **Claude Code** (`claude`) | `npm install -g @anthropic-ai/claude-code` | Auto-accepts folder trust prompt |
| **Forge** (`forge`) | Per your installation | Sets isolated `HOME`/`XDG_DATA_HOME`/`XDG_CONFIG_HOME` to `.sp/forge-home/` |

## Installation

### 1. Clone and Build

```bash
cd /path/to/sapphire-agent-factory
cargo build --release
```

The binary is available at `target/release/sp`.

### 2. Verify Build

```bash
cargo test
```

All unit tests should pass. This confirms the protocol parser, adapter layer, and store are functioning correctly.

### 3. (Optional) Install to PATH

```bash
cargo install --path .
```

Or add the `target/release` directory to your `$PATH`:

```bash
export PATH="$PWD/target/release:$PATH"
```

### 4. Verify Agent Availability

Ensure at least one agent CLI is installed:

```bash
which qwen    # Qwen Code
which codex   # OpenAI Codex
which claude  # Claude Code
which forge   # Forge
```

## First Run

### Dry Run (Plan Only, No Agents)

```bash
cargo run --bin sp -- codex 2 --repo . --mission "audit this repo and identify the top 3 risks" --dry-run
```

This decomposes the mission into workstreams, generates a plan, and exits without launching any agent sessions. Safe for testing.

### Live Run (Launch Agents)

```bash
cargo run --bin sp -- claude 4 --repo . --mission "refactor the auth module"
```

This launches 1 supervisor + 4 Claude workers (+ 1 AGENTS.md steward if count > 1). The current terminal becomes Sapphire's control UI, and a second terminal window opens the tmux teamwork grid.

### With tmux Teamwork Surface

```bash
cargo run --bin sp -- codex 4 --repo . --mission "implement feature X" --tmux-session-name sapphire
```

### Mixed Agents

Workers run one agent type; supervisor can be different:

```bash
cargo run --bin sp -- qwen 4 --repo . --mission "fix all failing tests" --supervisor-agent claude
```

## What Happens on Launch

1. **Bootstrap** — `.sp/` directory initialized, SQLite store created
2. **AGENTS.md check** — If missing in repo root, seeded from embedded instructions
3. **Supervisor planning** — Temporary supervisor session produces a JSON plan (45s timeout); launch fails if no valid plan
4. **Worker allocation** — Workstreams partitioned across workers; one slot reserved for AGENTS.md stewardship
5. **PTY launch** — All sessions spawned in isolated 30×140 PTYs with staggered prompts
6. **tmux surface** — Default teamwork grid opened in second terminal (unless `--dry-run`)
7. **Watchdog loop** — Real-time monitoring begins (1s tick interval)

## State Directory

All runtime state lives under `.sp/` in the repo root:

```
.sp/
├── sapphire.sqlite3      # SQLite database (9 tables)
├── control/
│   └── status.txt        # Live session states + watchdog stats
├── transcripts/          # Per-worker transcript logs (live runs)
└── prompts/              # Written prompt artifacts
```

## Next Steps

- [CLI Command Reference](./commands.md) — Full command and flag documentation
- [Contributor Guide](./contributor-guide.md) — Development workflow and change rules
- [AGENTS.md](../AGENTS.md) — Architecture, protocol spec, and runtime invariants
