# Personal Codex skills

Reusable skill source lives in this repository. Installed copies under `$CODEX_HOME/skills` (default `~/.codex/skills`) come from versioned release ZIPs. Edit this checkout, package a new version and reinstall it when changing a skill.

## Contents

```text
skills/
  maintainable-code/    # Instructions, Go CLI, JS/TS AST profile and locked dependencies
scripts/
  package.sh           # Build and package a skill
  install.sh           # Install a local ZIP or download a GitHub release asset
  package.py           # Archive, runtime and source metadata
  install.py           # Validation, staged replacement and recovery
  test_distribution.py # Distribution behavior and failure paths
VERSION                # Shared release version
dist/                  # Generated ZIPs and SHA256SUMS; ignored by Git
```

Only the custom `maintainable-code` skill is included initially. Add other authored skills under `skills/<name>/SKILL.md` with their supporting files. Imported third-party skills remain owned by their upstream repositories.

## Build and package

Use a POSIX shell, including Git Bash on Windows. All build entry points are `.sh`. Packaging uses Python 3.10+ from the shell's PATH; it has no Python package dependencies.

For `maintainable-code`, the build machine needs Go 1.26+, Node 22.14+ and pnpm 10.29.2. Dependencies are pinned in `go.sum` and `scripts/pnpm-lock.yaml`.

```sh
# Native CLI only: checks formatting, runs Go tests/vet, then builds.
sh skills/maintainable-code/build.sh

# Distribution tests.
python -m unittest discover -s scripts -p 'test_*.py'

# Build the native CLI and bundle the locked JS profile dependencies.
sh scripts/package.sh maintainable-code
```

The default Windows artifact is `dist/maintainable-code-0.1.0-windows-amd64.zip`. The package includes source, instructions, a native executable, embedded defaults, JS/TS runtime dependencies and dependency license notices. Consumers need no Go compiler or package manager. The native checks run standalone; the additional JS/TS profile requires Node 22.14+.

`distribution.json` records the version, target platform and source revision. `dist/SHA256SUMS` lists ZIP hashes. Set `VERSION` for the next release or pass `--version 0.2.0` to `package.sh`. Package from a clean commit to identify the exact source.

```sh
# Cross-build the native executable while running Go checks on the host.
GOOS=linux GOARCH=amd64 sh scripts/package.sh maintainable-code
```

Instruction-only skills produce `<name>-<version>-any.zip`. The Go build integration currently belongs to `maintainable-code`; add an explicit build adapter when another skill needs a different runtime.

## Install from ZIP or GitHub

Get these installer scripts by cloning this repository or downloading its source ZIP. Then install a release artifact:

```sh
sh scripts/install.sh dist/maintainable-code-0.1.0-windows-amd64.zip --replace

sh scripts/install.sh --github OWNER/codex-skills --version 0.1.0 --replace

sh scripts/install.sh --url https://example.com/maintainable-code-0.1.0-windows-amd64.zip --replace
```

Replace `OWNER` with the GitHub repository owner after publishing. `--github` downloads the asset for the current OS and architecture from release `v0.1.0`. For instruction-only skills, also pass `--skill <name> --platform any`. A GitHub source ZIP alone does not include compiled checker artifacts.

Installation validates the archive and its platform, stages the new copy, then replaces only that skill directory. `--replace` is required for an existing installation. Failed replacement restores the prior copy; if restoration itself fails, the backup is retained and its path is reported. Concurrent installs of the same skill are blocked. `.installation.json` records the downloaded source, ZIP hash and installation time. Use `--dest <directory>` for a separate skills directory.

Run installed checks by resolving the skill directory:

```sh
skill_root="${CODEX_HOME:-$HOME/.codex}/skills/maintainable-code"
"$skill_root/bin/maintainable-check.exe" capabilities --json
"$skill_root/bin/maintainable-check.exe" check --root /path/to/project --checks json,dependencies --json
node "$skill_root/scripts/check.mjs" --root /path/to/project
```

On Linux/macOS the executable is named `maintainable-check`. The native catalog covers JSON syntax, dependency counts, Go AST/formatting and optional Go comment checks. The JS/TS profile supplies the additional AST style rules. Both apply extension selection, `checkignore.json`, Git ignores and Docker ignores. See the skill's [check reference](skills/maintainable-code/references/checks.md) for coverage and configuration.

## Publish a release

1. Commit the source and set the release version.
2. Push this repository to GitHub.
3. Run `sh scripts/package.sh maintainable-code` for each supported platform.
4. Create release `v<version>` and attach the generated ZIPs and `SHA256SUMS`.

Local installed copies continue to update from those release ZIPs. The repository does not automatically publish or update installations.
