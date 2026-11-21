# Checker architecture

The runtime is one Rust executable with embedded native checks and optional adapters for tools on PATH. It reads source and reports findings. It accepts project policy as JSON data and runs adapters with read-only arguments. Build and ZIP distribution scripts run on the producer's machine, separately from analysis.

## Ownership

| Owner | Responsibility |
|---|---|
| `checker/src/main.rs` | CLI arguments, sequencing, output and exit codes |
| `checker/src/config.rs` | Strict typed configuration, overrides and rule policy |
| `checker/src/selection/` | Explicit scope, extension routing, Git/Docker exclusions and skip reasons |
| `checker/src/checks/mod.rs` | Available checks, dispatch and actual per-check coverage |
| `checker/src/checks/javascript.rs` | Structural rules, exports and import organization |
| `checker/src/checks/semantics.rs` | Oxc syntax, binding mutation, single-use helpers and type-only imports |
| `checker/src/checks/directives.rs` | JS/TS comment directives, statement-scoped bypasses and their audit entries |
| `checker/src/checks/ast.rs` | Native grammar parsing, Go inspection and comment groups |
| `checker/src/checks/metrics.rs` | Nesting, branch complexity and source-line measurements |
| `checker/src/checks/manifests.rs` | JSON/TOML syntax and declared dependency counts |
| `checker/src/checks/tools.rs` | Optional tool invocation, report decoding and coverage |
| `checker/src/report.rs` | Findings, locations, metrics and JSON envelope |
| `checker/config/`, `checker/rules/` | Embedded defaults and ast-grep YAML rules |
| `checker/tests/` | CLI regressions and reference fixtures |
| `skills/maintainable-code/` | Agent workflow and checking reference |
| `scripts/` | Shell build entry and standard-library Python ZIP distribution |

Use one Cargo package. Add a crate or trait when an actual second consumer or implementation needs it. Keep lifetime-heavy Oxc and ast-grep types local to their adapters. Reports own their strings and numbers. A new rule should concentrate edits in its language owner, declarative rule file and regression fixture.

## Analysis libraries

ast-grep embeds only the Go, JavaScript and TypeScript grammars used by current checks. Declarative patterns use ast-grep configuration, with explicit checks for export ordering and path-dependent import policy. Oxc supplies syntax validation and scoped reference analysis for `prefer-const`, single-use helpers and type-only imports. Files with decorators preserve value imports because decorator metadata may require them at runtime.

These responsibilities require two JS/TS parse representations. This adds build dependencies and parse work, but avoids maintaining a parser or name resolver. The implementation does not embed Oxlint's unpublished linter crate or execute Node plugins. Cargo.lock pins the graph; upgrades require the fixtures and shell build to pass.

## Selection contract

Apply embedded and project `checkignore.json`, scoped root/nested `.gitignore`, root `.dockerignore`, and extension routing before opening a file for analysis. Any excluding policy can reject a path. Git excluded parents stay pruned. Docker negations may restore a descendant, so an excluded Docker directory can require traversal. Symlinks are skipped. Explicit paths follow the same rules and must remain inside the root.

`ignore` owns Git matching. Docker's context-root matching and negations are implemented separately with `globset`; fixtures are compared with the previous Moby-backed checker. Nested `.dockerignore` files do not create another context. The checker does not infer Dockerfile-specific contexts or load global user Git exclusions.

Tool checks use the selected file set. dependency-cruiser receives an exact path filter, validates edges within the selected JS/TS graph, and can read a declared project tsconfig for aliases. jscpd compares copies of selected JS/TS, Python and Go files in a temporary directory. Ruff and gofmt receive selected source paths. Adapters resolve tools from absolute PATH directories. On Windows the Node adapters can use npm's package entry points directly.

## Reports and limitations

Exit codes are 0 for completed checks without errors, 1 for error findings, and 2 for invalid configuration, operational failures or incomplete requested tool checks. Warnings remain visible without failing the command. A list operation reports selection without claiming analysis. `--checks` runs only selected checks. Per-check status and checked/unchecked counts describe coverage.

`--rules` narrows configurable JS/TS rule IDs while preserving project overrides and syntax validation. Statement-scoped JS/TS bypasses and file-scoped project bypasses preserve suppressed findings with their reasons. The optional `directives` check reports work comments as data for the skill to use when the user requests edits.

- Syntax and local binding analysis do not replace project type checking.
- Go parsing, imports, functions and comment groups use the embedded grammar. `go-format` uses read-only `gofmt -l`. Run package-level Go linters through the project workflow.
- Dependency measurements count manifest declarations, not installed trees, vulnerability findings or bundle size. Go module analysis covers `require` declarations without resolving modules.
- Complexity and depth use the supplied native metric definitions. The engine does not host arbitrary ESLint plugins. Unknown rules and unsupported options fail configuration validation.
- The standalone checker makes no model calls. The skill provides opt-in contextual review of selected files and can use native findings as evidence. The CLI owns its diagnostics and exit codes.

## Distribution

`sh scripts/build.sh` checks formatting, tests and lints on the host, then builds the requested target. Windows MSVC builds link the C runtime statically. ZIPs contain skill instructions, references, the executable, source metadata and resolved dependency notices. They contain no Go checker source, `.mjs` or Node packages. Python is needed for the ZIP installer, not for running the checker.
