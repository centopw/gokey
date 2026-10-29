# GoKey — M0 turn ledger

Owner: PM (R1) · Opened 2026-10-29 · Updated by PM at each gate checkpoint

The ledger is the record of turns spent, not of turns planned. It exists because
DL-019 made the turn the unit of plan and `PLAN.md` §2 gave M0 a budget of 20:
without a per-turn record, the budget is discovered by overrun.

Rules this file enforces, all from existing decisions:

- **One row per turn.** A turn counts only with its artefact — a branch or PR and
  the recipe output — per `WORKFLOW.md` §6 ("an unevidenced turn does not count").
- **PM counts, lanes do not self-report.** Rows are derived from PR records, CI
  runs and `docs/DECISIONS.md`, not from a lane's summary of its own work.
- **M0 budget 20 = 17 task estimate + 3 fix reserve** (`PLAN.md` §2, nine M0 task
  rows). The reserve is for repairs. Work that draws on it is named below rather
  than absorbed.
- **A turn with no backlog row is still recorded**, tagged `—` and marked
  *unbudgeted*. Hiding such turns inside the reserve is how a budget stops
  meaning anything.

Recipe IDs are the ones in `WORKFLOW.md` §13.

---

## 1. Turn log — M0

| # | Lane | Task | Artefact | Recipes run | Result | M0 turns |
|---|---|---|---|---|---|---|
| T1 | R6 DevOps | K-001 (est 2) | #3 merged `a057821` | V-BUILD (3 OSes) | CI green: rustfmt drift fixed, `Cargo.lock` committed | 1 |
| T2 | R6 DevOps | K-001 + K-007 (est 2 + 2) | #4 merged `c6c2777` | V-BUILD, V-MAC (ad-hoc dmg) | Gate ladder + pinned toolchain + first dmg artefact. Time-to-artifact measured on five runs: 23 s (`af30fc5`), 17 s (`1578a0a`), 21 s (`c6c2777`), 18 s (`ec75ea7`), 21 s (`a59f7b6`). The 23→17 s change was the `REQUEST CHANGES` repair, not a timing fix; the spread after it is runner noise | 2 |
| T3 | R2 Backend | K-002 (est 1) | #5 open, 2 commits | V-DOC | ADR-0002 drafted; R6 issued `VERDICT: REQUEST CHANGES` at 09:52:02Z. Turn spent, task not closed | 3 |
| T4 | R1 PM | — *(governance register)* | #6 open, 4 commits | V-DOC | DL-023 (CI artefacts internal-only), DL-024 (enforced merge policy), DL-025 (reviewer independence); `RISKS.md` RK-01 dispositioned | 4 |
| T5 | R1 PM | — *(this ledger)* | #9, one commit | V-DOC | Turn log opened with turns as spent, not as estimated; exposes §4 | 5 |

Turns 1–5 are the whole of M0's evidenced spend. Everything else in M0 is either
delivered without a turn row (K-006, K-007's own budget) or not started (K-003,
K-004, K-005, K-008).

## 2. M0 task status

| Task | Est | Spent | Status |
|---|---|---|---|
| K-001 Monorepo, CI, branch protection | 2 | 2 | **Done** — #3, #4; protection enforced 2026-10-29 (DL-024) |
| K-002 ADR-0002 | 1 | 1 | **Open** — R6 requested changes on #5; needs another turn from R2 |
| K-003 Headless reference build | 2 | 0 | Not started (R5 interim → R3) |
| K-004 Parity corpus v0 | 2 | 0 | Not started |
| K-005 Corpus loader, replay, diff | 2 | 0 | Not started |
| K-006 LICENSE, NOTICE, attribution | 2 | **unrecorded** | Artefacts present (`LICENSE`, `NOTICE.md` incl. upstream attribution and the name/marks section) but no turn row exists. Not counted; if it cost its estimate, M0 stands at 6 |
| K-007 Packaging spike | 2 | 0 marginal | **Delivered inside T2** — see §5.1; the dmg job has since run on every merge to `main` |
| K-008 Adapter spike | 3 | 0 | Not started (R7 unstaffed; FE interim, one turn) |
| K-009 Kickoff | 1 | 1 | **Done** — scope, staffing and platform order confirmed at seq 7–39 (DL-003, DL-014) |

Arithmetic: 4 turns evidenced of 20. Remaining estimate for open work is
K-002 (1) + K-003 (2) + K-004 (2) + K-005 (2) + K-008 (3) = 10, and the 3-turn
reserve is untouched on paper. If K-006 took its estimate, the remaining figure
is 14 against 10 needed. Both are inside the budget; neither is a reason to slow
down, and neither survives contact with K-004 if corpus authoring stalls.

## 3. Turns drawn from other milestones during M0

M1 work started before M0 closed. That is allowed; what is not allowed is
charging it to M0.

| Lane | Task | Milestone | Artefact | Recipes | Result |
|---|---|---|---|---|---|
| R2 | C-001 (est 2) | M1 | #1 open, 6 commits | V-DOC | Porting map (66 symbols), pinned oracle, Rust word model. PM verdict on this turn |
| R2 | C-013 (est 2) | M1 | #2 merged `a59f7b6` | V-CORE (DL-025 comment-only certification) | Comment and task-ID correction in `options.rs`; 8 added / 4 removed, no executable logic changed |

Both are charged to M1's 51, not to M0's 20. This is the first draw on M1.

## 4. Unbudgeted turns — the gap this ledger exposes

| # | Work | Lane | Turns | Why it has no row |
|---|---|---|---|---|
| U1 | Governance register: DL-019…DL-025, `WORKFLOW.md` §1/§2/§6, `RISKS.md` RK-01, `TEAM.md` gate correction, ADR-0008 gate rename | R1 | 4 (seq 70, seq 113, #6, seq 132 review round) | No backlog row covers process governance |
| U2 | Kickoff detours: repo access, `docs/` restore, `~/.gitignore_global` root cause | R1 + R2 | 2 (1 each) | Same; the work is real, the plan has no line for it |
| U3 | This ledger | R1 | 1 | A process artefact cannot carry a task ID |

### 4.1 The three numbers the sponsor asked for

**(a) Turns per lane, M0, against the budget of 20**

| Lane | Task turns | Unbudgeted turns | Total |
|---|---|---|---|
| R1 PM | 1 (T4) | 6 (U1 4, U2 1, U3 1) | 7 |
| R2 Backend | 1 (T3) | 1 (U2) | 2 |
| R6 DevOps | 2 (T1, T2) | 0 | 2 |
| **Total** | **4** | **7** | **11 of 20** |

Remaining task estimate: K-002 1 + K-003 2 + K-004 2 + K-005 2 + K-008 3 = 10,
against 9 turns of budget left. K-006's 2 estimated turns are unrecorded and are
not counted here.

M0 is therefore **one turn over budget on paper before the reserve is touched**,
and three over if K-006 cost its estimate. Confidence: task turns are evidenced by
PR records; the U rows are R1's count of working rounds and are ±1 each. The
direction is not in doubt — unbudgeted work (7) is larger than budgeted task work
(4) — and that is the finding, not the exact total.

**(b) Parity corpus against the 300-case gate**

`tests/corpus/` contains `README.md` and **zero** case files
(`find tests/corpus -name '*.tsv' | wc -l` → `0`). The gate is `PLAN.md` §2 G0,
">=300 corpus cases". K-004 owns it, has spent no turns, and its staffing cliff is
"under 150 cases after three turns on K-004". Nothing is late yet, but the corpus
is the long pole: K-003 → K-004 → K-005 → C-001 validation all queue behind it.

**(c) PRs merged without a recorded verdict**

**Zero of three.** Merged PRs are #2, #3, #4. Test applied: at least one review
whose body begins `VERDICT: APPROVE`, per DL-021.

- #3 — R10 `VERDICT: APPROVE` 08:56Z (an R2 approval at 08:52Z predates the DL-021 format but is on the PR)
- #4 — R2 `VERDICT: REQUEST CHANGES` 09:12Z → `VERDICT: APPROVE` 09:20Z
- #2 — R6 `VERDICT: APPROVE` 09:51Z under DL-025

No unverdicted merges to name.

**Proposal to the sponsor, not applied by me.** One backlog row,
`K-010 Process governance and planning artefacts — R1, M0–M3, no fixed estimate`,
with turns charged to the milestone in which they occur and reported at each gate
checkpoint. I am not adding it: it changes the M0 estimate, and the estimate is
the sponsor's. Without it, every future governance round silently eats the fix
reserve — which, on (a) above, is 7 turns of M0 spend the plan cannot currently
name.

## 5. Merge records — DL-021 and DL-025

`Required GitHub reviews` are off because every agent pushes as `centopw`
(DL-024). The verdict in a `COMMENTED` review is the review record (DL-021), and
a non-authoring lane may issue it under the compensating control in DL-025.

| PR | Author role | Verdict | Independent? | DL-025 control |
|---|---|---|---|---|
| #3 | R6 | R2 `APPROVE` 08:52Z; R10 `APPROVE` 08:56Z | Yes, two lanes | n/a — build/manifest change, `WORKFLOW.md` §2 |
| #4 | R6 | R2 `REQUEST CHANGES` 09:12Z → `APPROVE` 09:20Z on `1578a0a` | Yes | Genuine repair cycle: both required changes are in the first verdict |
| #2 | R2 | R6 `APPROVE` 09:51Z | Yes | Comment-only certification: diff changes comments and doc text, no executable logic |
| #5 | R2 | R6 `REQUEST CHANGES` 09:52Z | Yes | Open — ADR-0002 needs another turn |
| #1 | R2 | R1 PM (this turn) | Yes | n/a — docs-only, V-DOC |
| #6 | R1 | pending R6 (reassigned at seq 123) | — | PM's read is posted as a comment without a verdict line, so it is a second pair of eyes and not the record |

The compensating control in DL-025 requires R1 to record it here. That is what
the last column is for; when a verdict relies on it, the row says which clause
(a: recipes run by the reviewer, b: no executable logic in `openkey-core`) was
satisfied and how.

## 6. Process deviations — recorded, not re-litigated

1. **#4 carried two task IDs** (K-001 and K-007) against the one-PR-per-task rule.
   Consequence visible in §2: K-007 shows 0 marginal turns, which flatters the
   M0 arithmetic by up to 2 turns. The saving is real; the attribution is not.
2. **K-006 has no turn row.** The artefacts shipped, so the work happened. Either
   the lane supplies the row or the estimate is fiction.
3. **Three deliverables have no backlog row** (§4). Recorded as a proposal, not a
   change.
4. **`docs/m0/` did not exist on `main` until this PR.** PR #1 also creates files
   there, so the two PRs are merge-order independent by construction — no shared
   files.
