# Schema Design Document

## Purpose

Replace dense, developer-unfriendly JSON structures with clear, documented, and readable alternatives while preserving all existing data contracts.

---

## 1. Control Protocol Directives

### Current (Dense)
```json
{"state":"progressing","summary":"working","files":["src/main.rs"],"commands":[],"risks":[]}
```

### Redesigned (Documented)
```yaml
# SAPPHIRE_STATUS — Worker state update
# Emit this line when your state changes: starting, blocked, finishing, validating, or failing
SAPPHIRE_STATUS {
  "state": "progressing",           # Required: One of the 16 lifecycle states
  "summary": "Refactoring lease conflict resolution",  # Required: What you're doing
  "files": ["src/orchestrator/mod.rs"],  # Optional: Files you touched
  "commands": ["cargo test"],       # Optional: Commands you ran
  "risks": ["May conflict with Architect's work"],  # Optional: Risks identified
  "overlap": "src/mail/mod.rs"      # Optional: Potential file conflicts with other workers
}
```

### State Enum (16-State Lifecycle)
```
Terminal states: Validated, Failed, Exited

Planned → Booting → NotStarted → Progressing → Blocked → Stalled 
→ DoneClaimed → NeedsValidation → WeakOutput → WrongDirection 
→ Contradictory → NeedsRetry → Validated → Failed → Exited
```

---

## 2. Mail Directive

### Current (Flat)
```json
{"to":"Engineer-2","message_type":"dependency_request","priority":"high","subject":"confirm contract","context":"need response shape","request":"share fields","expected_action":"reply"}
```

### Redesigned (Structured)
```yaml
# SAPPHIRE_MAIL — Inter-worker communication
# Use for: dependency requests, reviews, blockers, handoffs, architecture concerns
SAPPHIRE_MAIL {
  # Routing
  "to": "Engineer-2",               # Required: Target worker display name
  "cc": ["Supervisor"],             # Optional: Additional recipients
  "reply_to": "mail-abc123",        # Optional: Thread parent ID
  "thread_id": "thread-xyz789",     # Optional: Conversation thread ID
  
  # Message classification
  "message_type": "blocker",        # Required: One of the message types (see below)
  "priority": "high",               # Required: urgent | high | normal | low
  "subject": "Lease conflict on src/main.rs",  # Required: Brief subject
  
  # Content
  "context": "You claimed src/main.rs but I need to edit it for task X",  # Context
  "request": "Please release the lease or coordinate scope",              # What you need
  "expected_action": "release_lease",  # Required: What action you expect
  
  # Delivery
  "requires_ack": true             # Optional: Does this require acknowledgment?
}
```

### Message Types
| Type | When to Use |
|---|---|
| `dependency_request` | You need output from another worker |
| `dependency_response` | Replying to a dependency request |
| `review_request` | Ask another worker to review your work |
| `review_response` | Providing review feedback |
| `blocker` | You're blocked on another worker |
| `handoff` | Passing work to another worker |
| `collision_warning` | You detected a potential file conflict |
| `architecture_concern` | You spotted a design issue |
| `completion_notice` | You finished a task others depend on |
| `supervisor_directive` | Supervisor issuing a decision |

---

## 3. Ack Directive

### Current
```json
{"mail_id":"m-1","status":"acked","summary":"received and understood"}
```

### Redesigned
```yaml
# SAPPHIRE_ACK — Mail acknowledgment
# Confirms receipt and intent to act on a mail message
SAPPHIRE_ACK {
  "mail_id": "mail-abc123",        # Required: ID of the mail being acknowledged
  "status": "acked",               # Required: acked | done | cannot_comply
  "summary": "Will release lease by EOD"  # Required: What you'll do
}
```

---

## 4. Lease Directive

### Current
```json
{"paths":["src/main.rs"],"intent":"edit","status":"claim"}
```

### Redesigned
```yaml
# SAPPHIRE_LEASE — File ownership claim
# Must emit before editing any file. Release when done.
SAPPHIRE_LEASE {
  "paths": ["src/orchestrator/mod.rs", "src/mail/mod.rs"],  # Required: Files to claim
  "intent": "edit",                # Required: read | edit | review
  "status": "claim"                # Required: claim | release
}
```

---

## 5. Worker Packet (Mission Assignment)

### Current (Dense)
```json
{
  "worker_id": "worker-01",
  "role": "Debug and runtime validation",
  "starting_angle": "Hunt panics, unwrap errors, test gaps",
  "owned_scope": "Tests, error handling, runtime stability",
  "explicit_task": "Fix all failing paths",
  "out_of_scope": "Security and documentation",
  "definition_of_done": ["All tests pass", "No unwraps in hot paths"],
  "required_evidence": ["Test output logs", "Fixed file diffs"],
  "blocker_protocol": "Report blocking bugs with traces",
  "conflict_warning": "Claim files before editing",
  "communication_rules": ["Mail blockers immediately"],
  "validation_standard": ["Tests must pass locally"],
  "expected_output_format": ["Test results summary"]
}
```

### Redesigned (Documented)
```yaml
# Worker Assignment Packet
# Each worker receives one packet scoped to their role
{
  # Identity
  "worker_id": "Engineer-1",         # Display name shown in UI/logs
  "role_type": "software-engineer",  # Stable machine key for template lookup
  "role": "Software Engineer",       # Human-readable title (backward compat)
  
  # Mission Scope
  "starting_angle": "Audit test coverage gaps",    # Where to begin
  "owned_scope": "Unit tests, integration tests, CI pipeline",  # What you own
  "explicit_task": "Achieve 90% test coverage",    # Your primary task
  "out_of_scope": "Security auditing, documentation",  # What NOT to touch
  
  # Completion Criteria
  "definition_of_done": [          # Must-haves for completion
    "All tests pass locally",
    "No skipped test cases",
    "Coverage report generated"
  ],
  "required_evidence": [           # Proof you must provide
    "cargo test output",
    "Coverage percentage report"
  ],
  "expected_output_format": [      # How to format your output
    "Summary of test results",
    "List of fixed issues"
  ],
  
  # Communication
  "blocker_protocol": "Report blockers to Supervisor immediately",
  "conflict_warning": "Claim files via SAPPHIRE_LEASE before editing",
  "communication_rules": [
    "Mail blockers to other workers immediately",
    "Request dependencies via mail"
  ],
  "validation_standard": [         # How your work will be validated
    "Tests must pass locally",
    "No regressions introduced"
  ]
}
```

---

## 6. Mission Plan

### Current (Dense)
See `supervisor-templates/plan.json` — 4 workstreams, 4 workers, risk map in single line.

### Redesigned (Documented)
```yaml
# Mission Plan — Generated by supervisor before launch
{
  # Mission Overview
  "mission_rewrite": "Debug, harden security, document CLI architecture",
  
  # Workstreams — Decomposed mission into parallel/dependent tracks
  "workstreams": [
    {
      "id": "debug",
      "name": "Debug and validate codebase",
      "execution": "parallel",       # parallel | dependent | validation | integration
      "owned_scope": "All source files, tests, runtime correctness",
      "success_criteria": [          # How we know this workstream is done
        "Zero failing tests",
        "No runtime panics",
        "Clean cargo check"
      ],
      "depends_on": []               # Workstream IDs this depends on
    }
  ],
  
  # Risk Map — Known risks and mitigations
  "risk_map": [
    {
      "zone": "Shared files",
      "risk": "Workers edit same modules",
      "mitigation": "Strict lease ownership enforcement"
    }
  ],
  
  # Worker Packets — One per worker (see WorkerPacket schema above)
  "worker_packets": [...],
  
  # Supervision Strategy
  "supervision_strategy": "Monitor leases, validate claims, enforce accuracy"
}
```

---

## 7. Session Record

### Current
```json
{
  "id": "uuid",
  "mission_id": "uuid",
  "role": "Worker",
  "ordinal": 1,
  "agent": "qwen",
  "terminal_id": "term-1",
  "name": "Engineer-1",
  "owned_scope": "...",
  "status": "Progressing",
  "launch_command": ["sp", "qwen", "1", "--repo", "."],
  "last_heartbeat_at": "2024-01-01T00:00:00Z",
  "last_summary": "Working on debug pass"
}
```

### Redesigned
```yaml
# Session Record — Runtime state for a supervisor or worker session
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "mission_id": "550e8400-e29b-41d4-a716-446655440001",
  
  # Role and Identity
  "role": "Worker",                 # Supervisor | Worker
  "ordinal": 1,                     # Launch order
  "agent": "qwen",                  # qwen | codex | claude | forge
  "name": "Engineer-1",             # Display name
  "terminal_id": "term-1",          # PTY terminal identifier
  
  # State
  "owned_scope": "Source files and tests",  # What this session owns
  "status": "Progressing",          # Current 16-state lifecycle value
  "last_summary": "Working on debug pass",  # Latest status summary
  
  # Metadata
  "launch_command": ["sp", "qwen", "1", "--repo", "."],
  "last_heartbeat_at": "2024-01-01T00:00:00Z"
}
```

---

## 8. Mail Record (Persisted)

### Current
```json
{
  "id": "uuid",
  "mission_id": "uuid",
  "sender_worker_id": "uuid",
  "recipient_worker_id": "uuid",
  "message_type": "dependency_request",
  "priority": "high",
  "subject": "confirm contract",
  "status": "pending",
  "ack_state": "pending",
  "body_json": "{\"request\":\"share fields\"}",
  "created_at": "2024-01-01T00:00:00Z"
}
```

### Redesigned
```yaml
# Mail Record — Persisted inter-worker message
{
  "id": "mail-abc123",
  "mission_id": "mission-xyz",
  
  # Routing
  "sender_worker_id": "worker-1-uuid",
  "recipient_worker_id": "worker-2-uuid",
  
  # Classification
  "message_type": "dependency_request",
  "priority": "high",               # urgent | high | normal | low
  "subject": "Confirm API contract",
  
  # Delivery State
  "status": "pending",              # pending | delivered | acked | done | archived
  "ack_state": "pending",           # pending | acked | done | cannot_comply
  
  # Content (structured JSON)
  "body_json": {
    "context": "Need response shape for integration",
    "request": "Share your output fields",
    "expected_action": "Reply with schema"
  },
  
  "created_at": "2024-01-01T00:00:00Z"
}
```

---

## Implementation Plan

### Phase 1: Documentation
- [x] Create this schema design document
- [ ] Add inline comments to all serde structs in Rust source
- [ ] Create JSON Schema files (`.schema.json`) for validation

### Phase 2: Code Updates
- [ ] Add doc comments to all fields in `src/model.rs`
- [ ] Add doc comments to all fields in `src/protocol.rs`
- [ ] Add doc comments to all fields in `src/mail/types.rs`
- [ ] Update `supervisor-templates/plan.json` to readable format

### Phase 3: Verification
- [ ] Run `cargo test` to verify zero data loss
- [ ] Run `cargo build` to verify compilation
- [ ] Verify consumer tests pass
