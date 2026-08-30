# CLI Command Reference

Complete reference for every `sp` command, subcommand, and flag.

## Command Overview

`sp` has **7 actions** and **6 subcommands**, all flat (no nested subcommand enums):

| Action | Trigger | Description |
|---|---|---|
| **Run** | Default (no subcommand) | Launch a new mission with full orchestration |
| **Status** | `sp status` | Show active/running missions at a glance |
| **Sessions** | `sp sessions` | List all missions from SQLite with status, repo, and summary |
| **Resume** | `sp resume <mission_id>` | Resume a mission from durable orchestration state |
| **Replay** | `sp replay <mission_id>` | Show recent events across all workers for a mission |
| **Watch** | `sp watch <mission_id> <worker>` | Show recent events for a specific worker |
| **Summary** | `sp summary <mission_id>` | Show supervisor's latest summary |

---

## Run — Launch a Mission

The default action. No subcommand needed.

### Syntax

```bash
sp <agent> <count> [mission text] [flags]
```

### Positional Arguments

| Argument | Type | Required | Description |
|---|---|---|---|
| `agent` | `qwen`, `forge`, `codex`, `claude` | Yes | Worker agent type |
| `count` | Positive integer | Yes | Number of worker terminals |
| `mission` | String | Yes | Mission description text for planning |

### Flags

| Flag | Default | Description |
|---|---|---|
| `--repo <path>` | `.` | Target repository root |
| `--supervisor-agent <agent>` | Same as worker | Agent for supervisor session |
| `--db-path <path>` | `.sp/sapphire.sqlite3` | Custom SQLite database path |
| `--state-dir <path>` | `.sp/` | Custom state directory |
| `--dry-run` | false | Plan only, no agents spawned |
| `--stall-seconds <n>` | `45` | Stall threshold for watchdog |
| `--watchdog-max-seconds <n>` | None | Hard timeout for watchdog loop |
| `--watchdog-tick-millis <n>` | `1000` | Tick interval for watchdog loop |
| `--tmux-session-name <name>` | Auto-generated | Name for tmux teamwork surface |
| `--persist-transcripts` | true | Write per-worker transcripts to `.sp/transcripts/` |
| `--tui` | false | Force the fallback single-terminal TUI dashboard |
| `--worker-args <args>` | None | Extra arguments passed to all workers |
| `--supervisor-arg <arg>` | None | Extra argument passed to supervisor (repeatable) |

### Examples

```bash
# 2 Codex workers audit this repo
sp codex 2 "audit this repo and identify the top 3 risks"

# 8 Qwen workers fix failing tests
sp qwen 8 "fix all failing tests"

# 4 Claude workers build a CLI tool (dry run)
sp claude 4 "build a CLI task runner" --dry-run

# Qwen workers with Claude supervisor
sp qwen 4 "refactor the payment module" --supervisor-agent claude

# Custom stall detection and state directory
sp codex 3 "investigate performance regressions" --stall-seconds 30 --state-dir /tmp/sp-state

# Custom tmux session name
sp codex 4 "implement feature X" --tmux-session-name my-mission

# Pass extra args to supervisor
sp claude 2 "refactor auth" --supervisor-arg "--dangerously-skip-permissions"
```

### What Happens

1. Parses CLI args into `LaunchConfig`
2. Initializes `.sp/` directories and SQLite store (`bootstrap()`)
3. Ensures `AGENTS.md` exists in repo root (seeds if missing)
4. Persists mission record
5. Runs supervisor planning (45s timeout for JSON plan)
6. Reserves one worker for AGENTS.md stewardship (if count > 1)
7. Spawns all PTY sessions (1 supervisor + N workers + 1 steward)
8. Opens tmux teamwork grid in second terminal (unless `--dry-run`)
9. Runs live watchdog event loop
10. Writes status snapshots to `.sp/control/status.txt`

---

## Status — Show Active Missions

### Syntax

```bash
sp status [flags]
```

### Flags

| Flag | Default | Description |
|---|---|---|
| `--db-path <path>` | `.sp/sapphire.sqlite3` | Custom SQLite database path |
| `--repo <path>` | `.` | Target repository root |
| `--state-dir <path>` | `.sp/` | Custom state directory |

### Output

Displays active/running missions with status, mission ID, repo path, and current state of each worker.

---

## Sessions — List All Missions

### Syntax

```bash
sp sessions [flags]
```

### Flags

| Flag | Default | Description |
|---|---|---|
| `--db-path <path>` | `.sp/sapphire.sqlite3` | Custom SQLite database path |
| `--repo <path>` | `.` | Target repository root |
| `--state-dir <path>` | `.sp/` | Custom state directory |

### Output

Lists all missions from SQLite history with mission ID, status, repo path, mission text, and creation timestamp.

---

## Resume — Resume a Stalled Mission

### Syntax

```bash
sp resume <mission_id> [flags]
```

### Positional Arguments

| Argument | Type | Required | Description |
|---|---|---|---|
| `mission_id` | UUID | Yes | ID of the mission to resume |

### Flags

| Flag | Default | Description |
|---|---|---|
| `--db-path <path>` | `.sp/sapphire.sqlite3` | Custom SQLite database path |
| `--state-dir <path>` | `.sp/` | Custom state directory |
| `--stall-seconds <n>` | `45` | Stall threshold for watchdog |
| `--watchdog-max-seconds <n>` | None | Hard timeout for watchdog loop |
| `--watchdog-tick-millis <n>` | `1000` | Tick interval for watchdog loop |
| `--tmux-session-name <name>` | `sp-resume-{uuid}` | Name for tmux teamwork surface |
| `--persist Transcripts` | true | Write per-worker transcripts |
| `--tui` | false | Force the fallback single-terminal TUI dashboard |

### What Happens

1. Loads all persisted state from SQLite for the given mission ID
2. Deserializes worker packets, summaries, and replay entries
3. Re-launches only non-terminal workers with prior context
4. Includes summary and replay excerpts in worker prompts
5. Resumes watchdog loop

### Example

```bash
sp resume 550e8400-e29b-41d4-a716-446655440000
```

---

## Replay — Show Recent Events

### Syntax

```bash
sp replay <mission_id> [flags]
```

### Positional Arguments

| Argument | Type | Required | Description |
|---|---|---|---|
| `mission_id` | UUID | Yes | ID of the mission to replay |

### Flags

| Flag | Default | Description |
|---|---|---|
| `-n, --limit <n>` | `40` | Maximum number of events to show |
| `--db-path <path>` | `.sp/sapphire.sqlite3` | Custom SQLite database path |

### Output

Shows recent events across all workers for a mission, including output chunks, directives, state changes, automation events, mail, and lease events.

### Example

```bash
sp replay 550e8400-e29b-41d4-a716-446655440000 -n 100
```

---

## Watch — Monitor a Single Worker

### Syntax

```bash
sp watch <mission_id> <worker> [flags]
```

### Positional Arguments

| Argument | Type | Required | Description |
|---|---|---|---|
| `mission_id` | UUID | Yes | ID of the mission |
| `worker` | String | Yes | Worker identifier (e.g., `worker-01`, `supervisor`, `steward`) |

### Flags

| Flag | Default | Description |
|---|---|---|
| `-n, --limit <n>` | `20` | Maximum number of events to show |
| `--db-path <path>` | `.sp/sapphire.sqlite3` | Custom SQLite database path |

### Output

Shows recent events for a specific worker only.

### Example

```bash
sp watch 550e8400-e29b-41d4-a716-446655440000 worker-02 -n 50
```

---

## Summary — Supervisor's Final Summary

### Syntax

```bash
sp summary <mission_id> [flags]
```

### Positional Arguments

| Argument | Type | Required | Description |
|---|---|---|---|
| `mission_id` | UUID | Yes | ID of the mission |

### Flags

| Flag | Default | Description |
|---|---|---|
| `--db-path <path>` | `.sp/sapphire.sqlite3` | Custom SQLite database path |

### Output

Displays the supervisor's latest (and final) mission summary, including workstream outcomes, risk assessment, and synthesis of worker outputs.

### Example

```bash
sp summary 550e8400-e29b-41d4-a716-446655440000
```

---

## Flag Reference (All Commands)

### Runtime Flags

| Flag | Type | Default | Commands | Description |
|---|---|---|---|---|
| `--stall-seconds` | u64 | `45` | Run, Resume | Seconds of worker silence before stall detection |
| `--watchdog-max-seconds` | u64 | None | Run, Resume | Hard timeout for watchdog loop |
| `--watchdog-tick-millis` | u64 | `1000` | Run, Resume | Tick interval for watchdog loop |

### Path Flags

| Flag | Type | Default | Commands | Description |
|---|---|---|---|---|
| `--repo` | PathBuf | `.` | Run, Status, Sessions | Target repository root |
| `--db-path` | PathBuf | `.sp/sapphire.sqlite3` | All | Custom SQLite database path |
| `--state-dir` | PathBuf | `.sp/` | Run, Status, Sessions, Resume | Custom state directory |

### tmux Flags

| Flag | Type | Default | Commands | Description |
|---|---|---|---|---|
| `--tmux-session-name` | String | Auto-generated | Run, Resume | Name for tmux teamwork surface |
| `--persist Transcripts` | Flag | true | Run, Resume | Enable per-worker transcript persistence |

### UI Flags

| Flag | Type | Default | Commands | Description |
|---|---|---|---|---|
| `--tui` | Flag | false | Run, Resume | Force single-terminal TUI dashboard |
| `--dry-run` | Flag | false | Run | Plan only, no agents spawned |

### Agent Flags

| Flag | Type | Default | Commands | Description |
|---|---|---|---|---|
| `--supervisor-agent` | AgentKind | Same as worker | Run | Agent for supervisor session |
| `--worker-args` | Vec<String> | None | Run | Extra arguments for all workers |
| `--supervisor-arg` | Vec<String> | None | Run | Extra argument for supervisor (repeatable) |

### Display Flags

| Flag | Type | Default | Commands | Description |
|---|---|---|---|---|
| `-n, --limit` | usize | Varies | Replay (40), Watch (20) | Maximum events to display |

---

## Agent Reference

| Agent | Value for `<agent>` | Executable | Notes |
|---|---|---|---|
| Qwen Code | `qwen` | `qwen` | Uses `--screen-reader`; CarriageReturn submit mode |
| Forge | `forge` | `forge` | LineFeed submit mode; isolated home directories |
| Codex | `codex` | `codex` | Pinned to `gpt-5.4-mini`; low reasoning effort |
| Claude Code | `claude` | `claude` | CarriageReturn submit mode; auto-accepts folder trust |

---

## Exit Codes

| Code | Meaning |
|---|---|
| `0` | Mission completed successfully or introspection command succeeded |
| `1` | General error (launch failed, planning failed, missing agent) |
| `2` | CLI parsing error (missing required arguments, invalid values) |

---

## Environment Variables

| Variable | Purpose |
|---|---|
| `TERM_PROGRAM` | Detects terminal host (e.g., `ghostty`) for teamwork surface behavior |
| `RUST_LOG` | Controls tracing log level (e.g., `RUST_LOG=debug sp codex 1 "..."`) |

---

## See Also

- [Quickstart Guide](./quickstart.md) — Setup and first run
- [Contributor Guide](./contributor-guide.md) — Development workflow
- [AGENTS.md](../AGENTS.md) — Full architecture and protocol specification
