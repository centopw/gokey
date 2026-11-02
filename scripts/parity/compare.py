#!/usr/bin/env python3
"""Compare a harness run against a script's `expected` column.

The harness produces; this judges. Keeping the two apart is what makes a
baseline reviewable: the binary has no expected value inside it, so a wrong
baseline is a visible diff in the script file rather than a tautology.

Both sides are written in one escape vocabulary — the harness's own
(`decodeInput` for the input column, `escape` for the document): `\\b` `\\n`
`\\t` `\\r` `\\s` `\\\\`. The decoder is single-pass because a chain of
`replace()` calls re-reads its own output, so a document holding a literal
backslash-`n` decodes to a newline. A field of exactly `-` is the empty
document, on both sides.

This file is the gate's judge, so every path that cannot decide must fail: a
comparator that returns success when it compared nothing is a gate nobody
checks again.

Input:
  <script.tsv>   `id \\t method \\t options \\t input [\\t expected [\\t macros]]`
  <harness.out>  the harness's stdout for that script

The script row is 4, 5 or 6 fields and a seventh is an error. The sixth column
declares the macro table (`name=content;name=content`, literal UTF-8, `-` or an
absent column is the empty table). It is validated here and then ignored: this
comparator judges the document the harness printed, and the harness already
replayed the table. Accepting a seventh column, or a macros cell the harness
would refuse, would let a row be judged against a table nobody built.

Exit status:
  0  every case with an expected column agrees
  1  at least one case disagrees
  2  could not judge — the inputs do not line up, a case lost its expected
     column, a row has the wrong number of columns, or nothing was compared at all
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

# `b` and `s` appear only in the input/expected vocabulary, `r` only in the
# harness's output vocabulary; one table decodes both columns without
# ambiguity because the escape is always the character after the backslash.
ESCAPES = {"b": "\b", "n": "\n", "t": "\t", "r": "\r", "s": " ", "\\": "\\"}
EMPTY = "-"


class InputError(Exception):
    """The script or the results are malformed; exit 2 rather than 1."""


def unescape(text: str, where: str) -> str:
    """Decode one field. `where` is `file:line: column` for the message."""
    if text == EMPTY:
        return ""
    out: list[str] = []
    i = 0
    while i < len(text):
        ch = text[i]
        if ch != "\\":
            out.append(ch)
            i += 1
            continue
        if i + 1 == len(text):
            raise InputError(f"{where}: trailing backslash")
        esc = text[i + 1]
        if esc not in ESCAPES:
            raise InputError(
                f"{where}: unknown escape \\{esc} (allowed: \\b \\n \\t \\r \\s \\\\)"
            )
        out.append(ESCAPES[esc])
        i += 2
    return "".join(out)


def check_macros(text: str, where: str) -> list[tuple[str, str]]:
    """Validate the sixth column and return the pairs.

    Mirrors `corpus.rs::parse_macros` and `harness.cpp::parseMacros` exactly:
    literal UTF-8, no escape decoding, the first `=` splits a pair, a duplicate
    name is legal (`addMacro` replaces the content), `-` is the empty table and
    an empty cell is an error. The table is not used here — the harness already
    built it — but a cell it would refuse must not be judged as if it had.
    """
    if text == EMPTY:
        return []
    if text == "":
        raise InputError(f"{where}: empty macros cell (use `-` for the empty table)")
    pairs: list[tuple[str, str]] = []
    for raw in text.split(";"):
        if raw == "":
            raise InputError(
                f"{where}: empty macro pair (pairs are separated by `;`, and a trailing "
                "`;` is a pair)"
            )
        name, sep, content = raw.partition("=")
        if not sep:
            raise InputError(f"{where}: macro pair {raw!r} has no `=` (expected name=content)")
        if name == "":
            raise InputError(f"{where}: macro pair {raw!r} has an empty name")
        pairs.append((name, content))
    return pairs


def read_script(path: Path) -> list[tuple[str, str, str | None, int]]:
    cases: list[tuple[str, str, str | None, int]] = []
    seen: set[str] = set()
    for lineno, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        fields = line.split("\t")
        if len(fields) < 4 or len(fields) > 6:
            raise InputError(
                f"{path}:{lineno}: expected 4, 5 or 6 tab-separated fields, got {len(fields)}"
            )
        case_id = fields[0]
        if case_id in seen:
            raise InputError(f"{path}:{lineno}: duplicate case id {case_id!r}")
        seen.add(case_id)
        expected = (
            unescape(fields[4], f"{path}:{lineno}: expected") if len(fields) > 4 else None
        )
        if len(fields) > 5:
            check_macros(fields[5], f"{path}:{lineno}: macros")
        cases.append((case_id, fields[3], expected, lineno))
    return cases


def read_results(path: Path) -> dict[str, str]:
    """Final document per case, decoded from the harness's transport escaping."""
    finals: dict[str, str] = {}
    for lineno, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if not line or line.startswith("#"):
            continue
        fields = line.split("\t")
        if len(fields) < 7:
            raise InputError(
                f"{path}:{lineno}: expected at least 7 tab-separated fields, got {len(fields)}"
            )
        case_id, step, document = fields[0], fields[1], fields[6]
        if step != "final":
            continue
        if case_id in finals:
            raise InputError(f"{path}:{lineno}: two final lines for {case_id!r}")
        finals[case_id] = unescape(document, f"{path}:{lineno}: document")
    return finals


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("script", type=Path)
    ap.add_argument("results", type=Path)
    args = ap.parse_args()

    try:
        cases = read_script(args.script)
        finals = read_results(args.results)
    except InputError as exc:
        print(f"::error::{exc}", file=sys.stderr)
        return 2

    expected_ids = [case_id for case_id, _, _, _ in cases]
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

    unblessed = [(case_id, lineno) for case_id, _, expected, lineno in cases if expected is None]
    if unblessed:
        listed = ", ".join(f"{case_id} (line {lineno})" for case_id, lineno in unblessed[:5])
        print(f"::error::{args.script.name}: {len(unblessed)} case(s) have no expected column: "
              f"{listed}. Write the fifth field; there is no opt-out.", file=sys.stderr)
        return 2

    failed = checked = 0
    for case_id, typed, expected, _lineno in cases:
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

    if checked == 0:
        print(f"::error::{args.script.name}: nothing was compared — 0 of {len(cases)} case(s) "
              "carry an expected column, and an empty script is not a pass",
              file=sys.stderr)
        return 2

    print(f"{checked - failed}/{checked} cases agree")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
