#!/usr/bin/env python3
"""Prove what the repo's .gitleaks.toml allowlist actually does, and that it still detects.

`secrets` is a blocking gate, so its configuration is load-bearing in both directions: it
must not fail on the pinned oracle record (or every PR is red), and it must not exempt a
credential that happens to sit in a file whose content is mostly digests. A config change
here can pass review by looking narrower while being wider, so the measurements live in a
test rather than in a comment.

The fixture is generated, not committed: it plants credentials, and a repository that
scans itself for credentials should not carry four of them in history for a test. Tokens
and digests are derived from fixed labels, so the expected findings are stable across
runs without a single secret-shaped string being stored in git.

Two traps this suite exists to catch, both of which produced a confident and meaningless
"0 findings" during development:

1. Detectors are entropy-filtered. `ghp_AAAA…` and `a1b2c3d4…` repeated four times read as
   0 findings in *every* configuration. So the `none` column below is asserted first and
   with exact rows: if the detector stops firing, the run fails instead of passing empty.
2. A path-only allowlist exempts a whole file, so a test that only checks "the pin file is
   quiet" cannot tell a narrow rule from a blanket one. Every column is compared as a set
   of (file, line, rule), not as a count.

A third fact, measured late and the reason this suite runs every column twice:

3. **The result depends on the subcommand.** `gitleaks dir` ignores a top-level
   `condition = "AND"` and combines the criteria as OR; `gitleaks git` honours it. Same
   config, same binary, same fixture: `table-and` reports 3 findings under `dir` and 5
   under `git` — the two pin-file keys the AND is supposed to narrow down to. So a
   top-level allowlist whose meaning follows the invocation is a trap, and the shipped
   config attaches its allowlist to the rules, where it measures 5 in both modes. CI runs
   `git`; `dir` is what a developer or another tool reaches for. Assert both.

Running only `dir` would have certified a mode neither job uses, which is what this
increment exists to stop doing elsewhere.

Usage:
    scripts/ci/fetch-gitleaks.sh /tmp/gl && \\
        python3 scripts/ci/test_gitleaks_config.py --gitleaks /tmp/gl/gitleaks
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
PINNED_VERSION = "8.30.1"

# The columns. `repo` is the file under test; the other four are controls.
NONE = 'title = "fixture"\n[extend]\nuseDefault = true\n'

# The shape that was on main before this change: one top-level table, paths only.
PATHS_ONLY = """title = "fixture"
[extend]
useDefault = true
[allowlist]
paths = ['''docs/m0/ORACLE-PIN\\.json''', '''parity/oracle/pin\\.json''', '''(?i)(^|/)SHA256SUMS$''', '''(?i)\\.sha256$''']
"""

# The rejected fix: keep the table, add line targeting. Narrower than nothing, wider than
# the file it names — it exempts checksum-shaped lines in every file.
LINE_OR = """title = "fixture"
[extend]
useDefault = true
[allowlist]
regexTarget = "line"
paths = ['''docs/m0/ORACLE-PIN\\.json''', '''parity/oracle/pin\\.json''']
regexes = ['''"[^"]+\\.(cpp|h|hpp|mm|m|rs)":\\s*"[0-9a-f]{64}"''']
"""

# `condition = "AND"` on the top-level table. Included because it looks like it should work
# and its meaning is not stable: `gitleaks dir` ignores `condition` there and combines the
# criteria as OR (the pin-file keys stay hidden), while `gitleaks git` honours it and reports
# them. The shipped config attaches the allowlist to the rules instead, where the answer is
# the same in both modes. Both halves of that split are asserted below.
TABLE_AND = """title = "fixture"
[extend]
useDefault = true
[allowlist]
condition = "AND"
regexTarget = "line"
paths = ['''docs/m0/ORACLE-PIN\\.json''', '''parity/oracle/pin\\.json''']
regexes = ['''"[^"]+\\.(cpp|h|hpp|mm|m|rs)":\\s*"[0-9a-f]{64}"''']
"""

MODES = ("dir", "git")


def token(label: str) -> str:
    """A `github-pat`-shaped token with real entropy, derived from `label`."""
    digest = hashlib.sha256(("gokey-fixture/" + label).encode()).digest()
    return "ghp_" + base64.b32encode(digest).decode().lower().rstrip("=")[:36]


def sha(label: str) -> str:
    return hashlib.sha256(("gokey-digest/" + label).encode()).hexdigest()


def build_fixture(root: Path) -> None:
    """Five files. Line numbers matter: they are what the columns are compared on."""
    d1, d2 = sha("engine.cpp"), sha("engine.h")
    t_file, t_dline, t_unl, t_ctl = (token(x) for x in ("file", "dline", "unl", "ctl"))

    pin = (
        "{\n"
        '  "engine_files_sha256": {\n'
        f'    "Sources/OpenKey/engine/SmartSwitchKey.cpp": "{d1}",\n'
        f'    "note": "{t_file}"\n'
        "  },\n"
        f'  "tail": {{"Engine.cpp": "{d2}", "k": "{t_dline}"}}\n'
        "}\n"
    )
    files = {
        # line 3: a checksum line (the `Key` in the filename is what makes generic-api-key
        #         look at it at all); line 4: a key on a line of its own; line 6: a key
        #         appended to a checksum line, which is the residual this config accepts.
        "docs/m0/ORACLE-PIN.json": pin,
        "parity/oracle/pin.json": pin,
        # Unlisted file: a key on a checksum-shaped line. Reported by every config that
        # scopes by path, hidden by the one that scopes by line alone.
        "src/app.rs": f'const T: (&str, &str) = ("SmartSwitchKey.cpp": "{d2}", "{t_unl}");\n',
        # A key with nothing around it: reported everywhere, the canary for "did the scan
        # run at all".
        "src/plain.env": f'token = "{t_ctl}"\n',
        # `sha256sum` output. Asserted to produce no finding even with no allowlist: the
        # path entries for this shape are defensive, not demonstrated.
        "SHA256SUMS": f"{d1}  Engine.cpp\n{d2}  SmartSwitchKey.cpp\n",
    }
    for rel, body in files.items():
        path = root / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(body, encoding="utf-8")


def commit_fixture(root: Path) -> None:
    """Make the fixture a one-commit repository, for the `git`-mode columns.

    `git add -A` is not enough and fails silently: a machine with `docs/` in
    `core.excludesFile` drops `docs/m0/ORACLE-PIN.json` from the commit, and the `git` columns
    then report six rows instead of nine with no error anywhere. Hence `-f` and an explicit
    excludes file, and the assertion below that the committed tree is the whole fixture.
    """
    def git(*args: str) -> None:
        subprocess.run(["git", "-c", "core.excludesFile=/dev/null", *args],
                       cwd=root, check=True, capture_output=True, text=True)

    git("init", "-q", "-b", "main", ".")
    git("add", "-A", "-f", ".")
    git("-c", "user.name=fixture", "-c", "user.email=fixture@example.invalid",
        "commit", "-q", "-m", "fixture")
    listed = subprocess.run(["git", "ls-tree", "-r", "--name-only", "HEAD"],
                            cwd=root, check=True, capture_output=True, text=True).stdout.split()
    expected = sorted(p.relative_to(root).as_posix() for p in root.rglob("*")
                      if p.is_file() and ".git" not in p.relative_to(root).parts)
    if sorted(listed) != expected:
        raise SystemExit(f"fixture commit is not the whole fixture: {sorted(listed)} != {expected}")


def scan(binary: str, root: Path, work: Path, mode: str, name: str,
         config: str) -> set[tuple[str, int, str]]:
    """Run one configuration over the fixture in one mode; return (file, line, rule) findings.

    Both modes take the same config so a column means the same thing in each; `--log-opts=HEAD`
    keeps `git` mode to the single fixture commit.
    """
    cfg = work / f"{name}.toml"
    cfg.write_text(config, encoding="utf-8")
    report = work / f"{name}-{mode}.json"
    cmd = [binary, mode, "."]
    if mode == "git":
        cmd.append("--log-opts=HEAD")
    # The config and the report live outside the fixture, so neither can be scanned and
    # neither has to be committed for `git` mode to see it.
    cmd += ["--config", str(cfg), "--redact", "--no-banner",
            "--exit-code", "0", "-f", "json", "-r", str(report)]
    result = subprocess.run(cmd, cwd=root, capture_output=True, text=True)
    if not report.exists():
        raise SystemExit(f"gitleaks produced no report for {name}/{mode}: {result.stderr.strip()}")
    findings = json.loads(report.read_text(encoding="utf-8")) or []
    return {(f["File"], f["StartLine"], f["RuleID"]) for f in findings}


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--gitleaks", default="gitleaks",
                    help="path to the gitleaks binary (see scripts/ci/fetch-gitleaks.sh)")
    args = ap.parse_args()

    version = subprocess.run([args.gitleaks, "version"], capture_output=True, text=True)
    got = version.stdout.strip()
    if got != PINNED_VERSION:
        print(f"::error::gitleaks is {got!r}, this suite measures {PINNED_VERSION}", file=sys.stderr)
        print("run scripts/ci/fetch-gitleaks.sh and pass --gitleaks; the allowlist semantics "
              "below are version-specific", file=sys.stderr)
        return 2

    with tempfile.TemporaryDirectory() as tmp:
        root, work = Path(tmp) / "fixture", Path(tmp) / "work"
        root.mkdir()
        work.mkdir()
        build_fixture(root)
        commit_fixture(root)

        configs = {
            "none": NONE,
            "paths-only": PATHS_ONLY,
            "line-or": LINE_OR,
            "table-and": TABLE_AND,
            "repo": (REPO / ".gitleaks.toml").read_text(encoding="utf-8"),
        }
        columns = {(mode, name): scan(args.gitleaks, root, work, mode, name, cfg)
                   for mode in MODES for name, cfg in configs.items()}

    pin_a, pin_b = "docs/m0/ORACLE-PIN.json", "parity/oracle/pin.json"
    key_on_own_line = {(pin_a, 4, "github-pat"), (pin_b, 4, "github-pat")}
    checksum_lines = {
        (pin_a, 3, "generic-api-key"), (pin_b, 3, "generic-api-key"),
        (pin_a, 6, "github-pat"), (pin_b, 6, "github-pat"),
    }
    unlisted = {("src/app.rs", 1, "generic-api-key"), ("src/app.rs", 1, "github-pat")}
    canary = {("src/plain.env", 1, "github-pat")}
    expected_none = key_on_own_line | checksum_lines | unlisted | canary

    problems: list[str] = []

    def col(mode: str, name: str) -> set[tuple[str, int, str]]:
        return columns[(mode, name)]

    for mode in MODES:
        # 1. The control, in both modes. Both traps in the module docstring are caught here
        #    or nowhere.
        problems += check(f"control ({mode}): no allowlist reports every planted credential",
                          col(mode, "none"), expected_none)
        if any(path == "SHA256SUMS" for path, _, _ in col(mode, "none")):
            problems.append(f"control ({mode}): SHA256SUMS produced a finding; the defensive "
                            "note in .gitleaks.toml is now wrong and the entry can be tested")

        # 2. The shipped config: narrow, not blanket — and identical in both modes, which is
        #    the property the rule-level form buys over the top-level one.
        problems += check(f"repo ({mode}) reports a key on a non-checksum line in a pin file",
                          col(mode, "repo"), expected_none - checksum_lines)
        for row in checksum_lines & col(mode, "repo"):
            problems.append(f"repo ({mode}) did not exempt a checksum line: {row}")

        # 3. The rejected shapes, so the rejections stay falsifiable.
        problems += check(f"paths-only ({mode}) hides the pin-file keys",
                          col(mode, "paths-only"), unlisted | canary)
        problems += check(f"line-targeting alone ({mode}) blinds an unlisted file",
                          col(mode, "line-or"), canary)

    # 4. The mode split that made the first version of this suite certify the wrong mode:
    #    `dir` ignores a top-level `condition`, `git` honours it. Asserted in both directions
    #    so neither can change without the suite saying so.
    problems += check("a top-level condition=AND is ignored by `dir`",
                      col("dir", "table-and"), unlisted | canary)
    problems += check("a top-level condition=AND is honoured by `git`",
                      col("git", "table-and"), expected_none - checksum_lines)

    for mode in MODES:
        print(f"--- {mode} mode")
        print(f"{'column':<12} {'findings':>8}   rows")
        for name in configs:
            rows = col(mode, name)
            print(f"{name:<12} {len(rows):>8}   {sorted(rows)}")

    if problems:
        for p in problems:
            print(f"::error::{p}", file=sys.stderr)
        return 1
    print("\nall assertions hold")
    return 0


def check(label: str, got: set[tuple[str, int, str]], want: set[tuple[str, int, str]]) -> list[str]:
    if got == want:
        return []
    missing, extra = sorted(want - got), sorted(got - want)
    return [f"{label}: missing={missing} unexpected={extra}"]


if __name__ == "__main__":
    sys.exit(main())
