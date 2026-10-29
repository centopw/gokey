#!/usr/bin/env python3
"""Tests for the oracle-pin checker.

The checker's dangerous failure mode is a false *pass*: if it reported success
against a tree that is not the pin, every corpus baseline generated from that
tree would carry a claim about an unknown engine. So each test below pins one
way the check can go wrong — a corrupted file, a missing file, a pin that is not
the pin — plus a positive case so the suite cannot pass vacuously.

No test touches the network: `fetch()` is exercised on its cache-reuse branch and
`verify()` on a temporary source tree.

Run with the standard library only: `python3 -m unittest discover -s scripts/ci`
"""

from __future__ import annotations

import hashlib
import importlib.util
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

MODULE_PATH = Path(__file__).with_name("check-oracle-pin.py")
_spec = importlib.util.spec_from_file_location("check_oracle_pin", MODULE_PATH)
if _spec is None or _spec.loader is None:
    raise SystemExit(f"cannot load {MODULE_PATH}")
checker = importlib.util.module_from_spec(_spec)
sys.modules["check_oracle_pin"] = checker
_spec.loader.exec_module(checker)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def make_pin(files: dict[str, bytes], license_bytes: bytes = b"GPL-3.0\n") -> dict:
    return {
        "schema": "gokey.oracle-pin/v1",
        "upstream": {
            "repo": "https://invalid.example/never-fetched",
            "commit": "0" * 40,
            "license_file_sha256": sha256(license_bytes),
            "version_json": {"latestVersion": {"versionName": "test"}},
        },
        "engine_files_sha256": {rel: sha256(body) for rel, body in files.items()},
    }


def write_tree(root: Path, files: dict[str, bytes], license_bytes: bytes = b"GPL-3.0\n") -> None:
    for rel, body in files.items():
        path = root / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(body)
    (root / "LICENSE").write_bytes(license_bytes)


class Verify(unittest.TestCase):
    def test_matching_tree_has_no_problems(self) -> None:
        files = {"Sources/OpenKey/engine/Engine.cpp": b"// engine\n"}
        pin = make_pin(files)
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write_tree(root, files)
            self.assertEqual(checker.verify(pin, root), [])

    def test_corrupted_file_is_reported_with_the_digests(self) -> None:
        files = {"engine/Engine.cpp": b"// engine\n"}
        pin = make_pin(files)
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write_tree(root, files)
            (root / "engine/Engine.cpp").write_bytes(b"// patched\n")
            problems = checker.verify(pin, root)
        self.assertEqual(len(problems), 1)
        self.assertIn("engine/Engine.cpp", problems[0])
        self.assertIn(sha256(b"// engine\n"), problems[0])
        self.assertIn(sha256(b"// patched\n"), problems[0])

    def test_missing_file_is_reported(self) -> None:
        files = {"engine/Engine.cpp": b"// engine\n"}
        pin = make_pin(files)
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write_tree(root, files)
            (root / "engine/Engine.cpp").unlink()
            problems = checker.verify(pin, root)
        self.assertEqual(len(problems), 1)
        self.assertIn("missing", problems[0])

    def test_license_is_checked_even_though_it_is_not_an_engine_file(self) -> None:
        # The licence digest lives under `upstream`, not in engine_files_sha256,
        # so a loop over the latter alone would never look at it.
        pin = make_pin({})
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write_tree(root, {}, license_bytes=b"not the licence\n")
            problems = checker.verify(pin, root)
        self.assertEqual(len(problems), 1)
        self.assertIn("LICENSE", problems[0])


class Main(unittest.TestCase):
    def run_main(self, pin: dict, source: Path) -> int:
        with tempfile.TemporaryDirectory() as tmp:
            pin_path = Path(tmp) / "ORACLE-PIN.json"
            pin_path.write_text(json.dumps(pin), encoding="utf-8")
            argv = sys.argv
            sys.argv = [str(MODULE_PATH), "--pin", str(pin_path), "--source", str(source)]
            try:
                return checker.main()
            finally:
                sys.argv = argv

    def test_verified_source_exits_zero(self) -> None:
        files = {"engine/Engine.cpp": b"// engine\n"}
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write_tree(root, files)
            self.assertEqual(self.run_main(make_pin(files), root), 0)

    def test_corrupted_source_exits_one(self) -> None:
        files = {"engine/Engine.cpp": b"// engine\n"}
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write_tree(root, files)
            (root / "engine/Engine.cpp").write_bytes(b"// patched\n")
            self.assertEqual(self.run_main(make_pin(files), root), 1)

    def test_foreign_schema_is_refused_not_treated_as_current(self) -> None:
        pin = make_pin({})
        pin["schema"] = "gokey.oracle-pin/v0"
        with tempfile.TemporaryDirectory() as tmp:
            self.assertEqual(self.run_main(pin, Path(tmp)), 2)


class Fetch(unittest.TestCase):
    def test_a_checkout_already_at_the_pin_is_reused_without_network(self) -> None:
        # `repo` points at a name that cannot resolve: reaching the network here
        # would fail the test, which is the point — the reuse branch must decide
        # on the local HEAD alone.
        with tempfile.TemporaryDirectory() as tmp:
            dest = Path(tmp) / "src"
            dest.mkdir()
            git = ["git", "-C", str(dest), "-c", "user.email=t@example.com", "-c", "user.name=t"]
            subprocess.run(["git", "init", "--quiet", str(dest)], check=True)
            (dest / "file").write_text("x")
            subprocess.run(git + ["add", "file"], check=True)
            subprocess.run(git + ["commit", "--quiet", "-m", "pin"], check=True)
            head = subprocess.run(git + ["rev-parse", "HEAD"], check=True,
                                  capture_output=True, text=True).stdout.strip()

            pin = make_pin({})
            pin["upstream"]["commit"] = head
            self.assertEqual(checker.fetch(pin, dest), dest)


if __name__ == "__main__":
    unittest.main()
