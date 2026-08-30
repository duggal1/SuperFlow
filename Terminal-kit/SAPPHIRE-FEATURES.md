# Sapphire (`sp`) — Main Features & Real Pain Points

> `sp` is Sapphire: a terminal-first, **local** control plane that runs coding agents
> (Claude, Codex, Qwen, Forge) as a coordinated team instead of isolated tabs.
> It is a control plane, not a model runner — it launches, supervises, audits, and
> persists multi-agent missions entirely on the user's machine.
>
> Source of truth: `Terminal-kit/` (Cargo pkg `sapphire-agent-factory`, bin `sp`).

---

## 1. Why Sapphire Exists — The Pain It Solves

When you run multiple agents by hand, **you** become the scheduler, router, reviewer,
and status board. The real, named pain points:

| # | Pain Point (real) | What manual multi-agent runs cost you |
|---|---|---|
| P1 | **Operator-as-coordinator tax** | You manually split work, paste prompts into N tabs, and tab-switch to read progress. Context-switching destroys flow. |
| P2 | **No durable mission state** | Kill the terminal and the whole run evaporates. No replay, no resume, no audit trail. |
| P3 | **Silent stalls** | An agent hangs mid-task; you only notice 20 minutes later when nothing shipped. No watchdog. |
| P4 | **Cross-agent conflicts** | Two agents edit the same file, overwrite each other, and you get a silent regression nobody owns. |
| P5 | **No supervision** | Nobody checks whether "I'm done" is actually done. Claims of completion are trusted blindly. |
| P6 | **Zombie / dead sessions** | The tmux pane is alive but the agent process died; you stare at a frozen prompt. |
| P7 | **Crash loops** | A worker restarts forever, quietly burning time and tokens with zero signal. |
| P8 | **No team communication** | Agents can't ask each other questions, claim work, or escalate blockers. Everything routes through you. |
| P9 | **Uninspectable runs** | You can't see "what did worker 3 actually do?" after the fact. |
| P10 | **Cloud-billing & privacy drag** | Orchestration that phones home, needs an API key, or leaks your repo to a remote service. Sapphire stays 100% local. |

Every feature below is a direct answer to one or more of these.

---

## 2. Main Features (and the pain point each kills)

### F1 — Supervised multi-agent launch  →  kills P1, P5
`sp <agent> <count> "mission"` launches 1 supervisor + N workers in isolated PTYs and
keeps **your current terminal as the control UI** while the teamwork grid opens in a
second terminal (Ghostty tab-first, then window). One command replaces the whole
manual tab-dance.
- Per-agent launch specs (`src/agent/mod.rs`): executable, args, env, submit mode
  (`\n` vs `\r\n`), startup nudge timing, trust-prompt automation.
- 3-layer prompt injection reliability: PTY `send_prompt()` → timed `\n` wake nudge →
  durable file reference at `.sp/prompts/<name>.md`.

### F2 — Mission planning & decomposition  →  kills P1, P5
- Deterministic keyword planning selects workstreams (Baseline, UI/UX, Debug,
  Performance, Validation, Analysis, Refactor, Integration).
- **Supervisor override**: a live supervisor can emit a better plan inside 45s via
  `BEGIN_SAPPHIRE_PLAN_JSON` / `END_SAPPHIRE_PLAN_JSON` (strictly validated JSON;
  falls back to deterministic on failure).
- **Role-based activation** (no hardcoded personas): supervisor picks which of 15
  enterprise role templates to deploy (`software-engineer`, `designer-engineer`,
  `security-engineer`, `debug-and-review-engineer`, …). Extra workers become additional
  parallel passes, not fake reviewer bots.
- Risk map + coordination fingerprint (Blake3 hash of workstream IDs) embedded per
  worker packet for proof-of-scope.

### F3 — Durable state under `.sp/`  →  kills P2, P9
- SQLite store (`src/store/mod.rs`, `.sp/sapphire.sqlite3`) with 10 tables:
  `sessions`, `workers`, `tasks`, `events`, `messages`, `summaries`,
  `ownership_leases`, `normalized_updates`, `validation_results`, `session_restarts`.
- WAL mode + `SYNCHRONOUS=NORMAL`; legacy migration via opportunistic table rename.
- Transcripts, control artifacts, and prompt files all persisted on disk.

### F4 — Real-time watchdog loop  →  kills P3, P6, P7
Tick-based (default 1s) supervisor that never sleeps:
- **Stall detection** with 3-rung escalation ladder: 1st → corrective prompt; 2nd →
  narrowed-scope redirect; 3rd+ → force `Failed` + supervisor decides respawn/reassign.
- **Cooldown system**: `30s × intervention_count` (capped 120s), resets on output.
- **Zombie detection**: `SessionHealth` enum (`Healthy`/`Zombie`/`Hung`/`Dead`/`Starting`)
  via pane PID `kill -0` liveness check; 3-cycle debounce prevents false kills.
- **Health probe** at 75% of stall threshold before declaring stalled.
- **Persistent restart tracker** + crash-loop detection (`is_crash_loop()`): exponential
  backoff, escalate instead of auto-restarting forever.
- **Auto-respawn hook**: tmux panes use `remain-on-exit` + `set-hook pane-exited
  respawn-pane` for instant recovery.

### F5 — Sapphire Control Protocol  →  kills P4, P5, P8
Single-line directives parsed from noisy terminal output (`src/protocol.rs`, ANSI-
sanitized, brace-depth JSON extraction, partial-line buffering):
- `SAPPHIRE_STATUS` — worker state report (triggers validation + supervisor notice).
- `SAPPHIRE_MAIL` — 5 clean message types (`task`, `reply`, `notification`,
  `escalation` auto-CCs supervisor, `scavenge` first-to-claim). Legacy types normalize
  down to these 5.
- `SAPPHIRE_ACK` — idempotent acknowledgment with status.
- `SAPPHIRE_LEASE` — file-ownership claims; conflict auto-downgrades the challenger to
  `Contradictory` and notifies the supervisor with exact scope.

### F6 — Validation & quality control  →  kills P5
- Every `done_claimed` / `needs_validation` state fires a **validation challenge**.
- Low-confidence heuristic observations (2+) trigger a corrective prompt + supervisor notice.
- Reviewer workers challenge claims, inspect contradictions, escalate overlap risk.
- All validation outcomes persisted with evidence JSON.

### F7 — Engineering-team mail  →  kills P8
- Durable, SQLite-persisted routing before PTY injection.
- **Nudge queue** (filesystem, `<state_dir>/nudge_queue/<session_id>/`) = non-destructive
  delivery so an in-flight tool call is never cancelled; TTL 30m normal / 2h urgent.
- Atomic scavenge claim/release (first wins), idempotent ack, auto-archive resolved mail.
- Ack timeout probing at 20s; unacked mail escalates to supervisor.

### F8 — 16-state session lifecycle & Problems View  →  kills P3, P9
- `Planned → Booting → NotStarted → Progressing → Blocked → Stalled → DoneClaimed →
  NeedsValidation → WeakOutput → WrongDirection → Contradictory → NeedsRetry →
  Validated → Failed → Exited`.
- TUI **Problems View** (5th sidebar tab, `p`/`P`): filters to only workers needing
  attention — Critical (failed 💀, contradictory ⚠), Warning (stalled ⏸, blocked ⛔,
  wrong_direction ↩), Attention (awaiting validation ✓, needs_retry ↻).

### F9 — Heuristic state detection  →  kills P3 (when agents don't use protocol)
Adapter layer (`src/adapter.rs`) infers state from raw output keywords
(`validated`, `can't verify`, `took over`, `blocked`, `conflict`, …) with per-agent
confidence — so even non-compliant agents stay observable.

### F10 — Post-launch introspection  →  kills P2, P9
Six CLI actions turn any run into an audit trail:
`sp status` · `sp sessions` · `sp resume <id>` · `sp replay <id>` ·
`sp watch <id> <worker>` · `sp summary <id>`. Plus `sp push` (operator-owned git push).

### F11 — No-supervisor launcher  →  kills P1 (for power users)
`sp ns <agent> <count> "<p1>" ... "<pN>"` launches N distinct prompts directly when you
want to supervise manually (alias `np`). Supports up to 20 terminals in one shot.

### F12 — 100% local / private  →  kills P10
No network of its own. No API key. Agents run as local CLIs. All state, transcripts,
and mail stay under `.sp/` on the user's machine.

### F13 — Supervisor behavioral rules  →  kills P5 (governance)
- **Propulsion Principle**: supervisor auto-executes on restart, never waits for human approval.
- **Solo Artist Trap**: supervisor dispatches, doesn't implement — preserves context for team supervision.
- **Consecutive Failure Escalation**: 3rd consecutive stall forces `Failed`.

### F14 — Degraded-mode resilience  →  kills P3 (supervisor death)
If the supervisor stalls/exits, system transitions to `SupervisorMode::Degraded` and the
watchdog generates a fallback final synthesis instead of hanging.

---

## 3. Real Pain Points Still Open (gaps to fix — honest list)

These are the *genuine* weak spots in Sapphire today, drawn from its own `AGENTS.md`
"Current Gaps". They are the highest-value things to fix next:

| Gap | Pain it causes | Severity |
|---|---|---|
| **G1 — Bloated/complex stack** | Heavy ratatui/shimmer/theme + 15 role files + mail/zombie/crash-loop machinery is partially untested and drifted. Slower than needed. | High |
| **G2 — Cloud-agent hardcoding** | `src/agent/mod.rs` hardcodes CLAUDE/CODEX (cloud). No first-class **local** model path (Ollama/Nemotron/Qwen3, empty key). Blocks "100% free forever" goal. | High |
| **G3 — No SQLite migration framework** | Schema changes rely on opportunistic table rename; old runs can become unreadable. | High |
| **G4 — TUI not a real interactive dashboard** | Wired only as startup/fallback shell; modules carry `#![allow(dead_code)]`. No live interactive control surface yet. | Medium |
| **G5 — No integration test harness** | PTY orchestration (8/16-terminal mail routing, scale) proven only in shell scripts, not ported to Rust tests. | Medium |
| **G6 — Storage naming drift** | `sessions`/`workers`/`tasks` vocabulary doesn't match older modules; cross-module invariants need re-validation before refactor. | Medium |
| **G7 — Supervisor degraded synthesis is watchdog-generated** | Fallback final summary is heuristic, not model-driven — weaker conclusions when supervisor dies. | Medium |
| **G8 — No worktree/isolation** | Only Forge's `HOME` sandboxing; parallel workers share one checkout → higher conflict/overwrite risk. | Medium |
| **G9 — Empty `src/telemetry/`** | No observability/metrics expansion despite being reserved in architecture. | Low |
| **G10 — Launch fails without valid plan** | Supervisor-driven path fails hard rather than silently falling back to deterministic planning. | Low |

---

## 4. Suggested Priority Order (fix the pain, not the polish)

1. **G2** (local model path) — unlocks the privacy/free promise; highest user value.
2. **G1** (aggressive simplification) — drop unused shimmer/theme cruft; reclaim speed/reliability.
3. **G3** (migration framework) — make state durable across versions.
4. **G5** (integration harness) — prove 16-terminal scale in Rust, not just shell.
5. **G4 / G7** (real TUI + model-driven degraded synthesis) — turn observability into action.

---

## 5. One-Line Summary

> Sapphire turns "I ran 4 agents in 4 tabs and hoped" into "one command launched,
> supervised, validated, and audit-logged a coordinated team — locally, privately,
> and resumably." Its remaining pain is mostly **de-bloat + go fully local + durable
> migrations**, not missing orchestration ideas.
