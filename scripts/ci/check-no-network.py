#!/usr/bin/env python3
"""No-network gate (ADR-0008 §6).

1.0 ships inert AI seams: no model, no network call, no UI. That is a claim about
the dependency graph, so it is checked on the graph — direct declarations in every
workspace manifest and the resolved closure — rather than by grepping for
`reqwest::` in source, which misses the crate that pulls it in.

`openkey-ai` is allowed to exist (AI-002 creates it at M1, empty and
feature-gated); what it may not do is become reachable from `openkey-core`.

Exit status: 0 clean, 1 violation, 2 the gate could not run.
"""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

NETWORK_CRATES = {
    "reqwest",
    "hyper",
    "hyper-util",
    "ureq",
    "curl",
    "isahc",
    "surf",
    "attohttpc",
    "minreq",
    "awc",
    "tonic",
    "tokio-tungstenite",
    "tungstenite",
}

# `tokio` is banned outright, not just its net feature: 1.0 has no async runtime at
# all, and the idle-priority IME loop that needs one would be a design decision
# (an ADR), not a dependency bump.
RUNTIME_CRATES = {"tokio", "async-std", "smol", "rayon"}

BANNED = NETWORK_CRATES | RUNTIME_CRATES


def declared_dependencies() -> list[str]:
    problems: list[str] = []
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

    for package in metadata["packages"]:
        for dependency in package["dependencies"]:
            if dependency["name"] in BANNED:
                kind = {"normal": "dependency", "dev": "dev-dependency", "build": "build-dependency"}[
                    dependency["kind"] or "normal"
                ]
                problems.append(
                    f"::error::`{package['name']}` declares {kind} `{dependency['name']}`; "
                    f"{dependency['name']} is banned for 1.0 (no network, no async runtime)"
                )
    return problems


def resolved_graph() -> list[str]:
    tree = subprocess.run(
        ["cargo", "tree", "--workspace", "--edges", "normal", "--prefix", "none", "--locked"],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    if tree.returncode != 0:
        return [f"::error::cargo tree failed: {tree.stderr.strip()}"]

    problems: list[str] = []
    seen: set[str] = set()
    for line in tree.stdout.splitlines():
        name = line.split(" v", 1)[0].strip()
        if name in BANNED and name not in seen:
            seen.add(name)
            problems.append(f"::error::`{name}` resolved into the workspace graph: {line.strip()}")
    return problems


def main() -> int:
    problems = declared_dependencies() + resolved_graph()
    if problems:
        for problem in problems:
            print(problem)
        print(f"no-network: {len(problems)} violation(s)")
        return 1
    print("no-network: ok")
    return 0


if __name__ == "__main__":
    sys.exit(main())
