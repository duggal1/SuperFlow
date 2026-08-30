# Storage, Mail, and tmux Architecture

## Overview

Three subsystems handle Sapphire's coordination surface:

| Subsystem | Path | Role |
|---|---|---|
| Store | `src/store/mod.rs` | SQLite persistence — 9 tables, WAL mode, legacy migration |
| Mail | `src/mail/` | Durable inter-worker messaging with priority, threading, ack tracking |
| tmux | `src/tmux/` | Terminal teamwork surface — pane grid, dashboard, external terminal opening |

All three are consumed by the orchestrator (`src/orchestrator/mod.rs`). This document traces data flow, not orchestration logic.

---

## 1. Persistence Layer (`src/store/mod.rs`)

### 1.1 Database Bootstrap

`Store::open(path)` performs:
1. Creates parent directory (nested `mkdir -p` semantics)
2. Opens SQLite via `rusqlite` (bundled, no system dependency)
3. Sets pragmas: `journal_mode=WAL`, `synchronous=NORMAL`, `foreign_keys=OFF`
4. Runs legacy migration on 5 tables (`sessions`, `events`, `messages`, `normalized_updates`, `validation_results`) — renames to `{table}_legacy_v0` if column sets don't match
5. Creates all 9 tables via `CREATE TABLE IF NOT EXISTS`

Default path: `.sp/sapphire.sqlite3`

### 1.2 Schema — 9 Tables

#### `sessions` — Mission records

| Column | Type | Purpose |
|---|---|---|
| `id` | TEXT PK | Mission UUID |
| `created_at` | TEXT | ISO-8601 timestamp |
| `updated_at` | TEXT | ISO-8601 timestamp |
| `repo_path` | TEXT | Absolute path to target repo |
| `agent_type` | TEXT | Formatted as `"workers:{kind} supervisor:{kind}"` |
| `user_mission_raw` | TEXT | Original mission string from CLI |
| `mission_rewrite` | TEXT | Supervisor-refined mission description |
| `status` | TEXT | `MissionStatus` string (`planned`/`launching`/`running`/`completed`/`failed`) |
| `final_summary` | TEXT | Supervisor's closing synthesis (nullable) |
| `plan_json` | TEXT | Serialized `MissionPlan` JSON |

**Key operations**: `persist_mission()`, `update_mission_status()`, `update_mission_final_summary()`, `replace_mission_plan()`, `load_mission_snapshot()`, `list_sessions()`

#### `workers` — Session records (supervisor + all workers)

| Column | Type | Purpose |
|---|---|---|
| `id` | TEXT PK | Session UUID |
| `session_id` | TEXT FK→sessions.id | Parent mission |
| `role` | TEXT | `"supervisor"` or `"worker"` |
| `terminal_id` | TEXT | Human-readable ID (e.g. `"worker-01"`) |
| `name` | TEXT | Display name |
| `owned_scope` | TEXT | Assigned work scope |
| `status` | TEXT | `SessionState` string (16 states) |
| `last_heartbeat_at` | TEXT | Last activity timestamp |
| `last_summary` | TEXT | Latest status summary (nullable) |
| `agent` | TEXT | `AgentKind` string (`qwen`/`forge`/`codex`/`claude`) |
| `launch_command` | TEXT | JSON array of CLI args used to launch |
| `packet_json` | TEXT | Serialized `WorkerPacket` (nullable) |

**Key operations**: `persist_session()`, `update_session_state()`, `update_worker_heartbeat()`, `update_worker_summary()`, `load_workers()`

#### `tasks` — Task assignments per worker

| Column | Type | Purpose |
|---|---|---|
| `id` | TEXT PK | Task UUID |
| `session_id` | TEXT FK→sessions.id | Parent mission |
| `worker_id` | TEXT FK→workers.id | Assigned worker |
| `title` | TEXT | Task title |
| `description` | TEXT | Task description |
| `status` | TEXT | Task status string |
| `priority` | TEXT | Priority string |
| `depends_on` | TEXT | JSON array of dependency IDs |
| `definition_of_done` | TEXT | JSON array of completion criteria |

**Key operations**: `persist_task()`, `update_task_status()`, `find_task_id()`

#### `events` — All runtime events

| Column | Type | Purpose |
|---|---|---|
| `id` | TEXT PK | Event UUID |
| `session_id` | TEXT FK→sessions.id | Parent mission |
| `worker_id` | TEXT FK→workers.id (nullable) | Source worker (nullable for mission-level) |
| `event_type` | TEXT | Event kind string |
| `payload` | TEXT | JSON payload |
| `created_at` | TEXT | ISO-8601 timestamp |

**Key operations**: `persist_event()`, `append_json_event()`, `recent_replay_entries()`, `recent_worker_replay()`

#### `messages` — Durable inter-worker mail

| Column | Type | Purpose |
|---|---|---|
| `id` | TEXT PK | Message UUID |
| `session_id` | TEXT FK→sessions.id | Parent mission |
| `from_worker_id` | TEXT FK→workers.id | Sender |
| `to_worker_id` | TEXT FK→workers.id | Primary recipient |
| `message_type` | TEXT | `MailType` string |
| `body` | TEXT | Message body content |
| `status` | TEXT | Delivery status (`awaiting_ack`/`acked`/`responded`/`archived`) |
| `created_at` | TEXT | ISO-8601 timestamp |
| `acked_at` | TEXT | Ack timestamp (nullable) |
| `priority` | TEXT | `MailPriority` string (`urgent`/`high`/`normal`/`low`) |
| `subject` | TEXT | Subject line (≤120 chars) |

**Key operations**: `persist_message()`, `update_message_status()`, `search_messages()`, `list_thread_messages()`, `list_unread_messages()`, `archive_old_messages()`, `message_stats()`

#### `summaries` — Freeform summaries

| Column | Type | Purpose |
|---|---|---|
| `id` | TEXT PK | Summary UUID |
| `session_id` | TEXT FK→sessions.id | Parent mission |
| `worker_id` | TEXT FK→workers.id (nullable) | Source worker (nullable for mission-level) |
| `summary_type` | TEXT | Type tag (e.g. `"mission"`, `"worker"`, `"plan_source"`, `"resume"`, `"exit"`, `"surface"`, `"agents_bootstrap"`, `"preflight_failure"`) |
| `content` | TEXT | Summary text |
| `created_at` | TEXT | ISO-8601 timestamp |

**Key operations**: `persist_summary()`, `append_summary()`, `latest_supervisor_summary()`

#### `ownership_leases` — File ownership claims

| Column | Type | Purpose |
|---|---|---|
| `session_id` | TEXT (composite PK part) | Parent mission |
| `path` | TEXT (composite PK part) | File path being claimed |
| `owner_worker_id` | TEXT FK→workers.id | Current owner |
| `intent` | TEXT | Claim intent (`read`/`edit`/`review`) |
| `status` | TEXT | Lease status (`claim`/`release`) |
| `updated_at` | TEXT | Last update timestamp |

**Key**: `PRIMARY KEY (session_id, path)` — one owner per path per mission.

**Key operations**: `upsert_lease()` — uses `ON CONFLICT DO UPDATE` for idempotent replacement.

#### `normalized_updates` — Adapter-normalized state observations

| Column | Type | Purpose |
|---|---|---|
| `id` | TEXT PK | Update UUID |
| `session_id` | TEXT FK→sessions.id | Parent mission |
| `worker_id` | TEXT FK→workers.id | Source worker |
| `source` | TEXT | Origin tag (e.g. `"status_envelope"`) |
| `raw_excerpt` | TEXT | Original output snippet |
| `normalized_state` | TEXT | Inferred `SessionState` string |
| `confidence` | TEXT | Confidence level string |
| `summary` | TEXT | Human-readable summary |
| `adapter` | TEXT | Agent adapter name |
| `created_at` | TEXT | ISO-8601 timestamp |

**Key operations**: `persist_normalized_update()`, `recent_worker_replay()`

#### `validation_results` — Validation challenge outcomes

| Column | Type | Purpose |
|---|---|---|
| `id` | TEXT PK | Result UUID |
| `session_id` | TEXT FK→sessions.id | Parent mission |
| `worker_id` | TEXT FK→workers.id | Validated worker |
| `task_id` | TEXT FK→tasks.id (nullable) | Associated task |
| `outcome` | TEXT | Result outcome string |
| `summary` | TEXT | Validation summary text |
| `evidence` | TEXT | JSON evidence payload |
| `created_at` | TEXT | ISO-8601 timestamp |

**Key operations**: `persist_validation_result()`

### 1.3 Legacy Migration Strategy

`migrate_table_if_legacy()` checks if an existing table's columns match the expected set. If any expected column is missing, the entire table is renamed to `{table}_legacy_v0` and a fresh table is created. This is opportunistic — no data migration occurs. Old runs lose data on schema change.

### 1.4 Thread Safety

`Store` wraps `Connection` in `Mutex<Connection>`. All operations acquire the lock. This is simple and correct for the single-process, single-writer model. No connection pooling.

### 1.5 Query Patterns

- **Replay**: `recent_replay_entries()` merges events, summaries, and normalized updates, sorts by time desc, truncates to limit.
- **Worker replay**: `recent_worker_replay()` adds a `worker_id` filter and includes normalized state observations.
- **Message search**: `search_messages()` uses `LIKE` with wildcards on subject and body; supports optional sender filter.
- **Thread listing**: `list_thread_messages()` searches `body` for `"thread_id":"..."` JSON pattern — not indexed, full scan.
- **Mail stats**: `message_stats()` aggregates counts by status with a single query using `SUM(CASE WHEN ...)`.

---

## 2. Mail System (`src/mail/`)

### 2.1 Architecture

Sapphire mail is a durable, SQLite-backed messaging system for terminal-to-terminal agent communication. It is not ephemeral — every message is persisted before injection into the recipient's PTY.

Three modules:
- `types.rs` — Core types (envelope, addresses, delivery states, message types)
- `priority.rs` — Priority levels with routing rules
- `validation.rs` — Pre-routing validation rules

Re-exports: `SapphireMail`, `MailType`, `MailAddress`, `MailPriority`, `DeliveryState`

### 2.2 Mail Envelope (`SapphireMail`)

```
SapphireMail {
    id:                    Uuid              // Unique message ID
    thread_id:             String            // Conversation grouping (defaults to id)
    reply_to:              Option<String>    // Parent mail ID for reply chains
    sender_session_id:     Uuid              // Sender's session UUID
    recipient_session_id:  Uuid              // Primary recipient's session UUID
    cc:                    Vec<Uuid>         // Blind CC for visibility (not direct delivery)
    message_type:          MailType          // Categorization (request, response, etc.)
    priority:              MailPriority      // Urgency level
    delivery_state:        DeliveryState     // Two-phase tracking state
    subject:               String            // ≤120 chars
    context_json:          String            // Structured context (files, diffs, risks)
    body:                  String            // ≤8KB actual content
    expected_action:       String            // What recipient should do
    requires_ack:          bool              // Whether SAPPHIRE_ACK is required
    suppress_notify:       bool              // Suppress tmux/dashboard notifications
    created_at:            DateTime<Utc>     // Creation timestamp
}
```

#### Construction Patterns

- `SapphireMail::new(sender, recipient, type, priority, subject, body, expected_action)` — Fresh mail with auto-generated UUID and thread_id
- `mail.as_reply_to(original, sender, body, expected_action)` — Reply inheriting thread_id, priority, cc; auto-sets type to `Reply` and subject to `"Re: {original.subject}"`
- `mail.with_cc(session_id)` — Fluent builder for adding CC recipients
- `mail.with_context(json)` — Fluent builder for context JSON

#### Directive Serialization

`mail.to_directive_line()` produces:
```
SAPPHIRE_MAIL {"id":"...","thread_id":"...",...}
```

This is the exact line injected into the recipient's PTY. The orchestrator's protocol parser (`src/protocol.rs`) extracts it via regex: `SAPPHIRE_(STATUS|MAIL|ACK|LEASE) (\{.*\})`.

### 2.3 Message Types (`MailType`)

| Type | Direction | Requires Ack | Notifies Supervisor | Threaded |
|---|---|---|---|---|
| `Request` | Worker→Worker | Yes | No | Yes |
| `Response` | Worker→Worker | Yes | No | Yes |
| `Reply` | Any→Any | Inherited | No | Yes |
| `Escalation` | Worker→Supervisor | Yes | Yes | Yes |
| `Handoff` | Worker→Worker | Yes | No | Yes |
| `Blocker` | Worker→Worker/Sup | Yes | Yes | Yes |
| `ArchitectureConcern` | Worker→Worker | No | Yes | Yes |
| `CompletionNotice` | Worker→Workers | No | No | No |
| `SupervisorDirective` | Supervisor→Worker | Inherited | No | No |

String parsing aliases: `"dependency_request"` → `Request`, `"dependency_response"`/`"review_response"` → `Response`, etc.

### 2.4 Priority Levels (`MailPriority`)

| Level | Value | Ack Timeout | Escalates to Supervisor | Inject Immediately |
|---|---|---|---|---|
| `Low` | 0 | 45s | No | No (queued for idle) |
| `Normal` | 1 | 20s (default) | No | Yes |
| `High` | 2 | 15s | Yes | Yes |
| `Urgent` | 3 | 10s | Yes | Yes |

Parsing aliases: `"critical"` → `Urgent`. Unknown strings → `Normal`.

### 2.5 Delivery State Machine (`DeliveryState`)

```
Pending → Delivered → Acked → Done
                     ↓
                   Failed
```

| State | Meaning |
|---|---|
| `Pending` | Just routed, not yet delivered to recipient PTY |
| `Delivered` | Injected into recipient PTY, awaiting ack |
| `Acked` | Recipient acknowledged receipt (via `SAPPHIRE_ACK`) |
| `Done` | Thread completed (reply received or task resolved) |
| `Failed` | Could not deliver after retries, escalated to supervisor |

Terminal states: `Done`, `Failed`

### 2.6 Address Normalization (`MailAddress`)

Addresses are normalized to canonical form before routing:

| Input | Normalized |
|---|---|
| `"sup"`, `"SUPERVISOR"` | `"supervisor"` |
| `"w1"`, `"w01"` | `"worker-01"` |
| `"WORKER_01"`, `"worker-01"` | `"worker-01"` |

Rules: lowercase → expand `w` prefix with zero-padding → replace underscores with dashes.

### 2.7 Validation Rules (`MailValidationError`)

Before routing, every mail is validated:

| Check | Error | Threshold |
|---|---|---|
| Subject empty | `EmptySubject` | — |
| Subject too long | `SubjectTooLong` | >120 chars |
| Body empty | `EmptyBody` | — |
| Body too long | `BodyTooLong` | >8KB (prevents PTY buffer overflow) |
| Self-mail without suppression | `SelfMailWithoutSuppression` | sender == recipient && !suppress_notify |
| CC contains sender | `CCContainsSender` | — |
| CC contains recipient | `CCContainsRecipient` | — |

Warnings (non-fatal): subject >80 chars, urgent without supervisor notification, CC >5 recipients.

### 2.8 Mail Lifecycle

The full flow from creation to completion:

```
1. CREATION
   Worker emits SAPPHIRE_MAIL directive in PTY output
   ↓
2. ORCHESTRATOR PARSES
   protocol.rs extracts JSON via regex
   orchestrator receives MailRecord directive
   ↓
3. VALIDATION
   SapphireMail.validate() checks constraints
   On failure: error logged, mail not routed
   ↓
4. PERSISTENCE
   store.persist_message() → SQLite messages table
   Status: "awaiting_ack", ack_state: "pending"
   ↓
5. PTY INJECTION
   Orchestrator injects "SAPPHIRE_MAIL {json}" into recipient's PTY
   Priority.inject_immediately() determines timing:
     - Urgent/High/Normal: injected on next tick
     - Low: queued for recipient idle
   ↓
6. ACK TRACKING
   If mail.requires_ack:
     - Orchestrator tracks with 20s timeout (priority-dependent)
     - On ack received: status → "acked", ack_state → "acked"
     - On timeout: probe sender + recipient, escalate to supervisor
   ↓
7. REPLY / RESPONSE
   Recipient emits SAPPHIRE_MAIL as reply (inherits thread_id)
   Original mail: delivery_state → Done (thread complete)
   ↓
8. ARCHIVAL
   archive_old_messages() sets status = "archived" for resolved messages
   Excludes: awaiting_ack, pending, already archived
```

### 2.9 Shared Data Models with Store

**`MailRecord`** (`src/model.rs`) is the persistence representation:

```rust
MailRecord {
    id: Uuid,
    mission_id: Uuid,
    sender_worker_id: Uuid,        // Maps to SapphireMail.sender_session_id
    recipient_worker_id: Uuid,     // Maps to SapphireMail.recipient_session_id
    message_type: String,          // MailType.as_str()
    priority: String,              // MailPriority.as_str()
    subject: String,
    status: String,                // SQLite-level: "awaiting_ack"/"acked"/"responded"/"archived"
    ack_state: String,             // "pending"/"acked"
    body_json: String,             // Full SapphireMail JSON serialized
    created_at: DateTime<Utc>,
}
```

**Flag**: `MailRecord` stores the full `SapphireMail` envelope as JSON in `body_json`, while also extracting `message_type`, `priority`, `subject`, and `ack_state` as first-class SQLite columns for querying. This dual representation means `body_json` is the source of truth for thread_id, cc, context_json, etc., while the extracted columns power indexed lookups.

**Flag**: `row_to_mail()` maps DB rows to `MailRecord`, inferring `ack_state` from `acked_at` timestamp presence — if `acked_at` is non-null, ack_state becomes `"acked"`, otherwise `"pending"`.

### 2.10 Orchestrator Mail Routing (reference)

The orchestrator handles mail in three directive handlers:

- `handle_mail_directive()` — Routes between workers, persists to SQLite, injects into recipient PTY, tracks ack timeouts
- `handle_ack_directive()` — Processes `SAPPHIRE_ACK` from recipients, notifies original sender and supervisor
- `handle_pending_mail()` — Probes unacked mail after timeout (20s default), notifies both sender and recipient, escalates to supervisor

All mail between non-supervisor workers generates a supervisor notice.

---

## 3. tmux Control Surface (`src/tmux/`)

### 3.1 Architecture

The tmux module is a thin CLI wrapper around `tmux` commands. It provides no async, no event loop — just synchronous command construction and execution. The orchestrator uses it to build the default teamwork surface for live missions.

Two sub-modules:
- `grid.rs` — Pure functions for pane grid layout calculation
- `dashboard.rs` — ANSI-colored dashboard text generation

### 3.2 `Tmux` Struct

```rust
Tmux {
    socket: Option<String>,  // Custom tmux socket name (-L flag)
}
```

Core capabilities:

| Method | Purpose |
|---|---|
| `new_session(name, work_dir)` | Create detached session with remain-on-exit |
| `new_session_with_command(name, work_dir, command, env)` | Session with custom command + env vars |
| `create_session_for_workers(name, work_dir, cols, rows)` | Pre-sized session for later pane splitting |
| `split_window(target, horizontal)` | Split pane, returns new pane ID |
| `split_window_with_command(target, horizontal, work_dir, command)` | Split + launch command |
| `send_keys(pane, text)` | Send keystrokes + Enter (with 500ms delay) |
| `send_command(pane, text)` | Send literal keys + Enter (safe mode) |
| `capture_pane(pane, lines)` | Capture last N visible lines |
| `pipe_pane(pane, command)` | Pipe pane output to external command |
| `kill_session(name)` | Kill session (tolerates missing sessions) |
| `has_session(name)` | Check session existence |
| `open_external_terminal_for_session(session)` | Open terminal window attached to session |
| `is_available()` | Check if tmux binary exists |

### 3.3 Session Lifecycle for Live Missions

The orchestrator's `ensure_tmux_surface()` follows this sequence:

```
1. CREATE SESSION
   tmux.create_session_for_workers(name, work_dir, cols, rows)
   Session created with dimensions, remain-on-exit on

2. BUILD GRID
   Calculate layout via grid::calculate_layout(worker_count)
   Split panes: one per worker transcript tail + one for control panel

3. POPULATE PANES
   For each worker: tail -f .sp/transcripts/{worker_id}.txt
   Control panel: tail -f .sp/control/status.txt

4. OPEN IN TERMINAL
   macOS: prefers Ghostty (tab first, window fallback), then Apple Terminal
   Uses open_external_terminal_for_session()

5. REFRESH
   Dashboard content regenerated periodically from store + status file
```

### 3.4 Grid Layout (`grid.rs`)

Pure functions — no I/O. Calculates pane grid dimensions:

| Workers | Rows | Cols | Total Cells |
|---|---|---|---|
| 0-1 | 1 | 1 | 1 |
| 2 | 1 | 2 | 2 |
| 3-4 | 2 | 2 | 4 |
| 5-8 | 2 | 4 | 8 |
| 9-12 | 3 | 4 | 12 |
| 13+ | 4 | 4 | 16 |

Workers fill row-major (left-to-right, top-to-bottom). `GridLayout.total` may exceed `worker_count` — excess cells are unused.

### 3.5 Dashboard Content (`dashboard.rs`)

Generates plain-text ANSI-colored dashboard for the tmux control panel.

**Layout**:
```
 ────────────────────────────────────────
 SAPPHIRE
 {mission rewrite, truncated to 38 chars}
 ────────────────────────────────────────
 Agents: {agent} × {count}  |  {elapsed}
 Supervisor: {name}

 Workers:
  {dot} {state}  {name}
  ...

 ────────────────────────────────────────
 Recent events:
  {dot} [{time ago}] {message}
  ...

 ────────────────────────────────────────
 q=quit  ↑↓=scroll
```

**State dots** (ANSI color-coded):
| State | Symbol | Color |
|---|---|---|
| `Validated` | ✓ | Green (32) |
| `Failed` | ✗ | Red (31) |
| `Exited` | ⏹ | White (37) |
| `Progressing` | ● | Green (32) |
| `Booting` | ◐ | Yellow (33) |
| `Stalled` | ● | Yellow (33) |
| `DoneClaimed` / `NeedsValidation` | ◉ | Cyan (36) |
| `Blocked` | ■ | Red (31) |
| `Contradictory` | ◆ | Red (31) |
| `NeedsRetry` | ↻ | Yellow (33) |
| `WrongDirection` | ↯ | Red (31) |
| `WeakOutput` | ◌ | Yellow (33) |
| `NotStarted` / `Planned` | ○ | White (37) |

**Event dots** (derived from body content):
| Pattern | Symbol | Color |
|---|---|---|
| stall | ⚠ | Yellow |
| fail/conflict | ✗ | Red |
| mail/route | → | Blue (34) |
| supervisor/validat | ★ | Magenta (35) |
| claim/lease | ⊞ | Cyan (36) |
| progress/state | ● | Green |
| default | · | White |

### 3.6 External Terminal Opening (macOS)

`open_external_terminal_for_session()` uses a terminal priority chain:

```
1. VS Code (TERM_PROGRAM=vscode)
   → Opens Ghostty window directly via `open -na Ghostty.app --args -e ...`

2. Ghostty (TERM_PROGRAM=ghostty)
   → Try Ghostty tab first (AppleScript: Cmd+T, type attach command, Enter)
   → Fallback: new Ghostty window (`open -na Ghostty.app --args -e ...`)

3. Apple Terminal (fallback)
   → AppleScript: `tell application "Terminal" to activate` + `do script "tmux attach..."`
```

Ghostty app path discovery checks `/Applications/Ghostty.app` then `/System/Applications/Ghostty.app`.

### 3.7 Ghostty Integration Details

The `open_ghostty_tab_for_session()` method uses macOS AppleScript:
```applescript
tell application "Ghostty" to activate
tell application "System Events"
  keystroke "t" using command down    -- New tab
  delay 0.2
  keystroke "tmux attach-session -t {name}"
  key code 36                          -- Enter
end tell
```

If this fails (macOS may block keystroke automation), `open_ghostty_window_for_session()` falls back to:
```bash
open -na /Applications/Ghostty.app --args -e /bin/zsh -lc 'tmux attach-session -t {name}'
```

### 3.8 Shared Data Models

**`DashboardEvent`** — Rendered dashboard feed item:
```rust
DashboardEvent {
    age_label: String,    // "[5s ago]", "[2m ago]", "[1h ago]"
    dot: String,          // ANSI-colored symbol
    message: String,      // Truncated to 50 chars
}
```

**`DashboardContent`** — Full dashboard text:
```rust
DashboardContent {
    lines: Vec<String>,
    version: u64,         // Unix timestamp for cache invalidation
}
```

**`PaneState`** — tmux pane health check:
```rust
PaneState {
    dead: bool,
    exit_code: Option<i32>,
}
```

### 3.9 tmux Session Options

Default session settings applied by the orchestrator:
- `remain-on-exit on` — Panes stay visible after process exits
- `window-size latest` — Overrides tmux 3.3+ manual sizing for client auto-resize

---

## 4. Cross-Subsystem Data Flows

### 4.1 Lease → Conflict Resolution Flow

```
Worker emits: SAPPHIRE_LEASE {"path":"src/main.rs","intent":"edit","status":"claim"}
  ↓
Orchestrator: handle_lease_directive()
  ↓
Store: upsert_lease() → SQLite ownership_leases (ON CONFLICT DO UPDATE)
  ↓
If conflict detected (existing owner != current claimant):
  - Challenger → SessionState::Contradictory
  - Supervisor notified with scope details
  - Owner notified of claim attempt
```

**Shared model**: `LeaseRecord` (`src/model.rs`) maps directly to the `ownership_leases` table. The composite PK `(session_id, path)` ensures one owner per file per mission.

### 4.2 Mail → Ack → Supervisor Escalation Flow

```
Worker emits: SAPPHIRE_MAIL {json}
  ↓
Orchestrator: handle_mail_directive()
  - Validates via SapphireMail.validate()
  - Persists via store.persist_message()
  - Injects "SAPPHIRE_MAIL {json}" into recipient PTY
  - If requires_ack: tracks in pending_ack map
  ↓
Recipient processes, emits: SAPPHIRE_ACK {json}
  ↓
Orchestrator: handle_ack_directive()
  - Updates message status via store.update_message_status()
  - Notifies original sender via PTY injection
  - Notifies supervisor
  ↓
If no ack within timeout (priority-dependent):
  Orchestrator: handle_pending_mail()
  - Probes sender: "mail not acked by {recipient}"
  - Probes recipient: "unacknowledged mail from {sender}"
  - Escalates to supervisor
```

### 4.3 tmux Dashboard Data Source

```
Store.load_mission_snapshot(mission_id)  → mission name, plan
Store.load_workers(mission_id)           → worker states, names
Store.recent_replay_entries(mission_id)  → recent events
  ↓
build_dashboard() composes ANSI text
  ↓
Written to .sp/control/status.txt
  ↓
tmux control panel pane: tail -f .sp/control/status.txt
```

---

## 5. Key Invariants

1. **All messages are persisted before PTY injection** — mail is durable, not ephemeral
2. **Lease upsert is idempotent** — same session+path updates owner, doesn't duplicate
3. **Thread grouping uses mail ID as thread_id for new threads** — replies inherit the original's thread_id
4. **CC recipients are not directly delivered to** — they are for visibility only
5. **Dashboard content is regenerated, not streamed** — full rebuild each cycle
6. **tmux grid cells may exceed worker count** — excess cells are unused padding
7. **`foreign_keys=OFF` in SQLite** — referential integrity is enforced at the orchestrator level, not by SQLite
8. **WAL mode + SYNCHRONOUS=NORMAL** — crash-safe but not fsync-every-write
