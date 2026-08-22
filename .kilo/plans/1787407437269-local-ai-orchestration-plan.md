# Plan: Local-Only AI Orchestration for SuperFlow (100% Free, Private, Forever)

> Status: PLANNING — one decision still open (see §11). Feasibility below is grounded in the actual code in this repo.

## 1. Feasibility Verdict

**YES — 100% achievable**, with exactly one hard constraint (§2). The building blocks already exist in this repo; this plan is mostly integration + a modern re-implementation of the buggy old `Terminal-kit`, not invention from zero.

What is already proven local & free in the code:
- **Voice → text (fully local):** `src-tauri/src/managers/transcription.rs` + `transcribe-cpp`/`transcribe-rs` (Whisper/Parakeet), VAD with Silero. No network.
- **On-device intelligence (Apple):** `src-tauri/src/apple_intelligence.rs` (Swift `Foundation Models` bridge) + `src-tauri/src/intelligence/router.rs` + `src-tauri/src/actions.rs`. Runs on Apple Silicon, no API key, no cloud. This is the "piece-to-text + intelligence" you already ship.
- **Local model-server path (generic):** `src-tauri/src/llm_client.rs` sends OpenAI-compatible chat completions to any `base_url` + `api_key`. Pointing it at `http://localhost:11434/v1` (Ollama) with an empty key = fully local, free, private. Already supported via the `custom` provider.
- **Local multi-agent orchestration engine:** `Terminal-kit/src/` (`sp`) is a Rust control plane that launches agent CLIs in PTYs/tmux, supervises them with a watchdog, mail, leases, and a supervisor — entirely local, no network of its own.

## 2. The One Hard Constraint (the contradiction to resolve)

You said: *"No external API. No open API. 100% free forever. All local. Private."* But you also said *"use Open Code / Claude Code"* and *"Nemotron from opencode terminal."*

- **Claude Code** (Anthropic) and **Codex** require a cloud API key → violates the rule.
- **OpenCode + Nemotron** only stays free/local if Nemotron runs on a **local server** (Ollama / `llama.cpp` serving `nvidia/Llama-Nemotron-*`). If you use NVIDIA's free *hosted* API, that is an external API → violates the rule.
- **Apple Intelligence** cannot drive a Claude-Code-style agent CLI; it is a single-turn on-device chat. It is perfect for the *lightweight* voice→command path, not for autonomous multi-file coding agents.

**Resolution (CONFIRMED by user):** Two local tiers, no cloud agents.
1. **Tier A — Apple Intelligence (on-device):** fast, free, for intent classification, command parsing, short rewrites, awareness composition. Already wired.
2. **Tier B — OpenCode CLI (local binary) + local model:** the coding-agent harness is **OpenCode** (`opencode`), run as a *local* terminal binary, configured against a **local** `base_url` (Ollama serving Qwen3 / Nemotron locally). No Claude, no Codex, no hosted endpoint, no API key. OpenCode generates/executes; we paste its prompt output back. This is the "free local terminal" the user described.

This satisfies "100% free forever, no external API" on Apple Silicon. Claude/Codex/cloud-Nemotron are explicitly NOT used.

## 3. What Already Exists (evidence)

SuperFlow (the app under `src-tauri` + `src`):
- `src-tauri/src/apple_intelligence.rs`, `src-tauri/src/intelligence/router.rs`, `src-tauri/src/actions.rs`, `src-tauri/src/llm_client.rs`, `src-tauri/src/context/` (page/window context capture), `src-tauri/src/managers/transcription.rs`.

Terminal-kit (old repo, `Terminal-kit/`, Cargo pkg `sapphire-agent-factory`, bin `sp`):
- `src/main.rs` — CLI boot + dispatch.
- `src/cli.rs` — clap CLI surface.
- `src/orchestrator/mod.rs` — mission lifecycle + watchdog loop.
- `src/orchestrator/{mail,health,dedup}.rs` — mail, health, dedup.
- `src/runtime/mod.rs` — PTY spawn + `RuntimeEvent` stream.
- `src/adapter.rs` — per-agent `CliAdapter` trait + 4 adapters.
- `src/agent/mod.rs` — `AgentKind` launch specs (Claude/Codex/Qwen/Forge). **This is the file to change** to repoint agents at local Ollama.
- `src/protocol.rs` — `SAPPHIRE_STATUS/MAIL/ACK/LEASE` directive parser.
- `src/model.rs`, `src/store/mod.rs` — domain types + SQLite.
- `src/tmux/` — tmux control surface (Ghostty tab-first).
- `src/templates.rs` + `src/internal/agents/templetes/roles/job-roles/*.md` — 15 role templates. **Reuse these as the 5 roles' prompts.**
- `docs/`, `product-direction.md`, `How-should-the-supervisor-be-built.md` — design intent.

You said this code is "old (4–5 months), slower, bloated, complex, outdated, unreliable." Confirmed by its own `AGENTS.md` ("Current Gaps", "storage naming drift", "TUI not yet wired as interactive dashboard", "no migration framework"). It is a working *foundation*, not a finished product.

## 4. Target Architecture

```
Voice (mic)
  → SuperFlow transcription (local Whisper/Parakeet)        [exists]
  → Intent/command parse via Apple Intelligence (Tier A)     [exists, extend]
  → If coding task: hand mission to local orchestrator (Tier B)
        local "sp"-style supervisor + N worker agents
        each worker = Ollama/Nemotron in a PTY (no cloud)    [repoint Terminal-kit]
  → Results -> SuperFlow UI (design language) + paste back   [build]
```

Everything runs on-device. Apple Intelligence = fast path; local Ollama = heavy path. No key, no egress.

## 5. The Five Roles

Map the 15 Terminal-kit role templates down to 5 focused roles for this product:

1. **Design Language / UI-UX** — owns visual system, spacing, components, accessibility. Source: `designer-engineer.md`.
2. **Backend — Type Safety** — owns `tsc`/`cargo` typecheck, schema, contracts. Source: `software-engineer.md` + `architecture-engineer.md`.
3. **Backend — Debugging** — owns runtime bugs, crashes, regressions, SIGTRAP fixes (recent git history shows these). Source: `debug-and-review-engineer.md`.
4. **Transcription Intelligence** — owns the voice→text "supercharge": context-aware awareness, command parsing, local LLM post-processing. Source: `research-engineer.md` + existing `intelligence/`.
5. **Orchestration / Supervisor** — owns mission split, worker dispatch, watchdog, conflict resolution. Source: `product-manager.md` + Terminal-kit supervisor design.

Each role = a reusable prompt template (reuse the markdown files) + a scoped work packet.

## 6. Build Phases (ordered)

1. **Pin the local model contract.** Add a `local` provider in `llm_client.rs`/`settings.rs`: `base_url=http://localhost:11434/v1`, empty key, OpenAI-compatible. Verify Apple Intelligence remains Tier A default.
2. **Wire the local coding-agent backend = OpenCode CLI.** Implement a `LocalAgent` adapter in `Terminal-kit/src/agent/mod.rs` that shells out to the **local `opencode` binary**, configured with a local `base_url` (Ollama/Qwen3/Nemotron). Replace the Claude/Codex defaults. Never call a hosted endpoint. (This is the only thing blocking "100% local".)
3. **Thin the orchestrator.** Port only the reliable core from `Terminal-kit` into SuperFlow's Rust backend (or a side-car binary): PTY spawn (`runtime/mod.rs`), directive protocol (`protocol.rs`), watchdog stall/health (`orchestrator/{mod,health}.rs`), SQLite store (`store/mod.rs`). Drop the bloated TUI/shimmer/theme cruft the user flagged.
4. **Voice→command glue.** In SuperFlow Rust: after transcription, classify intent with Apple Intelligence; if it's a coding mission, serialize a `MissionPlan` and launch the local orchestrator with the 5 role packets.
5. **Design language + UI.** Build the SuperFlow surfaces (overlay/settings) per the Design role; keep it local, fast, private-first.
6. **Typecheck + debug hardening.** Wire roles 2 & 3 as always-on checks: `bun run typecheck`, `cargo check`, crash-loop detection from Terminal-kit's `session_restarts` health logic.

## 7. Reference Code the Agent Prompt Must Inherit

The long prompt in §8 hands an agent the *working* files below so it starts from a proven base, then simplifies:

- `Terminal-kit/src/main.rs`, `Terminal-kit/src/cli.rs`
- `Terminal-kit/src/orchestrator/mod.rs`, `Terminal-kit/src/orchestrator/health.rs`, `Terminal-kit/src/orchestrator/dedup.rs`
- `Terminal-kit/src/runtime/mod.rs`
- `Terminal-kit/src/adapter.rs`, `Terminal-kit/src/agent/mod.rs`
- `Terminal-kit/src/protocol.rs`, `Terminal-kit/src/model.rs`, `Terminal-kit/src/store/mod.rs`
- `Terminal-kit/src/templates.rs`, `Terminal-kit/src/internal/agents/templetes/roles/job-roles/*.md`
- SuperFlow: `src-tauri/src/intelligence/router.rs`, `src-tauri/src/apple_intelligence.rs`, `src-tauri/src/actions.rs`, `src-tauri/src/llm_client.rs`, `src-tauri/src/context/`

## 8. The Long Agent Prompt (hand to Claude Code / OpenCode when you say "please open")

```
You are extending SuperFlow (a Tauri + Rust, fully-offline speech-to-text app at
/Users/harshitduggal/workspace/SuperFLow-macos) with a LOCAL-ONLY, 100%-FREE-FOREVER,
PRIVATE AI orchestration layer. There is NO external API, NO cloud, NO API key, EVER.

HARD CONSTRAINTS (non-negotiable):
- Every model call is either Apple Intelligence on-device (already bridged in
  src-tauri/src/apple_intelligence.rs via Swift Foundation Models) or a LOCAL model
  server reached at http://localhost:11434/v1 (Ollama) with an EMPTY api_key.
- Never call Anthropic, OpenAI, Google, NVIDIA hosted, or any remote endpoint.
- "Nemotron" / "Qwen" must run LOCALLY via Ollama/llama.cpp, never via a hosted API.
- The coding-agent HARNESS is the LOCAL `opencode` CLI binary, configured against a
  LOCAL base_url (Ollama). It is NOT Claude, NOT Codex, NOT a hosted service. No API key.
- All data (audio, transcripts, plans, agent output) stays on the user's machine.

EXISTING WORKING FOUNDATION (READ THESE FIRST — they already compile and run):
- SuperFlow intelligence: src-tauri/src/intelligence/router.rs, apple_intelligence.rs,
  actions.rs, llm_client.rs (OpenAI-compatible client; add a `local` provider with
  base_url http://localhost:11434/v1 and empty key).
- Old orchestrator (4-5 months old, WORKING BUT BUGGY/SLOW/BLOATED/OUTDATED):
  Terminal-kit/src/main.rs, cli.rs, orchestrator/mod.rs, orchestrator/health.rs,
  orchestrator/dedup.rs, runtime/mod.rs, adapter.rs, agent/mod.rs, protocol.rs,
  model.rs, store/mod.rs, templates.rs, and the role prompts under
  Terminal-kit/src/internal/agents/templetes/roles/job-roles/*.md.

KNOWN PROBLEMS IN THE OLD CODE (fix, do not copy):
- agent/mod.rs hardcodes CLAUDE and CODEX (cloud agents). Repoint to the LOCAL
  `opencode` CLI binary (configured against a local Ollama base_url). No cloud, no key.
- Overly complex: heavy ratatui/shimmer/theme stack, 15 role files, mail/zombie/
  crash-loop machinery that is partially untested and drifted (per Terminal-kit AGENTS.md).
- Slower than needed; uses bloated approaches. Prefer a far more modern, simpler,
  reliable, faster design while keeping the useful ideas: PTY spawn, single-line
  directive protocol, stall watchdog, SQLite persistence, role-based packets.

YOUR TASK — build the following 5 ROLES as local, reliable, fast modules:
1. Design Language / UI-UX: define and implement SuperFlow's visual system (spacing,
   components, overlay/settings) — local-first, private-first, fast. Source:
   designer-engineer.md.
2. Backend — Type Safety: enforce `bun run typecheck` + `cargo check`; fix type/schema
   regressions; keep contracts clean. Source: software-engineer.md + architecture-engineer.md.
3. Backend — Debugging: fix runtime crashes/regressions (e.g. the recent SIGTRAP and
   history-DB desync fixes in git log); add crash-loop detection from the old
   session_restarts logic. Source: debug-and-review-engineer.md.
4. Transcription Intelligence: supercharge voice->text. Use Apple Intelligence (Tier A)
   for intent/command parsing + awareness, and local OpenCode + Ollama (Tier B) for
   heavier rewrites. Reuse intelligence/router.rs. Source: research-engineer.md.
5. Orchestration / Supervisor: split an unambiguous mission into N worker prompts
   (one prompt per worker), dispatch via the LOCAL `opencode` CLI in PTYs, supervise
   with a lightweight stall/health watchdog, persist to SQLite. Reuse the GOOD parts of
   orchestrator/mod.rs + protocol.rs + store/mod.rs, but simplify aggressively.

EXECUTION RULES:
- Start from the working files above; do not rewrite from zero.
- Keep it modular (per Terminal-kit AGENTS.md modularity rule) but SIMPLE.
- Everything local, free, private. No network egress. No API keys.
- Verify with `bun run typecheck`, `bun run lint`, `cargo check` / `cargo test` (Terminal-kit).
- If a requirement is ambiguous/incomplete, state the assumption and continue; do not
  block. Prefer the simplest reliable fixer.
```

## 9. Risks

- **Local model quality:** A 7–14B local model is weaker than Claude/Codex for hard coding tasks. Mitigation: Apple Intelligence for easy path; reserve heavy work for the local model; keep human-in-the-loop paste-back.
- **Mac-only Apple Intelligence:** Tier A only works on Apple-Silicon macOS (`#[cfg(all(target_os="macos", target_arch="aarch64"))]`). Cross-platform falls back to local Ollama only. Acceptable for this macOS product.
- **Old code drift:** Terminal-kit's storage naming drifted; port, don't lift wholesale.
- **Speed:** Voice→command must feel instant. Use Apple Intelligence (fast) for classification; lazy-spawn the heavier local orchestrator only when needed.

## 10. Validation

- `bun run typecheck` + `bun run lint` green on SuperFlow.
- `cargo check` / `cargo test` green on Terminal-kit's ported core.
- Manual: speak a command → transcribed locally → Apple Intelligence classifies → if coding mission, local orchestrator launches LOCAL `opencode` workers against a local Ollama model → result pasted, no network calls (verify with Little Snitch / `lsof`).
- Confirm no `api.anthropic.com` / `api.openai.com` / `integrate.api.nvidia.com` / `api.opencode.ai` egress in logs.

## 11. Resolved Decision (confirmed by user)

The coding-agent backend is the **local `opencode` CLI** pointed at a **local** model (Ollama / Qwen3 / Nemotron). No Claude, no Codex, no hosted endpoint, no API key. This satisfies "100% free forever, no external API, all local, private." The plan is implementation-ready.

I will not open Claude Code / a terminal until you explicitly say the trigger phrase ("please open"). This plan + the §8 prompt are ready to hand off the moment you do.
