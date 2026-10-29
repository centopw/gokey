# ADR-0008 — CI gate ladder, artefact policy and the G5-at-M0 ruling

- Status: proposed
- Date: 2026-10-29
- Owner: R6 (build and release engineer)
- Reviewers: R2 (technical lead)
- Supersedes: nothing. Extends the CI half of K-001.
- Backlog: K-001 (CI), K-007 (packaging spike)

## Context

`ci.yml` at `c64f8d6` had three jobs and a lockfile that did not exist. It is now green (PR #3),
but green means "fmt, clippy, test, doc pass and cargo-deny accepts the licence set". It does not
mean the repository can produce an artefact, that a secret cannot be committed, that the core crate
stays platform-neutral, or that the parity job is measuring anything. The scaffolding around the
port is where this project will fail silently, so the gates are worth more than usual.

Three facts shape the design.

1. **The parity oracle is headless.** The upstream C++ engine builds and runs on Linux
   (`-DLINUX`), so the parity job does not need a macOS runner. Evidence: a harness compiled
   against the pinned engine produces `aa → U+00E2`, `dd → U+0111`, `vieejt → U+00EA`.
2. **The repository is public.** Every GitHub Actions artefact is downloadable by anyone. The first
   uploaded dmg is therefore a GPL-3.0-covered distribution, not an internal build, whatever the
   intent behind it. G5 cannot exist without K-006's licence obligations being met in the artefact
   itself.
3. **1.0 is macOS-only** (DL-003 / DL-014, decided 2026-10-29). Windows and Linux ship as M4 and
   M5, but the workspace must keep building and testing on all three OSes so the platform
   abstraction cannot rot while nobody is looking at it.

## Decision

### 1. Gate ladder

Six blocking gates plus one nightly, ordered cheapest-first so a broken PR fails in ~30 s rather
than after a macOS build.

| Gate | Job | Runner | Trigger | Blocking |
|---|---|---|---|---|
| G1 hygiene | `build-test` (fmt, clippy `-D warnings`) | ubuntu, macOS, windows | PR + main | yes |
| G2 correctness | `build-test` (test, doctest, doc) | ubuntu, macOS, windows | PR + main | yes |
| G3 parity | `parity` | ubuntu | PR + main | yes once corpus fixtures exist; every PR once `scripts/parity.sh` exists |
| G4 supply chain | `licenses`, `secrets`, `sbom` | ubuntu | PR + main | yes |
| G5 artefact | `package-macos` | macOS | main, and PRs touching packaging | yes |
| G6 release | not in this ADR | — | tag | see §4 |
| G7 manual | checklist, human | 3 machines | every release | yes, manual |
| G8 nightly | `nightly` | ubuntu | cron + dispatch | **no** — triaged daily, never merge-blocking |

G1/G2 must run on all three OSes even though 1.0 ships on one. `openkey-core` is supposed to be
free of platform keycodes and I/O; a crate that only compiles on macOS has already broken that
rule, and the failure surfaces with a misleading error at M4.

`parity` is the one gate whose blocking state changes on its own, and it changes on **evidence, not
a date**. While there is neither a corpus nor `scripts/parity.sh`, there is nothing to compare and
the job reports a notice. The moment corpus fixtures are committed without a harness to replay
them, the job fails: fixtures plus no runner means parity is *unmeasured*, which is exactly the
state K-005 exists to end. A calendar deadline was written first and removed — on its expiry date it
would fail every PR in the repository at once, including ones that touched nothing related, and
nobody re-reads a stale date in a workflow file. That is the same day-based planning DL-019 removed,
in the place where it does the most damage.

### 2. G5 starts at M0, not M3

`K-007`'s acceptance is "artifacts produced; time-to-artifact documented". That is an artefact job,
so the gate and the task are the same thing. Deferring G5 to M2 would have made the first dmg
appear at the same moment it had to be notarised, meaning the bundle layout, `Info.plist`, the
InputMethodKit install path and any `install_name_tool` work would get their first exercise inside
a release.

Therefore: **every merge to `main` emits an ad-hoc signed dmg as a CI artefact.** No tag, no
GitHub Release, no notarisation, no external link, nobody is expected to install it.

Ad-hoc only (`codesign -s -`) until M2, for one specific reason: it needs **no secret**, so the job
works on fork PRs and no signing material exists in the organisation before it is needed. At M2 the
committed self-signed identity arrives (stable CDHash across rebuilds, so dogfooders keep their
Accessibility grant); at M3 Developer ID + notarisation + stapling replace it.

Three properties keep this from becoming a green-by-absence job:

1. the dmg is **mounted and its contents asserted** (app bundle, `LICENSE`, `NOTICE`,
   `SOURCE-OFFER.txt`) — not just "a file was created";
2. `codesign --verify --strict` and `hdiutil verify` run on the produced artefacts;
3. time-to-artifact is printed and written into the artefact, which is K-007's second acceptance
   criterion.

### 3. A public-repo artefact is a distribution

The dmg job therefore carries, inside the disk image:

- `LICENSE` (GPL-3.0 full text),
- `NOTICE.md` (attribution, upstream, name-and-marks position),
- `SOURCE-OFFER.txt` — the exact commit SHA, the repository URL, and how to obtain the identical
  corresponding source;

and the workflow artefact additionally contains the source tarball for that commit plus a
`SHA256SUMS` file. This is the minimum that makes G5-at-M0 lawful. It does **not** replace K-006's
human half (upstream contact, sponsor signature on DL-001), which is a precondition for the first
artefact, not for M0 exit.

`NOTICE.md` currently states that the project "publishes no binaries". That sentence becomes false
with this ADR and is corrected in the same change.

### 4. `release.yml` is deliberately not written yet

Earlier drafts of the plan had a signed-release workflow authored in this PR and disabled until M3.
That is the same mistake as a job that passes because it found nothing: a workflow that cannot be
exercised is untested automation, and it would sit for four milestones accumulating assumptions
about notarisation, stapling and asset naming that nobody can validate.

G6 at M3 is small and is written when it can be run against a real tag. What belongs here, now, is
the prerequisite list (see `M0-CI-gates-and-three-OS-packaging.md` §4): Developer ID Application
certificate, Apple org enrolment started in W2, protected `v*` tags, a `release` environment with
required reviewers, and the notarisation/stapling commands.

### 5. Runner and toolchain pinning

`*-latest` labels move, and they move the toolchain with them: `ubuntu-latest` became Ubuntu 26 on
2026-10-19, and `actions/checkout` was force-migrated to Node 24 the same week. A moving runner can
turn `main` red without a commit.

- Runner labels are pinned: `ubuntu-24.04`, `macos-15`, `windows-2025`.
- Every action is pinned by commit SHA with the human-readable version in a trailing comment.
- `rust-toolchain.toml` pins `channel = "1.99.0"` instead of `"stable"`. `rust-version = "1.85"`
  in `Cargo.toml` stays as the MSRV floor; the two are different claims: MSRV is what compiles,
  the toolchain file is what CI uses.
- `cargo` runs with `--locked` everywhere, so a dependency bump is a reviewable diff.

Upgrade procedure: change the pin, run the PR, merge. There is no floating reference left in the
pipeline.

### 6. Core purity and no-network are gates, not conventions

`openkey-core` carries two promises that are cheap to check and expensive to discover late:

- **Purity**: no platform keycodes, no I/O, no globals, no `unsafe`. Enforced by scanning
  `crates/openkey-core/src` for `std::fs`, `std::net`, `std::env`, `std::process`, `unsafe`, and by
  asserting `cargo tree -p openkey-core` contains no platform crate. **`#[cfg(test)]` items are
  excluded** — a test may read a fixture, and banning `std::fs` there only pushes fixtures into
  inline string literals, which is worse.

  The scan expands `use` trees to one fully-qualified path per line before matching, because
  matching physical lines misses the shape rustfmt actually emits: `use std::{fs, net};` contains
  neither `std::fs` nor `std::net`. Grouped, nested, aliased (`use std::fs as f`) and multi-line
  trees are all expanded. The shapes are pinned by `scripts/ci/test_check_core_purity.py`, which
  runs in the same job before the scan: a scanner with a known bypass and no test regresses to the
  bypass, and its failure mode is a false pass, which is worse than a false alarm.

  Scope is `crates/openkey-core/src`. A `build.rs` at the crate root would be out of scope, and
  deliberately so: a build script runs at build time on the host and legitimately reads files, so
  folding it into a source-purity rule would produce noise rather than evidence. `openkey-core`
  has no build script today; if one is added, this paragraph is the thing to revisit.
- **No network**: 1.0 ships inert AI seams. The workspace dependency graph may not contain
  `reqwest`, `hyper`, `ureq` or `tokio`, and `openkey-ai` — a workspace member from AI-002 (M1) —
  must not be reachable from `openkey-core`. The direction of that assertion is the whole claim: a
  later "core asks AI for a suggestion" is exactly the shortcut this blocks.

`std::process` is on the banned list on purpose: a core that shells out to `git` or `defaults` is
I/O with extra steps.

### 7. Nightly is non-blocking but triaged

WORKFLOW.md §3 puts the full corpus, differential fuzzing, benchmark regression and dependency
audit on a nightly schedule. A nightly that blocks merges is a nightly people disable. It is
non-blocking, it opens or updates a single issue on failure, and it is read every working day —
that is the only part made of process rather than code, and it is stated here so it is visible.

## Consequences

**Positive**

- The first artefact exists in W2, so M1 has no packaging hole and M2's dogfood channel starts from
  a bundle that already builds.
- A behaviour-free PR fails fast and cheaply; a supply-chain or purity regression cannot be
  reviewed away.
- GPL obligations are satisfied by construction for every uploaded artefact, and a public artefact
  never becomes an accidental licence violation.

**Negative / accepted**

- Nine jobs per PR. The three-OS matrix is the expensive part (~80 s wall clock) and it is
  deliberate: it is the only thing that keeps "stays buildable" true.
- Ad-hoc signatures mean the M0/M1 dmgs are `spctl`-rejected if opened normally; that is correct
  and nobody is meant to install them.
- Pinning the toolchain means security updates to `stable` are manual. Accepted: a compiler change
  that lands unannounced in `main` costs more than a deliberate monthly bump.
- The purity scanner is a text scan, not a compiler plugin. Expansion closes the grouped, nested,
  aliased and multi-line `use` forms, but it still cannot see I/O reached through a macro body, a
  helper crate, or a `build.rs`. It stops accidents and casual coupling; the real guarantees remain
  `#![forbid(unsafe_code)]`, the dependency-closure assertion, and review.

## Alternatives considered

1. **All gates in one job.** Faster to write, impossible to see which gate failed, and one runner
   image for all checks means a Windows-only failure reports as a hygiene failure.
2. **G5 at M2 as originally planned.** Rejected: makes K-007 unpassable and defers the first
   bundle exercise to the release milestone.
3. **`gitleaks-action`.** Requires a licence secret for organisations, which fork PRs do not get,
   so the job would skip exactly where it matters. A pinned, checksum-verified gitleaks binary has
   no secret and no conditional path.
4. **Weakening `deny.toml` with `allow-wildcard-paths`** to make the licence job green. Rejected in
   PR #3: the wildcard rule was right and the manifest was wrong.
5. **Publishing `release.yml` now, disabled.** Rejected in §4.
6. **A calendar deadline for the parity harness** (`PARITY_REQUIRED_FROM`). Rejected after review:
   a date cannot tell "nothing to compare yet" from "comparison silently skipped", it fails every
   unrelated PR on expiry, and it contradicts DL-019's turn-based planning. Replaced by the
   fixtures-without-harness trigger in §1.
7. **Matching banned tokens against raw source lines.** Rejected after review: it misses
   `use std::{fs, net};`, which is the form rustfmt produces for two imports from one root, so the
   gate would pass on the first idiomatic line of I/O anyone wrote. Replaced by use-tree expansion
   plus the scanner's own tests.
