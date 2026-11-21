# Native checker

The installed skill supplies one Rust executable at `bin/maintainable-check.exe` on Windows or `bin/maintainable-check` on Linux/macOS. Defaults and AST rules are embedded. Native checks require only the operating system. Opt-in tool checks use executables on PATH. Source lives in SkillFoundry's `checker/`.

## Skill shortcuts

Use `/help` to see the syntax and supported scope. These are skill prompt shortcuts. In a client with its own slash commands, invoke the skill explicitly, for example `$maintainable-code /check single-use helpers in src/routes`.

Use `$mc-init` or `$maintainable-code /init` to inspect the project and create its policy. Pass `questionnaire` to decide language coverage, style rules, and exclusions one topic at a time. The agent writes and verifies the configuration in either mode.

| Prompt | Checker arguments after `check --root <project> --json` |
|---|---|
| `/check` | Use default checks and project file selection. |
| `/check single-use helpers in src/routes` | `--rules maintainable/single-use-helper -- src/routes` |
| `/check TS prefer-const in src/orders.ts and src/users.ts` | `--checks typescript --rules prefer-const -- src/orders.ts src/users.ts` |
| `/check package dependency counts` | `--checks dependencies` |
| `/check dependency boundaries` | `--checks dependency-boundaries` |
| `/check duplicate code in src` | `--checks duplication -- src` |
| `/check Python lint and Go formatting` | `--checks python,go-format` |
| `/check pending AI work in src/routes` | `--checks directives -- src/routes` |

Use `/add-smell` to record a new smell and `/review-smells` to request contextual review. See [smell rules and examples](smells.md) for those skill prompts.

Resolve explicit paths relative to the target root. For descriptions such as “the order handlers” or “the changed TS files,” look up the matching paths first and pass only those paths. Use `capabilities --json` to validate rule and check IDs when needed. If a stated path scope resolves to zero files, report it and do not fall back to the whole project. If the requested rule is unavailable, report the limitation. Use an existing project command when the user requests an external check and the project supplies that command.

Report the resolved file and rule scope, findings with locations, and relevant skipped paths or bypasses. Language checks use `--checks`; `--rules` selects exact native JS/TS policy IDs and respects project severity and overrides. The checker still validates syntax before examining rules. A rule such as `no-restricted-syntax` can cover several patterns, so identify that shared scope when the request names one of them.

For broader checks on generated code, use the target project's configured tools. Examples include ESLint `no-useless-catch` and `@typescript-eslint/ban-ts-comment`, Ruff `S110` for `except: pass` and `TRY203` for a redundant re-raise, and Knip for module-level review. Apply only the requested and available rules.

## Commands and coverage

Resolve the executable from the installed skill and pass the target project through `--root`. Flags precede optional relative paths.

```sh
maintainable-check capabilities --json
maintainable-check select --root . --json
maintainable-check check --root . --json
maintainable-check check --root . --checks dependencies,json --json
maintainable-check check --root . --checks javascript,typescript src
maintainable-check check --root . --rules maintainable/single-use-helper -- src/routes
maintainable-check check --root . --checks directives -- src/routes
maintainable-check check --root . --checks go,comments src
```

| Check | Scope |
|---|---|
| `javascript`, `typescript` | Syntax, scoped prefer-const, const-arrow organization, bottom exports, type-only imports, cross-directory aliases, depth/complexity/line metrics |
| `json`, `toml` | Strict syntax with source locations |
| `dependencies` | Declared package groups and unique names in package.json/Cargo.toml; direct/indirect Go require declarations |
| `go` | Embedded grammar syntax, imports and function/method counts |
| `dependency-boundaries` | JS/TS dependency rules and cycles within selection via dependency-cruiser |
| `duplication` | Duplicate token sequences in JS/TS, Python and Go via jscpd 5 |
| `python` | Python lint and syntax via Ruff |
| `go-format` | Go formatting via gofmt |
| `comments` | AST comment groups and optional presence rule for JS/TS/Go |
| `directives` | Source locations and descriptions for `@impl`, `@refactor` and `@check` in JS/TS comments |

Plain `check` runs the native default groups and the project's `externalChecks`. Use `--checks` to choose exact groups. When `--rules` is supplied without `--checks`, check JavaScript and TypeScript only. Unknown or duplicate check and rule IDs fail. Reports state selected, checked and unchecked files, per-check applicability and coverage. `select` and `check --list` only list selection.

Keep project type checking, typed linting and behavior tests for their respective contracts. Manifest dependency metrics count declarations, and Go manifest inspection covers require directives. Native style metrics use the checker's definitions.

## Configuration and selection

Embedded defaults are owned by `checker/config/defaults.json` in the source checkout. A target project can provide `checkignore.json`, or select another JSON file with `--config`. It is parsed as data and never executed.

- Positive root-relative `ignore` globs add exclusions. Extension mappings and rule severity overrides merge with defaults.
- Root/nested Git ignores retain scope, ordering, anchors and negations. Git excluded parents stay pruned. Patterns apply to selected paths, including tracked files.
- Root Docker ignore has context-root matching and negations that may restore descendants. Set `--root` to the intended context. Dockerfile-specific contexts and global user Git exclusions are outside current coverage.
- Explicit paths use the same policy and must stay within root. JSON skip reasons include ignore provenance, disabled/unsupported extensions and symlinks.

Extension types are `javascript`, `typescript`, `python`, `json`, `toml`, `go` and `gomod`. The longest matching suffix wins case-insensitively. Set a mapping to `false` to disable it. Unknown keys, rules, severities and unsupported options fail configuration validation.

```json
{
  "styleNotes": ["Use const arrows for ordinary functions and collect exports at the end."],
  "rules": {"func-style": "error", "no-restricted-syntax": "error"},
  "bypasses": [{
    "files": ["src/framework/entry.ts"],
    "rule": "func-style",
    "reason": "The framework requires this function declaration."
  }],
  "ignore": ["archive/**", "tests/generated/**"],
  "extensions": {".source": "typescript", ".generated.ts": false},
  "limits": {"directDependencies": 40, "importsPerFile": 15, "requireComments": false},
  "overrides": [{"files": ["fixtures/**"], "rules": {"func-style": "off"}}]
}
```

Dependency/import limits are inactive until configured. Select `comments` to apply `requireComments`. Use `capabilities --json` for the native policy rule IDs, including `maintainable/single-use-helper`. IDs with legacy prefixes identify native policy rules in the standalone executable.

Rules accept off/warn/error or numeric severities. Depth/complexity accept numeric thresholds; max-lines accepts a number or max/skipBlankLines/skipComments options. Func-style supports the supplied expression form. Use file-specific overrides for explicit file policy. Changes to exclusions, thresholds or severities are policy changes that need explanation.

`styleNotes` gives agents the rationale for project conventions. The checker enforces `rules` and `overrides`. Each `bypasses` entry names a rule, positive root-relative file globs and a reason. A matching finding stays visible in the bypass report and the corresponding file is still checked. Inline JS/TS bypasses take precedence.

## Optional tool checks

```json
{
  "externalChecks": ["dependency-boundaries", "duplication", "python", "go-format"],
  "dependencyBoundaries": {
    "tsConfig": "tsconfig.json",
    "forbidden": [
      {"name": "ui-db", "from": "^src/ui/", "to": "^src/db/", "severity": "error"},
      {"name": "no-cycles", "circular": true, "severity": "warn"}
    ]
  },
  "duplication": {"minLines": 8, "minTokens": 80, "severity": "warn"},
  "pythonRules": ["E4", "E7", "E9", "F", "I"]
}
```

- Put dependency-cruiser with TypeScript available, jscpd 5, Ruff or gofmt on PATH for the checks you enable. PATH can point at the project's npm bin directory when running a trusted project. The checker does not install tools.
- Boundary paths are regular expressions against project-relative paths. Provide `tsConfig` to resolve TypeScript aliases and include both endpoints in the selected scope. Use a whole-project run for whole-project cycle checks. The graph checks report at the source file.
- jscpd compares selected files with their language peers. Duplicate findings point at the first matching block in path order and carry the other location. The documented example raises the clone thresholds above the tool defaults.
- Ruff uses the declared selectors or its baseline `E4,E7,E9,F`. It applies SkillFoundry bypasses and runs with Ruff configuration discovery, caching and `# noqa` disabled.
- Tool rule IDs are `dependency-cruiser/<name>`, `jscpd/duplicate`, `ruff/<code>` and `gofmt/format`. Put a finding's exact ID in the project bypass list.
- A missing tool is `unsupported`, reduced coverage is `partial`, and a check with no matching files is `not-applicable`. A requested tool check with incomplete coverage returns exit code 2. Python files are selected by extension and counted as unchecked when no applicable check runs.

## Local exceptions and work directives

`maintainable/single-use-helper` is an error by default. The checker counts same-file references to private JS/TS functions by their resolved binding and reports one direct call site. Public declarations, references as values or framework callbacks, recursive calls inside the declaration, overloads, and generators are outside this rule. The diagnostic points to the declaration. Review evaluation order and return behavior when inlining.

Put a standalone `@bypass` comment immediately before the JS/TS statement or declaration that contains the finding. Give its exact native policy rule ID and a reason. Use one directive per line.

```typescript
/* @bypass maintainable/single-use-helper -- Keep the retry and backoff policy together. */
const fetchWithRetry = async () => { /* retry policy */ }
```

The bypass applies to the named rule within that statement or declaration. The report includes each suppressed finding and its reason. A malformed directive or one without an attached statement is an error. An enabled rule that runs and has no matching finding produces an unused-bypass error. A filtered-out or disabled rule does not produce an unused-bypass error.

Record pending work in JS/TS comments. Put each tag and its description on one line. Block and documentation comments can contain multiple tag lines.

```typescript
// @impl Validate the order request body.
// @check A missing customer ID returns 400 before calling createOrder.
// @refactor Inline reserveStock into createOrder while preserving the transaction.
```

The `directives` check returns the tag, its text and its location in source order. When the user asks the agent to carry out the work, use nearby `@check` notes as acceptance criteria. Remove completed directives after verifying the behavior. Keep pending directives that still describe unresolved work.

## Reports and contextual review

JSON schema version 1 carries file/line/column, severity, rule, message and provenance, plus metrics and check coverage. Exit codes are 0 without error findings, 1 with error findings, and 2 for configuration failures, operational failures or incomplete requested tool checks. Warnings remain visible and require a disposition in the handoff.

An explicit `/review-smells` request applies the skill's contextual criteria to source. Use native findings as evidence when relevant. The CLI owns its static findings and exit code. See [contextual review](smells.md#review-a-scope) for evidence and reporting.

## Source verification

Read [rust.md](rust.md) before changing the checker. Run `sh scripts/build.sh` from SkillFoundry's root. It checks formatting, runs host tests and Clippy, then builds the requested release target. Package/install through the repository's shell entry points. The executable embeds ast-grep grammars/rules and Oxc syntax/binding analysis; it loads no Node profile. See [ast-grep library](https://ast-grep.github.io/guide/api-usage), [Oxc parser](https://oxc.rs/docs/guide/usage/parser.html) and [Git ignore library](https://docs.rs/ignore/latest/ignore/) for upstream behavior.
