# Core Orchestration Layer — Architecture Documentation

## Overview

The orchestration layer is the control plane of `sp`, a Rust CLI that coordinates multiple terminal-native coding agents (Codex, Claude, Qwen, Forge) in a supervisor-plus-workers topology. It manages the full mission lifecycle: planning, launching, monitoring, validating, and summarizing distributed agent sessions.

**Three core modules form the orchestration spine:**

| Module | File | Responsibility |
|---|---|---|
| Orchestrator | `src/orchestrator/mod.rs` (~5000 lines) | Mission lifecycle, watchdog loop, directive routing, supervisor actions |
| Runtime | `src/runtime/mod.rs` (~700 lines) | PTY/process spawning, prompt injection, output capture, startup automation |
| Protocol | `src/protocol.rs` (~300 lines) | Parses machine-readable JSON directives from noisy terminal output |

---

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────────────┐
│                            sp (Orchestrator)                             │
│                                                                          │
│  ┌──────────────┐   ┌──────────────┐   ┌──────────────┐                │
│  │   CLI /      │   │  Templates   │   │    Store     │                │
│  │   Config     │   │  (Prompts)   │   │  (SQLite)    │                │
│  └──────┬───────┘   └──────┬───────┘   └──────┬───────┘                │
│         │                  │                   │                         │
│         ▼                  ▼                   ▼                         │
│  ┌─────────────────────────────────────────────────────┐               │
│  │              Orchestrator (mod.rs)                   │               │
│  │                                                      │               │
│  │  launch() ──► plan_with_supervisor()                 │               │
│  │            ──► run_live_mission()                    │               │
│  │                  │                                    │               │
│  │                  ▼                                    │               │
│  │         ┌─────────────────┐                          │               │
│  │         │  Watchdog Loop  │◄── tick (configurable)   │               │
│  │         │                 │                          │               │
│  │         │  handle_runtime │──► handle_status         │               │
│  │         │    _event()     │──► handle_mail           │               │
│  │         │                 │──► handle_ack            │               │
│  │         │  handle_stalls()│──► handle_lease          │               │
│  │         │  handle_supervis│──► handle_normalized     │               │
│  │         │   or_health()   │    _observation()        │               │
│  │         │  handle_pending_│──► apply_supervisor      │               │
│  │         │   decisions()   │    _action()             │               │
│  │         └────────┬────────┘                          │               │
│  └──────────────────┼──────────────────────────────────┘               │
│                     │                                                    │
│                     ▼                                                    │
│  ┌─────────────────────────────────────────────────────┐               │
│  │              SessionRuntime (runtime/mod.rs)         │               │
│  │                                                      │               │
│  │  spawn() ──► PTY (30x140) or tmux pane              │               │
│  │                                                      │               │
│  │  ┌──────────────┐    ┌──────────────┐               │               │
│  │  │ PTY backend  │    │ tmux backend │               │               │
│  │  │ (portable_   │    │ (tmux CLI +  │               │               │
│  │  │  pty)        │    │  transcript  │               │               │
│  │  │              │    │  piping)     │               │               │
│  │  └──────┬───────┘    └──────┬───────┘               │               │
│  │         │                   │                         │               │
│  │         ▼                   ▼                         │               │
│  │  ┌──────────────────────────────────────┐            │               │
│  │  │  SessionHandle trait                 │            │               │
│  │  │  ├── PtySessionHandle                │            │               │
│  │  │  ├── TmuxSessionHandle               │            │               │
│  │  │  └── TestSessionHandle (tests)       │            │               │
│  │  └──────────────────┬───────────────────┘            │               │
│  │                     │                                 │               │
│  │                     ▼                                 │               │
│  │  ┌──────────────────────────────────────┐            │               │
│  │  │  Background reader thread            │            │               │
│  │  │  ├── Read PTY output (4KB chunks)    │            │               │
│  │  │  ├── Apply startup automation rules  │            │               │
│  │  │  ├── Buffer management (24KB cap)    │            │               │
│  │  │  └── Send RuntimeEvent via mpsc      │            │               │
│  │  └──────────────────────────────────────┘            │               │
│  └─────────────────────────────────────────────────────┘               │
│                     │                                                    │
│                     ▼                                                    │
│  ┌─────────────────────────────────────────────────────┐               │
│  │              Protocol Parser (protocol.rs)           │               │
│  │                                                      │               │
│  │  consume_directives(buffer, chunk)                  │               │
│  │  ├── Sanitize ANSI escapes                          │               │
│  │  ├── Buffer partial lines                           │               │
│  │  ├── Parse: SAPPHIRE_STATUS { ... }                 │               │
│  │  ├── Parse: SAPPHIRE_MAIL { ... }                   │               │
│  │  ├── Parse: SAPPHIRE_ACK { ... }                    │               │
│  │  └── Parse: SAPPHIRE_LEASE { ... }                  │               │
│  └─────────────────────────────────────────────────────┘               │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
                     │                    │                    │
                     ▼                    ▼                    ▼
          ┌──────────────┐   ┌──────────────┐   ┌──────────────┐
          │ Supervisor   │   │  Worker N    │   │ AGENTS.md    │
          │   PTY        │   │   PTY        │   │  Steward PTY │
          │ (qwen/codex/ │   │ (qwen/codex/ │   │ (maintains   │
          │  claude)     │   │  claude)     │   │  AGENTS.md)  │
          └──────────────┘   └──────────────┘   └──────────────┘
```

---

## Sequence Flow: Mission Lifecycle

```
User: sp codex 4 --repo . --mission "refactor auth and add tests"
  │
  ▼
┌─ Orchestrator::bootstrap(config) ─────────────────────────┐
│ 1. Create .sp/ directories (prompts, control, transcripts)│
│ 2. Open/create SQLite store (.sp/sapphire.sqlite3)        │
│ 3. Return Orchestrator { store, prompts }                 │
└───────────────────────────────────────────────────────────┘
  │
  ▼
┌─ Orchestrator::launch(config) ────────────────────────────┐
│ 1. Ensure AGENTS.md exists (seed from embedded source)    │
│ 2. Persist MissionRecord + placeholder plan to SQLite     │
│ 3. Create supervisor SessionRecord + TaskRecord           │
│ 4. plan_with_supervisor() — live supervisor plans (45s)   │
│    └── If supervisor fails to produce valid JSON plan →   │
│        mission fails (no silent fallback)                 │
│ 5. Replace plan with supervisor's plan                    │
│ 6. Create worker SessionRecords + TaskRecords             │
│ 7. run_live_mission() — the watchdog loop                 │
└───────────────────────────────────────────────────────────┘
  │
  ▼
┌─ run_live_mission() ──────────────────────────────────────┐
│ 1. Optional: provision tmux teamwork surface              │
│ 2. Spawn supervisor PTY (or tmux pane)                    │
│ 3. Spawn worker PTYs (staggered by per-agent delay)       │
│ 4. Inject prompts into each session                       │
│ 5. ┌── WATCHDOG LOOP (tick: 1s default) ──────────────┐  │
│    │ a. runtime.next_event(tick)                       │  │
│    │    └── handle_runtime_event()                     │  │
│    │ b. handle_supervisor_health()                     │  │
│    │ c. handle_stalls()                                │  │
│    │ d. handle_protocol_reminders()                    │  │
│    │ e. handle_pending_mail()                          │  │
│    │ f. handle_pending_supervisor_decisions()          │  │
│    │ g. handle_pending_restarts()                      │  │
│    │ h. write_status_snapshot()                        │  │
│    │ i. Supervisor state card refresh (every 30s)      │  │
│    │ j. Check terminal state → request final synthesis │  │
│    │ k. Exit when all workers terminal + synthesis done│  │
│    └───────────────────────────────────────────────────┘  │
│ 6. Terminate all sessions                                 │
│ 7. Update mission status (Completed/Failed)               │
└───────────────────────────────────────────────────────────┘
```

---

## 16-State Session State Machine

```
                    ┌──────────┐
                    │ Planned  │  (initial, dry-run only)
                    └────┬─────┘
                         │ launch
                         ▼
                    ┌──────────┐
                    │ Booting  │  (process spawning, startup automation)
                    └────┬─────┘
                         │ process ready
                         ▼
                    ┌────────────┐
              ┌─────┤ NotStarted ├─────┐
              │     └─────┬──────┘     │
              │           │ prompt     │
              │           ▼            │
              │     ┌─────────────┐    │
              │     │ Progressing │◄───┘ (default working state)
              │     └──────┬──────┘
              │            │
        ┌─────┼──────┬─────┼─────────┬──────────┐
        ▼     ▼      ▼     ▼         ▼          ▼
   ┌────────┐ ┌──────────┐ ┌────────────┐ ┌──────────────┐
   │ Blocked│ │ Stalled  │ │DoneClaimed │ │ NeedsValidation│
   └───┬────┘ └────┬─────┘ └──────┬─────┘ └──────┬───────┘
       │           │              │               │
       │    ┌──────┘         ┌────┘         ┌─────┘
       │    ▼                ▼              ▼
       │ ┌───────────┐  ┌──────────────┐ ┌───────────┐
       │ │ NeedsRetry│  │WeakOutput    │ │ Validated │◄── terminal
       │ └─────┬─────┘  └──────┬───────┘ └───────────┘
       │       │               │
       │       ▼               ▼
       │ ┌──────────────┐ ┌──────────────┐
       ├─┤Contradictory │ │WrongDirection│
       │ └──────┬───────┘ └──────┬───────┘
       │        │                │
       └────────┴────────────────┘
                │
                ▼
           ┌────────┐
           │ Failed │◄── terminal
           └────────┘

           ┌────────┐
           │ Exited │◄── terminal (process exited, no restart)
           └────────┘

TERMINAL STATES: Validated, Failed, Exited
  (Once a session reaches a terminal state, it does not transition further)
```

### State Transitions — Triggered By

| From State | To State | Trigger |
|---|---|---|
| `Planned` | `Booting` | `launch()` creates session |
| `Booting` | `Progressing` | PTY ready, prompt queued |
| `Progressing` | `Blocked` | `SAPPHIRE_STATUS {"state":"blocked"}` or heuristic |
| `Progressing` | `Stalled` | Watchdog: no output beyond `stall_seconds` |
| `Progressing` | `DoneClaimed` | `SAPPHIRE_STATUS {"state":"done_claimed"}` or heuristic |
| `Progressing` | `WeakOutput` | Heuristic: "probably fixed", "didn't run tests" |
| `Progressing` | `WrongDirection` | Heuristic: "full rewrite", "took over" |
| `Progressing` | `Contradictory` | Lease conflict or heuristic |
| `DoneClaimed` | `NeedsValidation` | Validation challenge issued |
| `NeedsValidation` | `Validated` | Supervisor `accept_worker` or validation passed |
| `NeedsValidation` | `NeedsRetry` | Validation failed, correction sent |
| `NeedsValidation` | `Failed` | Supervisor `fail_worker` |
| `Stalled` | `NeedsRetry` | Supervisor decision: `retry_worker` |
| `Contradictory` | `Blocked` | Challenger blocked, supervisor notified |
| `NeedsRetry` | `Progressing` | Worker resumes work after correction |
| Any non-terminal | `Exited` | Process exit event received |
| `Exited` | `Booting` | Auto-restart (for non-terminal states, limited retries) |

---

## Event Loop: The Watchdog Tick

The watchdog is a **pull-based** event loop. It does not wait for events; it polls with a configurable tick interval (default 1s).

```rust
loop {
    // 1. Max runtime check
    if max_runtime && started_at.elapsed() >= limit { break; }

    // 2. Collect runtime events (non-blocking poll with timeout = tick)
    if let Some(event) = runtime.next_event(tick).await {
        handle_runtime_event(...);  // Output / Automation / Exited
    }

    // 3. Supervisor health check (stall detection)
    handle_supervisor_health(...);

    // 4. Worker stall detection (silence beyond stall_seconds)
    handle_stalls(...);

    // 5. Protocol reminders (output without directives → nudge)
    handle_protocol_reminders(...);

    // 6. Pending mail timeout probing (20s ack timeout)
    handle_pending_mail(...);

    // 7. Supervisor decision queue processing
    handle_pending_supervisor_decisions(...);

    // 8. Session restart handling (backoff retry)
    handle_pending_restarts(...);

    // 9. Write .sp/control/status.txt snapshot
    write_status_snapshot(...);

    // 10. Supervisor state card refresh (every 30s)
    if last_state_card_sent.elapsed() >= 30s { ... }

    // 11. Final synthesis trigger
    if workers_terminal && !final_synthesis_requested {
        if supervisor_degraded {
            write_fallback_final_summary();  // watchdog generates
        } else {
            supervisor.runtime.send_prompt(final_summary_prompt);
        }
    }

    // 12. Exit condition
    if workers_terminal && (synthesis_done || supervisor_exited || degraded) {
        break;
    }
}
```

---

## Runtime Event Flow

```
PTY Output Thread                          Orchestrator (async)
─────────────────────                      ────────────────────
                                           
[read 4KB chunk]                           
       │                                   
       ▼                                   
BufferManager.append(chunk)                ← 24KB cap, 12KB keep
       │                                   
       ├──► RuntimeEvent::Output { chunk } ──────┐
       │                                          │
       ▼                                          ▼
Startup automation rules                 consume_directives(buffer, chunk)
  match_text found?                            │
       │                                       ├──► SAPPHIRE_STATUS
       ├──► RuntimeEvent::Automation           ├──► SAPPHIRE_MAIL
       │                                       ├──► SAPPHIRE_ACK
       │                                       └──► SAPPHIRE_LEASE
       │                                          │
       │                                          ▼
[process exits]                          handle_status_directive()
       │                                 ├── Update session state
       ▼                                 ├── Trigger validation (if done_claimed)
RuntimeEvent::Exited                     ├── Notify supervisor
                                         ├── Persist validation result
                                         └── Queue supervisor decision

                                         handle_mail_directive()
                                         ├── Persist to SQLite
                                         ├── Inject into recipient PTY
                                         └── Track ack timeout (20s)

                                         handle_lease_directive()
                                         ├── Upsert lease in SQLite
                                         ├── Detect conflicts
                                         ├── Downgrade challenger → Contradictory
                                         └── Notify supervisor

                                         handle_normalized_observation()
                                         (no explicit directive found)
                                         ├── Adapter heuristic state inference
                                         ├── Track low-confidence count
                                         └── Escalate after 2+ low-confidence
```

---

## Protocol Directive Parsing

The protocol parser extracts structured JSON directives from noisy terminal output using a two-phase approach:

### Phase 1: Sanitization + Buffering
```
raw chunk → sanitize_output() → strip ANSI escapes, normalize \r → \n
                              → append to session line_buffer
```

### Phase 2: Directive Extraction
```
regex: SAPPHIRE_(STATUS|MAIL|ACK|LEASE)\s+
       │
       ├── match found → extract JSON object via brace-counting parser
       │                    (handles nested braces, escaped quotes, strings)
       │                    │
       │                    ├── valid JSON → SapphireDirective enum variant
       │                    └── invalid JSON → skip, continue scanning
       │
       └── no match → buffer retained for next chunk

After parsing:
  - Consumed prefix drained from buffer
  - If buffer > 128KB → trim to last 64KB (prevent unbounded growth)
```

### Four Directive Types

| Directive | Purpose | Key Fields |
|---|---|---|
| `SAPPHIRE_STATUS` | Worker state report | `state`, `summary`, `files`, `commands`, `risks`, `overlap` |
| `SAPPHIRE_MAIL` | Inter-worker message | `to`, `cc`, `message_type`, `priority`, `subject`, `request`, `requires_ack` |
| `SAPPHIRE_ACK` | Mail acknowledgment | `mail_id`, `status` (acked/done/cannot_comply), `summary` |
| `SAPPHIRE_LEASE` | File ownership claim | `paths`, `intent` (read/edit/review), `status` (claim/release) |

---

## Heuristic State Detection (Adapter Layer)

When a worker produces output **without** an explicit `SAPPHIRE_STATUS` directive, the adapter layer infers state via keyword matching:

### Keyword → State Mapping

| State | Keywords (case-insensitive substring match) |
|---|---|
| `Validated` | "validation passed", "validated", "all checks passed" |
| `WeakOutput` | "probably fixed", "should work now", "didn't run tests", "cannot verify", "can't verify" |
| `WrongDirection` | "rewrote the architecture", "full rewrite", "changed unrelated", "refactored broadly", "took over" |
| `Blocked` | "blocked", "cannot proceed", "can't proceed", "waiting on", "need clarification" |
| `Contradictory` | "conflict", "overlap", "contradiction", "someone else changed" |
| `Progressing` | "investigating", "reproducing", "working on", "running tests", "profiling", "reviewing" |
| `DoneClaimed` | "i'm done", "completed the task", "task is complete", "finished the task" |

### Per-Agent Variants

- **Qwen**: Additional keyword lists for tool-call signals (`readfile`, `writefile`, `editfile`, `success:`) and prompt echo detection
- **Codex/Claude/Forge**: Standard keyword lists via `detect_state_impl()`

### Confidence Model

| Confidence | Condition |
|---|---|
| `High` | Explicit `SAPPHIRE_STATUS` directive parsed |
| `Medium` | Status envelope regex matched (`STATE: ... / SUMMARY: ...`) |
| `Low` | Heuristic keyword match only |

After **2+ consecutive low-confidence observations** for the same state, the orchestrator:
1. Sends a corrective status prompt to the worker
2. Notifies the supervisor
3. Queues a supervisor decision for intervention

---

## Supervisor Action Execution

The supervisor can emit structured actions parsed from its output:

```
ACTION: observe|validate_worker|retry_worker|redirect_worker|message_worker|accept_worker|fail_worker
TARGET: worker-01|worker-02|...|NONE
SUMMARY: one short sentence
MESSAGE: one short instruction or NONE
```

### Action Behaviors

| Action | Effect |
|---|---|
| `observe` | Passive monitoring (no intervention) |
| `validate_worker` | Sends validation challenge prompt to target |
| `retry_worker` | Sends correction prompt with supervisor message |
| `redirect_worker` | Sends correction prompt with new direction |
| `message_worker` | Sends arbitrary supervisor message to target |
| `accept_worker` | Forces session state → `Validated` |
| `fail_worker` | Forces session state → `Failed` |

### Deduplication

Actions are deduplicated using a composite key: `{action}_{target_session_id}`. Identical actions from the same signature are silently dropped to prevent supervisor spam loops.

---

## File Ownership & Lease Conflict Resolution

```
Worker A: SAPPHIRE_LEASE {"paths":["src/auth.rs"], "intent":"edit", "status":"claim"}
  │
  ├── No existing lease → upsert, notify owner
  │
Worker B: SAPPHIRE_LEASE {"paths":["src/auth.rs"], "intent":"edit", "status":"claim"}
  │
  ├── Lease exists → CONFLICT
  ├── Challenger (B) → state: Contradictory
  ├── Challenger (B) → blocked
  ├── Supervisor notified with scope details
  ├── Owner (A) notified: "another worker claimed your path"
  └── Supervisor decides: accept A, redirect B, or fail one
```

**Lease lifecycle:** claim → (active) → release. Leases are upserted by `(session_id, path)` — the same session re-claiming its own path is a no-op.

---

## Inter-Worker Mail System

```
Worker A → SAPPHIRE_MAIL {"to":"worker-02", "requires_ack":true, ...}
  │
  ├── Persist to SQLite (messages table)
  ├── Inject formatted mail into Worker B's PTY
  ├── If requires_ack → track in pending_mail with timeout
  ├── Notify supervisor of mail exchange
  │
  ▼
Worker B reads mail → works on request → SAPPHIRE_MAIL reply
  │
  └── Reply threaded via thread_id, reply_to

20s later: ack timeout check
  ├── If not acked → probe sender ("mail not acknowledged")
  ├── Probe recipient ("you have unread mail")
  └── Escalate to supervisor
```

### Mail Types
`dependency_request`, `dependency_response`, `review_request`, `review_response`, `blocker`, `handoff`, `collision_warning`, `architecture_concern`, `completion_notice`, `supervisor_directive`

### Delivery States
`pending` → `delivered` → `acked` → `done`

---

## Supervisor Health & Degraded Mode

```
Supervisor Healthy ──► no output for stall_seconds ──► Degraded
       │                                                      │
       │  ┌───────────────────────────────────────────┐       │
       │  │ In Degraded mode:                          │       │
       │  │ - Skip supervisor-dependent actions        │       │
       │  │ - Watchdog generates final synthesis       │       │
       │  │ - Validation challenges handled by watchdog │       │
       │  │ - Supervisor decisions skipped or faked    │       │
       │  └───────────────────────────────────────────┘       │
       │                                                      │
       └──────────── if supervisor recovers ──► Healthy? NO  │
                                                  (one-way)  │
                                                  Supervisor │
                                                  stays      │
                                                  Degraded   │
```

**Key invariant:** Once the supervisor transitions to `Degraded`, it does not recover. The watchdog takes over remaining supervision duties.

---

## Key Data Structures

### ActiveSession (per session in watchdog loop)
```rust
struct ActiveSession {
    record: SessionRecord,          // Persistent session metadata
    packet: Option<WorkerPacket>,   // Assigned work scope
    runtime: RunningSession,        // PTY/tmux handle
    launch_spec: ProcessLaunchSpec, // How the session was spawned
    launch_prompt: String,          // Initial prompt injected
    state: SessionState,            // Current 16-state value
    task_id: Option<Uuid>,          // Linked TaskRecord in SQLite
    line_buffer: String,            // Protocol directive buffer
    raw_buffer: String,             // Full output for heuristic parsing
    started_at: Instant,
    startup_grace_until: Instant,   // Automation rules window
    last_output_at: Instant,        // Stall detection anchor
    output_chunks: usize,           // Protocol reminder counter
    directive_count: usize,         // How many directives parsed
    stall_count: usize,             // Consecutive stall interventions
    restart_count: usize,           // Auto-restart attempts
    restart_at: Option<Instant>,    // When to retry (backoff)
    validation_pending: bool,       // Don't double-challenge
    low_confidence_count: usize,    // Escalation counter
    last_observation_key: Option<String>,  // Deduplication
    last_supervisor_action_key: Option<String>, // Deduplication
    escalation_sent_for_state: Option<SessionState>, // Don't re-notify
    protocol_reminder_sent: bool,   // Don't double-remind
}
```

### Watchdog Sub-structures
```rust
struct PendingMail { ... }           // Mail ack tracking with timeout
struct PendingSupervisorDecision { ... } // Decision queue with dedup
struct RecentFailure { ... }         // Mass failure detection window
struct LeaseOwner { ... }            // File ownership map
struct ControlSurface { ... }        // tmux + status file + transcripts
```

---

## Persistence Integration

All orchestration state flows through SQLite (`Store`):

```
Runtime Event ──► persist_runtime_event() ──► events table
                                                      │
Session State Change ──► update_session_state() ──► workers table
                                                           │
Status Directive ──► persist_normalized_status() ──► normalized_updates
                                                          │
Validation Result ──► persist_validation_result() ──► validation_results
                                                           │
Mail Directive ──► insert message ──► messages table
                                                │
Lease Directive ──► upsert lease ──► ownership_leases table
                                              │
Status Snapshot ──► write_status_snapshot() ──► .sp/control/status.txt
```

---

## Design Principles

1. **Directives are single-line JSON.** Multi-line directives are not parsed. Workers must emit `SAPPHIRE_X {json}` on one line.

2. **Done claim ≠ acceptance.** `done_claimed` triggers a validation challenge. Only supervisor `accept_worker` or explicit `validated` state marks work complete.

3. **Plan or fail.** If the supervisor cannot produce a valid JSON plan within the timeout, the mission fails. There is no silent deterministic fallback.

4. **Deduplicate everything.** Supervisor actions, observations, and state escalations are deduplicated to prevent feedback loops.

5. **Degraded is one-way.** Once the supervisor is marked `Degraded`, the watchdog owns final synthesis. No recovery path exists.

6. **Leases prevent collisions.** File ownership is claimed before editing. Conflicts immediately route the challenger to a contradiction path.

7. **Mail is durable.** All mail is persisted to SQLite before injection. Ack timeouts trigger escalation. Nothing is lost if a session dies.
