# Native checker

The installed skill supplies one Rust executable at `bin/maintainable-check.exe` on Windows or `bin/maintainable-check` on Linux/macOS. Defaults and AST rules are embedded. Running checks requires no Node, Go toolchain or package manager. Source lives in SkillFoundry's `checker/`.

## Commands and coverage

Resolve the executable from the installed skill and pass the target project through `--root`. Flags precede optional relative paths.

```sh
maintainable-check capabilities --json
maintainable-check select --root . --json
maintainable-check check --root . --json
maintainable-check check --root . --checks dependencies,json --json
maintainable-check check --root . --checks javascript,typescript src
maintainable-check check --root . --checks go,comments src
```

| Check | Scope |
|---|---|
| `javascript`, `typescript` | Syntax, scoped prefer-const, const-arrow organization, bottom exports, type-only imports, cross-directory aliases, depth/complexity/line metrics |
| `json`, `toml` | Strict syntax with source locations |
| `dependencies` | Declared package groups and unique names in package.json/Cargo.toml; direct/indirect Go require declarations |
| `go` | Embedded grammar syntax, imports and function/method counts |
| `comments` | AST comment groups and optional presence rule for JS/TS/Go |

Default checks include all except `comments`; `--checks` selects a subset. Unknown or duplicate check IDs fail. Reports state selected, checked and unchecked files, per-check applicability and unsupported checks. `select` and `check --list` only list selection.

Go formatting is explicitly unsupported and does not silently pass. Keep project gofmt/type-check/formatter/typed-lint/behavior commands for their boundaries. Dependency metrics count declarations, not installed/transitive graphs, vulnerabilities or bundle size. Go manifest inspection is limited to require directives. Native style metrics do not claim arbitrary ESLint plugin compatibility.

## Configuration and selection

Embedded defaults are owned by `checker/config/defaults.json` in the source checkout. A target project can provide `checkignore.json`, or select another JSON file with `--config`. It is parsed as data and never executed.

- Positive root-relative `ignore` globs add exclusions. Extension mappings and rule severity overrides merge with defaults.
- Root/nested Git ignores retain scope, ordering, anchors and negations. Git excluded parents stay pruned. Patterns apply to selected paths, including tracked files.
- Root Docker ignore has context-root matching and negations that may restore descendants. Set `--root` to the intended context. Dockerfile-specific contexts and global user Git exclusions are outside current coverage.
- Explicit paths use the same policy and must stay within root. JSON skip reasons include ignore provenance, disabled/unsupported extensions and symlinks.

Extension types are `javascript`, `typescript`, `json`, `toml`, `go` and `gomod`. The longest matching suffix wins case-insensitively; `false` disables it. Unknown keys, rules, severities and unsupported options fail configuration validation.

```json
{
  "ignore": ["archive/**", "tests/generated/**"],
  "extensions": {".source": "typescript", ".generated.ts": false},
  "limits": {"directDependencies": 40, "importsPerFile": 15, "requireComments": false},
  "overrides": [{"files": ["fixtures/**"], "rules": {"func-style": "off"}}]
}
```

Dependency/import limits are inactive until configured. Select `comments` to apply `requireComments`. The supplied rule IDs are `prefer-const`, `no-var`, `func-style`, `no-restricted-syntax`, `maintainable/cross-directory-imports`, `@typescript-eslint/consistent-type-imports`, `max-depth`, `complexity` and `max-lines`. IDs with legacy prefixes identify native policy rules; they do not invoke ESLint.

Rules accept off/warn/error or numeric severities. Depth/complexity accept numeric thresholds; max-lines accepts a number or max/skipBlankLines/skipComments options. Func-style supports the supplied expression form. Use file-specific overrides for explicit exceptions. Inline ESLint comments do not disable native checks. Changes to exclusions, thresholds or severities are policy changes that need explanation.

## Reports and future review

JSON schema version 1 carries file/line/column, severity, rule, message and static provenance, plus metrics and check coverage. Exit codes are 0 without error findings, 1 with error findings, and 2 for configuration/operational failures. Warnings remain visible and require a disposition in the handoff.

A future small-model stage can consume these reports and bounded source ranges. It needs explicit opt-in, a budget and separate model/prompt/usage provenance. The current binary makes no model calls. Comment usefulness, ownership and cohesion remain review tasks.

## Source verification

Read [rust.md](rust.md) before changing the checker. Run `sh scripts/build.sh` from SkillFoundry's root. It checks formatting, runs host tests and Clippy, then builds the requested release target. Package/install through the repository's shell entry points. The executable embeds ast-grep grammars/rules and Oxc syntax/binding analysis; it loads no Node profile. See [ast-grep library](https://ast-grep.github.io/guide/api-usage), [Oxc parser](https://oxc.rs/docs/guide/usage/parser.html) and [Git ignore library](https://docs.rs/ignore/latest/ignore/) for upstream behavior.
