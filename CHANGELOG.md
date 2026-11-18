# Changelog

## 0.3.0 - Unreleased

- Add skill shortcuts `/check [description]` and `/help`. Scope checks to resolved paths and exact native JS/TS rules with `--rules`.
- Add `/add-smell` and opt-in `/review-smells`, with criteria for ownership, naming, control flow, validation, FP/OOP, and behavior-focused tests.
- Report private JS/TS helpers with one direct call site as errors. Inline them or document a statement-scoped `@bypass` when a separate function owns a meaningful responsibility.
- Exempt inline exported functions and const bindings from single-use helper diagnostics.
- Add optional `directives` analysis for `@impl`, `@refactor`, and `@check` comments, and audit skipped findings in the bypass report.

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
