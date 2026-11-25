"""Record one Interact run and update its read-only repository activity page."""

import argparse
import json
import posixpath
import subprocess
import uuid
from datetime import datetime, timezone
from pathlib import Path


def timestamp():
    return datetime.now(timezone.utc).isoformat(timespec="milliseconds")


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


def record(run, number, state, detail, test=None):
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
    event = {"time": timestamp(), "state": state, "detail": detail}
    if test is not None:
        event["test"] = test
    item["events"].append(event)
    save(run, data)
    return {"item": number, "state": state, "view": str(run.with_suffix(".html"))}


def save(run, data):
    replace_file(run, json.dumps(data, ensure_ascii=False, indent=2) + "\n")
    view = run.with_suffix(".html")
    template = Path(__file__).with_name("interact_board.html").read_text(encoding="utf-8")
    if not view.exists() or view.read_text(encoding="utf-8") != template:
        replace_file(view, template)
    history = []
    for old_run in sorted(run.parent.glob("run-*.json"), reverse=True):
        if old_run == run:
            continue
        old = json.loads(old_run.read_text(encoding="utf-8"))
        history.extend({**item, "run": old_run.stem}
                       for item in old["items"] if item["state"] == "Cleared")
    snapshot = json.dumps({**data, "history": history}, ensure_ascii=True)
    replace_file(run.with_suffix(".data.js"), f"window.updateInteractBoard({snapshot});\n")


def test_result(args):
    values = (args.test_command, args.test_status, args.test_exit_code)
    if all(value is None for value in values):
        return None
    if not args.test_command or not args.test_command.strip() or not args.test_status:
        raise ValueError("A test result needs both --test-command and --test-status.")
    if args.test_status == "skipped":
        if args.test_exit_code is not None:
            raise ValueError("A skipped test does not have an exit code.")
    elif args.test_exit_code is None or (args.test_exit_code == 0) != (args.test_status == "passed"):
        raise ValueError("The test exit code must match its passed or failed status.")
    return {
        "command": args.test_command,
        "status": args.test_status,
        "exitCode": args.test_exit_code,
    }


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
    event.add_argument("--test-command")
    event.add_argument("--test-status", choices=("passed", "failed", "skipped"))
    event.add_argument("--test-exit-code", type=int)
    args = parser.parse_args()
    try:
        if args.command == "start":
            result = begin(args.root.resolve(), args.checker)
        else:
            result = record(args.run, args.item, args.state, args.detail, test_result(args))
    except (OSError, ValueError, KeyError) as error:
        parser.exit(2, f"Interact: {error}\n")
    print(json.dumps(result))


if __name__ == "__main__":
    main()
