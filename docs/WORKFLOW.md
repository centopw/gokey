# Workflow

How work moves from idea to shipped artifact. Applies to everyone, including
contractors.

## 1. Source control

- Trunk-based development. `main` is always releasable and protected.
- Branches: `feat/<ID>-<slug>`, `fix/<ID>-<slug>`, `chore/<slug>`,
  `spike/<slug>` (spikes are never merged; they produce an ADR).
- Conventional Commits: `feat(core): ...`, `fix(macos): ...`,
  `docs(plan): ...`. Scope is the crate or area, not the person.
- Squash merge only, enforced in repository settings (merge commits and rebase
  merges are disabled; the branch is deleted on merge). The PR title becomes
  the commit subject and must carry the task ID.
- No direct pushes to `main`, no force pushes, no self-merge on core or
  platform adapters. Enforced by branch protection (DL-024), not by convention.

## 2. Pull requests

Every PR states: task ID, milestone, what changed, how it was verified, and the
risk/rollback note (template in `.github/pull_request_template.md`).

| Area | Required approvals | Extra rule |
| --- | --- | --- |
| `openkey-core` | 2 (one must be R2) | Parity impact statement mandatory; when R2 authored the PR, DL-025 applies — a non-authoring lane reviews with the comment-only certification, preferring R3 then R7 — and R1 records the approval |
| Platform adapters | 1 platform owner + 1 other engineer | Permission or security impact noted |
| CI, packaging, signing | 1 (R6 or R2) | Must show artifact from the branch |
| Docs | 1 | None |
| Emergency fix | 1 | Retrospective note within 48 h |

Approval evidence (DL-021): every agent pushes as `centopw`, so GitHub refuses
`gh pr review --approve` on a PR the same account authored. A review by a
different **role** therefore counts when it is a `COMMENTED` review that starts
with `VERDICT: APPROVE` or `VERDICT: REQUEST CHANGES` and lists the commands
run. The reviewer must be a different role from the author; self-approval in any
form does not count. R1 records the merge approval in the turn ledger.

Because of that constraint, branch protection requires the status checks but not
a GitHub review approval (DL-024). The table above is still the rule: a merge
without a recorded verdict from the named role is a process defect and is
raised at the next gate checkpoint.

Reviewer independence (DL-025): when the lane that authored a PR also holds the
only eligible reviewer role, any other lane may review it, provided the reviewer
runs the task's verification recipes itself and pastes the output, and, for
`openkey-core`, certifies from the diff that no executable logic changed. A
logic change in `openkey-core` still waits for a second engineering lane. R1
records this as a compensating control until R3 and R7 are staffed.

`main` protection (DL-024): the nine required checks are `secrets`,
`build-test` on Ubuntu, macOS and Windows, `licenses`, `parity`, `core-purity`,
`no-network` and `sbom`; the branch must be up to date with `main` before
merging; linear history is required; force pushes and branch deletion are
refused; the rules apply to administrators too.

Review SLA: first response within one turn. A PR with no review after two turns
of its author's work is raised at the next gate checkpoint.

## 3. CI gates (blocking)

`cargo fmt --check`, `cargo clippy -D warnings`, `cargo test` on Ubuntu, macOS
and Windows, `cargo doc`, `cargo deny check` (license allowlist mirrors
`deny.toml`), and the parity smoke suite once the harness exists. A red pipeline
means no merge, no exceptions, including the release turn.

Nightly (non-blocking but triaged daily): full parity corpus, differential
fuzzing, benchmark regression against the previous `main`, dependency audit.

## 4. Work item taxonomy

- Types: `feat`, `fix`, `chore`, `spike`, `adr`, `docs`.
- Areas: `core`, `charset`, `macos`, `windows`, `linux`, `gui`, `ci`,
  `packaging`, `ux`, `legal`.
- Priority: `p0` blocks a gate, `p1` blocks a milestone, `p2` should ship,
  `p3` backlog.
- Extra labels: `needs-linguistics`, `needs-security`, `parity-diff`, `blocked`.

Definition of Ready: objective stated, acceptance criteria testable, owner
named, dependencies identified, verification recipe named, no unresolved design
question larger than one turn.

Definition of Done: code merged with tests; acceptance criteria demonstrated;
**verification recipe run and its output recorded on the task**; parity cases
added or explicitly waived; docs updated; changelog entry; no new `unsafe`
without a `// SAFETY:` note and reviewer; CI green.

## 5. Decisions and ADRs

- Any decision that is expensive to reverse gets an ADR in `docs/adr/NNNN-*.md`
  using the template below, and a row in `DECISIONS.md`.
- ADR status: `proposed`, `accepted`, `superseded`, `rejected`.
- Open decisions are reviewed at every gate checkpoint; the PM chases owners.
- Architecture decisions: R2 accepts. Scope/licence/naming: sponsor accepts.

ADR template: Context, Decision, Alternatives considered, Consequences,
Follow-up tasks.

## 6. Cadence

Cadence is event-driven, not calendar-driven (DL-019):

| Event | When | Output |
| --- | --- | --- |
| Turn log | After every turn, in the task thread | Task ID, what changed, verification output, turns spent, next action |
| Dependency unblock | As soon as a blocker clears | Message to the blocked lane with the commit SHA |
| Gate checkpoint | When a milestone's tasks are all verified, or its budget is 80% spent | Gate decision (pass / conditional / fail) with evidence |
| Risk review | At each gate checkpoint and on any trigger hit | Risk register delta |
| Demo | At each gate | Working build, parity report, latency numbers, known limitations |

Rules for a turn log: evidence first, then blockers, then decisions needed. No
narrative status reports; the verification output is the status. A turn without
recorded evidence does not count as spent against the milestone budget.

## 7. Parity policy

- The corpus is the contract. A behaviour change without a corpus case is a bug.
- Deliberate divergence from upstream requires sign-off from R2 and R11, a
  comment in the corpus line, and a changelog entry. Never "fix" a quirk silently.
- A parity regression blocks merge, not just release.
- Coverage gate: `openkey-core` line coverage >= 85% at G1.

## 8. Release and versioning

Semantic versioning. `0.x` until G5, then `1.0.0`. Pre-releases: `0.x.y-alpha.N`,
`-beta.N`.

Release checklist (all required for `1.x`):

1. All gate criteria for the milestone met and evidenced.
2. Full parity corpus green; benchmark within budget.
3. Security review signed off; privacy statement updated.
4. LICENSE and NOTICE present in every artifact and installer.
5. Install, upgrade and uninstall tested on a clean machine per OS.
6. Signing and notarisation verified; checksums published.
7. Release notes in Vietnamese and English, including known limitations.
8. Rollback path documented and tested (previous version reinstall).
9. Support channel and issue triage owner named for the release window.

## 9. Security and privacy rules

- No network calls at runtime except the signed update check.
- Never log, store or transmit typed text or buffer contents, including in
  crash reports; crash payloads are reviewed before enabling collection.
- Request the minimum OS permission; explain it in-product before triggering the
  system prompt.
- New dependencies require a license check and a security review if they touch
  input, filesystem or network.
- Report a vulnerability privately to the maintainers; do not open a public issue.

## 10. Upstream and licensing

- Credit OpenKey and Mai Vu Tuyen in NOTICE, README and the About dialog.
- Published builds are named **GoKey** (DL-010); "OpenKey" appears only in
  attribution text, never as the product name, package name or icon.
- Do not merge Rust code into upstream; instead contribute bug reports,
  reproductions and documentation where useful, and keep the relationship warm.
- Any binary distribution is a GPL-3.0 distribution: source must be available
  at the same version, and modifications must be stated.

## 11. Escalation

| Situation | Escalate to | Within |
| --- | --- | --- |
| Blocker older than 2 turns | PM (R1) | Same turn |
| Platform API dead end or permission change | Tech lead (R2) | 1 turn; spike timeboxed at 3 turns |
| Same task without measurable progress for 2 turns | Tech lead (R2) | Next turn |
| Scope change, extra platform, gate slip | Sponsor via PM | Before any commitment |
| Parity exception or user-visible divergence | R2 + R11, then PM | Before merge |
| Security or privacy concern | R12 | Immediately |
| Licence or naming risk | R13 + sponsor | Before any public artifact |

## 12. AI feature rules (post-1.0)

- No AI feature work before G3 (macOS 1.0). Until then only the inert seams in
  `ARCHITECTURE.md` Section 10 are built.
- Every AI feature ships behind its own flag, default off, with an opt-in that
  states exactly what text is read and where inference runs.
- No context text in logs, telemetry, crash payloads, or off-device requests
  without separate explicit consent.
- Secure fields, password managers and any field the OS marks secure are
  hard-blocked at the `ContextProvider` layer.
- Suggestions never block the keystroke path: worker thread, cancellation on the
  next keystroke, enforced deadline (150 ms target).
- **Correction suggestions follow DL-018 exactly:** highlight the error span,
  paint the replacement, `Tab` accepts, `Esc` dismisses, never auto-correct,
  one suggestion at a time, and never re-offer the same span after dismissal.
  The span must end at the caret; a correction that needs a selection is a new
  ADR, not a shortcut.
- Continuing to type drops the suggestion and changes nothing in the document.
  A new suggestion is allowed only when a different anchor word is wrong.
- Accepting a suggestion is a single undo unit, resets the engine word buffer,
  and must not corrupt the following keystrokes.
- Each feature needs an evaluation set and a measurable target before release;
  for correction the gate is false-positive rate, not only acceptance rate.
- Any new model or runtime dependency needs a GPL-3.0-compatible licence review
  and a security review.

## 13. Turn contract and verification recipes

A turn is the unit of work (DL-019). It is complete only when all four parts
exist:

1. **Scope**: one task ID, one branch, one artifact increment.
2. **Change**: the diff, small enough to review in one pass.
3. **Verification**: the task's recipe run from a clean checkout, with the
   command and a pass/fail summary pasted into the task thread.
4. **Ledger**: turns spent, what was learned, next action or blocker.

Recipes are referenced by ID in the `verify` column of `backlog.csv`:

| ID | Recipe | Pass condition |
| --- | --- | --- |
| V-BUILD | `cargo build --workspace --all-targets` on Ubuntu, macOS, Windows | all three green; no new warnings |
| V-TEST | `cargo test --workspace` | all pass; no ignored tests without a linked issue |
| V-LINT | `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo deny check` | clean |
| V-CORE | V-BUILD + V-TEST + `cargo test -p openkey-core` with `#![forbid(unsafe_code)]` intact | no `unsafe`, no `cfg(target_os)` in core |
| V-PARITY | `openkey-cli replay --corpus tests/corpus --diff ref-engine` | agreement >= target, every diff classified |
| V-MAC | build + install on a clean macOS 13+ machine, type the 20-sentence script | script reproduces expected text; latency p99 within budget |
| V-DOC | docs build/link check; every changed claim cites a task or decision | no dead links, no unsupported claim |
| V-SEC | R12 review of permissions, data flows, and dependency licences | written sign-off with date |
| V-REL | release checklist in Section 8, executed on a clean machine | every checklist item evidenced |
| V-AI | evaluation set run, precision/acceptance reported, secure-field block tested | metrics meet the flag's target; block proven |

Rules:

- A recipe is run by the author, not the reviewer; the reviewer checks that the
  output matches the recipe.
- If a recipe cannot run (missing machine, missing upstream build), that is a
  blocker, not a waiver. Escalate within one turn.
- Two failed repair attempts on the same task stop the loop: escalate per
  Section 11 instead of spending a third turn on the same approach.
