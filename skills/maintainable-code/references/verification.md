# Verification

## Run and report checks

1. **Before editing**, name the observable result and the affected entry point. Find an existing command or a concrete repro in the project instructions, scripts, or CI. For a reproducible bug, capture the failure on the original code. For a new static rule, use a fixture that should trigger it and a similar valid fixture that should pass.
2. **After editing**, use the matching static checks below and run the project's configured commands over the affected scope. For changed behavior, run the smallest test or manual repro that observes the result through the relevant entry point. Re-run the original repro when available. Include a failure or recovery case when the change affects it.
3. **At handoff**, report each command or manual action, what it covered, and what happened. Include warnings or coverage gaps that affect the outcome. When a check could not run, say which check, why, and which claim remains unverified.

| Change | Static check to run |
| --- | --- |
| JS/TS or Python fallback decisions, long conditions, or nesting | Run the project's configured lint or [SkillFoundry rules](smells.md), such as `maintainable/fallback-order`, `maintainable/long-condition`, or `max-depth`. Check the rule's severity in `checkignore.json` before running a rule-specific check. Use ESLint's `no-else-return` for an unnecessary `else` after a return. [ESLint rule](https://eslint.org/docs/latest/rules/no-else-return) |
| Module imports or ownership boundaries | Run the compiler for import resolution and the project's declared dependency rules for allowed edges or cycles. SkillFoundry can run `dependency-boundaries` on its configured JS/TS graph. |
| Repeated implementation | Run `duplication` or the project's clone detector in the feature and its likely peers. Inspect reported pairs to see whether they share a contract. |
| Test assertions | Run the configured test linter. For assertions a branch can skip, use a framework rule such as Jest or Playwright's `no-conditional-expect`. [Jest rule](https://github.com/jest-community/eslint-plugin-jest/blob/main/docs/rules/no-conditional-expect.md) · [Playwright rule](https://github.com/mskelton/eslint-plugin-playwright/blob/main/docs/rules/no-conditional-expect.md) |

Reuse a configured compiler, lint rule, or dependency check when it covers the pattern. Use the target project's policy to choose which checks block CI. For a scoped SkillFoundry run, include the selected rule, checked file count, and expected finding or clean result as evidence.

For example, a change to fallback order should run the [fallback-order check](smells.md#maintainablefallback-order--deterministic) on the changed module and test the case where two fallback conditions both match. The test asserts which mode wins according to the project's contract.

## What to check

Choose checks based on the failure the change could introduce. Use reproducible static rules where the criterion can be stated precisely. Use the project's CI policy to decide which findings block a merge.

| Quality | Automate where the criterion is explicit | Evidence that needs project context |
| --- | --- | --- |
| Readability | Check nesting, complexity, duplication, and configured import boundaries. | Trace the main flow from its public entry point. Check whether names reveal intent, decisions are easy to follow, and state and side effects have clear owners. Explain a finding using an actual caller or a likely change. |
| Correctness | Run the compiler, typed lint, test lint, and existing regression tests. | Verify affected failure and recovery paths, especially partial writes, retries, and compatibility with existing data. Use properties for rules across many inputs and model checking when event order is the risk. Report the verified scope and material unknowns. |
| Performance | Enforce declared budgets with a repeatable benchmark or build measurement. Use static patterns for precise risks, such as a known query API called inside a loop. | Compare before and after on equivalent inputs and environments. Measure the metric the change could affect, such as latency, query count, bundle size, or memory. Profile the affected path when the cause is unclear. |
| Business logic | Encode declared domain constraints in types, schemas, or database constraints where feasible. Execute approved decision tables and invariant tests in CI. | Derive expected outcomes from the project specification or domain owner's decision. Check precedence, boundaries, and affected callers. Surface conflicting requirements and material assumptions for human review. |

For example, when changing bulk import, trace which layer owns duplicate detection, test retry after a partial failure, and compare query counts for the same representative batch. Obtain the expected behavior for duplicate rows from the project contract and use it to judge the actual output.

## Prepare human review

Put the changed behavior, key code locations, and strongest evidence first. Include a representative failure case or measurement when it affects confidence. Identify the domain decisions, tradeoffs, and remaining risks that require human judgment. Link each material claim to a test result, trace, benchmark, or relevant source.

## Write tests around the contract

- Start with the outcome a caller can observe, such as a return value, public state, or required side effect. Read the requirement when deriving the expected result. Keep helper calls, incidental ordering, and private state out of assertions. Assert a call count when the count is itself required, such as one submission per accepted order.
- Keep an exact fixture for a specified result, meaningful boundary, or known regression. Group cases with the same contract in a readable table. Use [property-based testing](https://fast-check.dev/docs/introduction/what-is-property-based-testing/) when a relationship should hold over a broad input domain. For example, a sorted result must be ordered and contain each input value the same number of times. Generate empty lists, duplicates, and varying lengths. Ordering alone also accepts an implementation that always returns an empty list.
- Keep expected results independent of the implementation under test. Use a public rule, a trusted fixture, or a simpler reference implementation. For important new tests, demonstrate that a plausible defect fails the assertion. Use the original repro, a temporary behavior-breaking change, or a focused mutation run. A behavior-preserving refactor should still pass.

Static checks can catch specific weak test patterns. Assess whether an assertion captures the product contract by reading the callers and testing a representative failure.

## Check event ordering with TLA+

Use a small [TLA+ model](https://lamport.azurewebsites.net/tla/high-level-view.html) when retries, concurrent actions, or delayed events can violate a high-value property. Describe the initial state, allowed transitions, and invariant. Check that the transitions reflect the real state owner and which steps can run independently. Keep the project's model, its assumptions and finite bounds, and the rerun command with the project. Replay important counterexamples through the real code as regressions where practical.

This one-attempt example starts pending. Cancellation is accepted only while pending. The deliberately faulty `Complete` action also accepts a late response after cancellation. The `cancelled` flag records that cancellation was accepted even if the status is overwritten.

Save this as `CancelResponse.tla`.

```tla
---- MODULE CancelResponse ----
VARIABLES status, cancelled

Init == /\ status = "Pending"
        /\ cancelled = FALSE

Cancel == /\ status = "Pending"
          /\ status' = "Cancelled"
          /\ cancelled' = TRUE

Complete == /\ status \in {"Pending", "Cancelled"}
            /\ status' = "Succeeded"
            /\ UNCHANGED cancelled

Next == Cancel \/ Complete
Spec == Init /\ [][Next]_<<status, cancelled>>
NoSuccessAfterCancel == cancelled => status # "Succeeded"
====
```

Save this as `CancelResponse.cfg`. The example allows terminal states with no further actions.

```text
SPECIFICATION Spec
INVARIANT NoSuccessAfterCancel
CHECK_DEADLOCK FALSE
```

Run TLC with `java -jar tla2tools.jar CancelResponse.tla` using the [TLA+ tools JAR](https://github.com/tlaplus/tlaplus/releases). TLC finds `Pending -> Cancelled -> Succeeded`. Change the first condition of `Complete` to `status = "Pending"` and rerun it. The invariant then holds over the reachable states of this model. The check covers one attempt and two modeled actions. Apply the same property to the production entry point to check its behavior.
