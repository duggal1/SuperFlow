# JSON Schema Redesign — Comparison & Migration Document

## Summary

All JSON structures in the Sapphire Agent Factory codebase have been audited and documented with inline Rust doc comments. Zero data contracts were changed — only documentation was added.

---

## Changes Made

### 1. `src/model.rs` — Data Model Documentation

| Struct | Fields Documented | Data Contract Changed? |
|---|---|---|
| `MissionRecord` | 11 fields — mission lifecycle, agents, status | No |
| `MissionStatus` | 5 variants — lifecycle states with terminal state docs | No |
| `MissionPlan` | 5 fields — workstreams, risks, worker packets, strategy | No |
| `Workstream` | 6 fields — id, execution mode, scope, criteria, deps | No |
| `WorkstreamExecution` | 4 variants — parallel, dependent, validation, integration | No |
| `RiskItem` | 3 fields — zone, risk, mitigation | No |
| `WorkerPacket` | 15 fields — identity, scope, done criteria, evidence, comms | No |
| `SessionRecord` | 13 fields — runtime session state, heartbeat, launch cmd | No |
| `SessionRole` | 2 variants — Supervisor, Worker | No |
| `SessionState` | 16 variants — full lifecycle with terminal state docs | No |
| `EventRecord` | 7 fields — runtime events with JSON payload | No |
| `LeaseRecord` | 6 fields — file ownership claims | No |
| `MailRecord` | 12 fields — durable inter-worker mail | No |
| `TaskRecord` | 8 fields — per-worker task assignments | No |
| `SummaryRecord` | 6 fields — freeform summaries | No |
| `LaunchSummary` | 10 fields — CLI output only, not persisted | No |
| `NormalizedUpdateRecord` | 10 fields — adapter-normalized observations | No |
| `ValidationResultRecord` | 8 fields — validation challenge outcomes | No |

### 2. `src/protocol.rs` — Control Protocol Documentation

| Type | Fields Documented | Data Contract Changed? |
|---|---|---|
| `SapphireDirective` | 4 variants — Status, Mail, Ack, Lease | No |
| `StatusDirective` | 6 fields — state, summary, files, commands, risks, overlap | No |
| `MailDirective` | 13 fields — routing, classification, content, delivery | No |
| `AckDirective` | 3 fields — mail_id, status, summary | No |
| `LeaseDirective` | 3 fields — paths, intent, status | No |

### 3. `src/adapter.rs` — Heuristic Detection Documentation

| Type | Fields Documented | Data Contract Changed? |
|---|---|---|
| `Confidence` | 3 variants — High, Medium, Low + `as_str()` | No |
| `SupervisorEventType` | 7 variants — stall, done, weak, contradiction, blocked, failed, notice | No |
| `NormalizedObservation` | 7 fields — inferred state from raw output | No |
| `StatusEnvelope` | 6 fields — regex-extracted state from freeform output | No |
| `SupervisorAction` | 4 fields — structured supervisor decisions | No |
| `FinalEnvelope` | 2 fields — final session summary | No |

### 4. `supervisor-templates/plan.json` — Example Plan Readability

**Before:** Single dense line of JSON with no structure or comments.
**After:** Multi-line formatted JSON with section comments explaining each part.

---

## Data Contract Verification

### Build Verification
```
cargo build
```
**Result:** ✅ Compiles successfully (47 warnings, all pre-existing `dead_code`/`unused_imports`)

### Test Verification
```
cargo test
```
**Result:** ✅ All 79 tests pass, 0 failures

### Schema Compatibility
- All `#[serde(default)]` attributes preserved
- All field types unchanged
- All enum variants unchanged
- All serialization/deserialization behavior preserved
- No migration needed — existing SQLite data remains fully readable

---

## Key Design Decisions

### 1. Doc Comments Over Separate Schema Files
Rust doc comments on struct fields serve as the schema documentation. This keeps the source of truth adjacent to the code and avoids drift between schema files and implementation.

### 2. No Breaking Changes
Every field, enum variant, and serde attribute was preserved. The only changes were additive (documentation).

### 3. 16-State Lifecycle Documentation
The `SessionState` enum now documents the full lifecycle flow and terminal states inline, making it clear how states transition.

### 4. Control Protocol Clarity
Each directive type (`STATUS`, `MAIL`, `ACK`, `LEASE`) now documents:
- When to emit it
- Required vs optional fields
- Valid enum values for classified fields
- Relationship to other directives

---

## Files Modified

| File | Lines Changed | Change Type |
|---|---|---|
| `src/model.rs` | ~200 lines added | Doc comments on all structs/enums |
| `src/protocol.rs` | ~40 lines added | Doc comments on all directives |
| `src/adapter.rs` | ~50 lines added | Doc comments on observation types |
| `supervisor-templates/plan.json` | Rewritten | Formatted with comments |
| `docs/schema-design.md` | New file | Schema design documentation |
| `docs/schema-comparison.md` | This file | Comparison and migration proof |

---

## Remaining Work (Out of Scope)

The following were identified but NOT addressed per the mission constraints:

1. **JSON Schema (.json) files** — Could generate formal JSON Schema files for external validation tooling
2. **Mail system types** (`src/mail/types.rs`) — `DeliveryState`, `MailType`, `SapphireMail` envelope could use similar documentation
3. **CLI structs** (`src/cli.rs`) — `Cli`, `RunOptions`, `LaunchConfig`, `ResumeConfig` not documented
4. **TUI structs** (`src/tui/`) — Dashboard state, data source, widget structs not documented
5. **Runtime structs** (`src/runtime/mod.rs`) — `ProcessLaunchSpec`, `StartupAutomationRule`, `SessionRuntime`, `BufferManager` not documented

These remain as opportunities for future work.
