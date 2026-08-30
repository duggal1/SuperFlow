
---------------------------------------------------------------------------------------------

# Sapphire Agent Factory (`sp`)

**Thunder-fast terminal orchestration layer for existing AI coding CLIs**

## 1. Product truth

This product is **not** another coding agent.

It is a **local orchestration and supervision CLI** that sits on top of existing agent CLIs such as:

* Claude Code
* Codex CLI
* Qwen CLI
* other terminal-native agent tools

It does **not** require an API key for the orchestration layer itself, because it does not call models directly.
It **controls already-authenticated CLI agents running in local terminal sessions**.

That is the real wedge.

---

## 2. Core product idea

Today, your workflow is manual and inefficient:

* split terminal panes manually
* launch many agent CLIs manually
* copy/paste prompts manually
* monitor progress manually
* ask follow-up questions manually
* validate work manually
* remember which agent is touching what manually

That is not an AI factory.
That is human-operated terminal babysitting.

`sp` fixes that.

`sp` turns many terminal-native AI agents into a **coordinated local execution team** with:

* launch orchestration
* task distribution
* supervisor-driven follow-up
* validation loops
* progress tracking
* retry / escalation
* final aggregation

---

## 3. Product definition

## What `sp` is

A Rust CLI that:

1. launches many agent CLI sessions
2. assigns them structured work
3. supervises progress continuously
4. sends follow-up prompts automatically
5. validates outputs aggressively
6. aggregates results into one execution graph
7. keeps the human in command

## What `sp` is not

* not a foundation model
* not a hosted SaaS first
* not a worktree-heavy enterprise overbuild
* not an API gateway
* not a code editor
* not a generic chat UI

---

## 4. Product promise

**One command creates an AI execution team in your terminal.**

Example:

```bash
sp claude 20 --repo . --mission "Fix UI/UX, debug failures, test aggressively, validate, review performance, and ship the whole feature cleanly."
```

That command should:

* open or attach 20 terminal-backed Claude sessions
* assign structured sub-tasks
* designate 1 session as supervisor
* keep all sessions aligned to the same mission
* avoid duplicate effort where possible
* continuously supervise and validate work
* surface progress in a clean control view

---

## 5. The hard truth about your current assumptions

## Correct idea

Your intuition about local orchestration is strong.

The best part of your idea is:

* leverage tools users already pay for
* avoid forcing new model/API infrastructure
* use terminal-native agents as workers
* make orchestration the product

That part is real.

## Wrong idea

This part is weak:

> “We do not need worktrees because we can just prompt agents not to conflict.”

That is not robust enough.

Why it breaks:

* agents forget instructions
* agents race on the same files
* agents overwrite newer edits
* agents misread repository state
* agents contradict each other even with good prompts
* validation becomes expensive after the damage is already done

So no, prompt-only coordination is **not enough** as the primary safety model.

## Better position

Do **not** force Git worktrees by default.
But also do **not** rely on prompt discipline alone.

Use a **lightweight coordination model**:

* task/file ownership leases
* change-intent registry
* edit reservations
* pre-write freshness checks
* supervisor conflict detection
* optional isolated mode for risky tasks

That keeps it fast without being stupid.

---

## 6. Product principles

## Principle 1 — No orchestration API key

The orchestration layer should run without its own model billing path.

It should operate by controlling already-installed AI CLIs.

## Principle 2 — Terminal-first, not dashboard-first

The terminal is the product surface.
Do not build a bloated web app first.

## Principle 3 — Supervision is the moat

Launching many agents is easy.
Supervising them well is the actual product.

## Principle 4 — Fast by default

Low ceremony, instant launch, minimal config.

## Principle 5 — Structure beats hype

The system must convert vague “go fix everything” requests into explicit work partitions.

## Principle 6 — Validation is mandatory

Every completed task must be challenged, not merely accepted.

## Principle 7 — Human remains the final authority

The supervisor system proposes, checks, escalates, and organizes.
The human decides final acceptance.

---

## 7. Product architecture

## 7.1 High-level system

```text
User
  ↓
sp CLI
  ↓
Orchestrator Runtime
  ├─ Session Launcher
  ├─ Terminal Controller
  ├─ Task Planner
  ├─ Supervisor Engine
  ├─ Validation Engine
  ├─ Progress/Event Store
  ├─ Conflict Guard
  └─ Result Aggregator
       ↓
Existing Agent CLI Sessions
(Claude Code / Codex / Qwen / etc.)
```

---

## 7.2 Main components

## A. Session Launcher

Responsible for:

* opening N agent sessions
* attaching to terminal multiplexer / terminal app
* creating worker sessions + one supervisor session
* naming sessions deterministically

Example labels:

* `supervisor-01`
* `worker-ui-01`
* `worker-tests-02`
* `worker-debug-03`

---

## B. Terminal Controller

Responsible for:

* sending prompts to specific sessions
* reading output streams
* tracking active/inactive state
* capturing completion markers
* detecting stalls / hangs / no-progress states

This is the mechanical core.

Without this, the product is fake.

---

## C. Task Planner

Responsible for converting one large mission into:

* atomic workstreams
* execution dependencies
* ownership boundaries
* validation steps
* escalation rules

Example decomposition of:

> “fix UI, debug, test, validate, performance, refactor, ship”

into:

* workstream 1: reproduce current failures
* workstream 2: UI/UX audit and cleanup
* workstream 3: performance profiling
* workstream 4: bug isolation
* workstream 5: test coverage expansion
* workstream 6: validation and contradiction checks
* workstream 7: final synthesis and ship-readiness review

---

## D. Supervisor Engine

This is the actual product moat.

The supervisor does not merely launch workers.

It must:

* assign tasks
* watch agent outputs
* ask follow-up questions
* redirect failing workers
* detect shallow work
* demand proof
* demand validation
* escalate if agents stall or contradict themselves
* reissue refined prompts when needed

This is not passive monitoring.
This is active management.

---

## E. Validation Engine

Every worker output should go through a structured challenge loop.

Validation prompts must check:

* did you actually make the claimed change
* what files changed
* what proof exists
* what test was run
* what failed
* what remains risky
* what assumptions were made
* what could break from this change

This should be automatic.

Workers should not be trusted by default.

---

## F. Conflict Guard

Since you do not want worktrees by default, you need something lighter.

Minimum viable coordination:

* file ownership lease table
* task intent declarations
* edit reservation warnings
* freshness checks before write
* supervisor alerts on overlapping edits
* “respect newer changes” rebasing prompt logic
* optional lock override by supervisor

This is much better than “just hope they don’t collide.”

---

## G. Result Aggregator

Responsible for:

* collecting worker summaries
* merging status into one mission view
* identifying completed / blocked / failed work
* surfacing contradictions
* producing final synthesis for human review

---

## 8. User experience

## 8.1 Primary command

```bash
sp <agent> <count> --repo <path> --mission "<goal>"
```

Example:

```bash
sp claude 12 --repo . --mission "Fix UI/UX, debug broken flows, expand testing, validate changes, review performance, and prepare ship-ready output."
```

---

## 8.2 Expected flow

### Step 1 — Launch

`sp` launches 12 worker sessions and 1 supervisor session.

### Step 2 — Plan

The planner decomposes the mission into workstreams.

### Step 3 — Assign

Workers receive scoped instructions.

### Step 4 — Supervise

Supervisor monitors all workers continuously.

### Step 5 — Validate

Completed worker outputs are challenged automatically.

### Step 6 — Escalate

Weak or incomplete work is sent back.

### Step 7 — Aggregate

System produces one clean mission summary.

---

## 8.3 UX surfaces

## Surface A — Terminal grid / multiplexer view

You want visible terminal activity. Fine. That matters.

Support:

* tmux first
* Ghostty/iTerm integration later if needed
* pane labels
* status badges
* active task summaries
* stall indicators

## Surface B — Control view

A clean single-pane overview showing:

* session count
* worker status
* current tasks
* conflicts
* validation state
* failures
* completion summary

Do not make this pretty first. Make it legible first.

---

## 9. Supervisor behavior model

The supervisor needs explicit operational rules.

## 9.1 Supervisor responsibilities

* assign work
* monitor progress
* check if output matches task
* ask follow-up questions
* force validation
* reassign work when a worker fails
* detect redundancy
* detect contradictions
* prevent silent fake completion

## 9.2 Supervisor loop

For each worker:

1. inspect latest output
2. classify state:

   * progressing
   * blocked
   * stalled
   * done-claimed
   * contradictory
3. choose action:

   * continue
   * clarify
   * validate
   * redirect
   * escalate
   * terminate / replace
4. log event
5. update mission graph

---

## 10. Validation model

Every “done” claim triggers challenge questions.

## Required validation checks

* what exactly changed
* which files were touched
* what test or command was executed
* what was the result
* what remains unverified
* what risks still exist
* did you modify files outside your scope
* did you observe overlapping edits from other workers

## Validation result classes

* validated
* partially validated
* unproven
* contradicted
* failed

Only `validated` should count as complete.

---

## 11. Concurrency and conflict model

This is where most orchestration ideas die.

## 11.1 Default model

Fast shared-repo mode with coordination guards.

### Rules

* every worker declares intended files
* supervisor tracks ownership lease
* overlapping file claims trigger warning
* before write, worker must re-check file freshness
* if another worker changed the file, supervisor decides:

  * continue
  * merge carefully
  * re-scope
  * reassign

## 11.2 Optional isolated mode

For risky refactors:

```bash
sp claude 8 --isolation safe
```

This can use:

* branch-per-worker
* temp copies
* patch-based staging
* optional worktree mode

You do not want this as default. Fine.
But you absolutely need it as an option.

---

## 12. Supported execution modes

## Mode 1 — Shared Fast Mode

* fastest
* least overhead
* guarded shared-repo editing
* good for broad parallel work

## Mode 2 — Validation Heavy Mode

* slower
* more challenge loops
* ideal for critical refactors and bug fixing

## Mode 3 — Safe Isolation Mode

* for risky structural changes
* more overhead
* less collision risk

## Mode 4 — Review Swarm Mode

* workers do not edit
* workers inspect, critique, test, validate, benchmark

---

## 13. Initial scope

## v1 should do only this

* launch many sessions
* assign structured tasks
* supervisor follow-up loop
* progress tracking
* validation loop
* terminal grid compatibility
* clean mission summary

That is enough for v1.

## v1 should not do this

* full enterprise SaaS
* cloud collaboration
* fancy telemetry platform
* web dashboard obsession
* autonomous code merging
* multi-machine orchestration
* complex memory graphs
* “AI operating system” nonsense

Stay disciplined.

---

## 14. Rust implementation plan

## Why Rust

Rust is correct here because you want:

* speed
* terminal control reliability
* concurrency
* strong process management
* low overhead
* single-binary distribution

That is real. Rust fits.

---

## 15. Technical implementation plan

# Phase 1 — Foundation

## Goal

Create a local orchestrator that can launch and control many agent CLI sessions.

## Build

* Rust CLI app
* command parsing via `clap`
* async runtime via `tokio`
* PTY/process management
* event bus
* structured logging
* local state store

## Deliverables

* `sp claude 4 --mission "..."`
* session launcher
* session registry
* prompt sender
* output collector
* basic status tracker

---

# Phase 2 — Terminal integration

## Goal

Make multi-session local orchestration usable in the real world.

## Build

* tmux integration first
* pane/session naming
* attach/detach flows
* output tailing
* focused control commands

## Deliverables

* open visible terminal grid
* send prompts to all / some / one
* watch outputs live
* restart crashed sessions

---

# Phase 3 — Planning and task distribution

## Goal

Turn one vague mission into structured worker assignments.

## Build

* mission parser
* workstream generator
* worker role allocator
* dependency awareness
* task registry

## Deliverables

* mission → task tree
* automatic assignment
* clear ownership boundaries
* no duplicate worker spam by default

---

# Phase 4 — Supervisor engine

## Goal

Build the actual moat.

## Build

* session watcher
* state classifier
* auto-follow-up prompts
* blocked/stall detection
* contradiction detection
* escalation rules

## Deliverables

* active supervision loop
* auto-questioning
* recovery from weak outputs
* “prove it” prompts for done claims

---

# Phase 5 — Validation engine

## Goal

Stop fake completion.

## Build

* validation templates
* proof requests
* command/test capture
* risk extraction
* quality scoring

## Deliverables

* every completed task challenged
* validated vs unvalidated classification
* final confidence score per task

---

# Phase 6 — Conflict guard

## Goal

Make shared-repo parallelism usable without stupid collisions.

## Build

* intent declarations
* file claim leases
* freshness checks
* supervisor collision alerts
* override / reassign flow

## Deliverables

* low-overhead coordination
* fewer destructive conflicts
* faster than full isolation

---

# Phase 7 — Mission aggregation and UX polish

## Goal

Give the user one clean operational picture.

## Build

* mission summary view
* worker scoreboard
* blocked/completed grouping
* contradiction surfacing
* final output synthesis

## Deliverables

* human-readable control view
* final execution report
* session health + results summary

---

## 16. Command design

## Launch

```bash
sp claude 12 --repo . --mission "..."
```

## Launch with named mission

```bash
sp claude 12 --repo . --mission-file mission.md
```

## Status

```bash
sp status
```

## Watch supervisor

```bash
sp watch
```

## Send follow-up to one worker

```bash
sp ask worker-04 "Show exactly what changed and validate it."
```

## Force validation

```bash
sp validate worker-04
```

## Reassign task

```bash
sp reassign worker-04 worker-09
```

## Kill stalled worker

```bash
sp stop worker-07
```

## Relaunch replacement

```bash
sp restart worker-07
```

---

## 17. Internal data model

## Core entities

* Mission
* Workstream
* WorkerSession
* SupervisorSession
* FileLease
* ValidationRecord
* Event
* TaskState

## Minimum persisted state

* session ids
* agent type
* assigned role
* task summary
* last output timestamp
* state classification
* claimed files
* validation state
* result summary

Use SQLite locally.
Do not overcomplicate this.

---

## 18. What makes this product genuinely valuable

## Real value

* removes terminal babysitting
* turns paid agent CLIs into a coordinated execution team
* increases throughput on broad engineering tasks
* keeps user in local environment
* avoids new API/key friction
* makes supervision operational instead of manual

## Fake value

* “AI colony”
* “recursive intelligence”
* “autonomous software civilization”
* any grandiose nonsense not backed by the product loop

Stay grounded.

---

## 19. Biggest risks

## Risk 1 — Terminal control brittleness

Different CLIs and terminal apps behave differently.

### Fix

Start narrow:

* tmux first
* Claude Code first
* one supported terminal path first

---

## Risk 2 — Worker collision in shared repo

Your no-worktree stance increases collision risk.

### Fix

Use coordination guards, not just prompt instructions.

---

## Risk 3 — Supervisor becoming dumb spam

If the supervisor just asks generic follow-ups, it becomes noise.

### Fix

Make follow-ups state-aware and evidence-driven.

---

## Risk 4 — Too much ambition in v1

If you try to build “enterprise AI factory” on day one, you will ship nothing.

### Fix

Build the launch + supervise + validate loop first.

---

## Risk 5 — CLI-specific incompatibility

Different agent CLIs expose different behaviors.

### Fix

Use adapter architecture.

---

## 20. Adapter model

Each supported agent CLI should have an adapter:

* launch command
* prompt injection method
* output parsing rules
* completion markers
* failure markers
* interrupt behavior
* retry behavior

Example adapters:

* `ClaudeCodeAdapter`
* `CodexAdapter`
* `QwenAdapter`

This is mandatory.
Without adapters, multi-agent support becomes brittle garbage.

---

## 21. Recommended v1 scope

## Start here

* Rust CLI
* tmux integration
* Claude Code only
* one supervisor
* N workers
* mission decomposition
* follow-up prompts
* validation prompts
* status summary

That is enough to prove the product.

## Do not start with

* 10 providers
* complex GUI
* cloud sync
* marketplace
* plugin ecosystem
* multi-repo federation

That is dilution.

---

## 22. Product positioning

## One-line positioning

**A terminal-first orchestration layer that turns existing AI coding CLIs into a supervised execution team.**

## Short positioning

**Launch, supervise, validate, and coordinate many local AI agent sessions from one fast Rust CLI.**

## Stronger positioning

**`sp` is not another coding agent. It is the control plane for agentic software execution in your terminal.**

---

## 23. Build order I would recommend

```text
1. Claude Code only
2. tmux-based launcher
3. worker registry
4. supervisor session
5. mission decomposition
6. follow-up automation
7. validation automation
8. file ownership leases
9. clean control view
10. adapter system for more CLIs
```

That order is brutal but correct.

---------------------------------------------------------------------------------------------