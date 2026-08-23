> **9/10 — keep 90%:** Catalog of 20 killers, keep tone. For 9/10 use, scan the ranked table `§644` first, then jump to the one killer matching the trace. This file is the _catalog_, `skills/performance/SKILL.md` is the _workflow_ — use together.

For a **backend-heavy Next.js 16 x+ (for example right now we are at nextjs 16.3+) application**, Next.js itself usually isn't the thing murdering your latency. The real killers are the interaction between **request-time rendering, database access, uncached server functions, authentication, serialization, external APIs, and accidental waterfalls**.

Next.js 16 changed the caching model substantially: dynamic code executes at request time by default unless you explicitly opt into caching with Cache Components / `"use cache"`. That is a major architectural difference from older App Router assumptions. ([Next.js][1])

## The big performance killers

### 1. Database queries inside Server Components

This is probably the #1 issue in backend-heavy apps.

```ts
export default async function Page() {
  const user = await db.user.findUnique(...)
  const projects = await db.project.findMany(...)
  const analytics = await db.analytics.findMany(...)

  return ...
}
```

That's potentially:

```text
request
  ↓
user query
  ↓
projects query
  ↓
analytics query
  ↓
render
```

You just created a serial request waterfall.

Do:

```ts
const [user, projects, analytics] = await Promise.all([
  getUser(),
  getProjects(),
  getAnalytics(),
]);
```

And even better, consolidate related database work when the database can answer it efficiently in one query.

---

### 2. Calling your own API from the server

This shit is surprisingly common:

```ts
const data = await fetch("https://yourapp.com/api/projects");
```

from a Server Component.

You're already on the server.

You've turned:

```text
Server → DB
```

into:

```text
Server → HTTP → API route → DB
```

For internal server code, call the service/database layer directly.

```ts
const projects = await projectService.getProjects(...)
```

API routes should primarily be boundaries for external/client callers, not an elaborate game of telephone inside your own backend.

---

### 3. Authentication on every fucking layer

You can easily end up doing:

```text
page
 ├─ auth()
 ├─ getUser()
 ├─ getOrg()
 ├─ getPermissions()
 ├─ getSubscription()
 └─ getFeatureFlags()
```

And then each abstraction quietly hits your database.

Five innocent-looking functions become 15 queries.

Create a request-scoped authentication/context layer and reuse the result.

---

### 4. Uncached expensive computation

Next.js 16's new Cache Components model makes this particularly important.

By default, dynamic work executes at request time. `"use cache"` lets you explicitly cache functions/components/pages. ([Next.js][1])

For example:

```ts
export async function getCompanyAnalytics(companyId: string) {
  // expensive DB queries
}
```

If this gets hit on every request, you're paying repeatedly for identical work.

Cache stable data:

```ts
export async function getCompanyAnalytics(companyId: string) {
  "use cache";

  // expensive computation
}
```

Obviously don't blindly cache user-specific or security-sensitive data. Cache boundaries have to respect identity and authorization.

---

### 5. Giant server-side object serialization

You fetch:

```ts
const customer = await db.customer.findUnique({
  include: {
    invoices: true,
    contacts: true,
    activities: true,
    notes: true,
    events: true,
  },
});
```

Then send the whole damn thing through the React Server Component boundary.

You just paid for:

- database transfer
- Node memory
- serialization
- RSC payload generation
- network transfer
- client parsing

Select exactly what you need:

```ts
select: {
  id: true,
  name: true,
  status: true,
}
```

Don't ship a database schema through the UI because autocomplete made it easy.

---

### 6. Huge client components

This one doesn't necessarily destroy backend latency, but it can absolutely destroy perceived performance.

A server component doing:

```tsx
<Dashboard>
  <MassiveClientComponent />
</Dashboard>
```

means you're dragging that component and its dependencies into the client bundle.

The general architecture for serious Next.js applications should be:

```text
Server
  ↓
data fetching
  ↓
server-rendered UI
  ↓
tiny interactive client islands
```

Not:

```text
everything → "use client"
```

That pattern is basically React's version of setting your CPU on fire and wondering why the fan is loud.

---

### 7. `use client` propagating through the component tree

This is especially nasty.

Put:

```tsx
"use client";
```

at a high-level layout/component and you've potentially dragged a huge dependency graph into the client.

Keep client boundaries extremely low.

Prefer:

```text
DashboardShell [server]
 ├─ Header [server]
 ├─ Analytics [server]
 └─ FilterDropdown [client]
```

instead of:

```text
Dashboard [client]
 ├─ Header
 ├─ Analytics
 ├─ Charts
 └─ Everything else
```

---

### 8. External API waterfalls

Backend-heavy SaaS applications often do:

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

This is catastrophic latency.

Independent operations should execute concurrently:

```ts
const [customer, subscription, crm, enrichment] = await Promise.all([
  getCustomer(),
  getSubscription(),
  getCRM(),
  getEnrichment(),
]);
```

For expensive external work, move it into background jobs rather than blocking page rendering.

---

### 9. AI calls during page requests

This deserves its own category.

Doing:

```text
GET /dashboard
 ↓
LLM call
 ↓
embeddings
 ↓
vector search
 ↓
database
 ↓
render
```

is an excellent way to turn a 200 ms application into a 4-second application.

For AI-heavy software:

```text
request
 ↓
cached/current state
 ↓
render
```

and:

```text
background workflow
 ↓
LLM
 ↓
embedding
 ↓
classification
 ↓
persist result
```

Then the UI reads the result.

Don't make the browser wait while a stochastic machine writes a dissertation.

---

### 10. Bad database connection management

This becomes ugly under serverless/server-based deployments.

If you're repeatedly creating database clients/connections instead of correctly reusing/pooling them, latency can explode under load.

For PostgreSQL especially, connection pooling matters enormously.

Architecture should look more like:

```text
Next.js
   ↓
pool / managed DB connection
   ↓
Postgres
```

rather than a fresh expensive connection lifecycle for every operation.

This is often mistaken for "Next.js is slow."

It isn't.

Your database connection strategy is slow.

---

### 11. Middleware / `proxy.ts` doing expensive work

In Next.js 16, `middleware.ts` is replaced by `proxy.ts`, and `proxy.ts` runs on Node.js. ([Next.js][1])

Don't put:

```ts
proxy()
 ├─ database query
 ├─ permission lookup
 ├─ organization lookup
 ├─ API call
 └─ expensive computation
```

on every request.

Proxy should generally be lightweight:

```text
request
 ↓
cheap routing/auth boundary
 ↓
application
```

Not "let's run our entire backend before the page even starts."

---

### 12. Sequential Server Actions

This:

```ts
await updateUser();
await updateSubscription();
await updatePreferences();
await updateAnalytics();
```

can become a 4-step waterfall.

When operations are independent:

```ts
await Promise.all([
  updateUser(),
  updateSubscription(),
  updatePreferences(),
  updateAnalytics(),
]);
```

When they're dependent, obviously don't parallelize them like a drunk engineer with a benchmark.

---

### 13. Over-fetching from ORMs

Prisma/Drizzle/etc. make it incredibly easy to pull gigantic structures.

Bad:

```ts
include: {
  users: true,
  projects: true,
  events: true,
  logs: true,
  invoices: true,
}
```

Better:

```text
query only fields required for this operation
```

And for expensive dashboards:

```text
pre-aggregate
materialize
cache
incrementally update
```

Don't calculate a year's worth of analytics from raw event rows because "Postgres is fast."

Eventually Postgres sends you a strongly worded invoice.

---

### 14. Rendering enormous tables server-side

A dashboard with:

```text
10,000 rows
×
20 columns
×
nested components
```

is going to hurt.

Use:

- pagination
- cursor pagination
- virtualization
- selective columns
- server-side filtering
- server-side sorting

Do not render the universe because the database returned it.

---

### 15. Excessive `await` in component trees

This is subtle.

You can have:

```tsx
<Page>
  <Header />
  <Stats />
  <Revenue />
  <Users />
  <Activities />
</Page>
```

where each component independently waits on data.

That can produce request-time waterfalls depending on how the tree/data dependencies are structured.

Think in terms of a dependency graph, not just React components.

---

### 16. Bad caching architecture

Next.js 16's caching is now more explicit.

Cache Components uses `"use cache"` and integrates with Partial Prerendering. ([Next.js][1])

For a serious SaaS, classify data into:

```text
STATIC
  marketing copy
  docs
  configuration

SEMI-DYNAMIC
  analytics
  product metadata
  organization settings

REAL-TIME
  notifications
  active jobs
  recent events
  live status
```

Then choose caching/revalidation based on the actual freshness requirement.

Don't treat every database query as real-time.

---

### 17. Prefetching too much

Next.js 16 changed its prefetching behavior with layout deduplication and incremental prefetching. ([Next.js][1])

But indiscriminate navigation/prefetch behavior can still create unnecessary requests.

For giant applications, think carefully about:

```tsx
<Link prefetch={false}>
```

when navigation targets are expensive and users aren't likely to visit them.

---

### 18. Massive dependencies imported into server code

Something like:

```ts
import entireSdk from "giant-sdk";
```

can inflate:

- server bundle
- cold starts
- memory
- initialization time

Next.js 16.1 introduced an experimental bundle analyzer specifically to identify bloated dependencies and bundle problems, including server code affecting cold starts. ([Next.js][2])

This is very relevant to backend-heavy SaaS.

---

### 19. Heavy module initialization

This:

```ts
const model = initializeHugeModel();
const client = initializeHugeSDK();
const embeddings = initializeEmbeddingPipeline();
```

at module scope can make cold starts fucking awful.

Prefer lazy initialization where appropriate:

```ts
let client: Client | undefined;

function getClient() {
  client ??= createClient();
  return client;
}
```

Especially for AI/ML SDKs.

---

### 20. Logging too much

This is underrated.

Doing:

```ts
console.log(JSON.stringify(enormousObject));
```

for every request is not free.

And if you're shipping:

```text
request
 ↓
DB
 ↓
serialize 4MB object
 ↓
logger
 ↓
Sentry
 ↓
structured logging
```

you have invented a latency tax.

Next.js 16 improved development request logging to show where time is spent between compilation and rendering, and 16.2 added server-function execution logging for debugging. ([Next.js][1])

Use those measurements instead of guessing.

---

# For your kind of application, I'd rank the killers like this

| Problem                      |                                 Typical impact |
| ---------------------------- | ---------------------------------------------: |
| DB waterfalls                |                                     🔴 Extreme |
| External API waterfalls      |                                     🔴 Extreme |
| LLM calls in request path    |                                     🔴 Extreme |
| Missing caching              |                                     🔴 Extreme |
| Bad DB connection pooling    |                                     🔴 Extreme |
| Huge ORM queries             |                                        🔴 High |
| Server → internal API calls  |                                        🔴 High |
| Heavy `proxy.ts`             |                                        🟠 High |
| Giant RSC payloads           |                                        🟠 High |
| Excessive `use client`       |                                        🟠 High |
| Huge client bundles          |                                        🟠 High |
| Dependency bloat             |                                 🟠 Medium/High |
| Excessive logging            |                                      🟡 Medium |
| React rendering inefficiency |                                      🟡 Medium |
| Turbopack                    | 🟢 Usually not your production latency problem |

And this last point matters for what you've been seeing.

**If your logs show something like:**

```text
GET /dashboard
next.js:          250ms
proxy:              8ms
application-code: 1700ms
```

then **stop blaming Next.js**.

That measurement is basically screaming:

```text
Next.js framework overhead: small
Your application work: huge
```

Next.js 16.2 itself shipped substantial rendering and startup improvements, including reported ~50% faster rendering and ~400% faster `next dev` startup. ([Next.js][3])

For a **60k+ LOC backend-heavy SaaS**, the optimization target isn't "make Next.js faster." It's designing the request path so that:

```text
REQUEST
  ↓
cheap auth/context
  ↓
parallel data acquisition
  ↓
cached stable data
  ↓
minimal computation
  ↓
stream UI
  ↓
background expensive work
```

rather than the classic human-built software architecture:

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
feature flags
  ↓
render
```

That's where your seconds are going.

[1]: https://nextjs.org/blog/next-16?utm_source=chatgpt.com "Next.js 16 | Next.js"
[2]: https://nextjs.org/blog/next-16-1?utm_source=chatgpt.com "Next.js 16.1 | Next.js"
[3]: https://nextjs.org/blog/next-16-2?utm_source=chatgpt.com "Next.js 16.2 | Next.js"
