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

A private JS/TS helper with one direct call site adds a jump for the reader. The existing CLI checks resolved bindings and reports the declaration. See [native rule scope and bypasses](checks.md#local-exceptions-and-work-directives) for the precise match and exclusions.

In the example below, `get` only forwards the order lookup and has one direct caller. The revised example keeps the lookup where its result is used. A helper with multiple callers falls outside this signal. Review resource ownership and behavior before deciding whether a reported helper should be inlined.

### `maintainable/object-spread-newline` · Deterministic

Separate entries with line breaks in a JavaScript or TypeScript object literal that has a spread and at least three other properties. The checker reports one finding at the object when adjacent entries share a line. The rule defaults to off. To opt in, add this rule to the project's `checkignore.json`.

```json
{
  "rules": {
    "maintainable/object-spread-newline": "warn"
  }
}
```

For example, expand `{ ...data, id, source, target }` so each entry has its own line. A compact update such as `{ ...data, id }` stays within the rule's limit.

### `explicit-feature-flow` · Contextual review

Make ownership, names, decisions and public behavior easy to follow. Report a finding when a caller or likely change exposes a specific cost.

- **File ownership.** Trace callers before suggesting a move. An order-only projection belongs with the orders feature. A shared module is justified when real consumers share the same contract.
- **Names and flow.** Name values for their role, such as `orders` and `orderId`. Use an early `if` return to make a missing or invalid case explicit. Keep a short lookup and projection together. Keep a separate helper when it owns a meaningful responsibility such as retry cleanup.
- **Defensive code.** Trace the input to the runtime boundary. Validate untrusted TypeScript inputs with the project's schema tool, such as Zod. Keep fallbacks for known recoverable failures. Flag repeated guards after validation or a broad catch that turns unexpected failures into an ordinary result.
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
