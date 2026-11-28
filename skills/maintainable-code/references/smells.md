# Code smells

## Shortcuts

```text
$maintainable-code /add-smell Private lookup wrapper with one direct caller
$maintainable-code /add-smell Review internal fallbacks that hide unexpected failures
$maintainable-code /review-smells src/features/orders
```

## Add a smell

Use the supplied example or source location. Read the relevant caller and contract if they affect the rule. Record a short name, the maintenance cost, and a pair of examples showing when to raise the finding and when to leave the code alone. Reuse an existing entry when it already covers the same problem. Write the entry here in the SkillFoundry source checkout. If the checkout is unavailable, return the entry and its intended path.

- **Deterministic.** State the exact observable condition, language scope, exceptions, and the native rule or existing project command that can check it. Verify an existing checker against the examples when available. Mark a new check as proposed until it runs. When checker implementation is requested, start with a failing CLI regression and follow the repository instructions for verification and policy changes.
- **Contextual review.** State what the reviewer should inspect, what makes the issue worth fixing, and when the current design is justified. Include the public behavior to preserve if the example changes it.

## Review a scope

Resolve the named files or feature, then read the relevant declarations, callers, data boundaries and tests. Apply the contextual rules below or the ones the user names. The current agent performs the review. Use one read-only subagent only when explicitly requested. Report findings with file and line, observed behavior or maintenance cost, and the smallest justified change. Mark material gaps in evidence. When reviewing static findings, give each a disposition such as inline, retain with a reason, checker false positive, or needs evidence. The native CLI owns its findings and exit code.

## Rules

### `maintainable/single-use-helper` · Deterministic

A private JS/TS or Python helper with one direct call site adds a jump for the reader. The existing CLI checks resolved bindings and reports the declaration. See [native rule scope and bypasses](checks.md#local-exceptions-and-work-directives) for the precise match and exclusions.

In the example below, `get` only forwards the order lookup and has one direct caller. The revised example keeps the lookup where its result is used. A helper with multiple callers falls outside this signal. Review resource ownership and behavior before deciding whether a reported helper should be inlined.

### `maintainable/object-spread-newline` · Deterministic

Separate entries with line breaks in a JavaScript or TypeScript object literal that has a spread and at least three other properties, or a Python dict literal with a `**` unpacking and at least three other entries. The checker reports one finding at the object when adjacent entries share a line. The rule defaults to off. To opt in, add this rule to the project's `checkignore.json`.

```json
{
  "rules": {
    "maintainable/object-spread-newline": "warn"
  }
}
```

For example, expand `{ ...data, id, source, target }` so each entry has its own line. A compact update such as `{ ...data, id }` stays within the rule's limit.

### `maintainable/long-condition` · Deterministic

An `if` condition that joins four or more terms with `&&` or `||`, or `and` or `or` in Python, can hide the order of decisions. The checker reports one finding per `if`, `else if` or `elif`. Parentheses, negation and line breaks do not change the count. A function call, comparison or `??` expression counts as one term. The rule defaults to off. Set `"maintainable/long-condition": "warn"` in the project's `checkignore.json` to enable it.

For example, split `if (strict && !building && !level && !kiosk)` into an outer `if (strict)` and an inner `if (!building && !level && !kiosk)`. Preserve short-circuit behavior when separating conditions that call functions.

### `maintainable/fallback-order` · Deterministic

Make fallback precedence readable from top to bottom. The checker warns on two shapes in JS/TS and Python:

- An `else` branch whose only statement assigns a ternary, such as `else mode = mobile ? 'mobile' : 'desktop'` or Python's `mode = "mobile" if mobile else "desktop"`.
- Consecutive `if` statements in the same statement list that start with the same variable's `=== null` or `is None` test followed by `&&` or `and` and each only assign to that variable. The finding points to the repeated guard.

A branch may use braces. A standalone ternary, a guard for a different variable, or independent state updates stay within the rule's limits. For an ordered choice, `input.mode ?? inferMode()` can retain an explicit value and defer inference. Within `inferMode`, early returns can express each choice in priority order. Preserve short-circuit behavior, particularly when a condition calls a function.

The rule defaults to off. Set `"maintainable/fallback-order": "warn"` in the project's `checkignore.json` to enable it.

```sh
maintainable-check check --root . --rules maintainable/fallback-order --json -- src
```

Use the affected source path in place of `src`. The rule filter selects the check and uses the project's configured severity. Confirm that the report includes the intended files and that the expected warning appears in a known example before relying on a clean result.

### `maintainable/long-jsx-class-name` · Deterministic

Group long class lists so layout, focus, and selected styles are easy to find. The checker reports one finding per JSX `className` attribute when a static string or template fragment contains 20 or more whitespace-separated classes on one source line. It examines conditional branches and string arguments to `cn`, `clsx`, `classnames`, and `classNames`.

A single string with 20 classes raises a finding. The same classes in two strings on separate lines of `cn()` pass. Short lists and dynamic values such as `className={buttonClass}` pass. The warning concerns the source layout, so a component can keep its current styling mechanism.

### `maintainable/playwright-style-assertion` · Deterministic

Assert user-visible behavior in Playwright flow tests. The checker inspects JS/TS `.spec.*` and `.test.*` files that import `@playwright/test`. It reports `toHaveCSS()` and reads of `getComputedStyle()`, `boundingBox()`, `scrollWidth`, or `scrollHeight` used in `expect()` or `expect.poll()`. It follows one local variable binding that stores a measurement.

For example, a floor navigation test should assert the active floor and pressed route step. An exact transition time can go in a dedicated visual test. Files under a `visual` directory and files named `*.visual.spec.*` or `*.visual.test.*` pass. Coordinates used only for an interaction, accessibility attributes and ordinary visibility matchers pass. Use a scoped `@bypass` with a reason when geometry is a requirement of a particular flow.

### `maintainable/complex-mapped-button` · Deterministic

Keep a mapped list focused on the data and its action. The checker reports a native `<button>` in the callback passed directly to `.map()` when it has an `onClick` expression, uses a conditional, interpolated template or call for `className`, and has either an expression for an ARIA state or a direct JSX element or fragment child.

A route-hint button with conditional classes, `aria-pressed` and separate number and label spans raises a finding. A mapped `<FloorHintButton label={label} pressed={active} onClick={open} />` passes. A simple inline text button with a fixed class also passes. Use the feature's own props when it has distinct behavior or styling.

These three rules default to off. Set any of their values to `"warn"` under `rules` in the project's `checkignore.json`.

### `maintainable/similar-loops` · Deterministic

Flag JS/TS or Python loop bodies in different files that could need the same maintenance change. The checker compares JS/TS `for`, `for…in`, `for…of`, `while` and `do…while` bodies with each other, and Python `for` and `while` bodies with each other. It replaces ordinary identifier uses of bindings declared inside the loop with local identifiers in first-use order. It preserves names from outside the loop, property names, literals and operators. The similarity score is the number of shared three-token sequences divided by the number present in either body, counting repetitions. Comments, semicolons and type annotations are ignored.

Two loops that test `row.active` and push `normalizeId(row.id)` can still match when one calls the variable `item`. A loop that checks `item.expired` and calls `archive(item.id)` has a different body. Similar bodies can be intentional when iteration domains, lifecycle or ownership differ. Verify the loop headers and callers before sharing the implementation. See [thresholds and bypasses](checks.md#similar-loop-bodies).

### `explicit-feature-flow` · Contextual review

Make ownership, names, decisions and public behavior easy to follow. Report a finding when a caller or likely change exposes a specific cost.

- **File ownership.** Trace callers before suggesting a move. An order-only projection belongs with the orders feature. A shared module is justified when real consumers share the same contract.
- **Names and flow.** Name values for their role, such as `orders` and `orderId`. Use an early `if` return to make a missing or invalid case explicit. Keep a short lookup and projection together. Keep a separate helper when it owns a meaningful responsibility such as retry cleanup.
- **Defensive code.** Trace the input to the runtime boundary. Validate untrusted TypeScript inputs with the project's schema tool, such as Zod. Keep fallbacks for known recoverable failures. Flag repeated guards after validation or a broad catch that turns unexpected failures into an ordinary result.
- **Entry and bootstrap.** Trace one input from the request through the schema, the startup action, and the owning stores. Group related request reads, validate input once, and derive the shared type from the schema. Flag duplicate URL parsing or a page that sequences unrelated store operations. Keep HTTP response decisions with the request handler and tie client startup and cleanup to the mounted view. A conditional or costly read can stay behind the decision that needs it.
- **Render states and component ownership.** Follow ready, loading, and error paths and check what remains mounted when state changes. Flag nesting that hides which state controls the output, or an extraction that only sends the same props onward. Keep a clear local conditional in its owning component. Extract a component when it owns distinct behavior or has a real second consumer. Check the lifetime of shared elements such as a canvas or iframe bridge before changing the render structure.
- **FP or OOP.** Identify which rules and state change together. Use an object when it gives domain behavior, invariants, or lifecycle a clear owner. Use a pure function when the work is a transformation or calculation over explicit inputs. Explain how the choice affects an existing caller or a likely change.
- **Repeated setup.** Trace the callers and the cost of setup. For frequent or expensive construction, check whether callers can reuse the whole configuration or its stable parts. Verify when inputs change, who releases obsolete resources, and whether caching adds meaningful memory or invalidation costs. A route search can reuse a graph until the map changes. Keep local construction when reuse offers little benefit or the inputs change each time.
- **Overlapping requests.** Check whether requests can overlap and share results, errors, or loading state. Follow success, failure, cancellation, and late completion. Raise a finding when an older request can overwrite the current result or clear its loading state. Keep simple state when callers serialize requests or cancellation reliably prevents stale updates.
- **Tests.** Check observable behavior and meaningful failure paths. Flag assertions tied to an internal helper, call count, or exact call order when an equivalent implementation would fail them. Keep interaction assertions when count or order is part of the contract, such as one payment submission per order.

## Worked example

The HTTP boundary rejects malformed input with a 400. A valid request for an absent order returns a null summary. Order totals are the sum of each line's quantity times its unit price in cents.

### Before · `src/shared/utils.ts`

```typescript
type Order = { id: string; lines: { quantity: number; unitPriceCents: number }[] }

const get = (xs: readonly Order[], id: string) => xs.find(x => x.id === id)

const rec = (xs: readonly Order[], id: string) => {
  if (!Array.isArray(xs) || typeof id !== 'string') return null

  try {
    const x = get(xs, id)
    return x
      ? { orderId: x.id, totalCents: x.lines.reduce((n, l) => n + l.quantity * l.unitPriceCents, 0) }
      : null
  } catch {
    return null
  }
}

export { rec }
```

A test that only asserts `expect(findSpy).toHaveBeenCalledTimes(1)` can pass even if `rec` returns the wrong result. It can also fail when a behavior-preserving refactor uses a lookup table.

### After · `src/features/orders/order.ts`

```typescript
type OrderLine = { quantity: number; unitPriceCents: number }

class Order {
  constructor(readonly id: string, private readonly lines: readonly OrderLine[]) {}

  get totalCents(): number {
    return this.lines.reduce((total, line) => total + line.quantity * line.unitPriceCents, 0)
  }
}

export { Order }
```

The order owns the total formula. The request handler owns parsing, lookup, and the response cases.

### After · `src/features/orders/read-order-summary.ts`

```typescript
import { z } from 'zod'
import { Order } from './order'

const orderLookupSchema = z.object({
  orderId: z.string(),
  orders: z.array(z.object({
    id: z.string(),
    lines: z.array(z.object({
      quantity: z.number().int().positive(),
      unitPriceCents: z.number().int().nonnegative(),
    })),
  })),
})

const readOrderSummary = (input: unknown) => {
  const parsed = orderLookupSchema.safeParse(input)
  if (!parsed.success) return { status: 400 } as const

  const { orders, orderId } = parsed.data
  const orderRecord = orders.find(order => order.id === orderId)
  if (!orderRecord) return { status: 200, summary: null } as const

  const order = new Order(orderRecord.id, orderRecord.lines)
  return { status: 200, summary: { orderId: order.id, totalCents: order.totalCents } } as const
}

export { readOrderSummary }
```

```typescript
const orders = [
  { id: 'a', lines: [{ quantity: 2, unitPriceCents: 600 }] },
  { id: 'b', lines: [] },
]

expect(readOrderSummary({ orders, orderId: 'a' })).toEqual({
  status: 200, summary: { orderId: 'a', totalCents: 1200 },
})
expect(readOrderSummary({ orders, orderId: 'b' })).toEqual({
  status: 200, summary: { orderId: 'b', totalCents: 0 },
})
expect(readOrderSummary({ orders, orderId: 'missing' })).toEqual({ status: 200, summary: null })
expect(readOrderSummary({ orders: 'invalid', orderId: 'a' })).toEqual({ status: 400 })
```
