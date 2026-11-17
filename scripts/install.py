"""Install a versioned skill ZIP from a local archive or GitHub release."""

import argparse
from contextlib import contextmanager
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import platform
import re
import shutil
import stat
import tempfile
import urllib.parse
import urllib.request
import zipfile


NAME = re.compile(r"[a-z0-9]+(?:-[a-z0-9]+)*\Z")
VERSION = re.compile(r"\d+\.\d+\.\d+(?:-[A-Za-z0-9.-]+)?\Z")


def host_platform():
    systems = {"Windows": "windows", "Linux": "linux", "Darwin": "darwin"}
    architectures = {"AMD64": "amd64", "x86_64": "amd64", "arm64": "arm64", "aarch64": "arm64"}
    try:
        return f"{systems[platform.system()]}-{architectures[platform.machine()]}"
    except KeyError as error:
        raise ValueError("Unsupported host platform") from error


def ensure_child(path, root):
    resolved = path.resolve()
    if resolved == root or root not in resolved.parents:
        raise ValueError(f"Path escaped installation root {path.name}")
    return resolved


def remove_owned(path, root):
    ensure_child(path, root)
    if path.is_symlink():
        raise ValueError("Expected an owned directory")
    if path.exists():
        shutil.rmtree(path)


@contextmanager
def installation_lock(root, name):
    lock = root / f".{name}.install.lock"
    if lock.is_symlink():
        raise ValueError("Installation lock is a symlink")
    with lock.open("a+b") as stream:
        stream.seek(0, os.SEEK_END)
        if stream.tell() == 0:
            stream.write(b"0")
            stream.flush()
        stream.seek(0)
        if os.name == "nt":
            import msvcrt
            acquire = lambda: msvcrt.locking(stream.fileno(), msvcrt.LK_NBLCK, 1)
            release = lambda: msvcrt.locking(stream.fileno(), msvcrt.LK_UNLCK, 1)
        else:
            import fcntl
            acquire = lambda: fcntl.flock(stream.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
            release = lambda: fcntl.flock(stream.fileno(), fcntl.LOCK_UN)
        try:
            acquire()
        except OSError as error:
            raise RuntimeError(f"Another installation of {name} is in progress") from error
        try:
            yield
        finally:
            stream.seek(0)
            release()


def inspect_archive(archive):
    entries = archive.infolist()
    manifests = []
    roots = set()
    for entry in entries:
        name = entry.filename
        path = PurePosixPath(name)
        if "\\" in name or path.is_absolute() or ".." in path.parts or not path.parts:
            raise ValueError(f"Unsafe ZIP path {name}")
        if any(":" in part for part in path.parts):
            raise ValueError(f"Unsafe ZIP path {name}")
        if stat.S_ISLNK(entry.external_attr >> 16):
            raise ValueError("ZIP symlinks are unsupported")
        roots.add(path.parts[0])
        if len(path.parts) == 2 and path.name == "distribution.json":
            manifests.append(entry)
    if len(roots) != 1 or len(manifests) != 1:
        raise ValueError("Use a packaged skill ZIP containing distribution.json")
    metadata = json.loads(archive.read(manifests[0]))
    name = metadata.get("name", "")
    version = metadata.get("version", "")
    if metadata.get("schemaVersion") != 1 or not NAME.fullmatch(name) or not VERSION.fullmatch(version):
        raise ValueError("Invalid skill distribution metadata")
    if roots != {name} or f"{name}/SKILL.md" not in archive.namelist():
        raise ValueError("Archive root and skill metadata do not match")
    if metadata.get("platform") not in {"any", host_platform()}:
        raise ValueError(f"Archive targets {metadata.get('platform')}, host is {host_platform()}")
    if len({entry.filename for entry in entries}) != len(entries):
        raise ValueError("ZIP contains duplicate paths")
    if archive.testzip() is not None:
        raise ValueError("ZIP integrity check failed")
    return metadata


def extract_archive(archive, staging):
    for entry in archive.infolist():
        destination = staging.joinpath(*PurePosixPath(entry.filename).parts)
        ensure_child(destination, staging)
        if entry.is_dir():
            destination.mkdir(parents=True, exist_ok=True)
            continue
        destination.parent.mkdir(parents=True, exist_ok=True)
        with archive.open(entry) as source, destination.open("wb") as target:
            shutil.copyfileobj(source, target)
        mode = (entry.external_attr >> 16) & 0o777
        if mode:
            destination.chmod(mode)


def install_archive(file, destination, replace=False, source_label=None):
    file = Path(file).resolve()
    root = Path(destination).expanduser().resolve()
    root.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(file) as archive:
        metadata = inspect_archive(archive)
        name = metadata["name"]
        target = root / name
        ensure_child(target, root)
        with installation_lock(root, name):
            if target.is_symlink() or (target.exists() and not target.is_dir()):
                raise ValueError("Installed target must be a regular skill directory")
            if target.exists() and not replace:
                raise FileExistsError(f"{name} is installed. Use --replace to install this version.")
            staging = Path(tempfile.mkdtemp(prefix=f".{name}.install-", dir=root)).resolve()
            ensure_child(staging, root)
            cleanup = True
            try:
                extract_archive(archive, staging)
                incoming = staging / name
                receipt = {"name": name, "version": metadata["version"],
                           "source": source_label or str(file),
                           "sha256": hashlib.sha256(file.read_bytes()).hexdigest(),
                           "installedAt": datetime.now(timezone.utc).isoformat()}
                (incoming / ".installation.json").write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
                previous = staging / "previous"
                if target.exists():
                    ensure_child(previous, root)
                    os.replace(target, previous)
                try:
                    os.replace(incoming, target)
                except OSError:
                    if previous.exists():
                        try:
                            os.replace(previous, target)
                        except OSError as error:
                            cleanup = False
                            raise RuntimeError(f"Restore failed; previous installation retained at {previous}") from error
                    raise
            finally:
                if cleanup:
                    remove_owned(staging, root)
    return {"name": name, "version": metadata["version"], "installed": str(target)}


def fetch(url, destination):
    parsed = urllib.parse.urlparse(url)
    if parsed.scheme != "https":
        raise ValueError("Download URLs must use HTTPS")
    request = urllib.request.Request(url, headers={"User-Agent": "codex-skills-installer"})
    with urllib.request.urlopen(request, timeout=60) as response, destination.open("wb") as output:
        shutil.copyfileobj(response, output)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", nargs="?")
    parser.add_argument("--url")
    parser.add_argument("--github", help="OWNER/REPOSITORY containing release assets")
    parser.add_argument("--version")
    parser.add_argument("--skill", default="maintainable-code")
    parser.add_argument("--platform", choices=["any", host_platform()], default=host_platform(),
                        help="Release platform; use any for instruction-only skills")
    parser.add_argument("--replace", action="store_true")
    codex_root = Path(os.environ.get("CODEX_HOME", Path.home() / ".codex"))
    parser.add_argument("--dest", default=str(codex_root / "skills"))
    options = parser.parse_args()
    if sum(bool(value) for value in (options.archive, options.url, options.github)) != 1:
        parser.error("Choose one local ZIP, --url or --github source")
    url = options.url
    if options.github:
        version = (options.version or "").removeprefix("v")
        if not VERSION.fullmatch(version) or not NAME.fullmatch(options.skill):
            parser.error("GitHub installation requires --version and a valid --skill")
        if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", options.github):
            parser.error("Use --github OWNER/REPOSITORY")
        asset = f"{options.skill}-{version}-{options.platform}.zip"
        url = f"https://github.com/{options.github}/releases/download/v{version}/{asset}"
    if url:
        with tempfile.TemporaryDirectory(prefix="codex-skill-download-") as temporary:
            archive = Path(temporary) / "skill.zip"
            fetch(url, archive)
            result = install_archive(archive, options.dest, options.replace, url)
    else:
        result = install_archive(options.archive, options.dest, options.replace)
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
