# Workflow

How work moves from idea to shipped artifact. Applies to everyone, including
contractors.

## 1. Source control

- Trunk-based development. `main` is always releasable and protected.
- Branches: `feat/<ID>-<slug>`, `fix/<ID>-<slug>`, `chore/<slug>`,
  `spike/<slug>` (spikes are never merged; they produce an ADR).
- Conventional Commits: `feat(core): ...`, `fix(macos): ...`,
  `docs(plan): ...`. Scope is the crate or area, not the person.
- Squash merge only. The PR title becomes the commit subject and must carry
  the task ID.
- No direct pushes to `main`, no force pushes, no self-merge on core or
  platform adapters.

## 2. Pull requests

Every PR states: task ID, milestone, what changed, how it was verified, and the
risk/rollback note (template in `.github/pull_request_template.md`).

| Area | Required approvals | Extra rule |
| --- | --- | --- |
| `openkey-core` | 2 (one must be R2) | Parity impact statement mandatory |
| Platform adapters | 1 platform owner + 1 other engineer | Permission or security impact noted |
| CI, packaging, signing | 1 (R6 or R2) | Must show artifact from the branch |
| Docs | 1 | None |
| Emergency fix | 1 | Retrospective note within 48 h |

Review SLA: first response within one working day. A PR idle for three days is
raised in the weekly sync.

## 3. CI gates (blocking)

`cargo fmt --check`, `cargo clippy -D warnings`, `cargo test` on Ubuntu, macOS
and Windows, `cargo doc`, `cargo deny check` (license allowlist mirrors
`deny.toml`), and the parity smoke suite once the harness exists. A red pipeline
means no merge, no exceptions, including release week.

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
named, dependencies identified, no unresolved design question larger than a day.

Definition of Done: code merged with tests; acceptance criteria demonstrated;
parity cases added or explicitly waived; docs updated; changelog entry; no new
`unsafe` without a `// SAFETY:` note and reviewer; CI green.

## 5. Decisions and ADRs

- Any decision that is expensive to reverse gets an ADR in `docs/adr/NNNN-*.md`
  using the template below, and a row in `DECISIONS.md`.
- ADR status: `proposed`, `accepted`, `superseded`, `rejected`.
- Open decisions are reviewed at the weekly sync; the PM chases owners.
- Architecture decisions: R2 accepts. Scope/licence/naming: sponsor accepts.

ADR template: Context, Decision, Alternatives considered, Consequences,
Follow-up tasks.

## 6. Cadence

| Event | When | Output |
| --- | --- | --- |
| Async daily update | Each person, before 10:00 local, in the team thread | Done / next / blockers |
| Weekly sync (30 min) | Monday | Decision log updates, risk changes, dependency unblocks |
| Biweekly demo | Every second Thursday | Working build, parity report, latency numbers |
| Milestone review | End of each milestone | Gate decision (pass / conditional / fail) with evidence |
| Release candidate | Monthly from M2 | Signed artifacts, release notes draft |

Rules for the sync: status first, then decisions needed, then risks. No
narrative status reports; the board is the status.

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
- Do not merge Rust code into upstream; instead contribute bug reports,
  reproductions and documentation where useful, and keep the relationship warm.
- Any binary distribution is a GPL-3.0 distribution: source must be available
  at the same version, and modifications must be stated.
- No use of the OpenKey name or icon in a published build until DL-010 is closed.

## 11. Escalation

| Situation | Escalate to | Within |
| --- | --- | --- |
| Blocker older than 2 days | PM (R1) | Same day |
| Platform API dead end or permission change | Tech lead (R2) | 1 day; spike timeboxed at 5 days |
| Scope change, extra platform, date slip | Sponsor via PM | Before any commitment |
| Parity exception or user-visible divergence | R2 + R11, then PM | Before merge |
| Security or privacy concern | R12 | Immediately |
| Licence or naming risk | R13 + sponsor | Before any public artifact |
