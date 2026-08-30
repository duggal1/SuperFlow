# AGENTS.md

# Note 

Important note for all agents working in:

- `/Users/harshitduggal/workspace/sapphire-agent-Factory/src/orchestrator/mod.rs`
- `/Users/harshitduggal/workspace/sapphire-agent-Factory/src/runtime/mod.rs`

## Modularity rule

From this point on, use **modularity aggressively and consistently**.

## Requirements

- Do **not** delete the existing large orchestration file just to split it apart retroactively.
- Do **not** perform a giant rewrite only for cosmetic cleanliness.
- Do **learn from the current state and continue with stricter discipline from now on.**

## Strict rule

You are **not allowed** to keep dumping thousands of lines into one file.

For all new work:
- split logic into smaller files and focused modules
- group code by responsibility
- keep imports and boundaries clean
- use folders and files only when they improve coherence, structure, and maintainability
- do **not** create pointless fragmentation just to look clean


## Goal

Keep the codebase:
- structured
- coherent
- disciplined
- modular
- maintainable

Use modularity because it is necessary for correctness, clarity, and long-term maintainability — not for aesthetics alone.

## Purpose

This repository builds `sp`, a Rust CLI that orchestrates existing terminal-native coding agents such as Codex, Claude, Qwen, and Forge. It is a local control plane, not a model runner. The core job is to launch one supervisor session plus multiple worker sessions, push prompts into PTYs, normalize raw terminal output into Sapphire state, persist mission state into SQLite, and maintain a local control surface under `.sp/`.

## Phase Status

| Phase | Status | Notes |
|---|---|---|
| 1–3 | ✅ Done | Basic launcher, supervisor dispatch, full orchestration (79 tests) |
| 4 | ✅ Done | Live watchdog, role naming (13 templates), heuristic state detection, escalation ladder |
| 4.2 | ✅ Done | Supervisor behavioral rules (Propulsion Principle, Solo Artist Trap, Consecutive Failure Escalation) |
| 4.3 | ✅ Done | 3-layer prompt injection reliability (PTY + startup nudge + durable file reference) |
| 4.4 | ✅ Done | Health state + cooldown, zombie detection, persistent restart tracker, crash loop detection (gastown health/deacon patterns) |
| 4.5 | ✅ Done | Nudge queue, tmux auto-respawn hook, Problems View TUI, mass death + crash loop overlap |
| 4.6 | ✅ Done | Code quality cleanup: 54 → 0 warnings, dead code removed, PromptStyle enum eliminated |
| 4.7 | ✅ Done | Gas Town reliability: zombie TOCTOU, zombie debounce (3-cycle), health probe before stall, message dedup, sliding window mass death detection, session readiness. 3 new modules, 71 tests. |
| 5 | ✅ Done | Engineering-team mail: 5 message types, nudge queue (non-destructive delivery), scavenge claim/release, validation, idempotent ack, auto-archive, CC auto-CC on escalation |
| 6 | 🚧 TODO | 16-terminal orchestration, stress testing |
| 7 | 🚧 TODO | Integration test harness, worker prompt updates for mail protocol |

## Stack

- Rust compiler/toolchain: stable `1.94.1`
- Rust edition: `2024`
- Async runtime: `tokio`
- CLI parsing: `clap`
- Persistence: `rusqlite` with bundled SQLite
- PTY/process control: `portable-pty`
- TUI: `ratatui` + `crossterm` (startup/fallback dashboard shell, wired into `main.rs`)
- Markdown rendering: `pulldown-cmark`
- Serialization: `serde`, `serde_json`
- Logging: `tracing`, `tracing-subscriber`
- Concurrency: `parking_lot`
- UUID generation: `uuid`
- Content hashing: `blake3`
- Regex: `regex`
- Time handling: `chrono`
- Error handling: `anyhow`
- Misc: `directories`, `supports-color`, `unicode-width`

## Code Map

- `src/main.rs`
  Boots tracing, parses CLI args, builds the orchestrator, and dispatches to one of eight actions: `Run`, `Status`, `Sessions`, `Resume`, `Replay`, `Watch`, `Summary`. Live interactive runs keep the current terminal on Sapphire's control UI and open the tmux teamwork surface in a second terminal window; the TUI remains the startup/fallback dashboard shell.

- `src/cli.rs`
  Defines the full CLI surface via `clap` derive macros. Supports `sp <agent> <count> --repo ... --mission ...` plus subcommands for post-launch introspection (`status`, `sessions`, `resume`, `replay`, `watch`, `summary`). Converts CLI args into `LaunchConfig` or `ResumeConfig`, including tmux, transcript persistence, watchdog timing, and per-agent extra args. All subcommands are flat (no nested subcommand enums).

- `src/tui/`
  Ratatui/crossterm TUI dashboard shell. Modules: `app` (terminal lifecycle, event loop, launch dashboard, `p` key binding for Problems View), `data` (SQLite + status file data source), `state` (focus panes, sidebar tabs including **Problems tab**, scrolling), `render` (frame composition, `render_problems_tab()` with severity tiers), `widgets` (reusable UI components), `markdown` (pulldown-cmark rendering with theming), `time` (duration formatting). Exports `run_launch_dashboard()`, `attach_for_repo()`, `attach_for_mission()`, `run_enabled_for_launch()`. Wired into `main.rs` as the startup/fallback dashboard shell. Individual modules (`app.rs`, `data.rs`) carry `#![allow(dead_code)]` for unused helper functions.
  **Problems View** (5th sidebar tab, label "⚠ Problems"): Filters to workers needing attention — Critical (failed 💀, contradictory ⚠), Warning (stalled ⏸, blocked ⛔, wrong_direction ↩), Attention (awaiting validation ✓, needs_retry ↻). `p`/`P` key binding.

- `src/internal/`
  Private UI theming infrastructure. `ui/shimmer/` (animation effects) and `ui/theme/` (color palettes, markdown theme adapter). Re-exported for TUI consumption.

- `docs/architecture-storage-mail-tmux.md`
  Comprehensive architecture documentation for the persistence layer, mail system, and tmux control surface. Covers all 9 SQLite tables with column descriptions, mail lifecycle from creation to archival, pane grid layout algorithms, dashboard content generation, and cross-subsystem data flows (lease conflict resolution, mail→ack→supervisor escalation, tmux dashboard data sourcing).

- `src/orchestrator/mail.rs`
  Engineering-team mail subsystem. Message types: `task` (action required), `reply` (threaded response), `notification` (FYI), `escalation` (blocker → auto-CCs supervisor), `scavenge` (first-to-claim work). Legacy types (dependency_request, review_request, blocker, etc.) normalize to the 5 clean types. Nudge queue: filesystem-based non-destructive delivery (`<state_dir>/nudge_queue/<session_id>/`) — avoids cancelling in-flight tool calls. Claim/release: `attempt_scavenge_claim()` atomically claims via SQLite JSON patch (first wins); `release_scavenge()` releases back to pool. Validation: subject ≤120 chars, body ≤8KB, self-mail rejection. Idempotent ack. Auto-archive resolved mail. Delivery modes: `interrupt` (urgent/critical, direct PTY) vs `queue` (normal, filesystem queue with TTL).

- `src/tmux/`
  tmux CLI wrapper and control surface builder. Modules: `grid` (pane layout composition), `zombie` (zombie TOCTOU mitigation, session readiness verification). `Tmux` struct wraps session creation, pane splitting, key sending, pane capture, Ghostty integration on macOS (tab-first then window fallback), and external terminal opening. Used by the orchestrator for the default teamwork surface.
  **SessionHealth enum**: `Healthy` (process alive + output), `Zombie` (tmux alive but agent dead), `Hung` (alive but no output), `Dead` (no session), `Starting` (within grace period). Methods: `check_session_health()`, `set_remain_on_exit()`, `set_auto_respawn_hook()`, `verify_zombie()`, `wait_for_ready()`.

- `src/agent/mod.rs`
  Declares `AgentKind` (Qwen, Forge, Codex, Claude) and per-agent launch behavior: executable name, default args, environment variables, startup prompt delay, startup input nudge, submit mode, startup input rules, and automation rules for trust/setup prompts. Codex is pinned to `gpt-5.4-mini` with `--no-alt-screen` and low reasoning effort. Qwen launches with `--screen-reader` and relies on the user's existing approval configuration. All agents get a timed `\n` startup input to wake the TUI readline before the assignment prompt arrives. Includes heuristic state inference (`infer_state_from_output`) and a `protocol_nudge()` helper.

- `src/adapter.rs`
  The adapter layer that abstracts per-agent behavior. Defines the `CliAdapter` trait with methods for: state detection, prompt building (assignment, validation, correction, status), done-claim detection, supervisor plan/action prompt building, and final envelope extraction. Four concrete adapters (`QwenAdapter`, `CodexAdapter`, `ClaudeAdapter`, `ForgeAdapter`) are generated via the `impl_standard_adapter!` macro. All 4 share a single prompt path (the `PromptStyle` enum was removed — only `Standard` path remains). Contains all regex-based parsing logic for status envelopes, supervisor actions, final envelopes, and supervisor plan JSON blocks.
  Full architectural documentation: `ADAPTER-LAYER-ARCHITECTURE.md`.

- `src/orchestrator/`
  Core runtime coordinator. **Modular structure**:
  - `mod.rs` — mission lifecycle, watchdog loop, stall/protocol/supervisor handling, thin delegation to submodules
  - `mail.rs` — engineering-team mail subsystem (~1000 lines): message type normalization, validation, rendering, nudge queue, claim/release, auto-archive
  - `health.rs` — health state tracking (~290 lines): `SessionHealthState` (probe/response/failure counters), `ZombieDebounce` (3-cycle debounce), `MassDeathDetector` (sliding window mass death detection)
  - `dedup.rs` — message deduplication (~110 lines): `MessageDeduplicator` for idempotent mail/directive processing
  - Responsibilities in `mod.rs`:
    - `bootstrap()` / `open()` — initializes `.sp/` directories and SQLite store
    - `launch()` — full mission lifecycle: ensure `AGENTS.md` exists, persist mission, run supervisor planning, reserve one worker for `AGENTS.md` stewardship, spawn workers, run live watchdog loop
    - `plan_with_supervisor()` — launches a temporary supervisor session, sends the plan prompt, requires valid JSON plan between `BEGIN_SAPPHIRE_PLAN_JSON` / `END_SAPPHIRE_PLAN_JSON` tags; launch fails if supervisor doesn't produce valid plan
    - `run_live_mission()` — main watchdog event loop. Spawns sessions, provisions tmux, loops ticking for runtime events, stall detection, protocol reminders, supervisor health, status snapshots, nudge queue draining, mail queue draining, crash loop detection
    - `handle_runtime_event()` — processes Output/Automation/Exited events, extracts directives and heuristic observations, dispatches to handlers
    - `handle_status_directive()` — updates session state, triggers validation challenges for done claims, notifies supervisor, persists validation results
    - `handle_normalized_observation()` — uses adapter heuristics to infer state; escalates after 2+ low-confidence observations
    - `apply_supervisor_action()` — executes supervisor decisions with deduplication
    - `handle_mail_directive()` — delegates to `mail.rs`: routes, persists, delivers (nudge queue or interrupt), tracks ack, handles claim/release
    - `handle_ack_directive()` — delegates to `mail.rs`: idempotent ack processing
    - `handle_lease_directive()` — delegates to `mail.rs`: file ownership claims, conflict detection
    - `handle_stalls()` — 3-rung escalation ladder with cooldown (1st: corrective, 2nd: redirect, 3rd+: force Failed)
    - `handle_protocol_reminders()` — nudges workers producing output but no directives (cooldown-aware)
    - `handle_pending_mail()` — probes unacked mail after timeout, escalates to supervisor
    - `handle_pending_supervisor_decisions()` — processes queued decisions with deduplication, cooldown checks, mode-aware routing
    - `handle_supervisor_health()` — monitors supervisor liveness; transitions to `Degraded` mode; watchdog generates final synthesis as fallback
    - `write_status_snapshot()` — writes `.sp/control/status.txt` with live session states
    - `ensure_tmux_surface()` — provisions tmux teamwork surface with live Sapphire control pane
    - `resume()` — restarts mission from persisted state
  - **Health State Tracker**: `ActiveSession` gains `intervention_cooldown_until`, `last_intervention_type`, `total_interventions`, `last_response_time`, `last_intervention_at`, `queued_prompts`
  - **Nudge Queue** (gastown wait-idle pattern): `send_or_queue_prompt()` checks if agent is mid-response (< 3s since output). Queues prompt. `drain_prompt_queues()` called every tick fires queued prompts for quiet sessions.
  - **Mail Nudge Queue**: non-destructive delivery via `<state_dir>/nudge_queue/<session_id>/`. Atomic rename prevents double-delivery. TTL: 30 min normal / 2 hr urgent. Drained every watchdog tick.
  - **Crash Loop Detection**: On mass failure, checks `session_restarts` table for crash loop overlap. Emits `critical_failure` event with crash loop details.
  - Post-launch introspection: `render_sessions`, `render_replay`, `render_status`, `render_worker_replay`, `render_supervisor_summary`

- `src/runtime/mod.rs`
  PTY runtime. Uses `portable-pty` to spawn agent CLIs in pseudo-terminals (30x140 PTY pairs). `SessionRuntime` spawns sessions and produces `RuntimeEvent` (Output/Automation/Exited) via an `mpsc` channel. `RunningSession` provides `send_text()` and `send_prompt()` for prompt injection. A background thread reads PTY output, applies startup automation rules (matching text and auto-responding), and sends events back. Buffer management caps carry at 24KB to prevent unbounded growth. `SubmitMode` (LineFeed vs CarriageReturn) controls prompt termination per agent. All agents get a timed `\n` startup input via `startup_input` to wake the TUI readline before the assignment arrives. Includes `TestSessionHandle` for unit testing the runtime abstraction.
  **tmux backend**: Sets `remain-on-exit` and `set-hook pane-exited respawn-pane` on each pane for instant auto-respawn on process exit (gastown PATCH-010 pattern).

- `src/protocol.rs`
  The Sapphire Control Protocol parser. Parses single-line `SAPPHIRE_STATUS`, `SAPPHIRE_MAIL`, `SAPPHIRE_ACK`, and `SAPPHIRE_LEASE` JSON directives from noisy terminal output. `consume_directives(buffer, chunk)` sanitizes ANSI escape codes, buffers partial lines, and extracts directive JSON using a two-phase approach: regex for `SAPPHIRE_(STATUS|MAIL|ACK|LEASE)` prefix identification, then brace-depth counter for robust JSON object extraction (handles nested/multiline JSON). Includes unit tests for multi-directive parsing and partial-line buffering.

- `src/store/mod.rs`
  SQLite persistence layer. Uses `rusqlite` with bundled SQLite. Schema initialized inline with WAL mode and `SYNCHRONOUS=NORMAL`. Tables: `sessions`, `workers`, `tasks`, `events`, `messages`, `summaries`, `ownership_leases`, `normalized_updates`, `validation_results`, **`session_restarts`** (persistent restart tracking for crash loop detection). Includes legacy migration via table rename when column sets don't match expected schema. All CRUD operations for missions, sessions, tasks, events, messages, leases, summaries, normalized updates, and validation results. Supports mission snapshot loading, worker loading with packet deserialization, replay entries, and worker-specific replay queries. Includes unit tests.
  **Restart tracking methods**: `upsert_restart_attempt()` (increments count, computes exponential backoff), `load_restart_state()`, `reset_restart_tracker()`, `is_crash_loop()` (N restarts within M minutes), `get_crash_loop_sessions()`.

- `src/templates.rs`
  Prompt library. Uses `include_str!` to compile five prompt sources at build time: `product-direction.md`, `supervisor-templates/prompt.md`, `How-should-the-supervisor-be-built.md`, `Agent-to-agent communication-simulate-swe-team.md`, and `agents.md-instructions.md`. Renders supervisor prompts (with full mission context, workstreams, risk map, worker packets, supervision strategy, and control protocol) and worker prompts (with scoped work packet, communication rules, validation standard, and control protocol). Also loads 13 enterprise role templates from `src/internal/agents/templetes/roles/job-roles/` — each role is a full job description (responsibilities, operating rules, pushback policy, coordination protocol). Role templates are keyed by `role_type` (stable machine key like `software-engineer`) and rendered with the agent's specific assignment (display name, scope, task, evidence requirements). Provides `agents_instruction_source()` for the AGENTS.md seeding logic.

- `src/model.rs`
  Shared domain types:
  - `MissionRecord`, `MissionStatus` (Planned/Launching/Running/Completed/Failed)
  - `MissionPlan`, `Workstream`, `WorkstreamExecution` (Parallel/Dependent/Validation/Integration), `RiskItem`, `WorkerPacket`
  - `WorkerPacket` now has `role_type` (stable machine key, e.g. `software-engineer`) and `display_name` (runtime team label, e.g. `Engineer-1`) alongside `worker_id` and `role` (backward compat)
  - `SessionRecord`, `SessionRole` (Supervisor/Worker), `SessionState` (16 states)
  - `EventRecord`, `LeaseRecord`, `MailRecord`, `TaskRecord`, `SummaryRecord`
  - `LaunchSummary`, `NormalizedUpdateRecord`, `ValidationResultRecord`
  - `RestartRecord` (persistent restart tracking: `session_id`, `mission_id`, `restart_count`, `first_restart_at`, `last_restart_at`, `backoff_seconds`)
  - `SessionListItem`, `MissionSnapshot`, `WorkerSnapshot`, `ReplayEntry`

- `src/telemetry/`
  Reserved for telemetry-related expansion. Treat it as part of the intended architecture even if it is currently sparse.

## Prompt Sources

- `product-direction.md`
- `How-should-the-supervisor-be-built.md`
- `Agent-to-agent communication-simulate-swe-team.md`
- `supervisor-templates/prompt.md` — Now includes **Propulsion Principle** (supervisor auto-executes on restart, never waits for human approval) and **Solo Artist Trap** (supervisor must dispatch, not implement; decision tree: coordination → do it, implementation → sling to worker)
- `agents.md-instructions.md` (embedded for AGENTS.md seeding)

These files are part of runtime behavior, not passive docs. `src/templates.rs` compiles them into the binary with `include_str!`.

## Runtime Invariants

- `sp` always plans a mission before launch.
- `sp` checks for `AGENTS.md` in the repo root before agent execution. If missing, it seeds the file from the embedded `agents.md-instructions.md` source.
- One agent slot is reserved for `AGENTS.md` stewardship when the run has more than one agent terminal.
- Every newly launched non-steward agent is told, on first initialization only, to read `AGENTS.md` before real work.
- Multi-terminal runs launch directly. There is no preflight gate before agent and supervisor sessions start.
- State is written under `.sp/` by default.
- Prompt artifacts are written to `.sp/prompts/`.
- Control-surface artifacts are written under `.sp/control/`.
- Transcript files are written under `.sp/transcripts/` when transcript persistence is enabled or `tmux` mode is used.
- SQLite state defaults to `.sp/sapphire.sqlite3` unless overridden.
- The watchdog only understands machine-readable directives if they appear on a single line.
- Agents must claim and release file ownership through `SAPPHIRE_LEASE`.
- An agent completion claim is not acceptance. `done_claimed` triggers a validation challenge.
- Agent-to-agent coordination is routed through durable mail and remains visible to the supervisor.
- Lease conflicts downgrade the conflicting agent into a contradiction path and notify the supervisor.
- Every launched supervisor and agent also gets a persisted `TaskRecord`; task and summary persistence are now part of the control plane.
- Mail ack timeouts are probed at 20s; unacked mail triggers supervisor escalation.
- Low-confidence observations (2+) trigger a corrective prompt and supervisor notice.
- Supervisor action deduplication prevents repeated identical actions from the same signature.
- Observation deduplication prevents repeated handling of identical heuristic keys.
- Supervisor health is monitored; a stalled supervisor transitions to `Degraded` mode, and the watchdog falls back to generating its own final synthesis.
- Supervisor decisions (validation, stall recovery, low-confidence recovery) are queued and processed with deduplication.
- Agents are identified by `display_name` (e.g. `Engineer-1`, `Designer-2`), not by generic `worker-01` IDs. Each agent has a `role_type` for template lookup.
- **Cooldown invariant**: After any watchdog intervention, the session has a cooldown (`30s × intervention_count`, capped at `120s`). Redundant interventions (stall prompts, protocol reminders, fallback decisions) are skipped during cooldown. Cooldown resets on output arrival.
- **Nudge queue invariant (prompt)**: Prompts are never injected while the agent is mid-response (< 3s since last output). They are queued and drained when the session goes quiet.
- **Nudge queue invariant (mail)**: Non-urgent mail writes to filesystem queue (`<state_dir>/nudge_queue/<session_id>/`) instead of injecting into PTY. Prevents cancelling in-flight tool calls. Drained every watchdog tick. TTL: 30 min normal / 2 hr urgent.
- **Zombie debounce invariant**: Sessions are NOT killed on first zombie detection. Watchdog requires 3 consecutive zombie cycle detections before considering restart. Resets on any output arrival. Prevents false kills during slow startup or transient gaps.
- **Health probe invariant**: Before a session hits the stall threshold, a health probe is sent at 75% of the stall duration. Probes track probe/response/failure counters. Output resets the failure counter.
- **Message dedup invariant**: Mail directives with `mail_id` are deduplicated per session. Already-processed mail is silently skipped, preventing duplicate injection after orchestrator restart.
- **Mass death detection invariant**: 3+ session deaths within 30s triggers a `critical_failure` event with `tracing::error!`. Emit cooldown: 60s minimum between mass death events to prevent log spam.
- **Scavenge invariant**: Only one worker can claim a scavenge message — first wins via atomic SQLite JSON patch. Release returns it to the pool.
- **Auto-respawn invariant**: tmux panes are created with `remain-on-exit` + `set-hook pane-exited respawn-pane` for instant recovery on process exit.
- **Restart tracker invariant**: Restart attempts are persisted to `session_restarts` table with exponential backoff (`2s × 2^(count-1)`, capped at `300s`). N restarts within M minutes = crash loop → supervisor escalation.

## Sapphire Orchestration Layer — Real Capabilities

### Mission Planning & Decomposition
- **Deterministic keyword-based planning**: Parses mission text to select workstreams (Baseline, UI/UX, Debug, Performance, Validation, Analysis, Refactor, Integration).
- **Supervisor override**: A live supervisor session can produce a better plan within 45s using `BEGIN_SAPPHIRE_PLAN_JSON` / `END_SAPPHIRE_PLAN_JSON` tagged JSON blocks; falls back to deterministic on failure. Supervisor JSON is strictly validated: no markdown fences, no prose outside markers, arrays must be arrays (never strings), execution must be one of `parallel|dependent|validation|integration`.
- **Role-based agent activation**: Supervisor analyzes the mission and activates only relevant role types. Coding-only tasks activate `software-engineer` instances; multi-domain tasks activate `software-engineer` + `designer-engineer` + `security-engineer` etc. No hardcoding — the supervisor decides which roles to deploy.
- **Function-first team roles (no personas)**: Agents use `role_type` (stable machine key, e.g. `software-engineer`) for template lookup and `display_name` (runtime label, e.g. `Engineer-1`, `Designer-2`, `Reviewer-1`, `Architect-1`) for prompts/logs/UI. 15 roles available: Software Engineer, Research Engineer, Validation Engineer, Architecture Engineer, Security Engineer, Debug & Review Engineer, Testing & Automation Engineer, Designer Engineer, Sales Engineer, Solutions Engineer, Customer Success Engineer, Product Engineer, Product Manager, Revenue Engineer, Compliance Engineer. Each role has a full enterprise-grade job description template with responsibilities, operating rules, pushback policy, and coordination protocol.
- **Dynamic display naming**: `software-engineer` → `Engineer-1`, `Engineer-2`... | `designer-engineer` → `Designer-1`, `Designer-2`... | `debug-and-review-engineer` → `Reviewer-1`... | `validation-engineer` → `Validator-1`... | `architecture-engineer` → `Architect-1`... | `security-engineer` → `Security-1`... | `testing-and-automation-engineer` → `QA-1`... | `research-engineer` → `Researcher-1`... | `product-manager` → `ProductManager-1`... | `revenue-engineer` → `Revenue-1`...
- **Workstream allocation**: Workstreams are partitioned dynamically across workers. Extra workers do not become hardcoded reviewer personas; they become additional parallel passes on real workstreams with different starting angles and coverage lanes.
- **Coordination fingerprinting**: Blake3 hash of assigned workstream IDs embedded in each worker packet for proof of scope.
- **Risk map generation**: Automatic risk identification based on mission type (shared files, completion claims, cross-worker assumptions, UI regressions, performance claims).
- **AGENTS.md steward**: One worker slot is reserved to maintain `AGENTS.md` in the repo, seeded from embedded instructions if the file is absent.

### Session Lifecycle Management
- **Multi-agent PTY orchestration**: Launches 1 supervisor + N agents (+ 1 AGENTS steward) in isolated PTY sessions (30x140), each running a distinct agent CLI in **interactive mode** (full TUI, no `--screen-reader` for workers).
- **3-layer prompt injection reliability**:
  1. **PTY `send_prompt()`**: Full role template + assignment written to PTY master fd after boot delay (all agents).
  2. **Startup `\n` nudge**: Timed Enter key wakes TUI readline before assignment arrives — Qwen 500ms, Codex 400ms, Forge 400ms, Claude 600ms.
  3. **Durable file reference**: Full assignment saved to `.sp/prompts/<name>.md`; agents told to re-read if anything is unclear.
- **Per-agent launch specs**: Executable name, default args, environment variables, startup prompt delay, submit mode (LineFeed vs CarriageReturn), startup input injection, and automation rules per agent kind.
- **Role-based prompt injection**: Each agent receives its role template (13 available) plus a scoped assignment packet with `display_name`, `role_type`, scope, task, evidence requirements, and communication rules.
- **Startup automation**: Auto-dismisses agent-specific trust/setup prompts (Qwen IDE prompt, Codex directory trust, Claude folder trust, Forge isolation).
- **Submit mode handling**: Different agents require different prompt termination (`\n` vs `\r\n`). All agents get a timed initial `\n` via `startup_input`.
- **Staggered prompt injection**: Prompts are queued and sent after per-agent delay, sorted to minimize total boot wait.
- **Mission resume**: Reloads all persisted state, re-launches only non-terminal workers with prior context (summary + replay excerpts).

### 16-State Session Lifecycle
`Planned → Booting → NotStarted → Progressing → Blocked → Stalled → DoneClaimed → NeedsValidation → WeakOutput → WrongDirection → Contradictory → NeedsRetry → Validated → Failed → Exited`

Terminal states: `Validated`, `Failed`, `Exited`.

### Real-Time Watchdog Loop
- **Tick-based monitoring**: Configurable tick interval (default 1s) collects runtime events, processes directives, handles stalls, sends protocol reminders, checks supervisor health, writes status snapshots, drains nudge queues, and detects crash loops.
- **Stall detection**: Workers silent beyond `stall_seconds` are marked `Stalled`, tracked with `consecutive_stall_failures` counter (reset on any output/automation event), and run through an escalation ladder:
  - **1st stall**: Corrective status prompt + cooldown set
  - **2nd stall**: Redirect with narrowed scope sent immediately + cooldown set + supervisor notice
  - **3rd+ stall**: Force `Failed` state + supervisor must decide respawn or reassign
- **Cooldown system** (gastown health/health.md pattern): After ANY watchdog intervention (stall prompt, redirect, validation challenge, protocol reminder, fallback), cooldown is set to `30s × total_interventions` (capped at `120s`). Redundant interventions are skipped during cooldown. On output arrival, response time is recorded and cooldown resets.
- **Liveness tracking**: `last_confirmed_alive` timestamp updated on every output/automation event; distinguishes "stalled but was recently alive" from "dead".
- **Intervention response tracking**: `total_interventions`, `last_intervention_type`, `last_response_time` tracked per session.
- **Nudge Queue** (gastown wait-idle pattern): Before `send_prompt()`, checks if agent is mid-response (< 3s since output). If so, queues the prompt. `drain_prompt_queues()` called every tick fires queued prompts for quiet sessions. Prevents TUI readline conflicts.
- **Zombie Detection** (gastown witness/deacon pattern): `SessionHealth` enum distinguishes `Healthy`, `Zombie` (tmux alive but agent dead), `Hung`, `Dead`, `Starting` (within 5s grace). `check_session_health()` uses pane PID → `kill -0` process liveness check.
- **Persistent Restart Tracker** (gastown daemon pattern): Restart attempts persisted to `session_restarts` SQLite table. Exponential backoff: `2s × 2^(count-1)`, capped at `300s`. `is_crash_loop()` detects N restarts within M minutes → escalate, don't auto-restart.
- **Crash Loop Detection**: On mass failure, checks if dead sessions are in crash loop state. Emits `critical_failure` event (not just `mass_failure_detected`) with crash loop details in supervisor notice.
- **Auto-Respawn Hook** (gastown PATCH-010 pattern): tmux panes created with `remain-on-exit` + `set-hook pane-exited respawn-pane`. Instant recovery on process exit — no watchdog polling delay.
- **Protocol reminders**: Workers that produce output but no directives after 3+ chunks receive a strict status prompt (cooldown-aware).
- **Max runtime enforcement**: Optional `watchdog_max_seconds` hard timeout.
- **Supervisor health monitoring**: Supervisor is checked for stalls; transitions to `Degraded` mode when unresponsive; watchdog generates final synthesis as fallback when supervisor is unavailable.
- **Supervisor decision queue**: Decisions (validation, stall recovery, low-confidence recovery) are queued, deduplicated, cooldown-checked, and applied with mode awareness.

### Problems View (TUI Dashboard)
- **5th sidebar tab** (label "⚠ Problems"), `p`/`P` key binding.
- Filters to ONLY workers needing attention:
  - **Critical** (💀 failed, ⚠ contradictory)
  - **Warning** (⏸ stalled, ⛔ blocked, ↩ wrong_direction)
  - **Attention** (✓ awaiting validation, ↻ needs_retry)
- Shows total count: "N problem(s) total". "All clear" when none.

### Supervisor Behavioral Rules
- **Propulsion Principle**: Supervisor auto-executes on restart — checks persisted state, acts on pending work, then summarizes. Never waits for human approval. Action first, summary second.
- **Solo Artist Trap**: Supervisor must dispatch, not implement. Decision tree: coordination → do it directly, implementation → sling to worker, trivial → fix directly. Anti-pattern: reading code to "understand the issue" and fixing it burns context needed for team supervision.
- **Consecutive Failure Escalation**: The escalation ladder (1st/2nd/3rd+ stall) is enforced with `consecutive_stall_failures` counter that resets on any liveness signal. Third consecutive stall forces `Failed` state; supervisor decides respawn or reassign.

### Sapphire Control Protocol (4 directive types)
- **`SAPPHIRE_STATUS`**: Worker reports state with summary, files, commands, risks, overlap. Parses into `SessionState` and triggers validation, supervisor notification, task state updates, and validation result persistence.
- **`SAPPHIRE_MAIL`**: Structured inter-worker communication with 5 clean message types (`task` = action required, `reply` = threaded response, `notification` = FYI, `escalation` = blocker → auto-CCs supervisor, `scavenge` = first-to-claim work). Legacy types (dependency_request, review_request, blocker, handoff, etc.) normalize to the 5 clean types. Priority levels (urgent/high/normal/low), ack tracking, conversation threading, CC visibility, and supervisor visibility. Non-urgent mail uses filesystem nudge queue for non-destructive delivery; urgent mail injects directly into PTY. Agents can also send `claim` and `release` directives to claim or release scavenge work.
- **`SAPPHIRE_ACK`**: Mail acknowledgment with status (acked, done, cannot_comply). Notifies original sender and supervisor.
- **`SAPPHIRE_LEASE`**: File ownership claims with intent (read, edit, review) and status (claim, release). Upserts into SQLite; detects conflicts and triggers contradiction handling.

### Heuristic State Detection (Adapter Layer)
When no explicit directive is found, the adapter layer infers state from output keywords:
- `validation passed`, `validated`, `all checks passed` → `Validated`
- `probably fixed`, `didn't run tests`, `can't verify` → `WeakOutput`
- `rewrote the architecture`, `full rewrite`, `took over` → `WrongDirection`
- `i'm done`, `completed the task`, `task is complete` → `NeedsValidation` (with confidence varying by agent)
- `blocked`, `cannot proceed`, `need clarification` → `Blocked`
- `conflict`, `overlap`, `contradiction` → `Contradictory`
- `investigating`, `working on`, `running tests` → `Progressing`

Status envelope regex parsing extracts structured `STATE/SUMMARY/FILES/BLOCKER/DONE` blocks from freeform output.

### Supervisor Action Execution
The supervisor can issue structured actions parsed from output. Targets use display names (e.g. `Engineer-1`, `Designer-2`) not worker IDs:
- `observe` — passive monitoring
- `validate_worker` — sends validation challenge to target agent
- `retry_worker` / `redirect_worker` / `message_worker` — sends correction prompt with supervisor message
- `accept_worker` — forces `Validated` state on target
- `fail_worker` — forces `Failed` state on target
- Action deduplication prevents repeated identical actions

### Supervisor Health & Degraded Mode
- **Health monitoring**: The watchdog checks supervisor liveness on each tick via stall detection.
- **Degraded mode**: When the supervisor stalls or exits, the system transitions to `SupervisorMode::Degraded`.
- **Fallback final synthesis**: In degraded mode, the watchdog generates its own final synthesis instead of waiting for the supervisor.
- **Decision queue resilience**: Pending supervisor decisions are processed with mode awareness; degraded mode skips actions that require supervisor intelligence.

### Validation & Quality Control
- **Automatic validation challenges**: Every `done_claimed` or `needs_validation` state triggers a validation challenge prompt.
- **Validation result persistence**: All validation outcomes are persisted with evidence JSON.
- **Low-confidence escalation**: 2+ low-confidence observations trigger a corrective prompt and supervisor notice.
- **Reviewer workers**: Overflow workers act as secondary reviewers challenging claims, inspecting contradictions, and escalating overlap risk.

### File Ownership & Conflict Resolution
- **Lease-based file ownership**: Workers must claim files before editing; leases are upserted in SQLite on conflict.
- **Conflict detection**: When two workers claim the same path, the challenger is downgraded to `Contradictory`, blocked, and the supervisor is notified with exact scope details.
- **Owner notification**: The current owner is notified that another worker attempted to claim their path.

### Inter-Worker Communication
- **Durable mail routing**: Mail is persisted to SQLite before injection into recipient PTY. Mail targets use display names (e.g. `Engineer-2`, `Supervisor`) not worker IDs.
- **Ack tracking**: Mail requiring ack is tracked with 20s timeout probing.
- **Timeout escalation**: Unacked mail triggers probes to both sender and recipient, plus supervisor escalation.
- **Supervisor visibility**: All mail between non-supervisor agents generates a supervisor notice.

### tmux Control Surface
- **Default teamwork surface**: live runs create a tmux session by default unless the run is non-interactive or dry-run, and the pane grid is opened in a second terminal window.
- **Ghostty host rule**: when `TERM_PROGRAM=ghostty`, Sapphire prefers opening the teamwork surface in Ghostty itself; it tries a new tab first, then falls back to a new Ghostty window if macOS keystroke automation is blocked.
- **Control panel**: Displays `.sp/control/status.txt` with live session states.
- **Per-worker transcript tails**: Each worker gets a dedicated window tailing its transcript.

### Transcript & Event Persistence
- **Transcript files**: Written per-worker under `.sp/transcripts/` for live runs.
- **Event log**: All runtime events (output chunks, directives, state changes, automation, stalls, mail, leases, validation results) are persisted to SQLite.
- **Normalized updates**: Adapter-normalized state observations are stored with confidence scores and source attribution.

### Post-Launch Introspection (6 CLI Actions + run)
1. **`sp status`** — Show active/running missions at a glance.
2. **`sp sessions`** — List all missions from SQLite with status, repo, and rewrite.
3. **`sp resume <mission_id>`** — Resume a mission from durable orchestration state.
4. **`sp replay <mission_id>`** — Show recent events across all workers for a mission.
5. **`sp watch <mission_id> <worker>`** — Show recent events for a specific worker.
6. **`sp summary <mission_id>`** — Show supervisor's latest summary.
7. **`sp run`** (default) — Launch a new mission with full orchestration.

### Persistence Schema (10 tables)
| Table | Purpose |
|---|---|
| `sessions` | Mission records with plan JSON, status, and final summary |
| `workers` | Session records (supervisor + workers) with optional packet JSON |
| `tasks` | Task assignments per worker with depends_on and definition_of_done |
| `events` | All runtime events (output, directives, state changes, automation, stalls, mail, leases) |
| `messages` | Durable inter-worker mail with ack tracking and priority |
| `summaries` | Freeform summaries (mission-level, per-worker, plan_source, resume, exit, surface, agents_bootstrap, preflight_failure) |
| `ownership_leases` | File ownership claims (upsert on conflict by session+path) |
| `normalized_updates` | Adapter-normalized state observations with confidence and source |
| `validation_results` | Validation challenge outcomes with evidence JSON |
| `session_restarts` | Persistent restart tracking: session_id, mission_id, restart_count, first/last restart timestamps, backoff_seconds. Used for crash loop detection (N restarts within M minutes) |

### Agent Adapters (4 implementations)
| Agent | Default Args | Submit Mode | Startup Automation | Prompt Style |
|---|---|---|---|---|
| Qwen | `--screen-reader` | CarriageReturn | Auto-dismisses "Do you want to connect IDE" (sends "2") | Compact |
| Forge | (none) | LineFeed | Sets isolated `HOME`/`XDG_DATA_HOME`/`XDG_CONFIG_HOME` to `.sp/forge-home/` | Standard |
| Codex | `--no-alt-screen -m gpt-5.4-mini -c model_reasoning_effort=low` | CarriageReturn | Auto-accepts directory trust prompt (sends "1"); initial `\n` after 400ms | Standard |
| Claude | (none) | CarriageReturn | Auto-accepts "Yes, I trust this folder" prompt | Standard |

See `ADAPTER-LAYER-ARCHITECTURE.md` for full adapter layer documentation including heuristic state detection, regex contracts, prompt style variants, and macro architecture.

## Change Rules

- Do not break the contract between prompt text and runtime parsing. If you change protocol wording or supported states, update:
  - `src/protocol.rs`
  - `src/model.rs`
  - `src/orchestrator/mod.rs`
  - `src/adapter.rs`
  - `src/templates.rs`
  - any prompt markdown that teaches the protocol

- If you add a session state, update all state handling paths:
  - parsing in `SessionState::from_directive`
  - terminal-state logic in `is_terminal`
  - watchdog escalation logic in the orchestrator
  - prompt instructions that enumerate valid states
  - adapter heuristic keyword lists in `detect_state_impl`

- If you change persistence schema or stored payload shape, keep old runs readable or add a migration strategy. Right now schema creation is inline and optimistic, with legacy table rename on column mismatch.
- Mission/session/task/message/summary storage names do not perfectly match the older vocabulary in all modules. Read model, store, and orchestrator together before changing any schema-facing code.

- If you change `AgentKind` launch behavior, preserve safe startup automation for trust/setup prompts. A broken launch spec means the entire factory stalls at boot.

- Keep worker packets narrow. This codebase assumes ownership boundaries are a safety mechanism, not just prompt decoration.

- The adapter macro (`impl_standard_adapter!`) generates all four standard adapters. Changes to the trait or macro affect all agents simultaneously. Qwen alone uses `PromptStyle::Compact`.

- The runtime layer uses a `SessionHandle` trait with `PtySessionHandle` (real PTY) and `TestSessionHandle` (fake). Changes to the trait affect both implementations.

- `SubmitMode` (LineFeed vs CarriageReturn) is per-agent and affects both `send_prompt()` and startup automation rule responses.

## Documentation

Comprehensive contributor documentation lives in the `docs/` folder:

| File | Purpose |
|---|---|
| `docs/quickstart.md` | Setup, prerequisites, and first-run guide |
| `docs/commands.md` | Complete CLI command and flag reference |
| `docs/contributor-guide.md` | Development workflow, change rules, module dependencies, common tasks |

New contributors should read `docs/quickstart.md` first, then `docs/contributor-guide.md` before making changes.

## Working Style For Agents

- Prefer surgical changes. This project is small and tightly coupled.
- Preserve the control-plane nature of the product. Do not turn it into a hosted API client or a general chat tool.
- Treat prompt files as executable behavior. Small wording changes can materially alter orchestration quality.
- Maintain concise, operational output. The product direction explicitly rejects bloated manager-like prose.
- When fixing orchestration bugs, trace the full loop:
  `templates -> runtime/orchestrator -> protocol -> adapter -> store`
- Read model, store, and orchestrator together before changing any schema-facing code. Cross-module invariants exist and drift.

## Commands

For the complete command and flag reference, see `docs/commands.md`. Quick reference:

- Format: `cargo fmt`
- Test: `cargo test`
- Build (release): `cargo build --release`
- Run a dry plan: `cargo run --bin sp -- codex 2 --repo . --mission "debug and validate the repo" --dry-run`
- Run live: `cargo run --bin sp -- claude 4 --repo . --mission "..."`
- Run with default teamwork surface: `cargo run --bin sp -- codex 4 --repo . --mission "..." --tmux-session-name sapphire`
- Show active missions: `cargo run --bin sp -- status`
- List all sessions: `cargo run --bin sp -- sessions`
- Resume a mission: `cargo run --bin sp -- resume <mission_id>`
- Replay events: `cargo run --bin sp -- replay <mission_id>`
- Watch a worker: `cargo run --bin sp -- watch <mission_id> <worker>`
- Supervisor summary: `cargo run --bin sp -- summary <mission_id>`

## Current Gaps To Respect

- Planning is supervisor-driven on the primary path; launch now fails rather than silently inventing a deterministic fallback plan.
- There is no explicit migration framework for SQLite schema changes (legacy rename is opportunistic).
- There is no separate integration test harness around PTY orchestration yet.
- Prompt files are the source of truth for supervisor and worker behavior; changing them requires extra care.
- The codebase is still evolving quickly; storage naming and runtime responsibilities have drifted, so assume cross-module invariants need re-validation before refactors.
- The `src/telemetry/` directory is empty; reserved for future expansion.
- TUI (`src/tui/`) is wired into `main.rs` as the startup/fallback dashboard shell. Individual modules carry `#![allow(dead_code)]` for unused helper functions.
- No worktree/isolation modes beyond Forge's `HOME` sandboxing.
- Supervisor degraded mode is functional but fallback final synthesis is watchdog-generated, not model-driven.
- The `runtime/` directory at repo root is empty and unused.
- Phase 4 core watchdog loop, role naming, heuristic state detection, and escalation ladder are implemented in Rust. Remaining items are UI/dashboard wiring (TUI not yet wired as interactive dashboard).
- Shell-script proven capabilities (8-terminal mail routing, 16-terminal scale) not yet ported to Rust runtime.
