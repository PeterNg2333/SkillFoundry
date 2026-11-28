# Changelog

## 0.3.0 - Unreleased

- Add skill shortcuts `/check [description]` and `/help`. Scope checks to resolved paths and exact native JS/TS rules with `--rules`.
- Add `/add-smell` and opt-in `/review-smells`, with criteria for ownership, naming, control flow, validation, FP/OOP, and behavior-focused tests.
- Report private JS/TS helpers with one direct call site as errors. Inline them or document a statement-scoped `@bypass` when a separate function owns a meaningful responsibility.
- Exempt inline exported functions and const bindings from single-use helper diagnostics.
- Add optional `directives` analysis for `@impl`, `@refactor`, and `@check` comments, and audit skipped findings in the bypass report.
- Add `/interact` for repository work directives, including `@add-test` and Markdown master specs. Report verified and cleared work in a local activity view.
- Show Interact work on a live board, with structured test outcomes and an activity history per directive.
- Add native Python analysis as the default `python` check. It covers syntax, private helpers with one direct call site, cross-directory relative imports, nesting, complexity and file length, and the opt-in long-condition, fallback-order, object-spread-newline and similar-loops rules. Interact directives and statement-scoped `@bypass` comments use `#` comments.
- Share one implementation of each cross-language rule between JavaScript, TypeScript and Python through a language contract. `capabilities --json` reports `languages` and `ruleLanguages`. The build fails when a declared rule and language lack a parity fixture.

### Migration

- The Ruff tool check is now `python-lint`. `--checks python` selects native Python analysis. `externalChecks: ["python"]` fails validation and names `python-lint`.
- Python files that earlier counted as unchecked are now checked by default. `maintainable/single-use-helper` reports private Python helpers as errors, as it does for JS/TS. To phase it in, add an override such as `{"files": ["**/*.py"], "rules": {"maintainable/single-use-helper": "warn"}}`.
- `--rules` without `--checks` also checks Python files. Each requested rule needs a selected language check that implements it. A statement bypass that names a rule the file's language does not implement is invalid.

## 0.2.0 - 2026-11-18

### Changed

- Use one Rust checker for JS/TS AST rules, native checks, selection and reports. Checks require no Node, Go compiler or package manager.
- Separate checker source from skill instructions and build through `sh scripts/build.sh`.
- Package the executable and Rust dependency notices without the former Go and ESLint runtimes.

### Added

- Scoped mutation analysis, explicit per-check coverage and TOML syntax/Cargo dependency inspection.
- CLI fixtures for decorators, overloads, ignores, rule overrides, syntax failures and partial-check isolation.

### Migration

- Replace `node .../scripts/check.mjs` with `.../bin/maintainable-check check`. `--checks` selects a subset.
- Go formatting is no longer implemented inside the checker. Reports mark `go-format` unsupported; retain the project's own `gofmt` check.
- Supplied rule IDs and severity overrides retain their policy names. Arbitrary ESLint plugins and selector options are unsupported and fail configuration validation.
