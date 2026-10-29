#!/usr/bin/env python3
"""Tests for the oracle-pin checker.

The checker's dangerous failure mode is a false *pass*: if it reported success
against a tree that is not the pin, every corpus baseline generated from that
tree would carry a claim about an unknown engine. So each test below pins one
way the check can go wrong — a corrupted file, a missing file, a pin that is not
the pin, a copy that drifted from its record — plus a positive case so the suite
cannot pass vacuously.

No test touches the network: `fetch()` is exercised on its cache-reuse branch and
`verify()` on a temporary source tree.

Run with the standard library only: `python3 -m unittest discover -s scripts/ci`
"""

from __future__ import annotations

import hashlib
import importlib.util
import io
import json
import subprocess
import sys
import tempfile
import unittest
from contextlib import redirect_stderr
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
    def run_main(self, pin: dict, source: Path, record: bytes | None = None,
                 write_record: bool = True) -> tuple[int, str]:
        """Run `main()` against a temp layout shaped like the repo.

        The copy and the record are separate files, because the whole point of
        the pair is the check that they agree. `record` overrides the bytes
        written to the record; the default is the same bytes as the copy, i.e.
        an in-sync pair. `write_record=False` models the record being absent.
        """
        with tempfile.TemporaryDirectory() as tmp:
            repo = Path(tmp)
            body = json.dumps(pin).encode("utf-8")
            copy_path = repo / "parity" / "oracle" / "pin.json"
            record_path = repo / "docs" / "m0" / "ORACLE-PIN.json"
            copy_path.parent.mkdir(parents=True)
            record_path.parent.mkdir(parents=True)
            copy_path.write_bytes(body)
            if write_record:
                record_path.write_bytes(body if record is None else record)

            argv = sys.argv
            sys.argv = [str(MODULE_PATH), "--pin", str(copy_path),
                        "--record", str(record_path), "--source", str(source)]
            err = io.StringIO()
            try:
                with redirect_stderr(err):
                    return checker.main(), err.getvalue()
            finally:
                sys.argv = argv

    def test_verified_source_exits_zero(self) -> None:
        files = {"engine/Engine.cpp": b"// engine\n"}
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write_tree(root, files)
            rc, err = self.run_main(make_pin(files), root)
        self.assertEqual((rc, err), (0, ""))

    def test_corrupted_source_exits_one(self) -> None:
        files = {"engine/Engine.cpp": b"// engine\n"}
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write_tree(root, files)
            (root / "engine/Engine.cpp").write_bytes(b"// patched\n")
            self.assertEqual(self.run_main(make_pin(files), root)[0], 1)

    def test_foreign_schema_is_refused_not_treated_as_current(self) -> None:
        pin = make_pin({})
        pin["schema"] = "gokey.oracle-pin/v0"
        with tempfile.TemporaryDirectory() as tmp:
            self.assertEqual(self.run_main(pin, Path(tmp))[0], 2)

    def test_a_copy_that_drifted_from_the_record_exits_one(self) -> None:
        # A whitespace-only difference is still a difference: the two files are
        # read and edited by different people, so "same JSON" is not the
        # invariant — the same bytes are.
        files = {"engine/Engine.cpp": b"// engine\n"}
        pin = make_pin(files)
        drifted = json.dumps(pin, indent=2).encode("utf-8")
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write_tree(root, files)
            rc, err = self.run_main(pin, root, record=drifted)
        self.assertEqual(rc, 1)
        self.assertIn("not the same bytes", err)
        # Both digests, so the report says which side moved without a second run.
        self.assertIn(sha256(drifted), err)
        self.assertIn(sha256(json.dumps(pin).encode("utf-8")), err)
        # And the remediation names the direction: the copy is derived.
        self.assertIn("cp ", err)

    def test_a_missing_record_exits_two(self) -> None:
        files = {"engine/Engine.cpp": b"// engine\n"}
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write_tree(root, files)
            rc, err = self.run_main(make_pin(files), root, write_record=False)
        self.assertEqual(rc, 2)
        self.assertIn("pin record not found", err)

    def test_a_missing_machine_copy_exits_two(self) -> None:
        # Deleting the copy must not fall back to the record: the path CI
        # verifies is the one that has to exist.
        files = {"engine/Engine.cpp": b"// engine\n"}
        with tempfile.TemporaryDirectory() as tmp:
            repo = Path(tmp)
            source = repo / "source"
            write_tree(source, files)
            record = repo / "docs" / "m0" / "ORACLE-PIN.json"
            record.parent.mkdir(parents=True)
            record.write_text(json.dumps(make_pin(files)), encoding="utf-8")
            argv = sys.argv
            sys.argv = [str(MODULE_PATH),
                        "--pin", str(repo / "parity" / "oracle" / "pin.json"),
                        "--record", str(record), "--source", str(source)]
            err = io.StringIO()
            try:
                with redirect_stderr(err):
                    rc = checker.main()
            finally:
                sys.argv = argv
        self.assertEqual(rc, 2)
        self.assertIn("pin not found", err.getvalue())


class CommittedCopies(unittest.TestCase):
    def test_the_machine_copy_is_a_verbatim_copy_of_the_record(self) -> None:
        # The invariant this PR adds is also a property of the checkout, so it is
        # cheaper to pin it here than to discover the drift in a parity run that
        # has already fetched the oracle.
        repo = MODULE_PATH.parents[2]
        copy_path = repo / "parity" / "oracle" / "pin.json"
        record_path = repo / "docs" / "m0" / "ORACLE-PIN.json"
        self.assertEqual(
            copy_path.read_bytes(), record_path.read_bytes(),
            "parity/oracle/pin.json must be a byte-for-byte copy of "
            "docs/m0/ORACLE-PIN.json; refresh it from the record, not the other way round",
        )


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
