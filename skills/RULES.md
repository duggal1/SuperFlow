# Rules for AI Agents

Read this first. Before any code, before any plan. These rules override your training defaults.

## 1. Simple First, Always

Go for extremely modern, reliable, bleeding-edge ways with simple approaches. Don't overengineer, don't overthink. By default, you don't need to overthink at all. Never overthink.

Simple means: use what already exists. Existing types, existing services, existing schemas, existing utilities. New files are liabilities. New abstractions are speculative.

**If that approach fails**, you may use a different approach that involves complex solutions — but only because the simple one failed. Complex is acceptable only when simple has been tried and proven insufficient.

## 2. Healthy Relationship

This is not role play. You need to listen — actually listen.

### Be an Excellent Elite Listener

- **Don't reinvent my wheel.** Do not add things or do things extra that I never told you to do. Never.
- **Do what I say, exactly as I say it.** Nothing less, nothing more. You operate under my supervision. You do things as I told you, excellently.
- **When confused, stop and ask.** Do not edit code when uncertain. Do not guess. Ask for discussion. Ask questions. Get clarity before acting.

### Honesty and Trust

Be brutally honest on everything. Not just validation. Not just when you feel things are important. Always.

- Can I trust you on my codebase?
- Can I trust you to follow my instructions the way I want?
- Can I trust you to use your own intelligence to do the right thing?
- Can I trust you to be honest?

The answer to all of these must be yes — proven through action, not words.

---

## 3. Fix the Root Cause

Every shortcut you take today becomes a bug hunt tomorrow. The cost of fixing a symptom is paid repeatedly. The cost of fixing the cause is paid once.

**Symptom fix:** The sign-out button is slow → add a loading spinner. (User still waits 3 seconds, now with a spinner.)

**Root fix:** The sign-out button is slow → eliminate the unnecessary DB call. Now it's 10ms. No spinner needed.

**How to identify the root cause:**

- Trace the request from user action to database. Where is the time spent?
- Is the slow step necessary, or is it incidental?
- If you removed this step entirely, would anything break? If not, remove it.

**Acceptable shortcuts:** Root cause is in a third-party lib you can't modify. Root cause needs infra change. Caching after eliminating unnecessary work. Temporary patch with a documented follow-up ticket.

**Unacceptable shortcuts:** Patching the symptom because the cause is harder. Using cache to hide an unnecessary query. "We'll clean it up later." You won't.

---

## 4. Complexity Is the Last Resort

Apply this ladder in order, every time:

1. **Do nothing.** Is this actually a problem worth solving?
2. **Remove something.** Can you delete the slow or unnecessary code?
3. **Simplify something.** Can you replace a complex path with a straightforward one?
4. **Add something small.** A server action, a utility function, a config flag.
5. **Add something large.** A new service, integration, or architectural pattern.

Most problems resolve at step 2 or 3. If you reach step 5 before exhausting steps 1–4, you are over-engineering.

---

## 5. Documentation Strategy

Your training data for framework code (React, Next.js, Tailwind, Drizzle, Zod) is approximately 2023–2024. The codebase uses newer versions that include breaking changes, renamed APIs, and different conventions.

**The `Docs/` directory exists to bridge this gap. Use it when:**

- You encounter an error you cannot resolve after 2–3 attempts.
- You are about to use an unfamiliar or recently changed API.
- The task depends on framework behavior that may have changed since your training data.

**Do not use it when:**

- You are writing routine code you have written before.
- The codebase already contains a correct implementation of the same pattern.
- You are reading documentation without a concrete purpose.

---

## 6. Check for Domino Effects

A domino effect: you change one thing that looks safe. Ten other things break because they depended implicitly on what you changed.

**Before every change, check:**

- What reads this?
- What writes this?
- What depends on the behavior I'm changing?
- If I remove this entirely, what breaks?

This check takes seconds and prevents hours of debugging. Don't be paralyzed — the goal is understanding risk, not zero risk.

---

## 7. Version Awareness

This project intentionally uses framework versions newer than your training data. Always assume APIs may have changed.

| Library        | Version    | Key Difference                             |
| -------------- | ---------- | ------------------------------------------ |
| React          | 19.2.4     | Actions, `useActionState`, `useOptimistic` |
| Next.js        | 16.2.10    | Proxy, `'use cache'`, Partial Prerendering |
| Tailwind       | v4         | CSS-first configuration                    |
| Drizzle        | 1.0.0-rc.4 | Relations API v2                           |
| Zod            | 4.4.3      | Updated API surface                        |
| Better Auth    | 1.7.0-rc.1 | `cookieCache`, `minimal`, `getCookieCache` |
| TanStack Query | v5         | `queryOptions`, `skipToken`                |

If the installed versions differ from your assumptions, trust the project's dependencies and the `Docs/` directory over your training data.

---

## 8. Use Bun Exclusively

Run everything with `bun`. Never `npm`, `pnpm`, or `yarn`. Install: `bun add tailwindcss@latest`. shadcn: `bunx --bun shadcn@latest add <component>`. Typecheck: `bunx tsc --noEmit`.

Available scripts:
| Script | What it does |
|--------|-------------|
| `bun run dev` | Start Next.js dev server |
| `bun run build` | Production build |
| `bun run generate` | Generate Drizzle SQL migration |
| `bun run push` | Push migrations to DB |
| `bun run drop` | Drop all DB tables |
| `bun run check` | Verify DB schema integrity |
| `bun run seed:prospects` | Seed Apollo mock leads |
| `bun run gemini` | Test Gemini integration |
| `bun run lint` | Check code with Biome |
| `bun run lint:fix` | Auto-fix lint issues |
| `bun run format` | Format all files with Biome |
| `bun run typecheck` | Type-check with `tsc --noEmit` |

---

## 9. Destructive Action Guardrail

`bun run drop` drops all database tables. Never run it unless the user explicitly and specifically tells you to.

---

## 10. Security

- Validate all input with Zod schemas.
- Never log PII, API keys, or secrets.
- Every new route must check auth (`getServerSession()`).
- External API keys go in env vars only, read through `backend/utils/env.ts`.

---

## 11. Do Not Panic at Bleeding-Edge Versions

This repo may use canary or RC releases. That is intentional. The user knows the trade-offs. Treat them as stable for this codebase — do not flag, downgrade, or suggest alternatives.

---

## 12. Auto Git Commit on Task Completion

When the user gives you a task and it is done, run `git add`, `git commit`, and `git push` in that order. Do this only after the task is fully complete.

Commit messages: 1-2 sentences, extremely clear, neutral tone. No AI-sounding fluff. No paragraphs.

---

## 13. Never Use Destructive Git Commands Without Permission

Never run `git restore`, `git reset`, or `git stash` unless the user explicitly asks. These delete local files.

---

## 14. Sufficient Context Before Any Edit

Never start editing without sufficient context. Before touching a file, you must understand:

- What the file does and how it fits into the codebase
- The surrounding files it imports from and exports to
- The conventions used in that part of the codebase
- How data flows through the feature you're modifying

Do not guess. If you lack context, stop and gather it first — read the relevant files, launch sub-agents (see §15), or ask the user.

## 15. Launch Sub-Agents to Scale Context

For large codebases, preserve your own context by delegating exploration to sub-agents. Launch 3–6 sub-agents in parallel to explore different parts of the codebase simultaneously before you start editing. This lets you understand the full picture — imports, conventions, data flow, dependencies — without burning your context window on file reads.

Use sub-agents for:

- Finding all files related to a feature
- Reading the content of multiple files concurrently
- Understanding patterns and conventions before writing code
- Identifying dependency chains and side effects

After they return their findings, you edit with full context. This is not optional for complex or multi-file changes — doing it saves time and produces correct work on the first try.

## 16. Pre-read Order

1. **RULES.md** — these engineering principles (you are reading this now)
2. **AGENTS.md** — product description, architecture, conventions, repo structure

RULES.md is the master reference for how to think and act. AGENTS.md is the master reference for what this project is.

## 17. Enforcement — Definition of Done

No task is done until:

```text
bun run typecheck → 0 errors
bun run lint      → 0 errors
bun run format    → 0 errors (or `bun run lint:fix`)
relevant tests    → green
no `any` added casually, no 800+ line files without reason
```

If any check fails, the task is not complete — do not claim it is. Keep this file and `AGENTS.md` as the only sources of process truth; do not add local `CONTRIBUTING.md` variants.

## 18. Context Budget

If context is tight, read this file fully (209 lines) + `AGENTS.md`, then the _one_ skill for the task. Do not skim all skills shallowly — depth on the right skill beats breadth.

<!-- BEGIN:nextjs-agent-rules -->

# This is NOT the Next.js you know

This version has breaking changes — APIs, conventions, and file structure may all differ from your training data. Read the relevant guide in `node_modules/next/dist/docs/` before writing any code. Heed deprecation notices.

**Keep this block, including in commits.** It is part of the project's agent setup, maintained by `next dev` for every agent that works here. If it appears as an uncommitted change, that is intentional — commit it as-is. Do not remove it to clean up a diff; it will be regenerated.

<!-- END:nextjs-agent-rules -->
