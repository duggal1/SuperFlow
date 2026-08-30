# Sapphire Communication Fabric

Agents communicate through a **supervised communication fabric** controlled by Sapphire.

Architecture:

* **Engineers** execute scoped assignments
* **Supervisor** owns global authority and conflict resolution
* **Mail system** enables engineer-to-engineer coordination via `SAPPHIRE_MAIL`
* **Watchdog** observes every message and every state transition
* **SQLite** holds durable outputs, decisions, and communication history

That is how Engineer-1 and Designer-2 coordinate safely.

---

## Communication Model

### Direct Engineer-to-Engineer

An engineer sends another a structured coordination request such as:

* need interface confirmation
* need file ownership clarification
* need output from your module
* warning: my change may affect your scope
* review this before I proceed
* blocker: dependency on your result

Use a strict schema. Free-form chat is not the default.

Example:

```markdown
FROM: Engineer-1 (software-engineer)
TO: Designer-2 (designer-engineer)
TYPE: dependency_request
PRIORITY: high
SUBJECT: confirm API response shape before UI integration

CONTEXT:
I am implementing the UI consumer for the settings panel.

REQUEST:
Confirm the final response shape and any field renames.

NEEDED BY:
before final integration

IMPACT IF WRONG:
UI contract mismatch and broken rendering
```

---

### Supervisor-Visible Only

Every inter-agent message must be visible to the supervisor and watchdog.

Why?

Because engineer-to-engineer communication can:

* drift scope
* create side agreements
* hide contradictions
* propagate wrong assumptions
* cause silent architecture divergence

Agents talk. But they talk through a **supervised channel**, not a secret one.

---

### Durable, Not Ephemeral

Do not rely on terminal scrollback as the communication layer.

Sapphire uses durable messaging with:

* message id
* sender (display_name + role_type)
* recipient (display_name)
* timestamp
* message type
* status
* ack state
* optional artifact links

All mail is persisted to SQLite before injection. If a session crashes or stalls, the coordination history survives.

---

## Message Types

### Engineer-to-Engineer

Used for peer coordination:

* `dependency_request`
* `dependency_response`
* `review_request`
* `review_response`
* `handoff`
* `collision_warning`
* `completion_notice`

### Engineer-to-Supervisor

Used for:

* `blocker`
* `architecture_concern`
* `escalation`
* `completion_claim`
* `contradiction_report`

### Supervisor-to-Engineer

Used for:

* assignments and corrections
* conflict rulings
* validation challenges
* reroutes and retries
* priority shifts

---

## Communication Flow Example

Scenario: Designer-2 depends on Engineer-1's API contract.

1. **Designer-2** sends a structured dependency request to Engineer-1.
2. **Watchdog** records it in SQLite and marks Designer-2 as dependency-waiting.
3. **Engineer-1** replies with current contract, pending changes, and exact risk if unstable.
4. **Supervisor** observes the exchange. If the contract is sound, work continues. If Engineer-1 is unstable, the supervisor intervenes.

---

## Supervisor Responsibilities

The supervisor is never bypassed by engineer communication. It becomes **more important** when communication exists.

1. **Observe all communications** — read every message or summary.
2. **Detect bad coordination** — two engineers redefining ownership informally, wrong architecture guidance, scope drift, hidden dependency chains, weak peer advice.
3. **Rule on conflicts** — if Engineer-1 and Designer-2 disagree, the supervisor decides, overrides, re-scopes, or assigns a reviewer.
4. **Force explicit handoffs** — require exact interface, exact file, exact dependency, exact expectation. No "I think it should be fine."

---

## Design Principle

Real teams communicate. But real high-functioning teams do **not** operate as unstructured everyone-talks-to-everyone chaos. That is not senior engineering. That is entropy.

The correct model is **bounded peer coordination under a central technical authority**.

---

## Mail Schema

Each `SAPPHIRE_MAIL` directive carries:

* `mail_id` — unique identifier
* `reply_to` — optional thread linkage
* `to` — recipient display name (e.g. `Engineer-2`, `Supervisor`)
* `from` — sender display name (inferred by watchdog)
* `message_type` — one of the types listed above
* `priority` — `low`, `medium`, `high`
* `subject` — one short sentence
* `context` — background and current state
* `request` — the concrete ask
* `expected_action` — what the recipient should do
* `requires_ack` — whether acknowledgment is required

Ack timeouts trigger escalation: the watchdog probes both sender and recipient, then notifies the supervisor if no resolution occurs.

---

## Watchdog Classification

The watchdog monitors communication health and classifies:

* `healthy` — productive coordination with shipping progress
* `dependency_wait` — one agent blocked awaiting peer response
* `excessive_chatter` — over-communicating without shipping
* `contradiction_risk` — messages suggest scope drift or conflict
* `blocked_on_peer` — dependency delay is blocking progress
* `supervisor_intervention_needed` — escalation required

---

## Practical Rules

### Engineers may communicate directly when:

* they have a concrete dependency
* they need a contract clarification
* they need peer review
* they must warn about a collision

### Engineers must escalate to supervisor when:

* architecture disagreement exists
* ownership is unclear
* contradiction affects repo coherence
* another agent is clearly wrong
* peer response is missing or low quality
* dependency delay is blocking progress

### Engineers must not:

* silently redefine scope
* override supervisor rulings
* negotiate hidden ownership changes
* issue vague chatty messages
* spam coordination without action

---

## The Answer

How do agents coordinate?

Through a **Sapphire-controlled structured mail layer**, visible to the supervisor, tracked by the watchdog, and bounded by ownership rules.

Not by pretending agents natively support teamwork. Not by letting terminals freestyle chat. Not by relying only on manager-style reporting.

That gives you:

* real peer coordination
* real collaboration
* real dependency handling
* real supervisor authority
* much lower chaos
