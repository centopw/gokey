#!/usr/bin/env python3
"""Compare a harness run against a script's `expected` column.

The harness produces; this judges. Keeping the two apart is what makes a
baseline reviewable: the binary has no expected value inside it, so a wrong
baseline is a visible diff in the script file rather than a tautology.

Input:
  <script.tsv>   `id \\t method \\t options \\t input [\\t expected]`
  <harness.out>  the harness's stdout for that script

`-` means "empty document" in both files; the harness writes it that way and
the script reads it back the same way, so a case whose correct answer is an
empty document is written `-` rather than a dangling empty field.

Exit status: 0 all agree, 1 a case disagrees, 2 the inputs do not line up.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path


def read_script(path: Path) -> list[tuple[str, str, str | None]]:
    cases: list[tuple[str, str, str | None]] = []
    seen: set[str] = set()
    for lineno, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        fields = line.split("\t")
        if len(fields) < 4:
            raise SystemExit(f"{path}:{lineno}: expected at least 4 tab-separated fields")
        case_id = fields[0]
        if case_id in seen:
            raise SystemExit(f"{path}:{lineno}: duplicate case id {case_id!r}")
        seen.add(case_id)
        expected = fields[4] if len(fields) > 4 else None
        if expected == "-":
            expected = ""
        cases.append((case_id, fields[3], expected))
    return cases


def read_results(path: Path) -> dict[str, str]:
    """Final document per case. The harness escapes `\\`, TAB and LF; undo that
    here so a comparison is against the text a field would hold, not its
    transport encoding."""
    finals: dict[str, str] = {}
    for lineno, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if not line or line.startswith("#"):
            continue
        fields = line.split("\t")
        case_id, step, document = fields[0], fields[1], fields[6]
        if step != "final":
            continue
        if case_id in finals:
            raise SystemExit(f"{path}:{lineno}: two final lines for {case_id!r}")
        document = document.replace("\\\\", "\\").replace("\\t", "\t").replace("\\n", "\n")
        finals[case_id] = "" if document == "-" else document
    return finals


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("script", type=Path)
    ap.add_argument("results", type=Path)
    args = ap.parse_args()

    cases = read_script(args.script)
    finals = read_results(args.results)

    expected_ids = [c for c, _, _ in cases]
    missing = [c for c in expected_ids if c not in finals]
    extra = [c for c in finals if c not in set(expected_ids)]
    if missing or extra:
        if missing:
            print(f"::error::{args.script.name}: the harness did not report {len(missing)} case(s): "
                  + ", ".join(missing[:5]), file=sys.stderr)
        if extra:
            print(f"::error::{args.script.name}: the harness reported {len(extra)} case(s) that are "
                  "not in the script: " + ", ".join(extra[:5]), file=sys.stderr)
        return 2

    failed = checked = 0
    for case_id, typed, expected in cases:
        if expected is None:
            print(f"SKIP {case_id}: no expected column (input-only script)")
            continue
        checked += 1
        actual = finals[case_id]
        if actual == expected:
            continue
        failed += 1
        print(f"FAIL {case_id}")
        print(f"  input    {typed!r}")
        print(f"  expected {expected!r}")
        print(f"  actual   {actual!r}")
        for i, (a, b) in enumerate(zip(expected, actual)):
            if a != b:
                print(f"  first difference at code point {i}: U+{ord(a):04X} != U+{ord(b):04X}")
                break
        else:
            print(f"  one is a prefix of the other: expected {len(expected)} code points, "
                  f"actual {len(actual)}")

    print(f"{checked - failed}/{checked} cases agree"
          + (f", {len(cases) - checked} skipped" if len(cases) != checked else ""))
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
