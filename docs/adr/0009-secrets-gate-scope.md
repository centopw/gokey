# ADR-0009 — What the `secrets` gate scans, and what it exempts

- Status: proposed
- Date: 2026-10-29
- Owner: R6 (build and release engineer)
- Reviewers: R2 (technical lead)
- Supersedes: the `secrets` row of ADR-0008 §1 in respect of *scope only*. ADR-0008's decision
  that the gate exists, is blocking, and uses a pinned checksum-verified binary is unchanged.
- Backlog: K-001 (CI). Follows PR #16; the configuration defect it fixes predates it.

## Context

The `secrets` job runs `gitleaks git .`. On gitleaks 8.30.1, that scans **every fetched ref**, not
the ref under test. With `fetch-depth: 0` the runner has every branch, so a secret-shaped string on
an unrelated, unmerged branch fails this job on every open PR until that branch is rebased or
deleted. This happened: PR #15 was red with `leaks found: 1` on a commit that was on no ancestor of
the PR's history.

The job went green afterwards, and the stated reason at the time — that the offending commit "became
an ancestor of `main`" — was wrong. A squash merge does not make the squashed commits ancestors:
`git merge-base --is-ancestor f0eb4ce origin/main` returns non-zero. It went green because the branch
ref that carried the commit was deleted, so the commit stopped being reachable. A gate whose result
depends on which branches happen to exist on someone's remote is not measuring the change under
review. That is the first half of this ADR.

The second half is the allowlist, which has the same disease in miniature. `.gitleaks.toml` on `main`
is a single top-level `[allowlist]` with four paths: the two pinned-oracle files and the checksum
manifests. A path allowlist exempts the **whole file**. The pinned oracle record is a JSON object
whose values are SHA-256 digests of C++ sources; `SmartSwitchKey.cpp` in a key position trips
`generic-api-key`'s keyword heuristic on every line that carries a digest, so a file-level exemption
was the quick fix. The cost is that a credential committed to `docs/m0/ORACLE-PIN.json` — a file
people do edit — is invisible to the gate. The gate was quiet on that file by construction, not by
evidence.

## Decision

### 1. The blocking scan covers the ref under test

`secrets` scans `--log-opts=HEAD`. On a `push` that is the branch tip's history; on a
`pull_request` the checked-out merge commit's history, which contains the PR's commits and the base
branch's. History is still scanned in full for those commits — a secret added and then removed in a
later commit of the same branch is still found, which is the property the original `fetch-depth: 0`
comment was defending. What is excluded is *other branches'* commits.

### 2. Full-history coverage moves to a scheduled sweep

`nightly.yml` gains a `secrets-history` job that runs with the same pinned binary, the same config,
and no `--log-opts`, so it scans every ref. A secret that only ever existed on a deleted branch, or
on a branch nobody opened a PR for, is found there rather than being found never.

It is a job in `nightly.yml` rather than a workflow of its own because that is where this repository
already keeps the checks that are not tied to a change, and it inherits that file's triage: one
issue, updated on repeat failure, read every working day (ADR-0008 §7). A separate `schedule:` would
have needed a second copy of the issue-reporting step and a second cron to remember. The scan itself
is 150 ms, so the cadence is daily rather than the weekly interval a separate file would have
suggested — the only per-run cost is the binary download.

The split is deliberate: a per-change gate answers "did *this* change introduce a credential", and a
sweep answers "is there a credential anywhere in this repository's history". One job doing both
answers the second question with a result about other people's branches, which is how the gate
started failing PRs for commits they did not contain.

Scoped figures on the author's clone at this ADR's branch point, same binary and config:

| invocation | commits scanned | bytes | findings |
|---|---|---|---|
| `gitleaks git .` (all refs) | 51 | 576 986 | 0 |
| `gitleaks git . --log-opts=HEAD` | 28 | 457 859 | 0 |

The 23 commits that differ are other branches. Both are green here because the branch in question is
rebase-clean; the difference in scope is the point, not the difference in result.

### 3. The allowlist moves from the file to the line, on the rules

The pinned-digest exemption is attached to the two rules that actually fire, not to the file:

```toml
[[rules]]
id = "generic-api-key"
[[rules.allowlists]]
condition = "AND"
regexTarget = "line"
paths = ['''docs/m0/ORACLE-PIN\.json''', '''parity/oracle/pin\.json''']
regexes = ['''"[^"]+\.(cpp|h|hpp|mm|m|rs)":\s*"[0-9a-f]{64}"''']
```

(`github-pat` carries an identical block; the checksum-manifest paths stay on a top-level
`[allowlist]` because there is no line shape to AND them against — see §5.)

A finding survives only if **all** of: the file is one of the two pin files, the line matches the
`"File.cpp": "<64 hex>"` shape, and the rule that found it is one of the two. Everything else in
those files is now scanned like any other file.

It is on the rules rather than on the top-level table for the reason §4 measures: the top-level form
answers differently under `gitleaks dir` and `gitleaks git`, and the rule-level form does not.

### 4. The measurements, and the reason they are a test rather than a comment

`scripts/ci/test_gitleaks_config.py` generates a five-file fixture in a temp directory, commits it
as a one-commit repository, and runs five configurations over it **in both subcommands** —
`gitleaks dir` and `gitleaks git` — comparing **sets of `(file, line, rule)`**, not counts, because
two rules fire on the same line and a count hides which one. The fixture is generated rather than
committed: the repository scans itself for credentials and should not carry planted ones in
history, so the tokens are derived from fixed labels with `hashlib`/`base64` and are stable across
runs.

The two-mode shape is not belt-and-braces. It is the correction of an error in the first version of
this ADR, which reported one mode's answer as a property of the pinned version (see the table). CI
runs `git`; `dir` is what a developer or another tool reaches for; the suite measures the mode CI
actually uses, and keeps `dir` because the difference is the finding.

Columns on the pinned 8.30.1 (`.gitleaks.toml` is the shipped one; ×2 = two rules on one line):

| fixture row | none | paths-only (pre-change) | line-OR (rejected) | table-AND (rejected) | rule-AND (shipped) |
|---|---|---|---|---|---|
| `docs/m0/ORACLE-PIN.json:3` — checksum line | reported | hidden | hidden | hidden | hidden |
| `docs/m0/ORACLE-PIN.json:4` — key on its own line | reported | hidden | hidden | **`dir`: hidden / `git`: reported** | **reported** |
| `docs/m0/ORACLE-PIN.json:6` — key on a checksum line | reported | hidden | hidden | hidden | hidden |
| `parity/oracle/pin.json` — same three rows | reported | hidden | hidden | as above | as above |
| `src/app.rs:1` — key on a checksum-shaped line, file not listed | reported ×2 | reported ×2 | **hidden** | reported ×2 | reported ×2 |
| `src/plain.env:1` — key, nothing else (canary) | reported | reported | reported | reported | reported |
| totals, `gitleaks dir` | 9 | 3 | 1 | 3 | 5 |
| totals, `gitleaks git` | 9 | 3 | 1 | **5** | 5 |

Three of those columns are the argument:

- **`line-OR` is worse than the file exemption it replaced.** The top-level allowlist is global, so
  a line regex applies everywhere: a credential on any line that happens to carry a checksum-shaped
  pair is hidden *in a file the allowlist never names*. It reads narrower than `paths-only` and
  reports fewer findings.
- **`table-AND` means different things depending on the subcommand.** `gitleaks dir` ignores a
  top-level `condition` and combines the criteria as OR, so the whole file is exempt; `gitleaks git`
  honours it, so a key on a non-checksum line is reported. Both spellings of the table
  (`[allowlist]` and `[[allowlists]]`) behave the same way within a mode — the variable is the
  subcommand, not the TOML. Neither answer is stable, so the shipped config does not use the form.
  That rejection is stronger than the first draft's ("it is ignored"), because a config that is
  correct only in the invocation CI happens to use is the same silent pass this ADR exists to
  remove, one level down.
- **`rule-AND` is the same answer in both modes** (5 rows each), which is the property that makes it
  safe to rely on. It is also the narrowest of the working forms: the pin files lose nothing but
  their checksum lines.

Both mode columns are asserted, in both directions: `table-and` must stay at 3 under `dir` and 5
under `git`, so a version change that makes the two agree — either way — stops the privacy of the
difference from being assumed, and the ADR's reason for the rule-level form has to be re-measured
rather than inherited.

Two guards in the suite are worth naming because both failures it prevents actually occurred during
development and both produced a confident, meaningless "0 findings":

Two guards in the suite are worth naming because both failures it prevents actually occurred during
development and both produced a confident, meaningless "0 findings":

1. **The control column is asserted first and exactly.** gitleaks' generic detector is
   entropy-filtered: `ghp_AAAA…` and `a1b2c3d4…`-repeated-four-times read as zero findings in *every*
   configuration, including no allowlist at all. A suite that only asserts the hidden cells passes
   whether or not the detector works. The `none` column must report the full set or the run fails.
2. **Partial rule override is relied on.** `[[rules]] id = "github-pat"` with only an `allowlists`
   block merges into the default rule rather than replacing it; restating the regex and keywords
   would fork them from the default ruleset, which is the thing `useDefault` exists to avoid. If a
   future version changed that merge, `github-pat` would stop firing everywhere — and the `repo`
   column, which asserts that the shipped config still reports all four non-checksum rows, fails.
3. **The `git` fixture is committed, and `git add` is the trap.** A machine with `docs/` in
   `core.excludesFile` silently drops `docs/m0/ORACLE-PIN.json` from a plain `git add -A`, and the
   `git` columns then report six rows instead of nine with no error anywhere. The fixture is added
   with `-f` and `core.excludesFile=/dev/null`, and the commit is checked against the fixture's
   file list before anything is scanned, because the failure mode of the alternative is nine
   assertions failing for the wrong reason — or, with a lazier assertion, passing.

### 5. The checksum-manifest entries are defensive, not demonstrated

`SHA256SUMS` and `*.sha256` keep a path allowlist, because `sha256sum` output is `hash  filename`
with no quoted digest to anchor a line regex on, so there is nothing to AND against.

In the fixture, a bare `<hash>  Engine.cpp` line produced **no finding at all with no allowlist**
on 8.30.1. So these two entries currently exempt nothing that would otherwise be reported: the
assertion "the manifest is allowlisted" would pass vacuously. The suite therefore asserts the
opposite and useful thing — that `SHA256SUMS` produces no finding — and treats a finding there as a
failure, because that event means the entry is now load-bearing and should be tested directly
instead of trusted. The entries are inherited from `origin/main` and this ADR introduces no change
to them; they stay because a digest manifest is not a credential store and a future manifest shape
may well trip the generic heuristic.

### 6. The version pin is part of the design

`condition` and `regexTarget` are the load-bearing fields, and their meaning is both
version-specific and subcommand-specific — the second half is what the first version of this ADR got
wrong. Every claim in §3-§4 is a measurement on one pinned binary, in both modes. The
binary is pinned to 8.30.1 by SHA-256 in `scripts/ci/fetch-gitleaks.sh` — one place, used by the
`secrets` job, the nightly sweep and a developer's laptop, so the three cannot drift.

If that pin moves: fetch the new binary, run `scripts/ci/test_gitleaks_config.py`, and read both
mode blocks before touching the config. A version that stops distinguishing the modes at top level,
that changes how `dir` treats `condition`, or that changes how a partial rule override merges, fails
the suite rather than quietly widening or narrowing the gate. Bumping the pin without running the suite is the one move that can turn a
version change into a silent policy change.

### 7. Accepted residual

A credential appended to a checksum line *inside* a pin file matches both halves of the AND and is
hidden. This is the narrowest residual available with this mechanism: one line shape inside two
files, versus every line of two files (the pre-change config), versus every checksum-shaped line in
the repository (the line-OR proposal).

It costs a one-sided edit. `scripts/ci/check-oracle-pin.py` refuses to run unless the two pin files
are byte-identical (exit 1), and `scripts/parity.sh` runs it in the blocking `parity` job, so the
credential has to be added to both copies by hand. Nothing automatic detects it after that: the
record's digests describe the upstream engine tree, not the record file itself, so a key appended to
a digest line leaves every digest valid. Review of a file whose entire content is a digest list is
the control, and it is a weaker one than a detector. Stated rather than papered over.

## Consequences

**Positive**

- A PR is failed by its own history, not by other branches' commits, so a red `secrets` means
  something about the change in front of you.
- A credential in a pin file — the file most likely to be hand-edited in this repository — is
  reported unless it sits on a checksum line.
- The allowlist's behaviour is a test, so "it is as narrow as the ADR says" is verified on every PR
  rather than read from a comment.
- Full-history coverage is retained daily, including commits on branches that were never opened as
  PRs, and its failure opens an issue like the rest of the nightly.

**Negative / accepted**

- History coverage of other branches is up to a day old, and a leak found there is found by a job
  nobody is watching in real time. The issue is the channel, and it is the only job that can find
  such a leak without blaming a PR that never contained it.
- `nightly.yml` is now 20 lines longer and duplicates the issue-reporting step. That is the price of
  keeping the two failures distinguishable in the issue tracker; a shared composite action would be
  the fix if a third job needs it.
- The scan in `secrets` runs after a fixture suite, so a config regression is reported as a config
  failure rather than as "leaks found". Anyone reading a red `secrets` should read which step failed
  before assuming history is dirty.
- One more process to keep working: `fetch-gitleaks.sh` replaces an inline `curl`, and the config
  test adds a few seconds. The suite is skipped-and-green on nothing — it exits non-zero if the
  binary is not 8.30.1, and there is no "could not measure, carry on" path.

## Alternatives considered

1. **Leave `gitleaks git .` and accept cross-branch failures.** Rejected: it makes a blocking gate
   depend on remote ref state, and the remedy (delete or rebase someone else's branch) is not
   something a PR author owns.
2. **`--log-opts=origin/main..HEAD`.** Narrower still, but a `push` to `main` then has an empty
   range and the gate becomes green-by-absence on exactly the branch that ships. `HEAD` keeps the
   full history of the ref under test.
3. **Drop the all-refs sweep and rely on `HEAD` alone.** Rejected: a secret on an unmerged branch is
   still a leak once that branch is pushed to a public remote, and nothing would ever look.
4. **Keep the file-level allowlist and accept that pin files are unscanned.** Rejected: those two
   files are the ones a human edits, and the exemption was introduced to silence a false positive
   on checksum lines, not to exempt the file's content.
5. **Line-targeted allowlist on the top-level table.** Rejected on measurement: it is global, so it
   hides credentials in any file whose line happens to match (see §4).
6. **`condition = "AND"` on the top-level table.** Rejected on measurement: its meaning follows the
   subcommand — `dir` combines the criteria as OR (behaving like option 4 while claiming otherwise),
   `git` applies the AND. It would work in CI today and stop working for anyone running `dir`, which
   is the failure this ADR is about. The rule-level form measures the same in both modes.
7. **Drop the `SHA256SUMS` entries as unproven.** Kept as defensive (§5) and asserted to be
   currently inert, so if they become load-bearing the suite says so.
