#!/usr/bin/env python3
"""Verify the parity oracle's pin (docs/m0/ORACLE-PIN.json).

The corpus baselines are artefacts of one upstream commit. If the tree those
baselines were generated from is not the tree this file names, every "agrees"
in the parity report is a claim about an unknown engine. So the check has two
halves, and the first one is the one that matters:

1. **Fetch by hash, never by ref.** ORACLE-PIN.json's `retrieval.caveat` says
   the tip can move; fetching `master` and comparing afterwards would mean the
   build used whatever the tip was and only the file hashes caught it. Here the
   commit is the fetch argument and the resolved HEAD is asserted against it.
2. **Hash every pinned file.** A commit id alone proves the tree if the object
   store is trusted; the digests prove it to a reader who does not trust ours,
   and they catch a local `--source` checkout that was edited in place.

Exit status: 0 verified, 1 mismatch, 2 could not run.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_PIN = ROOT / "docs" / "m0" / "ORACLE-PIN.json"


def run(cmd: list[str], cwd: Path | None = None) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, check=False)


def fetch(pin: dict, dest: Path) -> Path:
    commit = pin["upstream"]["commit"]
    repo = pin["upstream"]["repo"]

    if (dest / ".git").exists():
        head = run(["git", "-C", str(dest), "rev-parse", "HEAD"])
        if head.returncode == 0 and head.stdout.strip() == commit:
            print(f"oracle: reusing {dest} at {commit[:12]}")
            return dest
        # A cache at a different commit is not repairable in place: reuse the
        # object store and let the fetch below bring the pin in, then assert the
        # resolved HEAD. The digests then decide whether the worktree is right.
        print(f"oracle: {dest} is not at {commit[:12]}; refetching")

    dest.mkdir(parents=True, exist_ok=True)
    init = run(["git", "init", "--quiet", str(dest)])
    if init.returncode != 0:
        raise SystemExit(f"::error::git init failed: {init.stderr.strip()}")

    print(f"oracle: fetching {repo} at {commit} (by hash, depth 1)")
    got = run(["git", "-C", str(dest), "fetch", "--depth", "1", "--no-tags", repo, commit])
    if got.returncode != 0:
        raise SystemExit(
            "::error::cannot fetch the pinned commit by hash. The pin requires fetching the commit "
            f"itself, not a branch: {got.stderr.strip()}"
        )
    checkout = run(["git", "-C", str(dest), "checkout", "--quiet", "FETCH_HEAD"])
    if checkout.returncode != 0:
        raise SystemExit(f"::error::checkout of FETCH_HEAD failed: {checkout.stderr.strip()}")

    head = run(["git", "-C", str(dest), "rev-parse", "HEAD"]).stdout.strip()
    if head != commit:
        raise SystemExit(
            f"::error::resolved commit {head} is not the pinned {commit}; refusing to build an "
            "oracle from an unpinned tree"
        )
    return dest


def digest(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def verify(pin: dict, source: Path) -> list[str]:
    problems: list[str] = []
    files: dict[str, str] = dict(pin["engine_files_sha256"])
    files["LICENSE"] = pin["upstream"]["license_file_sha256"]

    for rel, want in sorted(files.items()):
        path = source / rel
        if not path.is_file():
            problems.append(f"::error::pinned file is missing: {rel}")
            continue
        got = digest(path)
        if got != want:
            problems.append(
                f"::error::{rel} does not match the pin\n"
                f"    pinned   {want}\n"
                f"    observed {got}"
            )
    return problems


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--pin", type=Path, default=DEFAULT_PIN)
    ap.add_argument("--dest", type=Path, default=ROOT / ".oracle" / "src",
                    help="checkout cache for the pinned commit (created if absent)")
    ap.add_argument("--source", type=Path,
                    help="verify an existing checkout instead of fetching; no network")
    args = ap.parse_args()

    if not args.pin.is_file():
        print(f"::error::pin not found: {args.pin}", file=sys.stderr)
        return 2
    pin = json.loads(args.pin.read_text(encoding="utf-8"))
    if pin.get("schema") != "gokey.oracle-pin/v1":
        print(f"::error::{args.pin} schema is {pin.get('schema')!r}, expected gokey.oracle-pin/v1",
              file=sys.stderr)
        return 2

    commit = pin["upstream"]["commit"]
    try:
        source = args.source if args.source else fetch(pin, args.dest)
    except SystemExit as exc:
        print(str(exc), file=sys.stderr)
        return 2

    if not source.is_dir():
        print(f"::error::source checkout not found: {source}", file=sys.stderr)
        return 2

    problems = verify(pin, source)
    if problems:
        print("\n".join(problems), file=sys.stderr)
        return 1

    count = len(pin["engine_files_sha256"]) + 1
    print(f"oracle: {count} files verified against {commit[:12]} "
          f"({pin['upstream']['version_json']['latestVersion']['versionName']})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
