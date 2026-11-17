"""Build a versioned skill ZIP with its native checker and dependency notices."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import stat
import subprocess
import tempfile
import zipfile


REPOSITORY = Path(__file__).resolve().parent.parent
EXCLUDED = {"node_modules", "bin", "dist", ".git", "__pycache__"}


def run(arguments, directory):
    return subprocess.run(arguments, cwd=directory, check=True, text=True,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE).stdout


def copy_source(source, target):
    shutil.copytree(source, target,
                    ignore=shutil.ignore_patterns(*EXCLUDED, "*.pyc", ".installation.json"))


def source_revision():
    if not (REPOSITORY / ".git").exists():
        return None, None
    revision = subprocess.run(["git", "rev-parse", "HEAD"], cwd=REPOSITORY,
                              text=True, capture_output=True)
    dirty = bool(run(["git", "status", "--porcelain"], REPOSITORY).strip())
    return revision.stdout.strip() if revision.returncode == 0 else None, dirty


def add_rust_notices(checker, target, payload):
    metadata = json.loads(run(["cargo", "metadata", "--locked", "--format-version", "1",
                               "--filter-platform", target], checker))
    notices = payload / "THIRD_PARTY" / "rust"
    inventory = []
    for package in metadata["packages"]:
        if package["source"] is None:
            continue
        directory = Path(package["manifest_path"]).parent
        destination = notices / f"{package['name']}-{package['version']}"
        destination.mkdir(parents=True)
        for file in directory.iterdir():
            if file.is_file() and file.name.upper().startswith(("LICENSE", "COPYING", "NOTICE", "UNLICENSE")):
                shutil.copy2(file, destination / file.name)
        inventory.append({key: package.get(key) for key in ("name", "version", "license", "repository")})
    (notices / "dependencies.json").write_text(json.dumps(inventory, indent=2) + "\n", encoding="utf-8")


def rust_target(checker):
    target = os.environ.get("CHECKER_TARGET")
    if not target:
        target = next(line.removeprefix("host: ") for line in run(["rustc", "-vV"], checker).splitlines()
                      if line.startswith("host: "))
    architecture = {"x86_64": "amd64", "aarch64": "arm64"}.get(target.split("-")[0])
    operating_system = next((name for token, name in (("windows", "windows"), ("linux", "linux"), ("darwin", "darwin"))
                             if token in target.split("-")), None)
    if not architecture or not operating_system:
        raise ValueError(f"Unsupported release target {target}")
    return target, f"{operating_system}-{architecture}"


def prepare_payload(skill, payload, shell):
    copy_source(skill, payload)
    platform = "any"
    checker = REPOSITORY / "checker"
    if skill.name == "maintainable-code" and (checker / "Cargo.toml").is_file():
        target, platform = rust_target(checker)
        subprocess.run([shell, str(REPOSITORY / "scripts" / "build.sh")], cwd=REPOSITORY, check=True)
        binary_name = "maintainable-check.exe" if platform.startswith("windows-") else "maintainable-check"
        binary = skill / "bin" / binary_name
        (payload / "bin").mkdir()
        shutil.copy2(binary, payload / "bin" / binary_name)
        add_rust_notices(checker, target, payload)
    return platform


def archive_payload(payload, archive):
    with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED,
                         compresslevel=6) as output:
        for directory, names, files in os.walk(payload, followlinks=False):
            names[:] = sorted(name for name in names if name not in {"__pycache__"})
            for name in sorted(files):
                file = Path(directory) / name
                if file.is_symlink():
                    raise RuntimeError(f"Runtime contains an unresolved link {file.relative_to(payload)}")
                relative = file.relative_to(payload.parent).as_posix()
                entry = zipfile.ZipInfo(relative)
                entry.create_system = 3
                executable = file.suffix == ".sh" or file.parent == payload / "bin"
                mode = 0o755 if executable else 0o644
                entry.external_attr = (stat.S_IFREG | mode) << 16
                entry.compress_type = zipfile.ZIP_DEFLATED
                output.writestr(entry, file.read_bytes())
            for name in names:
                if (Path(directory) / name).is_symlink():
                    raise RuntimeError(f"Runtime contains a linked directory {Path(directory) / name}")


def package_skill(name, version, shell):
    if not re.fullmatch(r"[a-z0-9]+(?:-[a-z0-9]+)*", name):
        raise ValueError("Use a skill directory name containing lowercase words and hyphens")
    if not re.fullmatch(r"\d+\.\d+\.\d+(?:-[A-Za-z0-9.-]+)?", version):
        raise ValueError("Use a version such as 0.1.0")
    skill = REPOSITORY / "skills" / name
    if not (skill / "SKILL.md").is_file():
        raise ValueError(f"Skill not found {name}")
    destination = REPOSITORY / "dist"
    destination.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".package-", dir=destination) as temporary:
        staging = Path(temporary).resolve()
        if staging.parent != destination.resolve():
            raise RuntimeError("Unexpected package staging location")
        payload = staging / name
        platform = prepare_payload(skill, payload, shell)
        revision, dirty = source_revision()
        manifest = {"schemaVersion": 1, "name": name, "version": version,
                    "platform": platform, "sourcePath": f"skills/{name}",
                    "sourceRevision": revision, "sourceDirty": dirty}
        (payload / "distribution.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
        archive = destination / f"{name}-{version}-{platform}.zip"
        candidate = staging / archive.name
        archive_payload(payload, candidate)
        os.replace(candidate, archive)
    checksums = []
    for file in sorted(destination.glob("*.zip")):
        checksums.append(f"{hashlib.sha256(file.read_bytes()).hexdigest()}  {file.name}\n")
    (destination / "SHA256SUMS").write_text("".join(checksums), encoding="utf-8")
    print(archive)
    return archive


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("skill")
    parser.add_argument("--version", default=(REPOSITORY / "VERSION").read_text().strip())
    parser.add_argument("--shell", required=True)
    options = parser.parse_args()
    package_skill(options.skill, options.version, options.shell)


if __name__ == "__main__":
    main()
