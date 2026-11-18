# Current handoff

## Status

Version 0.3.0 adds scoped checking and work directives to the `maintainable-code` skill. The Rust migration in 0.2.0 established the standalone executable and shell build and distribution entry points.

The Playback pilot verified the inline export fix on 2026-11-18. The Windows ZIP records its exact source commit in `distribution.json`.

## Delivered

- `/check [description]` resolves the requested paths and rules. `/help` shows usage. The CLI accepts exact JS/TS rule IDs through `--rules` and publishes them through `capabilities`.
- The `maintainable/single-use-helper` rule is an error by default because the skill prefers a single caller to own its helper logic. JS/TS checks accept rule-specific `@bypass` comments scoped to a following statement or declaration, with auditable reasons and unused-bypass diagnostics.
- The optional `directives` check lists `@impl`, `@refactor`, and `@check` comments. Python source checks use the target project's tools.
- Checker source is one Cargo package under `checker/`, separated from skill instructions.
- ast-grep supplies structural rules and embedded grammars; Oxc supplies JS/TS syntax and scoped mutation/type-reference analysis. Git/Docker/config exclusions and extension routing run before analysis.
- One CLI supplies selection, partial checks, metrics, static provenance and unsupported-check reporting. Go formatting is explicitly unsupported; project gofmt and type-check commands remain required.
- Reusable defaults and YAML rules have single owners. The old Go source, Node profile, bridge and pnpm dependencies are removed from current source and new release ZIPs.
- Build/package/install use `scripts/*.sh`. Python standard-library tools own ZIP distribution; no Python participates in checking.

## Verification

- For 0.3.0, the new CLI regression failed against 0.2.0 at the missing `--rules` flag. `sh scripts/build.sh` passed rustfmt, all 17 CLI tests, Clippy with warnings denied, and the Windows MSVC release build. All seven distribution tests and the skill frontmatter validator passed.
- The 0.3.0 Windows ZIP passed isolated installation and checksum verification. With an empty PATH, the installed executable verified rule and path filtering, an audited bypass, and directive discovery.

### Playback pilot

- At Playback revision `caaf55a`, the original checker reported 34 single-use helpers across 106 TypeScript files in `app-temp/web`. Nine were inline exports, including `useDismiss`, which is used by other modules. The regression covers named function and const exports, a default export, and a private function nested in an exported function.
- After the fix, `sh scripts/build.sh` passed all 18 CLI tests, rustfmt, Clippy, and the Windows release build. The focused run reported 25 findings in 16 files. Keep helpers that own a distinct lifecycle, such as the upload retry wait, eligible for a documented local bypass.
- The default run checked 137 files and reported 1,006 errors and 151 warnings. Of the errors, 966 came from existing function, export, and import style rules. Compared with the previous run, only the 15 single-use export false positives disappeared. The C# backend is outside the native check scope.

### 0.2.0 migration verification

- CLI regressions passed 14/14, including native syntax/style, decorators, overload scope, semantic mutation, type-only imports, partial-check isolation, nested ignores, normalized import paths, Cargo target dependencies and failure cases.
- Docker selection matched the previous Moby-backed checker for 14 reference patterns; those expectations are retained as fixtures.
- rustfmt and Clippy with warnings denied passed. Windows MSVC static-runtime release build passed through `sh scripts/build.sh`.
- Distribution regressions passed 7/7, including replacement/rollback, retained recovery backup and instruction-only packaging.
- Rust checked all 33 selected LinkOS files with 0 errors and the same two max-depth warnings at `apps/desktop/scripts/dev.mjs:59` and `:61`. Preserve those warnings for review.
- The 3.65 MB Windows ZIP installed successfully into an isolated skills directory. Its release executable ran with an empty PATH and no sidecar files; native JS/TS, JSON, Go, dependencies and comments completed successfully. Installed-version receipts own the final source revision and archive hash.

## Next phase: Code smell checks and optional subagent review

1. Start with the Playback single-use findings under `app-temp/web/src/features/recording`. Rerun the scoped static check and, when explicitly requested, delegate review to one read-only subagent. Give it each finding, the declaration, the actual caller, and relevant tests. Ask it to recommend inlining, retaining the helper with a factual bypass reason, fixing a checker false positive, or gathering more evidence. Require file and line references. `speaking` and `backoff` are useful examples with different outcomes.
2. Capture that workflow in the skill. Keep plain `/check` deterministic. An explicit request for a subagent enables contextual review of the selected findings. Keep the native findings and exit code authoritative for CI. Review reports recommend changes for the user to approve.
3. Add high-signal rules one at a time from real examples. Check empty or redundant exception handlers with AST rules or the target project's existing linter. Use compiler information for type-dependent rules and a dependency graph for module boundaries. Start duplicate detection with token or normalized AST matching. Evaluate code embeddings later if real duplicates with different structures are missed, and use similarity results as review candidates.

The subagent workflow and additional smell checks are planned work. The default Playback scan is a baseline for review.
