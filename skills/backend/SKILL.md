# Backend Engineering Skill

> **9/10 — keep 90%:** This file is 1,714 lines. If context is tight, read `§1 Core Rules` + `§8 Type Safety` + `§10 Drizzle Only` + the one domain section for the task (e.g., `§20 oRPC` / `§28 Service`). Full read once, then reuse. Scope: Drizzle-only Neon; adapt only if stack differs with explicit reason.

## Purpose

Build backends that are:

- Extremely type-safe
- Modular
- Production-reliable
- Current with verified stable technology
- Simple enough to understand
- Fast by default
- Easy to extend
- Difficult to misuse

The goal is **modern engineering without architectural cosplay**.

The backend should never become a 5,000-line file, a maze of abstractions, or a framework zoo.

---

# 1. Core Rules

These rules are non-negotiable.

```text
Modularity → aggressive
Type safety → strict
Architecture → modern + simple
Abstractions → justified
Comments → almost none
Duplication → remove
Dead code → remove
Warnings → zero
Type errors → zero
Lint errors → zero
Formatting errors → zero
Production checks → mandatory
```

Do not overengineer.

Do not under-engineer.

Build the smallest architecture that is genuinely reliable for the problem.

---

# 2. NEVER Dump the Backend Into One File

AI agents frequently produce:

```text
src/server.ts
src/api.ts
src/db.ts
```

with thousands of lines inside them.

Do not do this.

A large backend must be split into focused modules.

Prefer:

```text
src/
├── app/
│   └── ...
├── db/
│   ├── index.ts
│   ├── schema/
│   ├── queries/
│   └── migrations/
├── modules/
│   ├── users/
│   │   ├── schema.ts
│   │   ├── queries.ts
│   │   ├── service.ts
│   │   ├── router.ts
│   │   └── types.ts
│   ├── billing/
│   ├── projects/
│   └── ...
├── lib/
│   ├── auth/
│   ├── validation/
│   └── ...
└── app/
```

Organize by **domain**, not by giant technical dumping grounds.

---

# 3. Modularity

Use modularity aggressively.

A module should have one understandable responsibility.

Good:

```text
modules/
└── users/
    ├── schema.ts
    ├── queries.ts
    ├── service.ts
    ├── router.ts
    └── types.ts
```

Bad:

```text
src/
└── everything.ts
```

Do not create abstractions merely to make folders look impressive.

A module should earn its existence.

---

# 4. Do Not Overthink Architecture

The AI must actively resist architectural overthinking.

Do not automatically introduce:

```text
microservices
event buses
CQRS
repository interfaces
factory factories
dependency injection containers
domain event frameworks
service locator patterns
hexagonal architecture
six-layer abstractions
```

unless the actual product requires them.

Start with:

```text
Next.js
+
oRPC
+
Server Actions where appropriate
+
TanStack Query
+
Drizzle
+
Neon Postgres
```

Add complexity only when there is a concrete reason.

---

# 5. Preferred 2026 Stack

When this stack is appropriate, prefer the latest **verified stable** versions of:

```text
Next.js
React
TypeScript
Zod
oRPC
TanStack Query
Drizzle ORM
Neon
Biome
Bun
```

Current verified documentation indicates:

```text
Next.js → 16.3
React → 19.2
Zod → 4
TanStack Query → v5
```

Do not hard-code future or imaginary versions.

The rule is:

> **Latest verified stable technology, not fake cutting-edge technology.**

---

# 6. Version Verification

When a package API, version, or recommended pattern is uncertain:

```text
1. Inspect installed package
2. Inspect local node_modules/docs when available
3. Check package metadata
4. Search the official documentation
5. Implement against the verified API
```

Do not rely on stale model knowledge for current APIs.

Do not use random blog posts as the primary authority.

Prefer:

```text
Official documentation
Official repository
Installed package types/source
Official migration guides
```

Never avoid web research when correctness depends on current information.

---

# 7. Comments

Comments should be extremely rare.

Do not write comments explaining obvious code.

Bad:

```ts
// Get the user from the database
const user = await getUser(id);
```

Bad:

```ts
// Return the result
return result;
```

Bad:

```ts
// Check if the user exists
if (!user) {
  throw new Error("User not found");
}
```

Only comment when there is genuine ambiguity.

Allowed:

```text
TODO
FIXME
WIP
non-obvious workaround
external-system constraint
critical architectural reason
```

Example:

```ts
// TODO: Replace with provider-native webhook once the provider exposes event ordering.
```

Do not litter production code with commentary.

---

# 8. Type Safety

TypeScript must be treated as a correctness tool, not decoration.

Use:

```json
{
  "compilerOptions": {
    "strict": true,
    "noUncheckedIndexedAccess": true,
    "exactOptionalPropertyTypes": true,
    "noImplicitOverride": true,
    "noFallthroughCasesInSwitch": true
  }
}
```

Avoid:

```ts
any;
```

unless there is no technically valid alternative.

Prefer:

```ts
unknown;
```

and narrow it.

Do not silence the compiler with:

```ts
as any
```

Do not randomly cast types into existence.

Type errors should be fixed at the source.

---

# 9. Runtime Validation

TypeScript does not validate runtime input.

Validate external boundaries.

Use Zod 4 when a runtime schema is needed.

Example:

```ts
import * as z from "zod";

export const CreateProjectInput = z.object({
  name: z.string().min(1).max(100),
  description: z.string().max(500).optional(),
});
```

Infer types instead of duplicating them:

```ts
export type CreateProjectInput = z.infer<typeof CreateProjectInput>;
```

Validate:

```text
HTTP input
Server Action input
Webhook payloads
Environment variables
External API responses
User-controlled data
```

Do not validate already-trusted internal values five times.

---

# 10. Database: Drizzle Only

This backend standard uses:

```text
Drizzle ORM
+
Neon Postgres
```

Do not introduce Prisma.

Do not introduce a second ORM.

Do not create repository abstractions merely to hide Drizzle.

Use Drizzle directly inside the data layer.

Typical structure:

```text
src/db/
├── index.ts
├── schema/
│   ├── users.ts
│   ├── projects.ts
│   └── ...
├── queries/
│   ├── users.ts
│   ├── projects.ts
│   └── ...
└── migrations/
```

---

# 11. Drizzle Connection

Use the appropriate Neon driver for the execution environment.

Example:

```ts
import { neon } from "@neondatabase/serverless";
import { drizzle } from "drizzle-orm/neon-http";

const sql = neon(process.env.DATABASE_URL!);

export const db = drizzle({
  client: sql,
});
```

Environment configuration should be validated rather than silently accepted.

---

# 12. Drizzle Schema

Keep schema definitions modular.

Bad:

```text
schema.ts
→ 4,000 lines
```

Good:

```text
db/schema/
├── users.ts
├── organizations.ts
├── projects.ts
├── billing.ts
└── index.ts
```

Example:

```ts
import { integer, pgTable, text, timestamp, uuid } from "drizzle-orm/pg-core";

export const projects = pgTable("projects", {
  id: uuid("id").primaryKey().defaultRandom(),
  name: text("name").notNull(),
  organizationId: uuid("organization_id").notNull(),
  createdAt: timestamp("created_at", {
    withTimezone: true,
  })
    .notNull()
    .defaultNow(),
});
```

---

# 13. Drizzle Queries

Keep queries close to the domain.

Example:

```ts
import { eq } from "drizzle-orm";

import { db } from "@/db";
import { projects } from "@/db/schema/projects";

export async function getProjectById(id: string) {
  return db.select().from(projects).where(eq(projects.id, id)).limit(1);
}
```

Do not create:

```text
GenericRepository<T>
BaseRepository<T>
AbstractRepository<T>
RepositoryFactory<T>
```

unless there is an actual repeated requirement that makes the abstraction worthwhile.

---

# 14. Database Batching

Batch related database operations when appropriate.

Example:

```ts
const [user, projects, usage] = await Promise.all([
  getUser(userId),
  getProjects(userId),
  getUsage(userId),
]);
```

For operations supported by Drizzle/Neon batching:

```ts
const result = await db.batch([
  db.insert(users).values(user),
  db.insert(projects).values(project),
  db.select().from(usage).where(eq(usage.userId, userId)),
]);
```

Do not blindly batch unrelated work.

Optimize real query paths, not theoretical ones.

---

# 15. Database Performance

Prefer:

```text
correct indexes
select only required columns
pagination
batching
prepared statements where valuable
proper joins
reasonable query shapes
connection reuse
```

Avoid:

```text
SELECT everything
N+1 queries
unbounded pagination
loading entire tables
unnecessary round trips
```

Measure expensive paths before inventing exotic optimizations.

---

# 16. Migrations vs Push

Production databases should use migrations.

Development prototyping may use:

```text
drizzle-kit push
```

Production should use an explicit migration workflow.

Never allow an AI agent to forget database synchronization.

The workflow must include:

```text
schema change
↓
migration generation
↓
migration review
↓
migration application
↓
typecheck
↓
production verification
```

---

# 17. Package Scripts

If these scripts do not exist, add them.

```json
{
  "scripts": {
    "postinstall": "if [ -n \"$DATABASE_URL\" ]; then bun run generate && bun run push; else echo 'Skipping DB sync (DATABASE_URL not set)'; fi",
    "git": "bun run scripts/git-commit/commit.ts",
    "lint": "bunx biome lint .",
    "lint:fix": "bunx biome lint --write .",
    "format": "bunx biome format --write --format-with-errors=true .",
    "typecheck": "bunx tsc --noEmit"
  }
}
```

The exact database lifecycle may differ in production, but the agent must ensure database tooling is present and not silently forgotten.

---

# 18. Biome

Use Biome for formatting and linting.

Example:

```json
{
  "vcs": {
    "enabled": true,
    "clientKind": "git",
    "useIgnoreFile": true
  },
  "files": {
    "ignoreUnknown": false
  },
  "formatter": {
    "enabled": true,
    "indentStyle": "tab"
  },
  "linter": {
    "enabled": true,
    "rules": {
      "preset": "recommended"
    }
  },
  "css": {
    "parser": {
      "tailwindDirectives": true
    }
  },
  "javascript": {
    "formatter": {
      "quoteStyle": "double",
      "trailingCommas": "es5"
    }
  },
  "assist": {
    "enabled": true,
    "actions": {
      "source": {
        "organizeImports": "on"
      }
    }
  }
}
```

Do not add a second formatter unless necessary.

Do not fight the formatter manually.

---

# 19. Required Verification

Before considering backend work complete:

```text
bun run typecheck
bun run lint
bun run format
```

All must pass.

The acceptable state is:

```text
0 TypeScript errors
0 lint errors
0 formatting errors
0 avoidable warnings
```

Do not claim completion while checks are failing.

---

# 20. oRPC

Use oRPC for typed API procedures where an API boundary is actually needed.

oRPC routers should be modular and domain-oriented.

Example:

```ts
import { os } from "@orpc/server";
import * as z from "zod";

const createProject = os
  .input(
    z.object({
      name: z.string().min(1).max(100),
    }),
  )
  .handler(async ({ input }) => {
    return createProjectService(input);
  });

export const projectRouter = {
  create: createProject,
};
```

Then compose:

```ts
export const router = {
  project: projectRouter,
  user: userRouter,
  billing: billingRouter,
};
```

Do not build one enormous router file.

oRPC routers are plain, nestable objects and support middleware, lazy loading, typed inputs/outputs, and server-side clients.

---

# 21. oRPC Middleware

Use middleware for cross-cutting concerns:

```text
authentication
authorization
request context
logging
rate limiting
common validation
```

Example:

```ts
import { ORPCError, os } from "@orpc/server";

const requireAuth = os.middleware(async ({ next, context }) => {
  if (!context.user) {
    throw new ORPCError("UNAUTHORIZED");
  }

  return next();
});

export const getPrivateProject = os
  .use(requireAuth)
  .handler(async ({ context }) => {
    return getProjectForUser(context.user.id);
  });
```

Do not duplicate the same auth check in 40 procedures.

Do not stack redundant middleware.

---

# 22. Server Actions

Use Next.js Server Actions when the operation is naturally a direct server mutation invoked by the application UI.

Good:

```text
form submission
simple mutation
redirect-after-action
server-only UI action
```

Do not create an API endpoint merely to call it from the same Next.js server.

Example:

```ts
"use server";

import * as z from "zod";

const inputSchema = z.object({
  name: z.string().min(1),
});

export async function updateProfile(input: unknown) {
  const data = inputSchema.parse(input);

  return updateProfileService(data);
}
```

If the operation needs a reusable API contract or external API access, prefer oRPC.

---

# 23. oRPC + Server Actions

They are not competing systems.

Use them for different jobs.

```text
Server Actions
→ direct application mutations

oRPC
→ typed API boundary

Drizzle
→ database access

Services
→ domain logic
```

Do not route every operation through every layer.

---

# 24. TanStack Query v5

Use TanStack Query for client-side server state:

```text
fetching
caching
background refetching
deduplication
pagination
mutations
optimistic updates
invalidation
```

TanStack Query v5 is the current Query architecture and supports typed query options, optimistic updates, SSR, and Suspense patterns.

Example:

```ts
import { queryOptions } from "@tanstack/react-query";

export const projectQuery = (id: string) =>
  queryOptions({
    queryKey: ["project", id],
    queryFn: () => client.project.get({ id }),
  });
```

Use:

```ts
useQuery(projectQuery(projectId));
```

instead of scattering query keys and functions throughout components.

---

# 25. Optimistic Updates

Use optimistic UI when the operation is safe to predict.

Good candidates:

```text
toggle
favorite
rename
archive
reorder
simple status changes
```

Avoid optimistic updates for operations where the final server result is unpredictable.

Keep mutation rollback/error behavior explicit.

Do not mutate random local state and pretend it is a cache.

---

# 26. SSR / Server Components

Do not make the server call its own API over HTTP merely because an API exists.

For server-side rendering:

```text
Server Component
↓
server-side procedure/client
↓
service
↓
Drizzle
↓
Postgres
```

instead of:

```text
Server Component
↓
HTTP
↓
own API
↓
service
↓
database
```

Avoid unnecessary request waterfalls.

oRPC explicitly supports server-side clients for direct procedure calls during SSR, avoiding redundant HTTP requests.

---

# 27. Recommended Backend Flow

For a normal product feature:

```text
UI
 ↓
TanStack Query / Server Action
 ↓
oRPC when API boundary is required
 ↓
Router / procedure
 ↓
Service
 ↓
Drizzle query
 ↓
Neon Postgres
```

Not every request needs every layer.

For example:

```text
simple Server Action
↓
service
↓
Drizzle
```

is perfectly valid.

Do not force unnecessary layers.

---

# 28. Service Layer

Services own meaningful business operations.

Example:

```ts
export async function createProject(input: CreateProjectInput) {
  const existing = await getProjectByName(input.name);

  if (existing) {
    throw new ProjectAlreadyExistsError(input.name);
  }

  return insertProject(input);
}
```

Services should not become giant "god services."

A service should represent a meaningful domain operation.

---

# 29. Query Layer

Queries should primarily represent database operations.

Example:

```ts
export async function getProjectByName(name: string) {
  return db.select().from(projects).where(eq(projects.name, name)).limit(1);
}
```

Business decisions belong in services.

Database mechanics belong in queries.

---

# 30. Error Handling

Do not throw arbitrary strings.

Bad:

```ts
throw "Something went wrong";
```

Bad:

```ts
throw new Error("lol");
```

Prefer meaningful errors:

```ts
export class ProjectNotFoundError extends Error {
  constructor() {
    super("Project not found");
    this.name = "ProjectNotFoundError";
  }
}
```

At API boundaries, map errors into typed application-safe responses.

Never leak:

```text
database credentials
SQL
stack traces
internal filesystem paths
provider secrets
```

to clients.

---

# 31. Environment Variables

Validate required environment variables at startup.

Example:

```ts
import * as z from "zod";

const envSchema = z.object({
  DATABASE_URL: z.url(),
});

export const env = envSchema.parse({
  DATABASE_URL: process.env.DATABASE_URL,
});
```

Prefer one validated environment module over scattered:

```ts
process.env.X!;
process.env.Y!;
process.env.Z!;
```

throughout the codebase.

---

# 32. Authentication / Authorization

Separate:

```text
authentication
authorization
```

Authentication:

```text
Who are you?
```

Authorization:

```text
Are you allowed to do this?
```

Do not assume authentication automatically provides authorization.

Enforce authorization close to the protected operation.

---

# 33. Data Access Rules

Do not allow arbitrary database access everywhere.

Prefer:

```text
route/procedure
↓
service
↓
query
↓
db
```

rather than:

```text
random component
↓
db
```

This keeps security and business rules centralized.

---

# 34. Transactions

Use transactions when multiple mutations must succeed or fail together.

Example:

```ts
await db.transaction(async (tx) => {
  await tx.insert(projects).values(project);
  await tx.insert(projectMembers).values(member);
});
```

Do not use transactions for every query.

Transactions are for atomicity, not decoration.

---

# 35. Concurrency

Prefer concurrency where operations are independent.

Good:

```ts
const [user, projects, usage] = await Promise.all([
  getUser(userId),
  getProjects(userId),
  getUsage(userId),
]);
```

Do not unnecessarily serialize independent I/O.

But never parallelize operations that have ordering or transactional dependencies.

---

# 36. Caching

Use caching deliberately.

Possible layers:

```text
Next.js cache
TanStack Query cache
database indexes
prepared statements
CDN
application-level cache
```

Do not add Redis merely because the architecture diagram looks lonely.

Introduce a distributed cache only when there is a measurable need.

---

# 37. API Design

APIs should be:

```text
small
typed
predictable
versionable when necessary
validated
authenticated
authorized
```

Do not expose database tables directly as public API contracts.

Public contracts should represent product behavior.

---

# 38. Pagination

Never return unbounded datasets.

Prefer:

```text
cursor pagination
```

when datasets can grow substantially.

Example shape:

```ts
const input = z.object({
  cursor: z.string().optional(),
  limit: z.number().int().min(1).max(100).default(25),
});
```

Do not accept arbitrary:

```text
limit=999999999
```

---

# 39. External APIs

External providers are unreliable by definition.

Handle:

```text
timeouts
rate limits
retries
invalid payloads
provider errors
partial failure
idempotency
```

Do not blindly retry non-idempotent mutations.

Keep provider-specific logic inside a provider module.

Example:

```text
integrations/
├── stripe/
├── resend/
├── openai/
└── github/
```

Do not spread provider-specific code throughout the product.

---

# 40. Webhooks

Webhooks must be:

```text
verified
idempotent
observable
safe to retry
```

Persist event identifiers when provider events can be duplicated.

A webhook handler should be thin:

```text
verify
↓
parse
↓
dedupe
↓
dispatch
```

Do not bury a 700-line business workflow inside the route handler.

---

# 41. Background Jobs

Use background jobs for genuinely asynchronous work:

```text
emails
large imports
AI processing
webhook follow-up
reports
long-running jobs
scheduled work
```

Do not make every function asynchronous just because `async` exists.

---

# 42. Logging

Logs should contain useful operational information.

Good:

```text
request id
user id when appropriate
operation
duration
failure reason
provider
job id
```

Bad:

```text
console.log("here");
console.log("here2");
console.log(data);
```

Never log secrets or credentials.

---

# 43. Observability

Production-critical flows should be observable.

At minimum:

```text
errors
latency
database failures
external provider failures
background job failures
important mutations
```

Do not build an enormous observability platform before the product requires it.

---

# 44. Security

Always assume external input is hostile.

Validate:

```text
request bodies
query parameters
headers where relevant
webhooks
uploaded files
URLs
environment configuration
provider responses
```

Use least privilege.

Do not expose secrets.

Do not trust client-provided authorization claims.

---

# 45. Performance Rule

First optimize the obvious things:

```text
database queries
N+1 queries
network round trips
large payloads
unnecessary serialization
unnecessary API hops
render waterfalls
unbounded queries
```

Only then consider more sophisticated mechanisms.

Do not optimize imaginary bottlenecks.

---

# 46. Architecture Rule

The preferred architecture is:

```text
simple
modular
typed
domain-oriented
observable
testable
```

Not:

```text
maximally abstract
maximally distributed
maximally clever
```

The best backend is the one another strong engineer can understand quickly.

---

# 47. Folder Structure

A strong default:

```text
src/
├── app/
│   ├── api/
│   ├── actions/
│   └── ...
│
├── db/
│   ├── index.ts
│   ├── schema/
│   │   ├── users.ts
│   │   ├── projects.ts
│   │   └── ...
│   ├── queries/
│   │   ├── users.ts
│   │   ├── projects.ts
│   │   └── ...
│   └── migrations/
│
├── modules/
│   ├── users/
│   │   ├── service.ts
│   │   ├── router.ts
│   │   ├── schemas.ts
│   │   └── types.ts
│   ├── projects/
│   │   ├── service.ts
│   │   ├── router.ts
│   │   ├── schemas.ts
│   │   └── types.ts
│   └── billing/
│
├── lib/
│   ├── auth/
│   ├── env/
│   ├── errors/
│   └── ...
│
└── rpc/
    ├── router.ts
    ├── context.ts
    └── client.ts
```

Adapt the structure to the actual application.

Do not manufacture empty folders.

---

# 48. File Size

Avoid giant files.

As a heuristic:

```text
< 200 lines → normal
200–400       → inspect
400–800       → likely split
800+          → strong reason required
```

These are heuristics, not laws.

A 500-line cohesive schema file may be acceptable.

A 500-line function is almost certainly a smell.

---

# 49. Do Not Create Abstractions Prematurely

Bad:

```text
IRepository
Repository
RepositoryFactory
RepositoryManager
ServiceFactory
ServiceRegistry
DIContainer
```

when the application simply needs:

```ts
await db.select().from(users);
```

Use direct code until duplication or complexity creates a real reason for abstraction.

---

# 50. DRY, But Not Dogmatically

Do not duplicate genuinely shared behavior.

But do not create abstraction layers merely to remove two repeated lines.

Prefer:

```text
clarity
>
clever reuse
```

A little duplication can be cheaper than a bad abstraction.

---

# 51. Latest-Technology Rule

When choosing a library or API:

```text
Current stable
>
maintained
>
well-supported
>
appropriate
>
simple
```

Do not choose a technology because it is merely new.

Do not stay on an obsolete major because the model remembers it better.

Do not invent versions.

Always verify when the exact current version matters.

---

# 52. No Technology Worship

Do not use:

```text
oRPC
Redis
Kafka
Temporal
GraphQL
gRPC
microservices
CQRS
event sourcing
```

because they sound sophisticated.

Use a technology only when it solves a real problem.

---

# 53. Elite Backend Principle

The standard is:

> **Extremely modern technology with extremely boring architecture.**

That means:

```text
latest stable frameworks
+
excellent type safety
+
clear modules
+
simple control flow
+
strong database design
+
minimal abstraction
+
real observability
+
strict verification
```

This is preferable to an architecture diagram that looks like a multinational corporation accidentally hired twelve consultants.

---

# 54. Final Completion Checklist

Before declaring backend work complete:

```text
□ TypeScript passes
□ Biome lint passes
□ Biome formatting passes
□ No unnecessary comments
□ No `any` added casually
□ No giant files
□ No accidental duplicated logic
□ Database schema is synchronized
□ Migration strategy is correct
□ Environment variables are validated
□ API inputs are validated
□ Authorization is enforced
□ Queries are bounded
□ N+1 queries are avoided
□ Independent I/O is concurrent
□ External API failures are handled
□ Webhooks are idempotent
□ Secrets are never logged
□ Architecture is simpler than necessary, not more complicated than necessary
□ Latest relevant stable APIs were verified
```

## 55. When to Adapt This Stack (9/10 completeness)

If the project is not Drizzle/Neon (e.g., Prisma, PlanetScale, Supabase), keep 90%: modular domain slices, `strict` TS, Zod at boundaries, small files, and this checklist. Replace `§10-13` Drizzle specifics with the equivalent for the chosen ORM, but do not add a second ORM.

# Final Rule

**Build the most modern backend that remains extremely easy to understand.**

Use modularity aggressively.

Use types aggressively.

Use validation at boundaries.

Use Drizzle directly.

Use Neon Postgres correctly.

Use oRPC where a typed API boundary is useful.

Use Server Actions where direct server mutations make more sense.

Use TanStack Query for client-side server state.

Use database batching, transactions, indexes, pagination, and concurrency where they solve real problems.

Keep files small.

Keep modules focused.

Keep comments rare.

Keep architecture simple.

Keep the entire backend production-safe.

**Modern technology. Simple architecture. Extreme type safety. Zero unnecessary bullshit.**
