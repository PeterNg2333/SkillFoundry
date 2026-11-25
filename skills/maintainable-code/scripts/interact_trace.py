"""Record one Interact run and update its read-only repository activity page."""

import argparse
import html
import json
import posixpath
import subprocess
import uuid
from datetime import datetime, timezone
from pathlib import Path


def timestamp():
    return datetime.now(timezone.utc).isoformat(timespec="seconds")


def replace_file(path, content):
    temporary = path.with_name(f".{path.name}.{uuid.uuid4().hex}.tmp")
    try:
        temporary.write_text(content, encoding="utf-8")
        temporary.replace(path)
    finally:
        temporary.unlink(missing_ok=True)


def activity_directory(root):
    git = subprocess.run(
        ["git", "rev-parse", "--absolute-git-dir"],
        cwd=root,
        capture_output=True,
        text=True,
        encoding="utf-8",
    )
    if git.returncode:
        raise ValueError("Start Interact from a Git repository.")
    return Path(git.stdout.strip()) / "skillfoundry" / "interact"


def begin(root, checker):
    scan = subprocess.run(
        [str(checker.resolve()), "--root", str(root), "check", "--checks", "directives", "--json"],
        cwd=root,
        capture_output=True,
        text=True,
        encoding="utf-8",
    )
    if scan.returncode:
        raise ValueError(scan.stderr.strip() or scan.stdout.strip() or "Directive scan failed.")
    directives = json.loads(scan.stdout)["directives"]
    links = {}
    for directive in directives:
        if directive["kind"] == "spec":
            file = directive["file"]
            spec = directive["text"].partition("#")[0]
            links[file] = posixpath.normpath(posixpath.join(posixpath.dirname(file), spec))

    items = []
    for directive in directives:
        if directive["kind"] in {"spec", "question", "reply"}:
            continue
        items.append({
            **directive,
            "item": len(items) + 1,
            "group": links.get(directive["file"], directive["file"]),
            "state": "Pending",
            "events": [],
        })
    directory = activity_directory(root)
    directory.mkdir(parents=True, exist_ok=True)
    run = directory / f"run-{datetime.now(timezone.utc):%Y%m%dT%H%M%S%f}-{uuid.uuid4().hex}.json"
    save(run, {"root": str(root), "started": timestamp(), "items": items})
    return {"run": str(run), "view": str(run.with_suffix(".html")), "items": items}


def record(run, number, state, detail):
    if not detail.strip():
        raise ValueError("Provide a detail or verification result for the event.")
    data = json.loads(run.read_text(encoding="utf-8"))
    item = next((item for item in data["items"] if item["item"] == number), None)
    if item is None:
        raise ValueError(f"Unknown directive item {number}.")
    transitions = {
        "Pending": {"Running", "Needs input"},
        "Running": {"Running", "Needs input", "Verification failed", "Verified"},
        "Needs input": {"Running"},
        "Verification failed": {"Running"},
        "Verified": {"Cleared", "Running", "Needs input"},
        "Cleared": set(),
    }
    if state not in transitions[item["state"]]:
        raise ValueError(f"Cannot change item {number} from {item['state']} to {state}.")
    item["state"] = state
    item["events"].append({"time": timestamp(), "state": state, "detail": detail})
    save(run, data)
    return {"item": number, "state": state, "view": str(run.with_suffix(".html"))}


def item_html(item):
    escape = html.escape
    title = escape(f"@{item['kind']} {item['text']}")
    location = escape(f"{item['file']}:{item['line']}:{item['column']}")
    events = "".join(
        f"<li><time>{escape(event['time'])}</time> "
        f"<strong>{escape(event['state'])}</strong><pre>{escape(event['detail'])}</pre></li>"
        for event in item["events"]
    )
    return (
        f"<article><details><summary><span>{escape(item['state'])}</span> "
        f"<code>{title}</code></summary><p>{location}</p>"
        f"<ol>{events}</ol></details></article>"
    )


def save(run, data):
    replace_file(run, json.dumps(data, ensure_ascii=False, indent=2) + "\n")
    current = data["items"]
    groups = {}
    for item in current:
        groups.setdefault(item["group"], []).append(item)
    sections = "".join(
        f"<section><h2>{html.escape(group)}</h2>{''.join(map(item_html, items))}</section>"
        for group, items in groups.items()
    )
    history = []
    for old_run in sorted(run.parent.glob("run-*.json"), reverse=True):
        if old_run == run:
            continue
        old = json.loads(old_run.read_text(encoding="utf-8"))
        history.extend(item for item in old["items"] if item["state"] == "Cleared")
    counts = " · ".join(
        f"{state}: {count}"
        for state in ("Pending", "Running", "Needs input", "Verification failed", "Verified", "Cleared")
        if (count := sum(item["state"] == state for item in current))
    )
    refresh = (
        '<meta http-equiv="refresh" content="8">'
        if any(item["state"] in {"Pending", "Running", "Verified"} for item in current)
        else ""
    )
    view = f"""<!doctype html>
<html lang="en">
<meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
{refresh}
<title>Interact activity</title>
<style>
:root {{color-scheme: light dark; font: 15px/1.5 system-ui, sans-serif}}
body {{max-width: 68rem; margin: 2rem auto; padding: 0 1rem}}
header {{border-bottom: 1px solid GrayText; overflow-wrap: anywhere}}
h2 {{font-size: 1.1rem; margin-top: 1.5rem; overflow-wrap: anywhere}}
article {{padding: .55rem .25rem; border-bottom: 1px solid color-mix(in srgb, CanvasText 20%, Canvas)}}
summary {{cursor: pointer; overflow-wrap: anywhere}}
summary span {{font-weight: 700; margin-right: .5rem}}
pre {{white-space: pre-wrap; overflow-wrap: anywhere}}
time {{font-size: .85rem}}
</style>
<header><h1>Interact activity</h1><p>{html.escape(data["root"])}</p>
<p>{html.escape(counts or "No pending directives")} · {data["started"]}</p></header>
<main>{sections or "<p>No actionable directives were discovered.</p>"}
<section><h2>Previously cleared</h2>{''.join(map(item_html, history)) or "<p>No earlier results.</p>"}</section>
</main>
</html>"""
    replace_file(run.with_suffix(".html"), view)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    start = commands.add_parser("start")
    start.add_argument("--root", type=Path, required=True)
    start.add_argument("--checker", type=Path, required=True)
    event = commands.add_parser("event")
    event.add_argument("--run", type=Path, required=True)
    event.add_argument("--item", type=int, required=True)
    event.add_argument("--state", required=True)
    event.add_argument("--detail", required=True)
    args = parser.parse_args()
    try:
        if args.command == "start":
            result = begin(args.root.resolve(), args.checker)
        else:
            result = record(args.run, args.item, args.state, args.detail)
    except (OSError, ValueError, KeyError) as error:
        parser.exit(2, f"Interact: {error}\n")
    print(json.dumps(result))


if __name__ == "__main__":
    main()
