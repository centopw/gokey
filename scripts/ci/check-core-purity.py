#!/usr/bin/env python3
"""Core purity gate (ADR-0008 §6).

`openkey-core` is the platform-neutral engine: no I/O, no OS calls, no globals,
no `unsafe`, and no dependency on a platform crate. Those promises are the whole
reason the parity harness can diff one implementation against the C++ oracle on
any OS, so they are checked mechanically rather than trusted to review.

Scope: `crates/openkey-core/src`, excluding `#[cfg(test)]` items. Tests may read
fixtures; banning `std::fs` there would either fail honest tests or push fixtures
into inline string literals, which is worse than the thing being prevented.

Exit status: 0 clean, 1 violation, 2 the gate could not run.
"""

from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CRATE = ROOT / "crates" / "openkey-core"
CRATE_NAME = "openkey-core"

BANNED_TOKENS = {
    "std::fs": "filesystem I/O",
    "std::net": "network I/O",
    "std::env": "process environment",
    "std::process": "process spawning",
    "std::thread": "threading",
    "std::fs::": "filesystem I/O",
    "static mut": "process-global mutable state",
    "unsafe": "unsafe code",
}

PLATFORM_CRATES = {
    "openkey-platform",
    "openkey-macos",
    "openkey-windows",
    "openkey-linux",
}


def strip_cfg_test(source: str) -> list[tuple[int, str]]:
    """Return (line_number, text) with `#[cfg(test)]` items blanked out."""
    lines = source.splitlines()
    out: list[tuple[int, str]] = []
    i = 0
    depth_guard = 0
    while i < len(lines):
        line = lines[i]
        if line.strip().startswith("#[cfg(test)]"):
            # Skip attributes attached to the item, then the item itself.
            i += 1
            while i < len(lines) and lines[i].strip().startswith("#["):
                i += 1
            depth = 0
            started = False
            while i < len(lines):
                depth += lines[i].count("{") - lines[i].count("}")
                if lines[i].count("{"):
                    started = True
                if (started and depth <= 0) or (not started and lines[i].rstrip().endswith(";")):
                    i += 1
                    break
                i += 1
                depth_guard += 1
                if depth_guard > 100_000:
                    raise RuntimeError("brace matching did not terminate")
            continue
        out.append((i + 1, line))
        i += 1
    return out


def scan_sources() -> list[str]:
    problems: list[str] = []
    sources = sorted((CRATE / "src").rglob("*.rs"))
    if not sources:
        problems.append(f"::error::no Rust sources found under {CRATE / 'src'}")
        return problems
    for path in sources:
        rel = path.relative_to(ROOT)
        for lineno, line in strip_cfg_test(path.read_text(encoding="utf-8")):
            stripped = line.strip()
            if stripped.startswith("//"):
                continue
            for token, why in BANNED_TOKENS.items():
                pattern = r"\bunsafe\b" if token == "unsafe" else re.escape(token)
                if re.search(pattern, line):
                    problems.append(
                        f"::error file={rel},line={lineno}::`{token}` in openkey-core ({why}): {stripped}"
                    )
    return problems


def scan_dependencies() -> list[str]:
    try:
        metadata = json.loads(
            subprocess.run(
                ["cargo", "metadata", "--format-version", "1", "--locked", "--no-deps"],
                cwd=ROOT,
                check=True,
                capture_output=True,
                text=True,
            ).stdout
        )
    except (subprocess.CalledProcessError, FileNotFoundError) as exc:
        raise SystemExit(f"::error::cargo metadata failed: {exc}")

    packages = {p["name"]: p for p in metadata["packages"]}
    core = packages.get(CRATE_NAME)
    if core is None:
        raise SystemExit(f"::error::{CRATE_NAME} is not a workspace member")

    problems: list[str] = []
    normal = [d["name"] for d in core["dependencies"] if d["kind"] is None]
    for name in normal:
        if name in PLATFORM_CRATES:
            problems.append(
                f"::error::{CRATE_NAME} depends on `{name}`; core must not know about the platform layer"
            )
        if name == "openkey-ai":
            problems.append(
                f"::error::{CRATE_NAME} depends on `openkey-ai`; the AI seam must stay inert in 1.0"
            )

    closure = subprocess.run(
        ["cargo", "tree", "-p", CRATE_NAME, "--edges", "normal", "--prefix", "none", "--locked"],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    if closure.returncode != 0:
        problems.append(f"::error::cargo tree failed: {closure.stderr.strip()}")
    else:
        for line in closure.stdout.splitlines():
            name = line.split(" v", 1)[0].strip()
            if name in PLATFORM_CRATES or name == "openkey-ai":
                problems.append(
                    f"::error::`{name}` is in {CRATE_NAME}'s dependency closure: {line.strip()}"
                )
    return problems


def main() -> int:
    problems = scan_sources() + scan_dependencies()
    if problems:
        for problem in problems:
            print(problem)
        print(f"core purity: {len(problems)} violation(s)")
        return 1
    print("core purity: ok")
    return 0


if __name__ == "__main__":
    sys.exit(main())
