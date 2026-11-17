# Supplied checker programs

The skill ships a Go CLI at `bin/maintainable-check.exe` on Windows, with source under `cmd/` and `internal/`. Its native checks run from that executable with embedded defaults. The existing Node profile at `scripts/check.mjs` supplies JS/TS syntax and style analysis. Both use the Go selector for paths, ignore files and extension routing.

## Native Go CLI

Resolve the executable from the skill directory and pass the target project through `--root`. Flags precede optional relative paths.

```powershell
maintainable-check capabilities --json
maintainable-check select --root . --json
maintainable-check check --root . --checks dependencies,json --json
maintainable-check check --root . --checks go,comments src
```

| Check ID | Current scope | Result |
|---|---|---|
| `json` | JSON files | Strict syntax and error locations, including BOM handling. |
| `dependencies` | `package.json`, `go.mod` | Runtime/dev/peer/optional counts and unique declared package names, or direct/indirect Go requirements. |
| `go` | Go files | Standard Go parser/AST, gofmt comparison, import and function-declaration counts. |
| `comments` | Go files | AST comment-group counts and an optional presence rule. Comment-like string literals remain strings. |

Default native checks are `json,dependencies,go`. `--checks` chooses a subset. Unavailable check IDs are errors. The report lists selected versus actually checked files and per-check applicability. JS/TS structural rules use the Node profile today. Dependency metrics describe declarations in each selected manifest. Lockfile graph size and installed/transitive package counts are separate future checks.

## JS/TS AST profile

```powershell
node /path/to/maintainable-code/scripts/check.mjs --root .
node /path/to/maintainable-code/scripts/check.mjs --root . --list --json
node /path/to/maintainable-code/scripts/check.mjs --root . src/changed.ts
```

This profile supplies the pinned ESLint/parser dependencies and rules. It checks const-arrow bindings, function forms, separate bottom exports, type-only imports, and aliases for cross-directory static/dynamic imports and re-exports. Class methods, overloads, named Next-style defaults and Nest decorator metadata cases have fixtures. Depth, complexity and file size emit review warnings. JSON receives strict syntax checking. Go-family files are reported as requiring the native profile when using this entry point.

Run the project's existing compiler, formatter, typed Promise/unsafe-value checks and relevant behavior tests for their owning boundaries. Comment usefulness, semantic ownership, alias runtime resolution and cohesion remain review topics.

## Shared ignore and extension policy

The compiled defaults come from the skill's root `checkignore.json`. A project may supply its own `checkignore.json`, or choose one with `--config`. The selector resolves each ignore source independently, then excludes paths rejected by any source.

- `checkignore.json` adds positive forward-slash globs relative to the target root. Its extension mappings and explicit rule overrides govern applicable profiles.
- Root and nested `.gitignore` files apply with their directory scopes, ordering, anchoring and negation. An excluded Git parent directory stays pruned. Patterns apply to candidate paths, including tracked files.
- The target root's `.dockerignore` uses Docker's patternmatcher and ignorefile parser. Docker negations can reinclude descendants, so those excluded parents remain traversable when needed. Set `--root` to the intended context. Dockerfile-specific ignore files are outside this current scope.
- JSON output records `ignore`, `gitignore`, `dockerignore`, disabled/unsupported extensions and symlink skips. Explicit path arguments use the same policies. Symlinks are skipped.

Extension mappings support `javascript`, `typescript`, `json`, `go` and `gomod`. Set a suffix to `false` to skip it. The longest matching suffix wins case-insensitively. JavaScript/TypeScript `rules` and file-specific `overrides` apply through the supplied AST profile. Native optional limits live under `limits`.

```json
{
  "ignore": ["archive/**", "tests/generated/**"],
  "extensions": { ".jsx": false, ".source": "typescript", ".generated.ts": false },
  "limits": { "directDependencies": 40, "importsPerFile": 15, "requireComments": false },
  "overrides": [
    { "files": ["fixtures/**"], "rules": { "func-style": "off" } }
  ]
}
```

The example limits are project choices. Dependency/import limits are inactive until configured. Comment presence is opt-in. Exclusion, rule and threshold changes are visible policy changes. Explain costly rules, false positives and custom-rule maintenance when they affect adoption.

## Output and future model review

The JSON report has `schemaVersion: 1`. Findings carry file, line, column, severity, rule, message and `origin: "static"`. Native reports also carry typed metrics and per-check coverage. List/selection mode records zero checked files. Exit 0 means selected checks produced no errors, exit 1 means findings at error severity, and exit 2 means configuration or execution failure. Warnings remain visible and require a reasoned disposition in the handoff.

A future small-model review can consume the selected findings/metrics and bounded source ranges from the same selection. Keep that stage explicit, budgeted and separately attributable through finding origin, model/prompt version and usage. Deterministic checks and their findings retain their own provenance. Review comment usefulness or cohesive structure in that stage when requested. The current implementation supplies static analysis and the report boundary.

## Source, build and verification

Read [go.md](go.md) before changing Go code. Go 1.26 builds the current source. From the skill's source checkout, `sh build.sh` checks gofmt, runs Go tests/vet on the host, then builds with `CGO_ENABLED=0`, frozen modules and trimmed build paths. Set `GOOS` and `GOARCH` to cross-build. The resulting native executable carries its defaults. Rebuild after Go code or embedded defaults change.

The JS/TS profile uses Node 22.14+ and its own locked package dependencies. Install once in `scripts/` with `pnpm.cmd install --frozen-lockfile --ignore-scripts`, then verify with `pnpm.cmd test`. The tests exercise the actual CLI and shared selector against isolated projects.

For future native JS/TS expansion, evaluate ast-grep for declarative AST rules and Oxlint for its existing lint catalog before implementing more custom analysis. The supplied JS/TS rules already exercise the user's keyword/style conventions. Both candidate tools have their own coverage and runtime packaging choices to review in a later working slice. [ast-grep](https://ast-grep.github.io/guide/introduction.html), [Oxlint](https://oxc.rs/docs/guide/usage/linter.html).

Selection uses upstream libraries and their syntax. [Git ignore](https://git-scm.com/docs/gitignore), [Docker ignore](https://docs.docker.com/build/concepts/context/#dockerignore-files), [go-git matcher](https://pkg.go.dev/github.com/go-git/go-git/v5/plumbing/format/gitignore), [Docker matcher](https://pkg.go.dev/github.com/moby/patternmatcher). Native Go checks use the standard parser and AST. [Go parser](https://pkg.go.dev/go/parser).
