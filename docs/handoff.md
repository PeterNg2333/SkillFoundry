# Current handoff

Updated 2026-11-19.

## Status

- The standalone checker is at 0.3.0. `/check` resolves paths and exact native rule IDs. The current JS/TS rules cover syntax, selected style and import policy, nesting, complexity, file length, and private helpers with one direct caller. The CLI reports unsupported checks explicitly and supports audited, statement-scoped `@bypass` comments.
- `/add-smell` records a deterministic check or contextual rule from examples. `/review-smells` is an explicit read-only skill prompt that examines callers, boundaries, and tests. Native findings and CI exit codes come from `/check`. The [worked example](../skills/maintainable-code/references/smells.md) covers naming, feature ownership, early returns, a Zod input boundary, FP/OOP, and behavior-focused tests.
- The 0.3.1-smells.2 Windows preview was built from a working tree with source changes. The version for a formal release is pending.

## Verified evidence

- `sh scripts/build.sh` passed rustfmt, all 18 CLI tests, Clippy, and the Windows release build. All seven distribution tests and the skill validator passed. The preview passed isolated installation, ZIP hash verification, and a standalone run with an empty PATH. Refreshing `dist/SHA256SUMS` was blocked by read permissions on the existing 0.3.0 ZIP.
- The worked example passed strict TypeScript and runtime Zod checks. The native scanner reported one single-use lookup helper before the rewrite and zero after it. The examples verified normal totals, zero totals, missing orders, and rejection of malformed input at the revised boundary.
- At Playback revision `caaf55a`, the export fix reduced the `app-temp/web` single-use scan from 34 to 25 findings across 106 TypeScript files. The focused recording scan returned 10 findings across eight files. Manual source review suggested inlining two, retaining seven, and testing the `audioContext` failure path. `speaking` and `backoff` are examples of a local predicate and a helper that owns cleanup.
- The Playback default scan reported 1,006 errors across 137 files, including 966 from function, export, and import style rules. A separate AST scan of 105 JS/TS source files found 20 catch clauses, with zero empty blocks and zero direct rethrows. The C# backend is outside native coverage.
- Earlier migration baselines covered 14 Moby ignore patterns, a 33-file LinkOS scan, and isolated installation of the 0.2.0 and 0.3.0 ZIPs. See [changelog](../CHANGELOG.md) and [architecture](architecture.md).

## Existing tools

| Scope | Existing tools |
| --- | --- |
| Maintainability baselines | SonarQube tracks code smells, cognitive complexity, duplication, and metrics for quality gates on new code. [SonarQube metrics](https://docs.sonarsource.com/sonarqube-server/user-guide/code-metrics/metrics-definition) |
| Syntax and type signals | ESLint checks redundant catches, depth, and cyclomatic complexity. typescript-eslint can flag conditions whose truthiness is fixed by the type. [ESLint rules](https://eslint.org/docs/latest/rules/) · [Type-aware conditions](https://typescript-eslint.io/rules/no-unnecessary-condition/) |
| Dependencies and duplication | dependency-cruiser validates declared import boundaries and cycles. jscpd finds duplicate token sequences and near-matches using normalized function ASTs. [Dependency rules](https://github.com/sverweij/dependency-cruiser/blob/main/doc/rules-reference.md) · [jscpd similarity](https://jscpd.dev/guides/similarity) |
| Design smells and contextual review | JDeodorant detects Java smells such as Feature Envy and God Class. CodeRabbit combines path-specific review guidance with optional ast-grep patterns. [JDeodorant](https://github.com/tsantalis/jdeodorant) · [CodeRabbit review rules](https://docs.coderabbit.ai/pr-reviews/review-instructions) |

## Decisions pending

1. **CI policy.** Decide which TS rules should block CI on the pilot project. Review single-use severity and the function, export, and import rules against the Playback baseline before adopting a project profile. Project overrides belong in `checkignore.json`.
2. **Next check.** Decide whether the next gap is duplication or import boundaries. For duplication, compare jscpd token and AST results on a single feature and assess which pairs should change together. For boundaries, inspect the project's intended import graph before writing dependency-cruiser rules. Use real counterexamples to justify a new native rule.
3. **Reviewer acceptance.** Compare `/review-smells` with the ten manually reviewed recording findings. Check whether it cites the real caller and relevant behavior tests, preserves ownership and lifecycle boundaries, and marks uncertainty about failure paths. Test the optional subagent path when explicitly requested.
4. **Release.** Decide when to publish the shortcuts after the pilot. Build the candidate ZIP from a clean commit and verify the checksum index and installed skill before promoting it.
