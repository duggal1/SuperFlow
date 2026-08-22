# Performance Engineering Skill

> **9/10 — keep 90%:** This file is 1,162 lines. For 9/10 use, read `Purpose` + `§1 Request Chain` + `§2 DB First` + the one bottleneck section for the task. For deep cause reference, open `skills/performance/performance-causes/SKILL.md`. Do not read both fully when one suffices — this skill is the workflow, `performance-causes` is the catalog.

## Purpose

Make the application **measurably faster**, not merely "optimized-looking."

The agent must:

```text
Measure
→ Find bottleneck
→ Fix highest-impact issue
→ Measure again
→ Compare before/after
```

Never optimize blindly.

Never add performance machinery without evidence.

---

# 1. Highest-Priority Rule

For every slow path, inspect the entire request chain:

```text
UI
↓
Client state
↓
Network
↓
API / Server Action / oRPC
↓
Auth / context
↓
Service
↓
Database
↓
External APIs / AI
↓
Serialization
↓
Rendering
```

Find where the time is actually being spent.

Do not blame Next.js, React, Drizzle, Neon, or the browser without measurements.

---

# 2. Database Comes First

For backend-heavy applications, inspect the database path before doing superficial frontend optimization.

Every performance investigation must check:

```text
□ Number of DB round trips
□ Sequential DB queries
□ DB batching
□ Query shape
□ Selected columns
□ Joins
□ Indexes
□ Constraints
□ Pagination
□ N+1 queries
□ Aggregation
□ Connection strategy
□ Prepared statements where justified
```

The target is not:

> "Use caching."

The target is:

> **Get the required data with the fewest necessary database operations and the cheapest correct query plan.**

Drizzle's Neon integration supports batching, relational queries, and prepared statements; its documentation specifically highlights prepared statements for repeated query execution and Neon HTTP for single non-interactive operations.

---

# 3. Database Schema Must Be Modular

Do not let the schema become one giant file.

Required pattern for non-trivial projects:

```text
src/db/schema/
├── users.ts
├── organizations.ts
├── projects.ts
├── billing.ts
├── events.ts
└── index.ts
```

The database model must remain understandable.

Schema modularity is mandatory because:

```text
better organization
→ easier query optimization
→ easier indexing
→ easier ownership
→ easier migrations
→ easier reasoning
```

Do not sacrifice schema structure for convenience.

---

# 4. Database Batching

Aggressively reduce database round trips when operations can be grouped safely.

Bad:

```ts
const user = await getUser(id);
const projects = await getProjects(id);
const usage = await getUsage(id);
const billing = await getBilling(id);
```

Potentially better:

```ts
const [user, projects, usage, billing] = await Promise.all([
	getUser(id),
	getProjects(id),
	getUsage(id),
	getBilling(id),
]);
```

Better still when the database can answer the required information efficiently in fewer queries:

```text
1 optimized query
>
4 independent queries
>
4 sequential queries
```

For Neon + Drizzle, use `db.batch(...)` where the operations are genuinely batchable and doing so reduces network round trips.

Do not force unrelated queries into a batch just to brag about "batching."

---

# 5. Query Design

Never fetch more data than the operation requires.

Bad:

```ts
const customer = await getCustomer({
	with: {
		invoices: true,
		contacts: true,
		activities: true,
		notes: true,
		events: true,
	},
});
```

Better:

```text
Fetch exactly what this operation needs.
```

Optimize for:

```text
few columns
few rows
correct indexes
correct joins
bounded results
minimal serialization
```

Avoid N+1 queries aggressively.

---

# 6. Indexes and Schema Optimization

When a slow query is found, inspect its database shape.

Check:

```text
WHERE columns
JOIN columns
ORDER BY columns
frequent filters
unique lookups
foreign keys
range queries
```

Add indexes based on actual query patterns.

Do not create indexes everywhere.

Every index has storage and write-maintenance costs.

---

# 7. Query Performance

For expensive or repeated queries:

```text
Inspect generated SQL
→ inspect query plan
→ measure execution time
→ identify scan/join/sort bottleneck
→ optimize
→ re-measure
```

Use prepared statements when a repeated query benefits from them. Drizzle documents prepared statements and placeholders specifically for repeated execution.

Do not micro-optimize trivial queries.

---

# 8. API Round Trips

API batching matters just as much as DB batching.

Bad:

```text
Client
→ /api/user
→ /api/projects
→ /api/usage
→ /api/billing
→ /api/activity
```

Better:

```text
Client
→ one request for the required view data
```

Or use concurrent requests when the endpoints genuinely need to remain separate.

The rule:

> **Minimize network round trips without creating giant, unmaintainable endpoints.**

---

# 9. Server-to-Server Calls

Never call your own API from server code merely because an API endpoint already exists.

Bad:

```ts
const data = await fetch("https://app.com/api/projects");
```

from a Server Component or server-side function.

Prefer:

```text
Server
→ service
→ database
```

instead of:

```text
Server
→ HTTP
→ API route
→ service
→ database
```

Every unnecessary hop costs latency and serialization.

Your supplied performance analysis identifies this as a major latency source.

---

# 10. Eliminate Waterfalls

Any sequence like:

```text
A
↓
B
↓
C
↓
D
```

must be questioned.

If operations are independent:

```ts
const [a, b, c, d] = await Promise.all([
	getA(),
	getB(),
	getC(),
	getD(),
]);
```

If the database can consolidate them efficiently, prefer the database solution.

If the operations are genuinely dependent, keep the dependency.

Never parallelize blindly.

---

# 11. External API Performance

External calls are expensive.

Watch for:

```text
Stripe
HubSpot
Google
OpenAI
Anthropic
Vector DBs
Web scraping
Third-party APIs
```

Bad:

```text
DB
↓
Stripe
↓
HubSpot
↓
OpenAI
↓
Google
↓
render
```

Independent calls should execute concurrently where safe.

```ts
const [stripe, hubspot, enrichment] = await Promise.all([
	getStripeData(),
	getHubSpotData(),
	getEnrichment(),
]);
```

Long-running external work should move out of the request path.

---

# 12. AI Work Must Not Block Normal UI

Do not make the user wait for unnecessary AI computation.

Bad:

```text
GET /dashboard
↓
LLM
↓
embedding
↓
vector search
↓
classification
↓
DB
↓
render
```

Prefer:

```text
Request
↓
current persisted state
↓
render
```

and:

```text
background job
↓
AI processing
↓
persist result
↓
UI reads result
```

AI should be asynchronous unless the AI result is genuinely required to produce the immediate response.

Your supplied source correctly identifies LLM calls inside request paths as a major latency killer.

---

# 13. Caching Is Not Automatically Good

Never add caching simply because:

> "Caching improves performance."

It can make the system worse.

Bad caching can introduce:

```text
extra serialization
extra memory
invalidation complexity
stale data
wrong authorization boundaries
cache misses
duplicate storage
hard-to-debug behavior
```

The rule is:

```text
Measure
→ identify repeated expensive work
→ determine freshness requirements
→ add cache
→ measure again
```

Use caching only where it eliminates meaningful repeated work.

---

# 14. Cache by Data Characteristics

Classify data:

```text
STATIC
→ cache aggressively

SEMI-DYNAMIC
→ cache with controlled invalidation/revalidation

USER-SPECIFIC
→ cache carefully

REAL-TIME
→ usually don't cache blindly
```

Never cache user-specific data without considering identity and authorization.

Never cache data simply because the query is slow.

Sometimes the correct answer is to fix the query.

---

# 15. Next.js Performance

Do not optimize Next.js in isolation.

Inspect:

```text
Server Components
Server Actions
oRPC
RSC payload
client boundaries
cache boundaries
streaming
prefetching
server functions
```

Next.js 16's caching model is more explicit, and Cache Components / `"use cache"` should be used intentionally rather than treating every dynamic operation as automatically cached.

---

# 16. Keep Client Boundaries Small

Avoid:

```tsx
"use client";
```

at high-level components unless required.

Prefer:

```text
Server Component
├── Server Component
├── Server Component
└── Tiny Client Component
```

not:

```text
Entire Dashboard
→ "use client"
→ everything becomes client-side
```

Keep interactive islands small.

This reduces:

```text
JS
hydration
client parsing
client execution
```

Your supplied performance analysis specifically identifies unnecessary `use client` propagation as a major source of client-side cost.

---

# 17. Serialization

Never serialize enormous objects across boundaries.

Bad:

```text
database result
→ entire object graph
→ RSC payload
→ client
```

Prefer:

```text
database
→ minimal projection
→ minimal response
→ UI
```

Only send the fields the UI actually needs.

---

# 18. Frontend Perceived Performance

Performance is not only raw server latency.

The application must **feel fast**.

Use:

```text
optimistic updates
prefetching
streaming
progressive rendering
skeletons where useful
minimal client JavaScript
```

But use them deliberately.

---

# 19. Optimistic UI

For mutations where the outcome is predictable, update the UI immediately.

TanStack Query v5 supports two main approaches:

```text
UI-level optimistic rendering
Cache-level optimistic updates
```

The simpler approach is UI-level rendering from mutation variables. Cache manipulation is more appropriate when multiple parts of the UI need to reflect the optimistic state.

Example:

```tsx
const mutation = useMutation({
	mutationFn: createTodo,
	onSettled: () =>
		queryClient.invalidateQueries({
			queryKey: ["todos"],
		}),
});

const { isPending, variables } = mutation;

return (
	<ul>
		{todos.map((todo) => (
			<li key={todo.id}>{todo.text}</li>
		))}

		{isPending && (
			<li className="opacity-50">
				{variables}
			</li>
		)}
	</ul>
);
```

This makes the interface feel immediate without prematurely rewriting the cache.

---

# 20. Cache-Level Optimistic Updates

Use cache manipulation when multiple UI consumers need the optimistic state.

```ts
const mutation = useMutation({
	mutationFn: updateTodo,

	onMutate: async (nextTodo, context) => {
		await context.client.cancelQueries({
			queryKey: ["todos", nextTodo.id],
		});

		const previousTodo = context.client.getQueryData([
			"todos",
			nextTodo.id,
		]);

		context.client.setQueryData(
			["todos", nextTodo.id],
			nextTodo,
		);

		return {
			previousTodo,
		};
	},

	onError: (_error, nextTodo, result, context) => {
		context.client.setQueryData(
			["todos", nextTodo.id],
			result?.previousTodo,
		);
	},

	onSettled: (_data, _error, nextTodo, _result, context) => {
		return context.client.invalidateQueries({
			queryKey: ["todos", nextTodo.id],
		});
	},
});
```

Rollback must be considered.

Never implement optimistic updates without an error strategy.

TanStack Query explicitly supports rollback through the `onMutate` result.

---

# 21. Query Cache Discipline

Do not add TanStack Query everywhere just because it exists.

Use it for server state that benefits from:

```text
cache
deduplication
background refetch
mutation coordination
optimistic updates
pagination
```

Do not turn ordinary local UI state into server-state machinery.

---

# 22. Pagination

Never fetch thousands of records just because the database can.

Use:

```text
cursor pagination
limit
server-side filters
server-side sorting
```

For infinite queries, bound retained pages when appropriate.

TanStack Query v5 supports `maxPages`, specifically to limit retained/refetched infinite-query pages and reduce memory/refetch cost.

---

# 23. Tables

Large tables should use:

```text
server-side pagination
server-side filtering
server-side sorting
selective columns
virtualization when necessary
```

Never render:

```text
10,000 rows
×
20 columns
×
heavy React components
```

just because the API returned them.

---

# 24. Request Context

Avoid repeating expensive work inside every function.

Bad:

```text
page
├── auth()
├── getUser()
├── getOrganization()
├── getPermissions()
├── getSubscription()
└── getFeatureFlags()
```

when each operation independently hits the database.

Create appropriate request-scoped context and reuse already-resolved information.

Your source identifies repeated auth/context lookups as a common multiplier of database traffic.

---

# 25. Proxy / Middleware

Keep `proxy.ts` lightweight.

Do not place:

```text
database queries
external APIs
analytics
permission trees
heavy computation
```

into the request proxy unless absolutely necessary.

The proxy should be a cheap boundary.

Your supplied analysis explicitly identifies expensive proxy work as a performance risk.

---

# 26. Serialization and Payload Size

Measure:

```text
request payload
response payload
RSC payload
JSON size
hydration cost
client bundle size
```

Smaller payloads usually mean:

```text
less serialization
less network transfer
less parsing
less memory
less rendering work
```

Do not ship database-shaped responses when the UI needs five fields.

---

# 27. Dependency Cost

A package can be expensive even if the code using it looks tiny.

Inspect:

```text
bundle size
server initialization
cold start cost
dependency graph
unused imports
heavy SDK initialization
```

Avoid importing giant SDKs when a narrow API or submodule is sufficient.

---

# 28. Initialization Cost

Be suspicious of expensive module-scope initialization:

```ts
const hugeClient = initializeHugeSDK();
const model = initializeModel();
const pipeline = createPipeline();
```

If initialization is expensive and not always needed, use lazy initialization or another appropriate strategy.

Do not pay cold-start cost for functionality that isn't used.

---

# 29. Rendering

Do not optimize React rendering before checking:

```text
database
network
serialization
client bundle
```

Those often dominate.

Then inspect:

```text
unnecessary rerenders
large client trees
expensive computations
unstable props
large lists
unnecessary effects
```

---

# 30. Performance Testing Is Mandatory

Any meaningful optimization must be benchmarked.

Before:

```text
request: 2.1s
DB: 1.4s
API: 400ms
render: 300ms
```

After:

```text
request: 620ms
DB: 240ms
API: 180ms
render: 200ms
```

Do not claim:

> "This is faster."

without measurement.

---

# 31. Before / After Table

Every significant optimization should produce a table:

| Metric         | Before | After | Change |
| -------------- | -----: | ----: | -----: |
| Total request  |   2.1s | 620ms |   -70% |
| DB time        |   1.4s | 240ms |   -83% |
| DB round trips |      9 |     3 |   -67% |
| API calls      |      5 |     2 |   -60% |
| Payload        |  1.8MB | 420KB |   -77% |
| Client JS      |  510KB | 290KB |   -43% |
| TTFB           |   1.7s | 410ms |   -76% |

The actual measurements must come from the application.

Never fabricate benchmark numbers.

---

# 32. Measurement Workflow

For a slow page:

```text
1. Capture baseline
2. Identify request timeline
3. Measure DB queries
4. Measure network calls
5. Measure server computation
6. Measure payload size
7. Measure client JS
8. Find largest bottleneck
9. Fix it
10. Re-run the exact same benchmark
11. Compare before/after
12. Keep the change only if it actually improves the result
```

---

# 33. Optimization Priority

Use this priority order:

```text
1. DB round trips
2. DB query shape
3. DB schema / indexes
4. External API round trips
5. Request waterfalls
6. AI work in request path
7. Payload size / serialization
8. Server/client boundaries
9. Client bundle
10. Rendering
11. Caching
12. Micro-optimizations
```

The exact order can change when measurements show otherwise.

---

# 34. Never Optimize the Wrong Layer

If measurement says:

```text
Next.js: 200ms
Application: 1.8s
```

do not spend two hours optimizing framework configuration.

Fix the application.

If:

```text
DB: 1.5s
```

do not start memoizing React components.

Fix the query.

If:

```text
LLM: 3.2s
```

do not optimize button rendering.

Move or cache the AI work.

The supplied performance analysis makes this distinction explicit: application work often dominates framework overhead.

---

# 35. Performance Anti-Patterns

Reject these immediately:

```text
sequential independent queries
N+1 queries
unbounded queries
giant DB payloads
server → own API calls
AI inside page requests without necessity
huge client components
high-level "use client"
heavy proxy logic
blind caching
cache-everything architecture
unnecessary API calls
large JSON responses
giant tables
heavy module initialization
premature micro-optimization
```

---

# 36. Performance Engineering Rule

The application should move toward:

```text
REQUEST
↓
cheap auth/context
↓
parallel data acquisition
↓
optimized DB access
↓
minimal computation
↓
minimal serialization
↓
stream / render
↓
background expensive work
```

Not:

```text
REQUEST
↓
auth
↓
DB
↓
API
↓
DB
↓
Stripe
↓
HubSpot
↓
LLM
↓
DB
↓
analytics
↓
permissions
↓
render
```

That second architecture is how a 200 ms product becomes a 4-second product.

---

## 37b. How This Relates to `performance-causes` (avoid overlap)

Use this file as the *workflow* (measure → fix → verify). Use `skills/performance/performance-causes/SKILL.md` as the *catalog* of 20 concrete Next.js 16 killers. Do not duplicate — reference the catalog when you need the kill list, keep this file for the process and before/after table `§31`.

# 37. Final Rule

**Performance work must be evidence-driven.**

Do not:

```text
add cache
add Redis
add memoization
add prefetching
add batching
add indexes
split components
```

because they sound fast.

Instead:

```text
Measure
→ identify bottleneck
→ minimize work
→ minimize round trips
→ optimize data flow
→ make the UI feel immediate
→ measure again
```

The winning backend is usually not the one with the most performance features.

It is the one that does **less unnecessary work**.

**Fewer DB trips.
Fewer API trips.
Smaller payloads.
Less serialization.
Less client JavaScript.
Less request-time AI.
More parallelism where safe.
Better schema.
Better queries.
Optimistic UI where appropriate.
Measured before/after results.**
