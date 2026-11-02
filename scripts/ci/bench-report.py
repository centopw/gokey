#!/usr/bin/env python3
"""Gate one latency measurement against V-PERF's budget and the rolling baseline.

The input is the harness's JSON (`openkey-bench/2`, C-016 contract rev. r2).
Three checks, in this order, because a number from a malformed run must not be
compared with anything:

0. **Contract shape.** Schema 2, 100,000 events, five repetitions, five per-rep
   p99 values, and a non-empty `runner_image`. A run that skipped the workload
   is not a slow run, it is not a measurement, and averaging it into the
   baseline would poison the sentinel.
1. **Absolute gate.** `p99_ms <= --gate` (0.500 ms, G1). A breach is `::error::`
   and exit 1: this is the number 1.0 ships against.
2. **Sentinel.** Comparable when `stream_fnv1a`, `os`, `arch` and `runner_image`
   match. `runner` is record-only and is never compared: on GitHub-hosted
   runners it is unique per job, so a stored baseline's runner can never equal
   the next run's and the ratio would never be evaluated. A schema-1 baseline
   (no `runner_image`) is still compared on `stream_fnv1a` + `os` + `arch`, with
   a `::notice::` that the key is weaker, so the first run after the schema bump
   produces a real ratio instead of discarding the baseline. Above `--sentinel`
   (1.10x) is `::warning::` and the job stays green — a nightly regression is
   triaged (plan §5.2), not a merge block. A different workload, platform or
   image is `::notice::`: this run becomes the baseline instead of being
   measured against an unrelated one.

The baseline is written whenever the gate passes, so the rolling value is the
last good run. On a gate failure nothing is written and the downloaded baseline
survives for the next run.

Exit status: 0 gate passed (a sentinel breach still exits 0), 1 contract
violation or gate breach, 2 could not read the inputs.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

SCHEMA = "openkey-bench/2"
LEGACY_SCHEMA = "openkey-bench/1"
STREAM_EVENTS = 100_000
REPETITIONS = 5


def fail(message: str, code: int = 1) -> int:
    print(message, file=sys.stderr)
    return code


def load_json(path: Path, label: str) -> dict[str, Any] | None:
    try:
        with path.open(encoding="utf-8") as fh:
            value = json.load(fh)
    except FileNotFoundError:
        return None
    except (OSError, json.JSONDecodeError) as exc:
        print(f"::warning::{label} {path} is unreadable ({exc}); treating it as absent", file=sys.stderr)
        return None
    if not isinstance(value, dict):
        print(f"::warning::{label} {path} is not a JSON object; treating it as absent", file=sys.stderr)
        return None
    return value


def is_number(value: Any) -> bool:
    return isinstance(value, (int, float)) and not isinstance(value, bool)


def is_text(value: Any) -> bool:
    return isinstance(value, str) and bool(value)


def contract_errors(doc: dict[str, Any]) -> list[str]:
    """Every reason this document is not an `openkey-bench/2` measurement."""
    errors: list[str] = []
    if doc.get("schema") != SCHEMA:
        errors.append(f"schema is {doc.get('schema')!r}, expected {SCHEMA!r}")
    if doc.get("stream_events") != STREAM_EVENTS:
        errors.append(f"stream_events is {doc.get('stream_events')!r}, expected {STREAM_EVENTS}")
    if doc.get("repetitions") != REPETITIONS:
        errors.append(f"repetitions is {doc.get('repetitions')!r}, expected {REPETITIONS}")
    per_rep = doc.get("p99_per_rep_ms")
    if not isinstance(per_rep, list) or len(per_rep) != REPETITIONS:
        errors.append(
            f"p99_per_rep_ms must be a list of length {REPETITIONS}, got {per_rep!r}"
        )
    for key in ("p50_ms", "p99_ms", "p999_ms", "max_ms", "keystrokes_per_s", "gate_ms"):
        if not is_number(doc.get(key)):
            errors.append(f"{key} is {doc.get(key)!r}, expected a number")
    for key in ("commit", "runner", "runner_image", "stream_fnv1a", "os", "arch"):
        if not is_text(doc.get(key)):
            errors.append(f"{key} is {doc.get(key)!r}, expected a non-empty string")
    p99 = doc.get("p99_ms")
    if is_number(p99) and p99 < 0:
        errors.append(f"p99_ms is {p99!r}, expected a non-negative duration")
    return errors


def usable_baseline(doc: dict[str, Any] | None) -> bool:
    """A baseline is usable if it carries a comparable key and a positive p99."""
    if doc is None or doc.get("schema") not in (SCHEMA, LEGACY_SCHEMA):
        return False
    if not is_text(doc.get("stream_fnv1a")) or not is_text(doc.get("os")) or not is_text(doc.get("arch")):
        return False
    if doc.get("schema") == SCHEMA and not is_text(doc.get("runner_image")):
        return False
    p99 = doc.get("p99_ms")
    return is_number(p99) and p99 > 0


def write_baseline(source: Path, destination: Path) -> None:
    """Copy the measurement bytes, so the baseline keeps exactly what the run emitted."""
    destination.parent.mkdir(parents=True, exist_ok=True)
    try:
        if source.resolve() == destination.resolve():
            return
    except OSError:
        pass
    destination.write_bytes(source.read_bytes())


def fmt(value: Any, digits: int = 3) -> str:
    if is_number(value):
        return f"{value:.{digits}f}"
    return str(value)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Gate one openkey-bench/2 measurement.")
    parser.add_argument("--current", required=True, type=Path, help="this run's JSON")
    parser.add_argument("--baseline", type=Path, help="previous baseline JSON (may be absent)")
    parser.add_argument("--write-baseline", type=Path, help="where to store this run as the baseline")
    parser.add_argument("--sentinel", type=float, default=1.10, help="regression factor (default 1.10)")
    parser.add_argument("--gate", type=float, default=0.500, help="absolute p99 gate in ms (default 0.500)")
    args = parser.parse_args(argv)

    current = load_json(args.current, "current")
    if current is None or not args.current.is_file():
        return fail(f"::error::no measurement at {args.current}", 2)

    errors = contract_errors(current)
    if errors:
        print(f"::error::{args.current} is not an openkey-bench/2 measurement", file=sys.stderr)
        for error in errors:
            print(f"  - {error}", file=sys.stderr)
        return 1

    p99 = float(current["p99_ms"])
    baseline = load_json(args.baseline, "baseline") if args.baseline else None
    baseline_ok = usable_baseline(baseline)
    baseline_p99 = float(baseline["p99_ms"]) if baseline_ok else None
    ratio: float | None = None

    if p99 <= args.gate:
        print(f"gate: PASS p99 {p99:.6f} ms <= {args.gate:.3f} ms")
    else:
        print(f"::error::gate: p99 {p99:.6f} ms > {args.gate:.3f} ms (V-PERF budget)")
        # The summary still prints, so the failing log carries the numbers.
        print_summary(current, baseline_p99, None)
        return 1

    if not baseline_ok:
        print("::notice::no usable baseline (first run or unreadable); this run seeds the baseline")
    elif (
        baseline["stream_fnv1a"] != current["stream_fnv1a"]
        or baseline["os"] != current["os"]
        or baseline["arch"] != current["arch"]
    ):
        print(
            "::notice::workload or platform changed "
            f"({baseline['stream_fnv1a']} {baseline['os']}/{baseline['arch']} -> "
            f"{current['stream_fnv1a']} {current['os']}/{current['arch']}); "
            "not comparable, this run replaces the baseline"
        )
    elif baseline["schema"] == SCHEMA and baseline["runner_image"] != current["runner_image"]:
        print(
            "::notice::runner image changed "
            f"({baseline['runner_image']} -> {current['runner_image']}); "
            "not comparable, this run replaces the baseline"
        )
    else:
        if baseline["schema"] == LEGACY_SCHEMA:
            print(
                "::notice::baseline is openkey-bench/1 (no runner_image); comparing on "
                "stream_fnv1a + os + arch, a weaker key"
            )
        ratio = p99 / baseline_p99  # type: ignore[operator]
        if ratio > args.sentinel:
            print(
                f"::warning::p99 regression {ratio:.3f}x > {args.sentinel:.2f}x sentinel "
                f"({p99:.6f} ms vs {baseline_p99:.6f} ms); triage, not a merge block"
            )
        else:
            print(f"sentinel: {ratio:.3f}x <= {args.sentinel:.2f}x")

    print_summary(current, baseline_p99, ratio)

    if args.write_baseline:
        write_baseline(args.current, args.write_baseline)
        print(f"baseline: wrote {args.write_baseline}")

    return 0


def print_summary(doc: dict[str, Any], baseline_p99: float | None, ratio: float | None) -> None:
    rows = [
        ("commit", doc.get("commit", "unknown")),
        ("runner", doc.get("runner", "unknown")),
        ("runner_image", doc.get("runner_image", "unknown")),
        ("events", doc.get("stream_events", "?")),
        ("p99", f"{fmt(doc.get('p99_ms'), 6)} ms"),
        ("p50", f"{fmt(doc.get('p50_ms'), 6)} ms"),
        ("p999", f"{fmt(doc.get('p999_ms'), 6)} ms"),
        ("keystrokes/s", fmt(doc.get("keystrokes_per_s"), 0)),
        ("baseline p99", f"{baseline_p99:.6f} ms" if baseline_p99 is not None else "n/a"),
        ("ratio", f"{ratio:.3f}x" if ratio is not None else "n/a"),
    ]
    width = max(len(label) for label, _ in rows)
    print()
    print("per-keystroke latency")
    print(f"{'-' * (width + 2)}{'-' * 24}")
    for label, value in rows:
        print(f"{label:<{width}}  {value}")


if __name__ == "__main__":
    sys.exit(main())
