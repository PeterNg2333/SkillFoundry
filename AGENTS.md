# Agent instructions

- Read `README.md` and the current `docs/handoff.md` before changing the checker or its distribution.
- Write code, comments, diagnostics, fixtures and repository documentation in English.
- Keep checker implementation in `checker/`. Skills own instructions and reference material under `skills/`.
- `checker/config/defaults.json` owns embedded defaults. `checker/rules/` owns declarative AST patterns. Project policy belongs in the target project's `checkignore.json`.
- Keep parsing-library types inside `checker/src/checks/`. Configuration, selection and reports use concrete application types.
- Add a failing CLI regression before changing checking behavior. Preserve ignore, syntax, style, partial-coverage and standalone-executable cases.
- Build and verify with `sh scripts/build.sh`. Packaging and installation use the corresponding `.sh` entry points. Read `docs/architecture.md` for ownership and coverage limits.
- Changes to ignores, rule severity, thresholds or coverage are policy changes and require an explanation. Preserve explicit unsupported-check reporting.
- Keep source edits in this checkout. Update installed skills through a verified versioned ZIP.
