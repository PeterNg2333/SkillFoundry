# SkillFoundry

Reusable agent skills with a standalone Rust code checker and versioned ZIP distribution. Code, comments, diagnostics and documentation are written in English.

## Source and installed copies

```text
checker/                 # One Cargo package: CLI, policy, selection, checks and reports
  config/                # Embedded defaults
  rules/                 # Embedded declarative AST rules
  tests/                 # CLI regressions and fixtures
skills/maintainable-code/ # Skill instructions and references
scripts/                 # sh build/package/install entry points
docs/                    # Architecture and current handoff
dist/                    # Generated ZIPs and SHA256SUMS; ignored by Git
```

Edit this checkout. Installed copies under `$CODEX_HOME/skills` (default `~/.codex/skills`) are updated from versioned ZIPs. Only the authored `maintainable-code` skill is included initially; add other authored skills under `skills/<name>/SKILL.md`.

## Build and check

Use a POSIX shell, including Git Bash on Windows, Rust 1.95+ with rustfmt/Clippy, and a C toolchain for the embedded grammars. Windows MSVC builds need Visual Studio C++ Build Tools. Cargo.lock pins dependencies. No Node or Go toolchain participates in the build.

```sh
sh scripts/build.sh
skills/maintainable-code/bin/maintainable-check.exe capabilities --json
skills/maintainable-code/bin/maintainable-check.exe check --root /path/to/project --json
skills/maintainable-code/bin/maintainable-check.exe check --root /path/to/project --checks dependencies,json --json
skills/maintainable-code/bin/maintainable-check.exe check --root /path/to/project --rules maintainable/single-use-helper -- src/routes
skills/maintainable-code/bin/maintainable-check.exe select --root /path/to/project --json
```

The executable is named `maintainable-check` on Linux/macOS. All parsing runs in that executable. It applies extension routing, `checkignore.json`, scoped Git ignores and the root Docker ignore before checking. Use `--checks` to run only the needed subset.

In the skill, use `$maintainable-code /check single-use helpers in src/routes` for a scoped check and `$maintainable-code /help` for examples.

Coverage includes JS/TS syntax and AST style, JSON/TOML syntax, manifest dependency counts, Go syntax/imports/functions, and optional comments. Go formatting is explicitly unsupported; keep the project's own `gofmt` check. The checker does not replace project type checking. See [check commands and policy](skills/maintainable-code/references/checks.md), [architecture and limits](docs/architecture.md) and [migration notes](CHANGELOG.md).

For a cross-build, set `CHECKER_TARGET` to an installed Rust target triple and provide its linker/C toolchain. Tests and Clippy still run on the host:

```sh
CHECKER_TARGET=x86_64-unknown-linux-musl sh scripts/build.sh
```

## Package and install

Packaging and the supplied installer need Python 3.10+ with its standard library. The installed checker itself needs only the operating system.

```sh
python -m unittest discover -s scripts -p 'test_*.py'
sh scripts/package.sh maintainable-code
sh scripts/install.sh dist/maintainable-code-0.3.0-windows-amd64.zip --replace
```

The ZIP includes the executable, instructions, source metadata and resolved Rust dependency notices. It contains no Node packages or legacy checker source. `distribution.json` records version/platform/source revision; `.installation.json` records the archive hash, source and time. `dist/SHA256SUMS` contains archive hashes. Package from a clean commit to identify exact source.

Instruction-only skills produce `<name>-<version>-any.zip`. The native build integration belongs to `maintainable-code`; another runtime needs an explicit build adapter.

Get the installer scripts by cloning this repository or downloading its source ZIP. Once published, download a release asset with:

```sh
sh scripts/install.sh --github OWNER/SkillFoundry --version 0.3.0 --replace
sh scripts/install.sh --url https://example.com/maintainable-code-0.3.0-windows-amd64.zip --replace
```

Replace `OWNER` after publication. `--github` chooses the host platform and the release matching `--version`. For an instruction-only skill, also pass `--skill <name> --platform any`. `--dest <directory>` chooses a separate installation root. Replacement is staged and restores the previous copy on failure. If restoration fails, the installer reports the retained backup path.

## Publish

Set `VERSION` and the checker package version, update the changelog, commit, build/package, then push to GitHub. Create release `v<version>` and attach platform ZIPs and `SHA256SUMS`. A source ZIP contains buildable source; release ZIPs contain ready executables. See [current handoff](docs/handoff.md) for verification and remaining work.
