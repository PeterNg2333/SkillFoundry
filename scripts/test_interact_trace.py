import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


RECORDER = Path(__file__).resolve().parents[1] / "skills/maintainable-code/scripts/interact_trace.py"


class InteractTraceTests(unittest.TestCase):
    def test_board_updates_its_snapshot_and_retains_test_attempts(self):
        with tempfile.TemporaryDirectory() as directory:
            run = Path(directory) / "run-current.json"
            view = run.with_suffix(".html")
            view.write_text("Old report", encoding="utf-8")
            item = {
                "item": 1, "state": "Running", "events": [], "kind": "add-test",
                "file": "module.spec", "line": 5, "column": 1, "group": "module.spec",
                "text": "Reproduce the error.",
            }
            run.write_text(json.dumps({"root": "project", "items": [item]}), encoding="utf-8")
            args = [
                sys.executable, str(RECORDER), "event", "--run", str(run), "--item", "1",
                "--detail", "Observed test outcome.", "--test-command", "python -m unittest",
            ]
            invalid = subprocess.run(
                [*args, "--state", "Verified", "--test-status", "passed", "--test-exit-code", "1"],
                capture_output=True,
            )
            self.assertEqual(invalid.returncode, 2)
            self.assertEqual(json.loads(run.read_text())["items"][0]["events"], [])

            subprocess.run(
                [*args, "--state", "Running", "--test-status", "failed", "--test-exit-code", "1"],
                check=True, capture_output=True,
            )
            board = view.read_bytes()
            self.assertIn(b"Test results", board)
            snapshot = run.with_suffix(".data.js")
            first = snapshot.read_text(encoding="utf-8")
            self.assertIn('"status": "failed"', first)

            subprocess.run(
                [*args, "--state", "Verified", "--test-status", "passed", "--test-exit-code", "0"],
                check=True, capture_output=True,
            )
            self.assertEqual(view.read_bytes(), board)
            self.assertNotEqual(snapshot.read_text(encoding="utf-8"), first)
            events = json.loads(run.read_text())["items"][0]["events"]
            self.assertEqual([event["test"]["exitCode"] for event in events], [1, 0])


if __name__ == "__main__":
    unittest.main()
