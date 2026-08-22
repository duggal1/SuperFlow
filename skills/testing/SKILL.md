---

name: debugging-strategies
description: Systematic debugging and backend testing for TypeScript systems. Use for real backend changes, regressions, performance issues, integration failures, and complex bugs. Skip for trivial backend edits.
--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------

# Debugging Strategies

> **9/10 — keep 90%:** This file is 790 lines. For 9/10 use, read `Two Testing Modes` `98` + `Serious Backend Workflow` `234` + `Scientific Debugging` `356` for the task. Keep `Bun Experimental` vs `Existing Backend` split — that distinction is the 9/10 value. Full read once, then reuse.

Systematic debugging is not guesswork. Reproduce the failure, form a hypothesis, test it, isolate the root cause, verify the fix, and repeat until the behavior is proven correct.

For serious backend work, **testing is mandatory**. Do not build an entire backend and only start testing afterward.

## When to Use This Skill

Use this skill for:

* Real backend changes
* New backend systems or modules
* Backend regressions
* Complex bugs
* Performance problems
* Integration failures
* Data-flow problems
* Async or concurrency problems
* Production failures
* Changes requiring systematic verification

For tiny backend edits with an obvious local impact, this process may be reduced.

---

# Core Rules

## 1. Always Test

Every meaningful backend implementation must be tested.

Do not assume:

* The code compiles, therefore it works
* A function looks correct, therefore it works
* The API responds once, therefore the system works
* A previous implementation proves the new one works

Verify actual behavior.

## 2. Test Before the Backend Is Fully Built

Do not spend a day implementing a backend and then discover that the architecture does not behave as expected.

For serious backend work:

1. Define the behavior to verify
2. Create the testing environment
3. Establish the test cases
4. Implement the smallest backend path
5. Run the tests
6. Inspect the result
7. Iterate
8. Expand the implementation only after the behavior is validated

The testing loop must run during implementation, not after it.

## 3. Use Bun

All testing infrastructure should use **Bun** unless there is a concrete technical reason not to.

Use Bun for:

* Running TypeScript
* Test execution
* Test scripts
* Local backend experimentation
* Test fixtures
* Test utilities
* Development tooling

Do not introduce another runtime unnecessarily.

## 4. TypeScript Only

All application code and testing code must be TypeScript.

Prefer:

* `.ts`
* `.tsx`
* Typed fixtures
* Typed test helpers
* Explicit interfaces and types
* Strict compiler settings

Avoid untyped escape hatches unless there is a documented reason.

---

# Two Testing Modes

There are two valid backend testing strategies.

Both are allowed.

## Mode 1: Experimental Testing

Use experimental testing when the purpose is to discover **what implementation actually works**.

This is exploratory engineering.

Create a temporary testing backend or isolated implementation specifically for experimentation.

Do **not** import the production backend.

The goal is to compare approaches rapidly.

Example:

```text
backend experiment
        ↓
approach A
        ↓
test
        ↓
approach B
        ↓
test
        ↓
approach C
        ↓
compare behavior
        ↓
select implementation
```

This mode is useful for:

* Unknown architecture
* New integrations
* Database approaches
* Queue designs
* API behavior
* Agent workflows
* Performance experiments
* Multiple possible implementations
* A/B testing of backend approaches

Experimental testing is allowed to be disposable.

The experiment exists to answer:

> "What actually works?"

It is not required to mirror the final production architecture.

### Experimental Testing Rule

Create the smallest possible experimental backend required to test the hypothesis.

Do not overbuild it.

---

# Mode 2: Existing Backend Testing

This is the default testing strategy.

When the backend already exists, **do not recreate it just to test it**.

Import the existing backend modules and test the actual implementation.

Example:

```text
existing backend
      ↓
import existing module
      ↓
execute real behavior
      ↓
assert result
      ↓
identify failure
      ↓
fix implementation
      ↓
rerun
```

The test should exercise the same backend code that production uses.

Use this mode for:

* Existing APIs
* Existing services
* Existing database modules
* Existing business logic
* Existing integrations
* Regression testing
* Backend refactors
* Bug fixes
* Performance verification

The purpose is to answer:

> "Does the backend we actually built behave correctly?"

Do not create a duplicate implementation merely to make the test easier.

---

# Choosing the Testing Mode

| Situation                             | Strategy         |
| ------------------------------------- | ---------------- |
| Unknown implementation                | Experimental     |
| Comparing multiple backend approaches | Experimental     |
| New integration behavior              | Experimental     |
| Performance experiment                | Experimental     |
| Existing backend bug                  | Existing backend |
| Regression                            | Existing backend |
| Refactor                              | Existing backend |
| API verification                      | Existing backend |
| Business logic verification           | Existing backend |
| Production fix                        | Existing backend |

Use experimentation to discover the solution.

Use existing-backend testing to verify the solution.

---

# Serious Backend Workflow

## Phase 1: Define the Behavior

Before implementation, define:

* What should happen?
* What inputs are valid?
* What outputs are expected?
* What should fail?
* What side effects should occur?
* What dependencies are involved?
* What edge cases matter?

Write the expected behavior before writing large amounts of backend code.

---

## Phase 2: Build the Test Harness

For meaningful backend work, create the testing environment first.

Example structure:

```text
backend/
  src/
  tests/
  fixtures/
  test-utils/
```

The harness should make it cheap to:

* Run a test
* Change an input
* Try another implementation
* Inspect output
* Repeat the experiment

Do not make testing expensive.

Expensive testing gets skipped. Humans have apparently learned this repeatedly and still keep designing systems that depend on everyone being unusually disciplined.

---

# Phase 3: Implement the Smallest Backend Path

Do not immediately build the complete backend.

Implement only enough functionality to test the first hypothesis.

Example:

```text
Request
  ↓
Validation
  ↓
Service
  ↓
Database
  ↓
Response
```

Test this path.

If it works, extend it.

If it fails, fix the smallest broken boundary.

---

# Phase 4: Run the Test

Every meaningful change should trigger a test.

Use the shortest useful loop:

```text
Change
  ↓
Test
  ↓
Observe
  ↓
Hypothesis
  ↓
Change
  ↓
Test
```

Do not accumulate ten unverified backend changes and then attempt to determine which one broke the system.

---

# Phase 5: Isolate the Failure

When something fails, reduce the problem.

Check boundaries individually:

```text
Input
 ↓
Validation
 ↓
Business Logic
 ↓
Database
 ↓
External Service
 ↓
Output
```

Determine the first boundary where actual behavior differs from expected behavior.

That is usually more useful than staring at the final error.

---

# Scientific Debugging Method

## 1. Observe

Record the actual behavior.

Do not substitute what you expected to happen.

## 2. Reproduce

Determine whether the failure is:

* Always reproducible
* Intermittent
* Data-dependent
* Environment-dependent
* Timing-dependent

## 3. Hypothesize

Generate plausible causes.

Prefer evidence-based hypotheses.

## 4. Experiment

Change one relevant variable.

Run the test again.

## 5. Analyze

Determine whether the result supports or rejects the hypothesis.

## 6. Repeat

Continue until the root cause is established.

---

# Debugging Techniques

## Binary Search

Reduce the problem space by testing boundaries.

Example:

```text
Entire request fails
        ↓
Service succeeds?
        ↓
Database succeeds?
        ↓
External call succeeds?
        ↓
Serialization succeeds?
```

Remove half the possible causes at each step.

---

## Differential Testing

Compare working and broken behavior.

```markdown
| Variable | Working | Broken |
|---|---|---|
| Input | A | B |
| Database state | Empty | Populated |
| Environment | Local | Production |
| Dependency version | Current | Previous |
| Timing | Immediate | Delayed |
```

Find the smallest meaningful difference.

---

## Trace Testing

Track a request through important backend boundaries.

```typescript
function traceStep(name: string, value: unknown): void {
  console.log(`[trace] ${name}`, value);
}
```

Use tracing to determine where state changes unexpectedly.

Avoid indiscriminate logging.

---

# TypeScript Debugging

Use TypeScript's type system as part of debugging.

Check:

* Incorrect types
* Nullable values
* Invalid unions
* Incorrect generics
* Missing return types
* Unsafe casts
* Incorrect async return values
* Invalid object shapes
* Incorrect dependency interfaces

Prefer fixing the type model over suppressing the compiler.

Avoid blindly using:

```typescript
const value = something as any;
```

A type error is often evidence of a real architectural mistake.

---

# Bun Testing

Use Bun as the default runtime and test runner.

Example:

```typescript
import { describe, expect, test } from "bun:test";

describe("OrderService", () => {
  test("calculates the order total", () => {
    const items = [
      { price: 20, quantity: 2 },
      { price: 10, quantity: 1 },
    ];

    const total = items.reduce(
      (sum, item) => sum + item.price * item.quantity,
      0,
    );

    expect(total).toBe(50);
  });
});
```

Run tests with Bun:

```bash
bun test
```

Use focused test execution while debugging:

```bash
bun test path/to/test.ts
```

Keep the iteration loop fast.

---

# Testing Real Existing Backend Code

Prefer importing the real implementation.

Example:

```typescript
import { describe, expect, test } from "bun:test";
import { createOrder } from "../src/services/order-service";

describe("createOrder", () => {
  test("creates an order with the expected total", async () => {
    const result = await createOrder({
      items: [
        { price: 20, quantity: 2 },
        { price: 10, quantity: 1 },
      ],
    });

    expect(result.total).toBe(50);
  });
});
```

This verifies the backend that actually exists.

Do not rewrite `createOrder` inside the test merely to prove that an independently written copy of the same logic works.

That proves almost nothing.

---

# Experimental Backend Testing

For unknown behavior, create an isolated experimental implementation.

Example:

```text
experiments/
  order-flow-a.ts
  order-flow-b.ts
  order-flow-c.ts
  order-flow.test.ts
```

Test several implementations against identical inputs.

```typescript
import { expect, test } from "bun:test";
import { implementationA } from "./order-flow-a";
import { implementationB } from "./order-flow-b";

test("compare implementations", async () => {
  const input = {
    items: [
      { price: 20, quantity: 2 },
      { price: 10, quantity: 1 },
    ],
  };

  const a = await implementationA(input);
  const b = await implementationB(input);

  expect(a.total).toBe(b.total);
});
```

The experiment exists to determine which implementation behaves correctly.

Once the winning approach is known, move the validated design into the real backend.

---

# Backend Testing Categories

For serious backend work, verify the relevant layers.

## Unit Behavior

Test isolated business logic.

## Integration Behavior

Test interactions between real backend modules.

## Data Behavior

Verify database reads, writes, constraints, transactions, and failure states.

## API Behavior

Verify:

* Inputs
* Validation
* Authentication
* Authorization
* Status codes
* Response shape
* Error behavior

## Failure Behavior

Test what happens when:

* Input is invalid
* Data is missing
* Dependencies fail
* Requests timeout
* Records conflict
* External services return unexpected data
* Concurrent operations occur

## Regression Behavior

Every important bug fix should gain a test that would fail if the bug returns.

---

# Performance Debugging

Profile before optimizing.

Measure:

* Request latency
* Database latency
* External service latency
* CPU usage
* Memory usage
* Query count
* Payload size
* Serialization cost

Common backend problems:

* N+1 queries
* Unbounded data loading
* Repeated expensive computation
* Excessive network calls
* Blocking operations
* Poor caching
* Large payloads
* Concurrency problems

Do not optimize based on intuition alone.

---

# Intermittent Bugs

For flaky behavior, investigate:

* Race conditions
* Timing dependencies
* Concurrent requests
* Retries
* Queue ordering
* Database transaction boundaries
* External service timing
* Shared mutable state

Increase observability.

Repeat the test many times.

Record the conditions under which the failure appears.

---

# Production Debugging

For production failures:

1. Gather evidence
2. Reproduce safely
3. Identify the failing boundary
4. Reproduce with representative data
5. Test the fix
6. Add regression coverage
7. Deploy the smallest safe change
8. Verify production behavior

Do not blindly modify production to "see what happens."

Production is not your laboratory.

---

# Backend Test Contract

Before declaring serious backend work complete, verify:

```markdown
- [ ] Expected behavior is explicitly defined
- [ ] Testing mode was selected
- [ ] Test harness exists
- [ ] Implementation was tested during development
- [ ] Happy path works
- [ ] Failure cases work
- [ ] Important edge cases work
- [ ] Existing backend code was tested directly when appropriate
- [ ] Experimental approaches were isolated when appropriate
- [ ] Regression coverage exists for important bugs
- [ ] Performance was measured when relevant
- [ ] TypeScript type checking passes
- [ ] Bun tests pass
- [ ] Final implementation was verified after the last meaningful change
```

---

# Common Mistakes

* Building the whole backend before testing it
* Testing only the happy path
* Reimplementing production logic inside tests
* Using experimental code as production architecture
* Changing many things before rerunning tests
* Ignoring type errors
* Assuming a successful response means correctness
* Testing mocks instead of important real behavior
* Fixing symptoms instead of root causes
* Removing a failing test because it is inconvenient
* Skipping tests because the change "is small"
* Optimizing before measuring

---

## Minimal Harness Template (9/10 — copy once)

```ts
// tests/setup.ts
import { describe, expect, test } from "bun:test";
import { yourService } from "@/modules/your/service";
// 1. define expected behavior above, 2. implement smallest path, 3. run `bun test`
```

Keep harness at `tests/` + `fixtures/` + `test-utils/` per `§252` — cheap to run, cheap to change.

# Final Rule

For any backend change large enough to require systematic engineering:

**Create the test loop first.**

Then:

```text
Define behavior
      ↓
Create test harness
      ↓
Choose testing mode
      ↓
Implement smallest backend path
      ↓
Test
      ↓
Observe
      ↓
Debug
      ↓
Iterate
      ↓
Expand implementation
      ↓
Test again
      ↓
Verify final system
```

Experimental testing and testing the existing backend are both valid.

The mistake is not choosing the "wrong" one.

The mistake is **building a large backend first and discovering afterward that nobody knows whether it works**.
