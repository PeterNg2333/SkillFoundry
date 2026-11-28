# Rust checker ownership

The SkillFoundry repository owns one Cargo package under `checker/`. Keep CLI sequencing in `main.rs`, configuration in `config.rs`, file policy in `selection/`, native analysis in `checks/`, and findings/metrics in `report.rs`. `rules/` owns embedded ast-grep YAML; `config/defaults.json` is the single source for defaults. Tests exercise the CLI with isolated project fixtures.

Use concrete structs and enums for configuration, source context and results. Introduce a trait or crate for an actual interchangeable implementation or second consumer. Keep parsing-library lifetimes, nodes and allocators local to language adapters; reports own their values. Keep state and errors with their responsibility, with contextual operational errors reaching the CLI.

Use ordinary Rust functions, named types and rustfmt. Make one policy change in its owner with a regression that demonstrates the public behavior. Declare each native rule's languages in `catalog.rs`, write rules that several languages share against the `Syntax` roles in `checks/language.rs`, and keep language-only rules in that language's adapter. Add a `tests/parity/` case with a flagged file for every declared language; the parity test reports any missing pair. Run `sh scripts/build.sh` from the source repository root: formatting, host tests, Clippy with warnings denied, and a release build. `CHECKER_TARGET` selects a cross-build when its Rust target and C/linker toolchain are installed.

All code, comments, diagnostics and repository documentation are English. Build through `.sh` entry points. Installed binaries and defaults are updated through versioned ZIPs. Check the source repository's `docs/architecture.md` and current handoff for supported coverage and release verification.
