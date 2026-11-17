# Current handoff

## Status

Rust migration implemented on 2026-11-18. Version 0.2.0 replaces the Go/Node checker with one Rust executable. Source, comments, diagnostics and documentation are English. Build entry points are shell scripts. The authored skill remains `maintainable-code`.

## Delivered

- Checker source is one Cargo package under `checker/`, separated from skill instructions.
- ast-grep supplies structural rules and embedded grammars; Oxc supplies JS/TS syntax and scoped mutation/type-reference analysis. Git/Docker/config exclusions and extension routing run before analysis.
- One CLI supplies selection, partial checks, metrics, static provenance and unsupported-check reporting. Go formatting is explicitly unsupported; project gofmt and type-check commands remain required.
- Reusable defaults and YAML rules have single owners. The old Go source, Node profile, bridge and pnpm dependencies are removed from current source and new release ZIPs.
- Build/package/install use `scripts/*.sh`. Python standard-library tools own ZIP distribution; no Python participates in checking.

## Verification

- CLI regressions passed 14/14, including native syntax/style, decorators, overload scope, semantic mutation, type-only imports, partial-check isolation, nested ignores, normalized import paths, Cargo target dependencies and failure cases.
- Docker selection matched the previous Moby-backed checker for 14 reference patterns; those expectations are retained as fixtures.
- rustfmt and Clippy with warnings denied passed. Windows MSVC static-runtime release build passed through `sh scripts/build.sh`.
- Distribution regressions passed 7/7, including replacement/rollback, retained recovery backup and instruction-only packaging.
- Rust checked all 33 selected LinkOS files with 0 errors and the same two max-depth warnings at `apps/desktop/scripts/dev.mjs:59` and `:61`. Preserve those warnings for review.
- The 3.65 MB Windows ZIP installed successfully into an isolated skills directory. Its release executable ran with an empty PATH and no sidecar files; native JS/TS, JSON, Go, dependencies and comments completed successfully. Installed-version receipts own the final source revision and archive hash.

## Next action

Review version 0.2.0 and publish the repository/release when ready. Keep future work scoped to an actual rule or language consumer. Model review remains unimplemented and opt-in by design. No LinkOS application behavior or phase approval changes with this migration.
