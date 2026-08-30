# AGENTS.md

Read this file before starting any task. It defines how to navigate, edit, validate, and report work in this repository.

---

## TABLE OF CONTENTS

1. [Product Overview](#1-product-overview)
2. [Tech Stack](#2-tech-stack)
3. [Architecture](#3-architecture)
4. [Repository File Tree](#4-repository-file-tree)
5. [Critical File Index](#5-critical-file-index)
6. [Known Issues & Active Debt](#6-known-issues--active-debt)
7. [Agent Notes](#7-agent-notes)
8. [Operating Protocol](#8-operating-protocol)
9. [Testing & Validation](#9-testing--validation)
10. [Debugging Protocol](#10-debugging-protocol)
11. [Security & Vulnerability Policy](#11-security--vulnerability-policy)
12. [Guardrails](#12-guardrails)

---

## 1. PRODUCT OVERVIEW

<!--
  One paragraph. Answer: what is this product, what problem does it solve, who uses it?
  Write for an engineer with zero prior context. No marketing language.
-->

**Product:** `[Name]` is a `[CLI / API / SaaS / SDK / platform]` that `[core function in one sentence]`.

**Operated by:** `[Internal team / open-source contributors / solo / enterprise customers]`

**Status:** `[Production / Beta / Active development / Deprecated]`

**Entry point:** `[path/to/main/entrypoint]`

---

## 2. TECH STACK

| Layer            | Technology                       | Version / Notes                 |
|------------------|----------------------------------|---------------------------------|
| Language         | `[e.g. Go / TypeScript / Rust]`  | `[version]`                     |
| Runtime          | `[e.g. Node.js / JVM / native]`  | `[version]`                     |
| Framework        | `[e.g. Gin / Next.js / Axum]`    | `[version]`                     |
| Database         | `[e.g. PostgreSQL / SQLite]`     | `[version, ORM if any]`         |
| Cache            | `[e.g. Redis / in-memory]`       | `[version]`                     |
| Queue            | `[e.g. NATS / RabbitMQ / none]`  | `[version]`                     |
| Auth             | `[e.g. JWT / OAuth2 / Clerk]`    | `[provider]`                    |
| Infra / Deploy   | `[e.g. Docker / K8s / Fly.io]`   | `[environment]`                 |
| CI/CD            | `[e.g. GitHub Actions / Circle]` | `[config path]`                 |
| Testing          | `[e.g. Jest / Go test / pytest]` | `[frameworks used]`             |
| Key Dependencies | `[list critical libs]`           | `[version-locked warnings]`     |

**Build:** `[exact command]`
**Run:** `[exact command]`
**Test:** `[exact command]`
**Lint:** `[exact command]`

---

## 3. ARCHITECTURE

### 3.1 System Model

<!--
  Describe the system as it actually operates, not as it was intended.
  Include a diagram for any non-trivial topology.
-->

```
[ASCII diagram of the system]

Example:

  ┌──────────────┐     ┌──────────────┐     ┌──────────────┐
  │   Client /   │────▶│   API Layer  │────▶│   Service    │
  │   CLI / UI   │     │  (routing,   │     │   Layer      │
  └──────────────┘     │   auth, val) │     │  (business   │
                        └──────┬───────┘     │   logic)     │
                               │             └──────┬───────┘
                               │                    │
                        ┌──────▼───────┐     ┌──────▼───────┐
                        │  Middleware  │     │  Data Layer   │
                        │  (logging,  │     │  (DB, cache,  │
                        │   tracing)  │     │   queue)      │
                        └─────────────┘     └──────────────┘
```

### 3.2 Module Boundaries

| Module / Package | Responsibility                  | Owner             |
|------------------|---------------------------------|-------------------|
| `[module]`       | `[one-line description]`        | `[team / person]` |
| `[module]`       | `[one-line description]`        | `[team / person]` |
| `[module]`       | `[one-line description]`        | `[team / person]` |

Cross-module access goes through exported interfaces only. No module imports a sibling module's internal types directly.

### 3.3 Data Flow

<!--
  Trace a request through the system. Reference actual files and functions where stable.
-->

1. Request enters at `[entry point — file:line]`
2. Passes through `[middleware chain — file]`
3. Dispatched to `[router / handler — file]`
4. Business logic runs in `[service layer — file]`
5. Persistence handled by `[repository / store — file]`
6. Response serialized and returned at `[file:line]`

### 3.4 Concurrency & State

- **Model:** `[goroutines / async-await / thread pool / single-threaded / actor model]`
- **Shared state:** `[what is shared — name the structs or objects]`
- **Synchronization:** `[mutex / channel / lock-free / event loop]`
- **Side effects:** `[where permitted / where forbidden]`

### 3.5 External Integrations

| Integration  | Direction        | Protocol         | Auth                | Config        |
|--------------|------------------|------------------|---------------------|---------------|
| `[service]`  | Inbound/Outbound | REST / gRPC / WS | `[API key / OAuth]` | `[file path]` |
| `[service]`  | Inbound/Outbound | REST / gRPC / WS | `[API key / OAuth]` | `[file path]` |

---

## 4. REPOSITORY FILE TREE

Canonical structure. Excludes generated files, vendor directories, and test fixtures unless architecturally significant. In a monorepo, each top-level directory is an independent domain.

```
/
├── AGENTS.md                          # Operating protocol for all agents
├── README.md                          # Public-facing overview
├── Makefile                           # Build, run, test, lint targets
├── .env.example                       # Required env vars — no secrets
├── .github/
│   └── workflows/
│       ├── ci.yml                     # CI — runs on PR
│       └── deploy.yml                 # Deploy — runs on merge to main
│
├── cmd/                               # Entrypoints only. No business logic.
│   ├── server/
│   │   └── main.go                    # HTTP server entrypoint
│   └── worker/
│       └── main.go                    # Background worker entrypoint
│
├── internal/                          # Private application code
│   ├── api/                           # Routing, handlers, request/response types
│   │   ├── router.go                  # Route registration
│   │   ├── middleware.go              # Auth, logging, rate-limiting
│   │   └── handlers/
│   │       ├── [resource]_handler.go  # One file per resource
│   │       └── ...
│   │
│   ├── service/                       # Business logic — no HTTP, no DB
│   │   ├── [domain]_service.go        # One file per domain
│   │   └── ...
│   │
│   ├── store/                         # Data access — DB queries only
│   │   ├── [domain]_store.go          # One file per domain
│   │   ├── db.go                      # DB connection and pool config
│   │   └── migrations/
│   │       └── *.sql                  # Ordered migration files
│   │
│   ├── model/                         # Shared domain types — no logic
│   │   └── [domain].go
│   │
│   ├── config/
│   │   └── config.go                  # Env loading, validation, defaults
│   │
│   └── pkg/                           # Shared utilities — no domain knowledge
│       ├── logger/
│       ├── errors/
│       └── ...
│
├── pkg/                               # Externally importable packages (if any)
│   └── [package]/
│
├── tests/
│   ├── unit/                          # Unit tests — mirror internal/ structure
│   ├── integration/                   # Integration tests — real DB, real network
│   └── e2e/                           # End-to-end — full system running
│
├── scripts/
│   ├── seed.sh                        # DB seeding
│   └── migrate.sh                     # Migration runner
│
├── docs/
│   ├── api/                           # API spec (OpenAPI / Protobuf / manual)
│   └── adr/                           # Architecture Decision Records
│
└── deployments/
    ├── docker-compose.yml             # Local dev environment
    ├── Dockerfile                     # Production image
    └── k8s/                           # Kubernetes manifests (if applicable)
```

If a task touches `api/`, check `service/` and `store/` before writing anything. Changes in this codebase rarely live in a single layer.

---

## 5. CRITICAL FILE INDEX

Files where a mistake has system-wide impact. Understand them before touching them.

| File                             | Purpose                                                               | Risk     |
|----------------------------------|-----------------------------------------------------------------------|----------|
| `cmd/server/main.go`             | Bootstraps the entire server. All dependencies are wired here.        | CRITICAL |
| `internal/config/config.go`      | Validates all env vars at startup. Bad config terminates the process. | CRITICAL |
| `internal/api/router.go`         | All route registrations. A missing route is a 404 in production.      | HIGH     |
| `internal/api/middleware.go`     | Auth enforcement. A bug here is a security issue.                     | CRITICAL |
| `internal/store/db.go`           | DB pool config. Misconfiguration breaks all queries.                  | CRITICAL |
| `internal/model/[domain].go`     | Canonical types. Every layer depends on these.                        | HIGH     |
| `Makefile`                       | All runnable commands. Broken targets break CI.                       | HIGH     |
| `.github/workflows/ci.yml`       | CI definition. A broken pipeline means blind merges.                  | HIGH     |
| `deployments/Dockerfile`         | Production image definition.                                          | HIGH     |
| `internal/store/migrations/`     | DB schema history. Order is permanent. Never reorder. Never delete.   | CRITICAL |

---

## 6. KNOWN ISSUES & ACTIVE DEBT

Confirmed, unresolved problems. Not speculation. If you fix one, remove it. If you find one, add it.

### 6.1 Active Bugs

| ID      | Severity | Location                     | Description                          | Status      |
|---------|----------|------------------------------|--------------------------------------|-------------|
| BUG-001 | HIGH     | `internal/[file]:[line]`     | `[Exact description of the failure]` | Open        |
| BUG-002 | MEDIUM   | `internal/[file]:[line]`     | `[Exact description]`                | In progress |

### 6.2 Technical Debt

| ID       | Area       | Description                   | Impact if unaddressed           |
|----------|------------|-------------------------------|---------------------------------|
| DEBT-001 | `[module]` | `[What was cut and why]`      | `[Consequence if left to grow]` |
| DEBT-002 | `[module]` | `[What was cut and why]`      | `[Consequence if left to grow]` |

### 6.3 Active Workarounds

Code that exists only as a temporary fix. Mark in source with `// WORKAROUND: [reason]`.

| Location                 | Description                              | Intended Fix           |
|--------------------------|------------------------------------------|------------------------|
| `internal/[file]:[line]` | `[What it does and why it exists]`       | `[Planned resolution]` |

---

## 7. AGENT NOTES

Written for agents operating in this codebase. Read before starting any task.

### 7.1 What Works Well Here

- **Small, targeted edits.** The codebase is large. Changes outside intended scope cause failures that are hard to trace.
- **Layer discipline.** Each layer has a defined contract. Handlers do not query databases. Services do not parse HTTP. Stores do not contain business logic.
- **Reading the source.** Do not infer behavior from filenames or structure alone. Read the relevant code.

### 7.2 Where Agents Commonly Make Mistakes

- **Editing shared models without tracing dependents.** Model changes affect every layer. Before editing any file in `internal/model/`, run `rg "[TypeName]" --type go` and check every usage.
- **Adding DB columns without a migration.** Schema changes in code with no corresponding migration cause drift in production.
- **Adding env vars without registering them.** All env configuration is validated at startup in `internal/config/config.go`. An unregistered var will be silently empty in production.
- **Writing logic in the wrong layer.** If you are writing SQL in a handler or HTTP logic in a service, the code belongs elsewhere.
- **Swallowing errors.** Errors are propagated explicitly throughout this codebase. Using `_` for error returns in production paths is a defect.

### 7.3 Conventions

- **File naming:** `[domain]_[layer].go` — e.g., `user_service.go`, `order_handler.go`
- **Error wrapping:** `fmt.Errorf("context: %w", err)` — never return a raw error from an internal layer
- **Logging:** Use the structured logger at `internal/pkg/logger` — no `fmt.Println` or `log.Println` in production paths
- **Constants:** All magic values go in `internal/model/constants.go` — no inline string literals or magic numbers in logic
- **Comments:** Write comments only for non-obvious decisions — do not narrate the code

### 7.4 Starting a Task

1. Read this file.
2. Find relevant files using `rg` — do not assume locations.
3. Confirm which layer the change belongs to.
4. Check section 6 (Known Issues) — the problem may already be tracked.
5. Make the change, run validation, report what changed.

---

## 8. OPERATING PROTOCOL

### 8.1 Navigation

```sh
# Find all files containing a symbol
rg "FunctionName" --type go

# Find files by name
rg --files | rg "keyword"

# Find all usages of a type
rg "TypeName" -l

# List files in a directory
find ./internal -type f -name "*.go" | sort
```

### 8.2 Edit Rules

| Rule                           | Detail                                                                  |
|--------------------------------|-------------------------------------------------------------------------|
| ASCII-only by default          | Non-ASCII only where the file already uses it and it is clearly needed  |
| Minimal diff                   | Change only what the task requires                                      |
| Style-consistent               | Match the surrounding code exactly — no unrelated reformatting          |
| No speculative cleanup         | Do not fix things noticed but not in scope                              |
| Root cause only                | Fix the cause, not the symptom                                          |
| Comments only when non-obvious | If the code is readable, no comment is needed                           |

### 8.3 Git Protocol

| Action                    | Policy                                 |
|---------------------------|----------------------------------------|
| Check worktree state      | Run `git status` before starting       |
| Commit                    | Only when explicitly requested         |
| Branch creation           | Only when explicitly requested         |
| `git reset --hard`        | Only when explicitly requested         |
| `git checkout -- [file]`  | Only when explicitly requested         |
| Amend commits             | Only when explicitly requested         |
| Revert user changes       | Never                                  |

### 8.4 Planning

Use a plan when the task touches multiple files or systems.

```
PLAN
[ ] Step 1 — [description]
[ ] Step 2 — [description]
[ ] Step 3 — [description]
```

- One step in progress at a time.
- Update the plan as steps complete.
- If scope changes, update the plan before continuing.
- Do not plan for single-file, clearly scoped tasks.

### 8.5 Autonomy

- If the task is clear and implementation is expected — implement.
- If the task is ambiguous — resolve what can be resolved, state what cannot.
- If a blocker appears — resolve it if feasible, state it clearly if not.
- If scope expands significantly mid-task — pause and confirm with the user.

---

## 9. TESTING & VALIDATION

### 9.1 Test Structure

| Type        | Location             | Command                 | Runs Against           |
|-------------|----------------------|-------------------------|------------------------|
| Unit        | `tests/unit/`        | `make test-unit`        | Mocks, in-memory       |
| Integration | `tests/integration/` | `make test-integration` | Real DB, real services |
| End-to-End  | `tests/e2e/`         | `make test-e2e`         | Full system running    |
| Lint        | —                    | `make lint`             | Source only            |

### 9.2 Validation Steps

Before marking a task complete:

1. Run the most targeted test for the changed file.
2. If it passes, run the full unit suite.
3. If the change touches DB or external calls, run integration tests.
4. Run the linter. Zero warnings required.
5. If validation could not run, state that explicitly.

### 9.3 Test Writing Rules

- Add tests only if the repo already has a pattern for that module.
- Do not introduce a test framework that does not already exist.
- Mirror the source file structure in the test directory.
- Test behavior, not implementation details.
- Every new exported function with logic requires at least one test.
- Use table-driven tests for functions with multiple input cases.

### 9.4 Changes That Require Tests Before Merge

| Change Type                       | Required Coverage                 |
|-----------------------------------|-----------------------------------|
| New business logic in service     | Unit test                         |
| New DB query                      | Integration test                  |
| New API endpoint                  | Integration or E2E test           |
| Auth or permission logic          | Unit test + integration test      |
| Data migration                    | Migration test or manual verify   |
| Changes to shared model types     | Unit tests for all affected paths |

---

## 10. DEBUGGING PROTOCOL

### 10.1 Failure Classification

| Class              | Symptoms                                           | Start Here                             |
|--------------------|----------------------------------------------------|----------------------------------------|
| Configuration      | Process exits at startup, missing env vars         | `internal/config/config.go`            |
| Schema drift       | DB errors, column not found, unexpected nulls      | `internal/store/migrations/`           |
| Routing failure    | 404 / 405, route not matched                       | `internal/api/router.go`               |
| Auth failure       | 401 / 403, token errors                            | `internal/api/middleware.go`           |
| Logic error        | Wrong output, bad state, incorrect calculation     | `internal/service/[domain]_service.go` |
| Data access error  | Query failure, transaction issue, deadlock         | `internal/store/[domain]_store.go`     |
| Dependency failure | External API timeout, integration broken           | Check integration config and secrets   |

### 10.2 Debugging Steps

1. Reproduce the failure with the minimum possible input.
2. Classify the failure using the table above.
3. Read the source at that layer.
4. Check logs for context.
5. Add targeted logging if needed — do not scatter debug prints.
6. Fix the root cause.
7. Verify the fix with a test.
8. Remove all debug logging before finishing.

### 10.3 Debugging Constraints

- Do not leave `fmt.Println` or equivalent debug prints in the codebase.
- Do not resolve a bug by catching and ignoring the error.
- Do not patch a symptom if the root cause is identifiable.
- Do not assume a fix worked because the error stopped appearing — verify with a test.

---

## 11. SECURITY & VULNERABILITY POLICY

### 11.1 Security-Sensitive Paths

Changes to these paths require manual review in addition to passing tests.

| Path                              | Risk                                               |
|-----------------------------------|----------------------------------------------------|
| `internal/api/middleware.go`      | Auth enforcement — a bypass is a full compromise   |
| `internal/store/[auth store]`     | Credential handling, token storage                 |
| `internal/config/config.go`       | Secret loading, env exposure                       |
| Any file handling file uploads    | Path traversal, MIME bypass, size abuse            |
| Any file handling user input      | SQL injection, command injection, template injection|

### 11.2 Security Rules

| Rule                                              | Requirement                                          |
|---------------------------------------------------|------------------------------------------------------|
| No secrets in source code                         | Use env vars — see `.env.example`                    |
| No raw SQL with user input                        | Parameterized queries only                           |
| No `os/exec` with unsanitized input               | Not permitted                                        |
| No logging of credentials, tokens, or PII         | Scrub before logging                                 |
| No disabled TLS verification in production        | Not permitted                                        |
| Auth middleware on all private routes             | Unauthenticated route requires explicit justification|
| All dependencies pinned to exact versions         | No floating versions in production                   |

### 11.3 Vulnerability Response

If a vulnerability is found during work:

1. Stop the current task.
2. Document it: file, line, risk, exploitability.
3. Report to the user before doing anything else.
4. Do not fix it silently.
5. Do not attempt a non-trivial fix without explicit approval.

### 11.4 Dependency Policy

- Pin all dependencies to exact versions.
- Justify any new dependency against existing solutions in the codebase.
- Check for known CVEs before adding a dependency.
- If a dependency is flagged by the security scanner, fix or document the justification — do not suppress.

---

## 12. GUARDRAILS

### 12.1 Never Do

```
Never fabricate facts about the codebase — read the source if unsure.
Never present a guess as a fact.
Never modify code outside the scope of the task.
Never suppress or ignore errors in production paths.
Never use destructive git operations without explicit instruction.
Never revert changes made by the user.
Never commit, branch, or push without explicit instruction.
Never add a dependency without justification.
Never build SQL queries with string concatenation and user input.
Never log credentials, tokens, secrets, or PII.
Never leave debug output in the codebase.
Never merge security-sensitive changes without flagging them for manual review.
Never access a sibling module's internal types directly — use exported interfaces.
Never reorder or delete migration files.
Never assume an env var exists — it must be registered in config.
```

### 12.2 Always Do

```
Always read this file before starting any task.
Always read the source before making assumptions about behavior.
Always use rg to locate relevant files before editing.
Always validate changes with the appropriate test command.
Always state uncertainty explicitly.
Always fix root causes, not symptoms.
Always match the surrounding code style.
Always report security findings before continuing other work.
Always keep migration files ordered and unchanged.
Always remove debug logging before completing a task.
Always prefer the smallest correct change.
Always check section 6 (Known Issues) before starting.
```

### 12.3 When Uncertain

Read the source. That is the only reliable reference.

---

> **Version:** `[x.y.z]` | **Last updated:** `[YYYY-MM-DD]` | **Maintained by:** `[team / person]`
> Keep this file current. An outdated AGENTS.md produces incorrect agent behavior./