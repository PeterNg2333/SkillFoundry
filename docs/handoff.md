# Current handoff

## Status

Version 0.3.0 adds scoped checking and work directives to the `maintainable-code` skill. The Rust migration in 0.2.0 established the standalone executable and shell build and distribution entry points.

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

### 0.2.0 migration verification

- CLI regressions passed 14/14, including native syntax/style, decorators, overload scope, semantic mutation, type-only imports, partial-check isolation, nested ignores, normalized import paths, Cargo target dependencies and failure cases.
- Docker selection matched the previous Moby-backed checker for 14 reference patterns; those expectations are retained as fixtures.
- rustfmt and Clippy with warnings denied passed. Windows MSVC static-runtime release build passed through `sh scripts/build.sh`.
- Distribution regressions passed 7/7, including replacement/rollback, retained recovery backup and instruction-only packaging.
- Rust checked all 33 selected LinkOS files with 0 errors and the same two max-depth warnings at `apps/desktop/scripts/dev.mjs:59` and `:61`. Preserve those warnings for review.
- The 3.65 MB Windows ZIP installed successfully into an isolated skills directory. Its release executable ran with an empty PATH and no sidecar files; native JS/TS, JSON, Go, dependencies and comments completed successfully. Installed-version receipts own the final source revision and archive hash.

## Next action

Review 0.3.0 against a consuming project before publication. Use the resolved check scope and bypass reasons to evaluate adoption of the single-use-helper rule.
