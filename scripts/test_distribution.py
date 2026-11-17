"""Exercise installation boundaries, rollback and portable ZIP contents."""

import json
from pathlib import Path
import stat
import tempfile
import unittest
from unittest.mock import patch
import zipfile

import install
import package


class DistributionTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="skill-distribution-test-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.destination = self.root / "installed"

    def archive(self, version="0.1.0", platform="any", extra=None):
        archive = self.root / f"skill-{version}.zip"
        metadata = {"schemaVersion": 1, "name": "sample-skill", "version": version,
                    "platform": platform}
        with zipfile.ZipFile(archive, "w") as output:
            output.writestr("sample-skill/distribution.json", json.dumps(metadata))
            output.writestr("sample-skill/SKILL.md", f"Skill {version}\n")
            for name, contents in (extra or {}).items():
                output.writestr(name, contents)
        return archive

    def test_install_and_explicit_replace_record_version_and_preserve_other_skills(self):
        archive = self.archive()
        result = install.install_archive(archive, self.destination)
        target = Path(result["installed"])
        self.assertEqual((target / "SKILL.md").read_text(), "Skill 0.1.0\n")
        receipt = json.loads((target / ".installation.json").read_text())
        self.assertEqual(receipt["version"], "0.1.0")
        self.assertEqual(len(receipt["sha256"]), 64)
        other = self.destination / "other-skill"
        other.mkdir()
        (other / "SKILL.md").write_text("Keep")
        with self.assertRaises(FileExistsError):
            install.install_archive(archive, self.destination)
        install.install_archive(self.archive("0.2.0"), self.destination, replace=True)
        self.assertEqual((target / "SKILL.md").read_text(), "Skill 0.2.0\n")
        self.assertEqual((other / "SKILL.md").read_text(), "Keep")
        self.assertEqual(list(self.destination.glob(".sample-skill.install-*")), [])

    def test_failed_replacement_restores_previous_installation(self):
        install.install_archive(self.archive(), self.destination)
        archive = self.archive("0.2.0")
        replace = install.os.replace

        def fail_incoming(source, target):
            if Path(source).name == "sample-skill" and Path(source).parent != self.destination:
                raise OSError("Simulated replacement failure")
            return replace(source, target)

        with patch.object(install.os, "replace", side_effect=fail_incoming):
            with self.assertRaisesRegex(OSError, "Simulated replacement failure"):
                install.install_archive(archive, self.destination, replace=True)
        self.assertEqual((self.destination / "sample-skill/SKILL.md").read_text(), "Skill 0.1.0\n")
        self.assertEqual(list(self.destination.glob(".sample-skill.install-*")), [])

    def test_failed_restore_retains_backup(self):
        install.install_archive(self.archive(), self.destination)
        archive = self.archive("0.2.0")
        replace = install.os.replace

        def fail_new_and_restore(source, target):
            if Path(source).parent != self.destination:
                raise OSError("Simulated target unavailable")
            return replace(source, target)

        with patch.object(install.os, "replace", side_effect=fail_new_and_restore):
            with self.assertRaisesRegex(RuntimeError, "previous installation retained"):
                install.install_archive(archive, self.destination, replace=True)
        backups = list(self.destination.glob(".sample-skill.install-*/previous/SKILL.md"))
        self.assertEqual(len(backups), 1)
        self.assertEqual(backups[0].read_text(), "Skill 0.1.0\n")

    def test_wrong_platform_and_traversal_leave_existing_installation_intact(self):
        install.install_archive(self.archive(), self.destination)
        for archive in (self.archive("0.2.0", "invalid-platform"),
                        self.archive("0.3.0", extra={"sample-skill/../../outside": "Bad"})):
            with self.subTest(archive=archive.name), self.assertRaises(ValueError):
                install.install_archive(archive, self.destination, replace=True)
        self.assertEqual((self.destination / "sample-skill/SKILL.md").read_text(), "Skill 0.1.0\n")
        self.assertFalse((self.root / "outside").exists())

    def test_concurrent_install_is_blocked(self):
        self.destination.mkdir()
        with install.installation_lock(self.destination, "sample-skill"):
            with self.assertRaisesRegex(RuntimeError, "in progress"):
                install.install_archive(self.archive(), self.destination)

    def test_archive_keeps_executable_bits_and_runtime_files(self):
        payload = self.root / "sample-skill"
        (payload / "bin").mkdir(parents=True)
        (payload / "bin/checker").write_bytes(b"binary")
        (payload / "build.sh").write_text("#!/bin/sh\n")
        (payload / "SKILL.md").write_text("Instructions")
        runtime = payload / "assets/rules"
        runtime.mkdir(parents=True)
        (runtime / "policy.json").write_text(json.dumps({"enabled": True}))
        archive = self.root / "portable.zip"
        package.archive_payload(payload, archive)
        with zipfile.ZipFile(archive) as result:
            for name in ("sample-skill/build.sh", "sample-skill/bin/checker"):
                self.assertEqual(stat.S_IMODE(result.getinfo(name).external_attr >> 16), 0o755)
            self.assertIn("sample-skill/assets/rules/policy.json", result.namelist())

    def test_instruction_only_skill_packages_and_installs(self):
        repository = self.root / "repository"
        source = repository / "skills/sample-skill"
        source.mkdir(parents=True)
        (source / "SKILL.md").write_text("Instructions")
        with patch.object(package, "REPOSITORY", repository):
            archive = package.package_skill("sample-skill", "0.1.0", "unused")
        self.assertEqual(archive.name, "sample-skill-0.1.0-any.zip")
        install.install_archive(archive, self.destination)
        self.assertEqual((self.destination / "sample-skill/SKILL.md").read_text(), "Instructions")
        self.assertTrue((repository / "dist/SHA256SUMS").is_file())


if __name__ == "__main__":
    unittest.main()
