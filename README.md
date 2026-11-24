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

Edit this checkout. Installed copies under `$CODEX_HOME/skills` (default `~/.codex/skills`) are updated from versioned ZIPs. Each authored skill starts at `skills/<name>/SKILL.md`.

## Build and check

Use a POSIX shell, including Git Bash on Windows, Rust 1.95+ with rustfmt/Clippy, and a C toolchain for the embedded grammars. Windows MSVC builds need Visual Studio C++ Build Tools. Cargo.lock pins dependencies. No Node or Go toolchain participates in the build.

```sh
sh scripts/build.sh
skills/maintainable-code/bin/maintainable-check.exe capabilities --json
skills/maintainable-code/bin/maintainable-check.exe check --root /path/to/project --json
skills/maintainable-code/bin/maintainable-check.exe check --root /path/to/project --checks dependencies,json --json
skills/maintainable-code/bin/maintainable-check.exe check --root /path/to/project --checks file-structure
skills/maintainable-code/bin/maintainable-check.exe check --root /path/to/project --rules maintainable/single-use-helper -- src/routes
skills/maintainable-code/bin/maintainable-check.exe select --root /path/to/project --json
```

The executable is named `maintainable-check` on Linux/macOS. It reads `checkignore.json` or falls back to `check.json`. Source checks apply the project policy, scoped Git ignores, the root Docker ignore and extension routing. The default run includes native checks and enforces `fileStructure` when declared. Use `--checks` to run only the needed subset.

In the skill, use `$maintainable-code /check single-use helpers in src/routes` for a scoped check and `$maintainable-code /help` for examples. Use `/add-smell` to record a rule or `/review-smells` to request contextual review. See [code smell rules and examples](skills/maintainable-code/references/smells.md).

Native coverage includes JS/TS syntax and AST style, JSON/TOML syntax, manifest dependency counts, Go syntax/imports/functions, and optional comments. Opt in to JS/TS dependency boundaries with dependency-cruiser, duplication with jscpd, Python lint with Ruff, and Go formatting with gofmt. The checker reports missing tools and incomplete coverage. Run project type checking separately.

Use [`checkignore.json` as the project policy declaration](skills/maintainable-code/references/checks.md#configuration-and-selection). `styleNotes` explains the conventions to agents, `rules` enforces native style, and `bypasses` records scoped exceptions with reasons. `externalChecks` adds optional checks to the project's default run. See the [architecture and limits](docs/architecture.md) and [migration notes](CHANGELOG.md).

`fileStructure` declares nested directories, accepted file types and ignored subtrees in the same policy file. Run `check --checks file-structure` to show the declared tree and report violations, or add `--json` for tooling. [File structure configuration](skills/maintainable-code/references/checks.md#file-structure)

Run `$mc-init` to set up `checkignore.json` from a project's existing conventions. Run `$mc-init questionnaire` to decide languages, style, and exclusions interactively. When both skills are installed together, `$maintainable-code /init [auto|questionnaire]` uses the same workflow. SkillFoundry's [own declaration](checkignore.json) uses the embedded and Git exclusions. Rust and shell checks run through the repository's shell entry points. Python lint is available through `--checks python`.

For a cross-build, set `CHECKER_TARGET` to an installed Rust target triple and provide its linker/C toolchain. Tests and Clippy still run on the host:

```sh
CHECKER_TARGET=x86_64-unknown-linux-musl sh scripts/build.sh
```

## Package and install

Packaging and the supplied installer need Python 3.10+ with its standard library. The installed checker needs only the operating system for native checks. Optional checks use tools on PATH.

```sh
python -m unittest discover -s scripts -p 'test_*.py'
sh scripts/package.sh maintainable-code
sh scripts/install.sh dist/maintainable-code-0.3.0-windows-amd64.zip --replace
```

The ZIP includes the executable, instructions, source metadata and resolved Rust dependency notices. It contains no Node packages or legacy checker source. `distribution.json` records version/platform/source revision; `.installation.json` records the archive hash, source and time. `dist/SHA256SUMS` contains archive hashes. Package from a clean commit to identify exact source.

Instruction-only skills produce `<name>-<version>-any.zip`. The native build integration belongs to `maintainable-code`; another runtime needs an explicit build adapter.

Build the initializer shortcut ZIP with `sh scripts/package.sh mc-init` and install it alongside `maintainable-code`.

Get the installer scripts by cloning this repository or downloading its source ZIP. Once published, download a release asset with:

```sh
sh scripts/install.sh --github OWNER/SkillFoundry --version 0.3.0 --replace
sh scripts/install.sh --url https://example.com/maintainable-code-0.3.0-windows-amd64.zip --replace
```

Replace `OWNER` after publication. `--github` chooses the host platform and the release matching `--version`. For an instruction-only skill, also pass `--skill <name> --platform any`. `--dest <directory>` chooses a separate installation root. Replacement is staged and restores the previous copy on failure. If restoration fails, the installer reports the retained backup path.

## Publish

Set `VERSION` and the checker package version, update the changelog, commit, build/package, then push to GitHub. Create release `v<version>` and attach platform ZIPs and `SHA256SUMS`. A source ZIP contains buildable source; release ZIPs contain ready executables. See [current handoff](docs/handoff.md) for verification and remaining work.
