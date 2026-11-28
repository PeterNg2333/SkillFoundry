# Checker architecture

The runtime is one Rust executable with embedded native checks and optional adapters for tools on PATH. It reads source and reports findings. It accepts project policy as JSON data and runs adapters with read-only arguments. Build and ZIP distribution scripts run on the producer's machine, separately from analysis.

## Ownership

| Owner | Responsibility |
|---|---|
| `checker/src/main.rs` | CLI arguments, sequencing, output and exit codes |
| `checker/src/catalog.rs` | Native rule IDs and the languages that implement each rule |
| `checker/src/config.rs`, `checker/src/config/structure.rs` | Strict typed configuration, per-file rule policy and file structure schema |
| `checker/src/selection/` | Scoped path traversal, exclusions, file structure matching and extension routing |
| `checker/src/checks/mod.rs` | Available checks, dispatch and actual per-check coverage |
| `checker/src/checks/language.rs` | Language contract: `Syntax` roles, binding `Facts` and grammar selection |
| `checker/src/checks/source.rs` | Per-file pipeline for JavaScript, TypeScript and Python |
| `checker/src/checks/javascript.rs` | JS/TS grammar roles, embedded ast-grep rules and export organization |
| `checker/src/checks/semantics.rs`, `checker/src/checks/ui.rs` | Oxc syntax, JS/TS binding facts, prefer-const, type-only imports, JSX and Playwright rules |
| `checker/src/checks/python.rs`, `checker/src/checks/python/scope.rs` | Python grammar roles, syntax validation and name binding |
| `checker/src/checks/metrics.rs` | Shared nesting, branch complexity and source-line measurements |
| `checker/src/checks/decisions.rs`, `literals.rs`, `imports.rs`, `helpers.rs` | Shared long-condition, fallback-order, object-spread-newline, cross-directory import and single-use helper rules |
| `checker/src/checks/loop_similarity.rs` | Cross-file fingerprints and import paths for similar loop bodies within one language family |
| `checker/src/checks/directives.rs` | Comment directives, master-spec work sections, and audited statement-scoped bypasses |
| `checker/src/checks/ast.rs` | Native grammar parsing, Go inspection and comment groups |
| `checker/src/checks/manifests.rs` | JSON/TOML syntax and declared dependency counts |
| `checker/src/checks/tools.rs` | Optional tool invocation, report decoding and coverage |
| `checker/src/report.rs` | Findings, locations, metrics and JSON envelope |
| `checker/config/`, `checker/rules/` | Embedded defaults and ast-grep YAML rules |
| `checker/tests/` | CLI regressions and reference fixtures |
| `checker/tests/parity/` | One case per rule with a flagged file for every declared language |
| `skills/maintainable-code/` | Agent workflow, checking reference, and local Interact activity recorder |
| `scripts/` | Shell build entry and standard-library Python ZIP distribution |

Use one Cargo package. Add a crate or trait when an actual second consumer or implementation needs it. The `Syntax` trait exists because JS/TS and Python implement the same rules. Keep lifetime-heavy Oxc and ast-grep types inside `checks/`. Reports own their strings and numbers.

## Shared rules across languages

Each rule is written once against the language contract and enabled per language in the catalog.

- **Catalog.** `catalog.rs` lists every native rule and its languages. `Policy` gives no severity to a rule that the file's language does not list, so an undeclared language cannot run it. Configuration validation, `--rules`, statement bypasses and `capabilities --json` (`ruleLanguages`) read the same table.
- **Contract.** `language.rs` defines `Syntax`, the roles that shared rules ask for. These cover comment bodies and bypass targets, statement lists, functions, nesting and branch points, `if`/`elif` decisions and their logical terms, null tests and plain assignments, object and dictionary entries, imports and module files, loop bodies and ignored tokens. Each adapter maps its tree-sitter node kinds to these roles and supplies wording for language constructs in diagnostics. A binding analysis supplies `Facts`: private helpers with resolved references, identifier-to-binding offsets and runtime imports. Oxc supplies them for JS/TS, and `python/scope.rs` supplies them for Python.
- **Parity corpus.** Each `tests/parity/<case>/expect.json` names one rule and the exact lines that each case file reports. Unlisted files must stay clean. `tests/parity.rs` fails when a declared rule and language have no flagged file, when a case uses an undeclared language, or when findings differ.

A rule whose construct exists in only one language stays in that language's adapter. Examples are const-arrow organization, `import type`, JSX and Playwright assertions, which the catalog declares for JS/TS only.

### Add a rule

1. Add a case under `checker/tests/parity/` with flagged and clean examples for every target language, plus any focused CLI regression. Confirm that it fails.
2. Add the rule ID and its languages to `catalog.rs`, and its default severity to `checker/config/defaults.json`.
3. Implement it in a shared module through `Syntax` and `Facts`, and call it from `source.rs`. When it needs a new role, add the role to `Syntax` and implement it in every adapter in the same change.
4. Run `sh scripts/build.sh`.

### Add a language

1. Enable its grammar in `checker/Cargo.toml`, map its extensions in `defaults.json` and accept the kind in configuration validation.
2. Implement `Syntax` in `checks/<language>.rs`, validate syntax, and supply `Facts` when helper or loop rules should run. Add the variant to `Language`.
3. Add the language to each rule it implements in `catalog.rs` and its check ID to the default checks.
4. Add a flagged file to each parity case it claims. The parity test lists every missing rule and language pair.

## Analysis libraries

ast-grep embeds only the Go, JavaScript, TypeScript and Python grammars used by current checks. Declarative patterns use ast-grep configuration, with explicit checks for export ordering and path-dependent import policy. Oxc supplies JS/TS syntax validation and scoped reference analysis for `prefer-const`, single-use helpers, loop bindings and type-only imports. Files with decorators preserve value imports because decorator metadata may require them at runtime. Python bindings are resolved natively over the tree-sitter tree.

Shared rules read the tree-sitter tree, and Oxc remains authoritative for JS/TS syntax. The grammar can recover from input that Oxc accepts, such as a TSX generic call. The shared decision and literal rules skip the recovered subtree. With these rules enabled, a comparison across eight JS/TS repositories reproduced all 11,638 earlier findings and their metrics.

JS/TS responsibilities require two parse representations. This adds build dependencies and parse work, but avoids maintaining a parser or name resolver. The implementation does not embed Oxlint's unpublished linter crate or execute Node plugins. Cargo.lock pins the graph; upgrades require the fixtures and shell build to pass.

## Selection contract

Both selection modes apply embedded and project ignores and scoped root/nested `.gitignore`. Git excluded parents stay pruned. Symlinks are skipped. Explicit paths follow the same rules and must remain inside the root. Source selection also applies root `.dockerignore` and extension routing before opening files. Docker negations may restore a descendant, so an excluded Docker directory can require traversal.

`ignore` owns Git matching. Docker's context-root matching and negations are implemented separately with `globset`; fixtures are compared with the previous Moby-backed checker. Nested `.dockerignore` files do not create another context. The checker does not infer Dockerfile-specific contexts or load global user Git exclusions.

The file structure check reuses scoped traversal and checks file and directory names, including assets outside source extension coverage. It skips subtrees marked with `$ignore` in the structure policy. It covers repository paths excluded from Docker and uses `globset` for inherited filename patterns and `termtree` for the text view.

Tool checks use the selected file set. dependency-cruiser receives an exact path filter, validates edges within the selected JS/TS graph, and can read a declared project tsconfig for aliases. jscpd compares copies of selected JS/TS, Python and Go files in a temporary directory. Ruff (`python-lint`) and gofmt receive selected source paths. Adapters resolve tools from absolute PATH directories. On Windows the Node adapters can use npm's package entry points directly.

## Reports and limitations

Exit codes are 0 for completed checks without errors, 1 for error findings, and 2 for invalid configuration, operational failures or incomplete requested tool checks. Warnings remain visible without failing the command. A list operation reports selection without claiming analysis. `--checks` runs only selected checks. Per-check status and checked/unchecked counts describe coverage.

`--rules` narrows native rule IDs while preserving project overrides and syntax validation. Each requested rule needs a selected language check that implements it. Statement-scoped bypasses in JS/TS and Python comments and file-scoped project bypasses preserve suppressed findings with their reasons. The optional `directives` check reports work comments and directives under `## Work` in Markdown-formatted `.spec` files. It applies project `workDirectives` scope and tag filters in addition to source selection. Master specs are selected only for explicit directive checks, which leaves ordinary analysis coverage unchanged.

- Syntax and local binding analysis do not replace project type checking.
- The opt-in similar-loops rule compares loop bodies within the selected files of one language family, JS/TS or Python. It reports both locations and optionally shows an import path of up to two edges. JS/TS evidence covers relative imports, without project aliases or Vue scripts. Python evidence covers relative imports and absolute modules found at the root or uniquely below it, such as under `src/`.
- Python syntax uses the embedded grammar. It reports parse errors and Python 2 `print` and `exec` statements. Compile-time errors such as `return` outside a function, and name or type errors, belong to `python-lint`, the interpreter or the project's type checker.
- Python binding analysis follows module, function, class and comprehension scopes, parameter defaults and annotations, and `global` and `nonlocal`. A module-level function is private when its name starts with `_` and `__all__` does not list it. A nested function is always private. Methods, decorated functions, generators and names bound more than once are outside the single-use helper rule. Star imports, `exec`, `getattr` and string references are not resolved.
- Go parsing, imports, functions and comment groups use the embedded grammar. `go-format` uses read-only `gofmt -l`. Run package-level Go linters through the project workflow.
- Dependency measurements count manifest declarations, not installed trees, vulnerability findings or bundle size. Go module analysis covers `require` declarations without resolving modules.
- Complexity and depth use the supplied native metric definitions. The engine does not host arbitrary ESLint plugins. Unknown rules and unsupported options fail configuration validation.
- The standalone checker makes no model calls. The skill provides opt-in contextual review of selected files and can use native findings as evidence. The CLI owns its diagnostics and exit codes.

## Distribution

`sh scripts/build.sh` checks formatting, tests and lints on the host, then builds the requested target. Windows MSVC builds link the C runtime statically. ZIPs contain skill instructions, references, the executable, source metadata and resolved dependency notices. They contain no Go checker source, `.mjs` or Node packages. Python is needed for the ZIP installer, not for running the checker.
