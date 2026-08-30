# sp — Sapphire CLI

```text
 ░██████      ░███    ░█████████  ░█████████  ░██     ░██ ░██████░█████████  ░██████████
 ░██   ░██    ░██░██   ░██     ░██ ░██     ░██ ░██     ░██   ░██  ░██     ░██ ░██
░██          ░██  ░██  ░██     ░██ ░██     ░██ ░██     ░██   ░██  ░██     ░██ ░██
 ░████████  ░█████████ ░█████████  ░█████████  ░██████████   ░██  ░█████████  ░█████████
        ░██ ░██    ░██ ░██         ░██         ░██     ░██   ░██  ░██   ░██   ░██
 ░██   ░██  ░██    ░██ ░██         ░██         ░██     ░██   ░██  ░██    ░██  ░██
  ░██████   ░██    ░██ ░██         ░██         ░██     ░██ ░██████░██     ░██ ░██████████
```

Launch, supervise, and audit multiple coding agents from one command.

`sp` is Sapphire: a terminal-first orchestration layer for running coding agents as a coordinated team instead of as isolated terminal tabs.

## Why Use It

When you run multiple agents by hand, the operator becomes the scheduler, router, reviewer, and status board.

Sapphire exists to take over that coordination layer:

- launch multiple workers from one command
- supervise them through a single control surface
- keep mission state durable
- make the run inspectable and resumable

## Core Capabilities

- supervised multi-agent launch
- no-supervisor terminal launch
- mission replay and summary views
- per-worker inspection
- durable state under `.sp/`
- tmux-based teamwork surface

## Launch With Supervision

```bash
sp <agent> <count> "mission text"
```

- `agent`: `qwen`, `codex`, `claude`, `forge`
- `count`: number of worker terminals
- `mission`: what the agents should do

### Examples

```bash
# 2 Claude workers audit this repo
sp claude 2 "audit this repo and identify the top 3 risks"

# 4 Claude workers build a CLI tool (dry run)
sp claude 4 "build a CLI task runner" --dry-run

# Claude workers with Codex supervisor
sp claude 4 "refactor the payment module" --supervisor-agent codex
```

## Launch Without a Supervisor

```bash
sp ns <agent> <count> "<prompt 1>" "<prompt 2>" ... "<prompt N>"
```

- `ns` is the no-supervisor terminal launcher
- `np` is supported as an alias
- prompt count must exactly match `count`
- use this when you want direct terminal launch capability and you supervise manually

### Examples

```bash
# 3 Claude terminals with distinct prompts
sp ns claude 3 "audit auth" "audit billing" "audit tests"

# 20 Claude terminals launched directly
sp ns claude 20 "prompt 1" "prompt 2" "prompt 3" "prompt 4" "prompt 5" "prompt 6" "prompt 7" "prompt 8" "prompt 9" "prompt 10" "prompt 11" "prompt 12" "prompt 13" "prompt 14" "prompt 15" "prompt 16" "prompt 17" "prompt 18" "prompt 19" "prompt 20"
```

## Inspect

```bash
sp status              # Active missions at a glance
sp sessions            # Full history
sp replay <id>         # Replay events
sp summary <id>        # Supervisor summary
sp resume <id>         # Resume stalled mission
sp watch <id> <worker> # Single worker journey
sp push                # Operator-owned git push path
```

## Common Flags

| Flag | What it does |
|---|---|
| `--tui` | Force the fallback single-terminal TUI dashboard instead of the default split teamwork surface |
| `--tmux-session-name <name>` | Override the default teamwork session name |
| `--dry-run` | Plan only, no agents spawned |
| `--repo <path>` | Target repo (default: `.`) |
| `--stall-seconds <n>` | Stall threshold (default: `45`) |
| `--supervisor-agent <agent>` | Different supervisor than workers |

## TUI Controls

| Key | Action |
|---|---|
| `Tab` / `Shift+Tab` | Switch pane / cycle tabs |
| `1` | Workers tab |
| `2` | Watchdog tab |
| `3` | Events tab |
| `4` | Supervisor tab |
| `j` / `k` or `↑` / `↓` | Scroll |
| `q` | Quit when done |

## How It Works

1. Plan: decomposes the mission into workstreams.
2. Launch: keeps the current terminal as Sapphire control UI and opens the tmux teamwork grid in Ghostty tabs when available.
3. Coordinate: uses leases, mail, and status markers.
4. Supervise: validates output and resolves conflicts.
5. Persist: writes mission state to `.sp/sapphire.sqlite3`.

## Host Behavior

- If launched from Ghostty, Sapphire opens the teamwork grid in Ghostty tabs.
- If Ghostty is not open yet, Sapphire launches Ghostty once, uses the first tab, then adds tabs for the rest.
- Sapphire does not fall back to opening extra Ghostty windows for the tmux teamwork grid.
- If launched from VS Code or another terminal host, Sapphire opens the teamwork grid in a separate external terminal window.

## Install

Full installer:

```bash
curl -fsSL https://raw.githubusercontent.com/duggal1/sapphire-harness/master/install.sh | bash
```

Simple installer:

```bash
curl -fsSL https://raw.githubusercontent.com/duggal1/sapphire-harness/master/install-simple.sh | bash
```

Build locally:

```bash
cargo build --release
```

## Requirements

- Rust
- tmux
- at least one supported agent CLI installed locally

Supported agent CLIs:

- qwen
- codex
- claude
- forge
