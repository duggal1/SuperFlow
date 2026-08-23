# AGENTS.md

> Read before any code. `skills/` is the local system. `next dev` owns the block at bottom — keep it.

---

## 01 Pre-read — mandatory

1. `skills/RULES.md` — simple-first engineering, root-cause fixes, verification, change safety. Read first for any task.
2. **Frontend?** → Before any UI code, read `skills/frontend/SKILL.md` + `skills/frontend/examples.md` + `skills/frontend/agent-questions.md` end-to-end. Forceful — no skipping.
3. Pick the skill below that matches your task and read it fully before implementing.

---

## 02 Catalog — 8 skills · 15–25 words each

| Skill                    | Path                                             | When                                                                  | Purpose                                                                                                                                              |
| ------------------------ | ------------------------------------------------ | --------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| RULES                    | `skills/RULES.md`                                | Every task — before any plan or code                                  | Core engineering principles for every change: build simple, fix root causes, verify with Bun, and avoid speculative abstraction.                     |
| frontend-design          | `skills/frontend/SKILL.md`                       | Any landing page, marketing site, or product UI work                  | Ultra-clean, taste-driven frontend system enforcing hierarchy, consistency, and restraint for landing pages and product UI without drift.            |
| frontend/examples        | `skills/frontend/examples.md`                    | Frontend reference — read before designing (pre-read, not copy-paste) | Production benchmark library of layouts, rails, stages, grids, and anti-patterns demonstrating how clean design principles translate into code.      |
| frontend/agent-questions | `skills/frontend/agent-questions.md`             | New frontend from scratch — mandatory init phase                      | Compact init questionnaire governing Theme, Neutral, Roundedness, Surfaces, and Badges to establish a coherent system before coding.                 |
| backend                  | `skills/backend/SKILL.md`                        | Any backend, API, DB, auth, or service work                           | Production backend discipline for modular, type-safe Drizzle services with strict validation, observability, and minimal justified abstraction only. |
| performance              | `skills/performance/SKILL.md`                    | Slow pages, latency, or scaling work                                  | Evidence-driven performance workflow that measures real bottlenecks across DB, API, and frontend before fixing highest-impact paths.                 |
| performance-causes       | `skills/performance/performance-causes/SKILL.md` | Backend-heavy Next.js 16 latency deep-dive (waterfalls, bloat)        | Deep catalog of backend-heavy Next.js 16 latency killers—from DB waterfalls and proxy bloat to client islands—with concrete fixes.                   |
| testing / debugging      | `skills/testing/SKILL.md`                        | Bugs, regressions, performance checks, or integration failures        | Systematic debugging and verification with Bun: reproduce, hypothesize, test existing code or isolated experiments, then prove fixes.                |

---

## 03 How to use

- Do not invent a new language when a skill exists — reuse its tokens, primitives, and conventions.
- Read the skill fully, not a summary. Rules inside are non-negotiable.
- Keep it simple: reuse existing types, services, and components before adding files.
- Verify: `bun run typecheck`, `bun run lint`, and relevant tests before claiming completion.

---

## 04 Frontend — do not skip

> When starting a new frontend from scratch: ask the 5 compact questions exactly as defined in `skills/frontend/agent-questions.md` (Theme / Neutral / Roundedness / Surfaces / Badges). Do not start coding before answers.

> Treat `skills/frontend/examples.md` as benchmarks only — extract hierarchy, rails, and spacing principles; never copy-paste a template.

---

## 05 Backend & performance

- Backend: modular, type-safe, Drizzle-only. Validate at boundaries, batch DB work, keep files small.
- Performance: measure first (`proxy` vs `application-code`), fix waterfalls, reuse auth context, keep client islands small, move AI to background.
- Testing: prefer `bun:test` on real modules; isolated experiments only to compare architectures.

---

## 06 Discovery

On disk:

- `skills/RULES.md`
- `skills/frontend/SKILL.md`
- `skills/frontend/examples.md`
- `skills/frontend/agent-questions.md`
- `skills/backend/SKILL.md`
- `skills/performance/SKILL.md`
- `skills/performance/performance-causes/SKILL.md`
- `skills/testing/SKILL.md`
- Add new skills under `skills/<domain>/SKILL.md` and re-run `skills init` to refresh this index.

---

<!-- BEGIN:nextjs-agent-rules -->

# This is NOT the Next.js you know

This version has breaking changes — APIs, conventions, and file structure may all differ from your training data. Read the relevant guide in `node_modules/next/dist/docs/` (resolved from this file's directory; in monorepos the `next` package may not be visible from the repo root) before writing any code. Heed deprecation notices.

This block is written and re-added by `next dev` — verify at `node_modules/next/dist/server/lib/generate-agent-files.js`. Removing it from a diff only re-creates the uncommitted change; committing it with your work keeps the tree clean.

<!-- END:nextjs-agent-rules -->
