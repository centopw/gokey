#!/usr/bin/env python3
"""Core purity gate (ADR-0008 §6).

`openkey-core` is the platform-neutral engine: no I/O, no OS calls, no globals,
no `unsafe`, and no dependency on a platform crate. Those promises are the whole
reason the parity harness can diff one implementation against the C++ oracle on
any OS, so they are checked mechanically rather than trusted to review.

Scope: `crates/openkey-core/src`, excluding `#[cfg(test)]` items. Tests may read
fixtures; banning `std::fs` there would either fail honest tests or push fixtures
into inline string literals, which is worse than the thing being prevented.

One invariant here is not about I/O: `Word::letters` may be indexed only inside
`word.rs` (ADR-0002 D3). That scan reads physical lines *including* test modules,
because a test outside `word.rs` is exactly as unentitled to the field as engine
code — so it does not use `logical_lines()`, whose `#[cfg(test)]` exclusion exists
for the I/O rules.

`use` statements are expanded to one fully-qualified path per line before
matching. Matching raw lines misses the *idiomatic* form rustfmt produces:
`use std::{fs, net};` contains neither `std::fs` nor `std::net`. Grouped, nested
and aliased trees are all expanded, and a multi-line tree is joined first.
`test_check_core_purity.py` pins these shapes — a scanner with a known bypass
and no test regresses to the bypass.

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
    "static mut": "process-global mutable state",
    "unsafe": "unsafe code",
}

PLATFORM_CRATES = {
    "openkey-platform",
    "openkey-macos",
    "openkey-windows",
    "openkey-linux",
}


USE_START = re.compile(r"^\s*(?:pub(?:\s*\([^)]*\))?\s+)?use\b")
USE_ALIAS = re.compile(r"^(?P<path>.+?)\s+as\s+[A-Za-z_][A-Za-z0-9_]*$")


def _split_top_level(text: str, sep: str) -> list[str]:
    """Split on `sep` outside `{}` groups."""
    parts: list[str] = []
    depth = 0
    current = ""
    i = 0
    while i < len(text):
        char = text[i]
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
        if depth == 0 and text.startswith(sep, i):
            parts.append(current)
            current = ""
            i += len(sep)
            continue
        current += char
        i += 1
    parts.append(current)
    return [part.strip() for part in parts]


def expand_use_tree(tree: str, prefix: str = "") -> list[str]:
    """Expand one Rust `use` tree into fully-qualified leaf paths.

    `std::{fs, net}` -> `["std::fs", "std::net"]`
    `std::fs::{self, File}` -> `["std::fs", "std::fs::File"]`
    `std::fs as f` -> `["std::fs"]`  (the alias is dropped, the path is not)
    """
    tree = " ".join(tree.split())
    if not tree:
        return []
    if tree.startswith("{") and tree.endswith("}"):
        leaves: list[str] = []
        for part in _split_top_level(tree[1:-1], ","):
            if part:
                leaves.extend(expand_use_tree(part, prefix))
        return leaves

    accumulated = prefix
    for raw in _split_top_level(tree, "::"):
        part = raw.strip()
        if part.startswith("{"):
            return expand_use_tree(part, accumulated)
        alias = USE_ALIAS.match(part)
        if alias:
            part = alias.group("path").strip()
        if part == "self":
            return [accumulated] if accumulated else []
        accumulated = f"{accumulated}::{part}" if accumulated else part
    return [accumulated] if accumulated else []


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


def logical_lines(source: str) -> list[tuple[int, str]]:
    """Return (line_number, text) with every `use` tree expanded.

    A multi-line `use` statement is joined first: rustfmt wraps long trees, and a
    scan that reads one physical line at a time sees neither half.
    """
    physical = strip_cfg_test(source)
    out: list[tuple[int, str]] = []
    i = 0
    while i < len(physical):
        lineno, line = physical[i]
        if USE_START.match(line):
            statement = line
            while ";" not in statement:
                i += 1
                if i >= len(physical):
                    break  # unterminated: let the compiler report it
                statement += " " + physical[i][1]
            tree = USE_START.sub("", statement.split(";", 1)[0], count=1).strip()
            for leaf in expand_use_tree(tree):
                out.append((lineno, f"use {leaf};"))
            i += 1
            continue
        out.append((lineno, line))
        i += 1
    return out


def offending_tokens(line: str) -> list[str]:
    """Banned tokens present in one logical line, ignoring comment-only lines."""
    if line.strip().startswith("//"):
        return []
    found: list[str] = []
    for token in BANNED_TOKENS:
        pattern = r"\bunsafe\b" if token == "unsafe" else re.escape(token)
        if re.search(pattern, line):
            found.append(token)
    return found


def scan_sources() -> list[str]:
    problems: list[str] = []
    sources = sorted((CRATE / "src").rglob("*.rs"))
    if not sources:
        problems.append(f"::error::no Rust sources found under {CRATE / 'src'}")
        return problems
    for path in sources:
        rel = path.relative_to(ROOT)
        for lineno, line in logical_lines(path.read_text(encoding="utf-8")):
            for token in offending_tokens(line):
                problems.append(
                    f"::error file={rel},line={lineno}::`{token}` in openkey-core "
                    f"({BANNED_TOKENS[token]}): {line.strip()}"
                )
    return problems


WORD_INDEX = re.compile(r"\bletters\s*\[")


def word_index_offences(source: str) -> list[int]:
    """Line numbers in `source` that index `Word::letters` (ADR-0002 D3).

    Physical lines, not `logical_lines()`: test modules count too, and there is
    no `use` tree here to expand.
    """
    return [
        lineno
        for lineno, line in enumerate(source.splitlines(), start=1)
        if not line.strip().startswith("//") and WORD_INDEX.search(line)
    ]


def scan_word_encapsulation() -> list[str]:
    """`Word::letters` is indexed only by the module that owns it."""
    problems: list[str] = []
    for path in sorted((CRATE / "src").rglob("*.rs")):
        if path.name == "word.rs":
            continue
        rel = path.relative_to(ROOT)
        for lineno in word_index_offences(path.read_text(encoding="utf-8")):
            problems.append(
                f"::error file={rel},line={lineno}::`letters[` indexed outside word.rs; "
                "read the word through get()/span() (ADR-0002 D3)"
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
    problems = scan_sources() + scan_word_encapsulation() + scan_dependencies()
    if problems:
        for problem in problems:
            print(problem)
        print(f"core purity: {len(problems)} violation(s)")
        return 1
    print("core purity: ok")
    return 0


if __name__ == "__main__":
    sys.exit(main())
