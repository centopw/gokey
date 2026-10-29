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
| T2 | R6 DevOps | K-001 + K-007 (est 2 + 2) | #4 merged `c6c2777` | V-BUILD, V-MAC (ad-hoc dmg) | Gate ladder + pinned toolchain + first dmg artefact. Time-to-artifact on five runs: 23 s (`af30fc5`), 17 s (`1578a0a`), 21 s (`c6c2777`), 18 s (`ec75ea7`), 21 s (`a59f7b6`). The 23→17 s change was the `REQUEST CHANGES` repair, not a timing fix; the spread after it is runner noise | 2 |
| T3 | R2 Backend | K-002 (est 1) | #5 merged `7194789` | V-DOC | ADR-0002 drafted, `REQUEST CHANGES` 09:52:02Z, repaired, `APPROVE` 09:59:03Z. **Closed** — the repair and merge happened after this row was first written | 3 |
| T4 | R1 PM | — *(governance register)* | #6 merged `22887b0` | V-DOC | DL-023 (CI artefacts internal-only), DL-024 (enforced merge policy), DL-025 (reviewer independence); `RISKS.md` RK-01 dispositioned | 4 |
| T5 | R1 PM | — *(this ledger)* | #9 merged `3127a75` | V-DOC | Turn log opened with turns as spent, not as estimated. Counted as U3, not as a task turn | 5 |
| T6 | R2 Backend | K-003 (est 2) | #11 merged `6cd6eeb` | V-PARITY (11/11 smoke at `46b33e0`), unit (39) | Harness + comparator against pin `89c2fd3b`; four false-pass paths converted to exit 2. **#12 was lost with its stacked base** when #11's branch was deleted and could not be reopened, so it was re-issued as #14 — a repair round spent on a merge-order artefact, not on the task | 6 |
| T7 | R2 Backend | K-004 (est 2) | #14 merged `e08016d` | V-PARITY (189/189), unit (39) | 178 cases in nine files, 11 smoke, 189 of the 300 the gate needs. Six rows record upstream's behaviour where our expectation differed; all six are reproduction under DL-013 | 7 |
| T8 | FE | K-008 (est 3) | #10 merged `74dab98` | V-DOC | ADR-0004: active HID tap, seven-type mask, 16 `extern` functions + 1 `extern static`. The trusted run is still open and is the only thing between DL-004 and MAC-002 | 8 |

Rows **T1–T4 and T6–T8** are M0's task spend: seven. T5 is not a task — a
process artefact cannot carry a task ID, so §4 counts it as U3 and the two sets
do not overlap. Everything in M0 that is neither here nor in §4 is either
delivered inside another row (K-007 inside T2, K-009 inside T4's kickoff) or has
no row at all (K-006, §2).

## 2. M0 task status

| Task | Est | Spent | Status |
|---|---|---|---|
| K-001 Monorepo, CI, branch protection | 2 | 2 | **Done** — #3, #4; protection enforced 2026-10-29 (DL-024) |
| K-002 ADR-0002 | 1 | 1 | **Done** — #5 merged `7194789` after one repair cycle |
| K-003 Headless reference build | 2 | 2 | **Done** — #11 merged `6cd6eeb`; harness, comparator and a pin check that fetches by hash |
| K-004 Parity corpus v0 | 2 | 1 | **Open — 178 of 300 cases.** #14 merged `e08016d`; 122 to go, and the 150-case cliff is cleared. The pinned-behaviour index is a condition of closure |
| K-005 Corpus loader, replay, diff | 2 | 0 | Not started. `parity` is already blocking on eleven smoke cases, so this is now the widest gap between a gate and its evidence |
| K-006 LICENSE, NOTICE, attribution | 2 | **unrecorded** | Artefacts present (`LICENSE`, `NOTICE.md` with upstream attribution and the name/marks section) but no turn row exists. Not counted; if it cost its estimate, add 2 to every total in §4.1 |
| K-007 Packaging spike | 2 | 0 marginal | **Delivered inside T2** — the dmg job has run on every merge to `main` since |
| K-008 Adapter spike | 3 | 1 | **Open** — ADR-0004 landed with #10; the trusted run that closes DL-004's open verification has not happened, and MAC-002 cannot start without it |
| K-009 Kickoff | 1 | 1 | **Done** — scope, staffing and platform order confirmed at seq 7–39 (DL-003, DL-014) |

Four task rows are closed (K-001, K-002, K-003, K-009), one was delivered
inside another row (K-007 in T2), and one shipped its artefacts with no turn row
at all (K-006). The three that remain open — K-004, K-005, K-008 — are the three
the milestone's gate actually reads: a 300-case corpus, a replay command, and an
adapter decision with a trusted run behind it. On turns, though, M0 is already
overspent: **7 task turns and 17 unbudgeted ones, 24 of 20**, with 5 more of task
estimate still outstanding (§4.1(a)).

## 3. Turns drawn from other milestones during M0

M1 work started before M0 closed. That is allowed; what is not allowed is
charging it to M0.

| Lane | Task | Milestone | Artefact | Recipes | Result |
|---|---|---|---|---|---|
| R2 | C-001 (est 2) | M1 | #1 merged `f7bda19` | V-DOC | Porting map (66 symbols), pinned oracle, Rust word model. PM `REQUEST CHANGES` 09:58Z on `7bdad6a` (two label strings); repaired in `7de616e`, `APPROVE` 10:08Z. The map's Q3 now resolves to DL-026 |
| R2 | C-013 (est 2) | M1 | #2 merged `a59f7b6` | V-CORE (DL-025 comment-only certification) | Comment and task-ID correction in `options.rs`; 8 added / 4 removed, no executable logic changed |

Both rows are charged to M1's 51, not to M0's 20. U4 is the exception and is the
one thing in this section charged to M0: reviewing #1 is M1 work, but the turn is
PM's and PM turns are M0's.

## 4. Unbudgeted turns — the gap this ledger exposes

| # | Work | Lane | Turns | Why it has no row |
|---|---|---|---|---|
| U1 | Governance register: DL-019…DL-025, `WORKFLOW.md` §1/§2/§6, `RISKS.md` RK-01, `TEAM.md` gate correction, ADR-0008 gate rename | R1 | 5 (seq 70, seq 113, #6, #8 review ×2) | No backlog row covers process governance |
| U2 | Kickoff detours: repo access, `docs/` restore, `~/.gitignore_global` root cause | R1 + R2 | 2 (1 each) | Same; the work is real, the plan has no line for it |
| U3 | This ledger | R1 | 1 | A process artefact cannot carry a task ID |
| U4 | #1 review round: `REQUEST CHANGES` 09:58Z on `7bdad6a`, then re-review and `APPROVE` 10:08Z on `7de616e` | R1 | 2 | Review work on another lane's task is not that task's turn |
| U5 | Merging the backlog: #9, #6, #8 onto refreshed heads, plus the gate-naming audit that found `PLAN.md`'s `G6` | R1 | 1 | Merge and audit work; the audit is what made #13 necessary |
| U6 | #14 review: both recipes run, 178 rows checked for duplicates and no-ops, six pinned-behaviour rows classified against DL-013 and DL-026 | R1 | 1 | Review work, same basis as U4 |
| U7 | #10 verification and the #13 batch: DL-026, DL-013, the PG gates, then three factual repairs after `REQUEST CHANGES` | R1 | 1 | Governance and decision recording |
| U8 | Review rounds by non-authoring lanes: #11 (R6, 1), #13 `REQUEST CHANGES` then `APPROVE` (R6, 2), #10 refresh against moving `main` (FE, 1) | R6 + FE | 4 | DL-021/DL-025 made review a recorded artefact with a cost; nothing budgets it |

### 4.1 The three numbers the sponsor asked for

**(a) Turns per lane, M0, against the budget of 20**

| Lane | Task turns | Unbudgeted turns | Total |
|---|---|---|---|
| R1 PM | 1 (T4) | 12 (U1 5, U2 1, U3 1, U4 2, U5 1, U6 1, U7 1) | 13 |
| R2 Backend | 3 (T3, T6, T7) | 1 (U2) | 4 |
| R6 DevOps | 2 (T1, T2) | 3 (U8) | 5 |
| FE | 1 (T8) | 1 (U8) | 2 |
| **Total** | **7** | **17** | **24 of 20** |

Remaining task estimate: K-004 1 (122 cases) + K-005 2 + K-008 2 = 5, against
**−4 turns of budget left**. K-006's 2 estimated turns are unrecorded and are not
counted here. If they cost their estimate, the totals become 26 and −6.

M0 is **four turns over budget with a third of its task estimate still unspent**,
and the projection is 29 of 20 before the fix reserve is considered — 31 if K-006
cost its estimate. Confidence: task turns are evidenced by PR records; the U rows
are R1's count of working rounds and are ±1 each. The direction is not in doubt —
unbudgeted work (17) is nearly two and a half times task work (7), and it grew
faster than the tasks did (10 → 17 against 4 → 7). **The estimate was not wrong
about the tasks; the plan has no line for the work that surrounds them.**

**(b) Parity corpus against the 300-case gate**

`find tests/corpus -name '*.tsv' -exec cat {} + | grep -vc '^#\|^$'` → **178**;
`grep -vc '^#\|^$' scripts/parity/smoke.tsv` → 11. The gate replays **189 of 300**,
so 111 to go, against `PLAN.md` §2 G0's ">=300 corpus cases". The 150-case
staffing cliff is cleared, one turn into K-004's estimate of two.

The number that matters more than the count: `parity` became blocking on every PR
when #11 merged, and until #14 landed its only evidence was eleven smoke cases.
A gate that runs on eleven cases is a gate; it is not a measurement. K-004 and
K-005 are now the two tasks standing between G0 and an honest parity number, and
K-005 has spent nothing.

**(c) PRs merged without a recorded verdict**

**Zero of twelve, as of `cda601b`.** Merged PRs: #1, #2, #3, #4, #5, #6, #8, #9,
#10, #11, #13, #14. Test applied: at least one review whose body begins
`VERDICT:`, per DL-021 — audited with a command rather than recalled, one API
call per PR:

```
for n in $(gh pr list --state merged --limit 30 --json number -q '.[].number' | sort -n); do
  gh api repos/centopw/gokey/pulls/$n/reviews --jq \
    '[.[] | select(.body | test("^VERDICT:"))] | length'
done
→ #1 2, #2 1, #3 2, #4 2, #5 2, #6 2, #8 2, #9 2, #10 2, #11 2, #13 2, #14 1
```

No unverdicted merges to name. The distribution is worth reading too: four PRs
needed two rounds, and every one of those rounds was an *independent lane
blocking* — #4 (R2), #1 (R1), #5 (R6), #10 (R1), #13 (R6). The control is doing
work, not decorating the record.

**Proposal to the sponsor, not applied by me.** One backlog row,
`K-010 Process governance and planning artefacts — R1, M0–M3, no fixed estimate`,
with turns charged to the milestone in which they occur and reported at each gate
checkpoint. I am not adding it: it changes the M0 estimate, and the estimate is
the sponsor's. The case for it is no longer the 10 turns of the first revision —
it is 17, against 7 of task spend, and the reserve it is eating is 3 turns.

## 5. Merge records — DL-021 and DL-025

`Required GitHub reviews` are off because every agent pushes as `centopw`
(DL-024). The verdict in a `COMMENTED` review is the review record (DL-021), and
a non-authoring lane may issue it under the compensating control in DL-025.

| PR | Author role | Verdict | Independent? | DL-025 control |
|---|---|---|---|---|
| #3 | R6 | R2 `APPROVE` 08:52Z; R10 `APPROVE` 08:56Z | Yes, two lanes | n/a — build/manifest change, `WORKFLOW.md` §2 |
| #4 | R6 | R2 `REQUEST CHANGES` 09:12Z → `APPROVE` 09:20Z on `1578a0a` | Yes | Genuine repair cycle: both required changes are in the first verdict |
| #2 | R2 | R6 `APPROVE` 09:51Z | Yes | Comment-only certification: comments and doc text, no executable logic |
| #5 | R2 | R6 `REQUEST CHANGES` 09:52Z → `APPROVE` 09:59:03Z | Yes | Genuine repair cycle; merged 10:16:08Z as `7194789` |
| #1 | R2 | R1 `REQUEST CHANGES` 09:58Z on `7bdad6a`; `APPROVE` 10:08Z on `7de616e` | Yes | n/a — docs-only, V-DOC; 11+/7- in two files, no executable logic |
| #6 | R1 | R6 `APPROVE` 10:05Z on `da43f03` | Yes | n/a — R6's own verdict; PM's read is a comment without a verdict line. Merged `22887b0` at a head verified byte-identical to `da43f03` |
| #8 | R6 | R1 `REQUEST CHANGES` → `APPROVE` on `e468b4f` | Yes | n/a — docs and CI job names. Merged `b672281` on the same byte-identical rule |
| #9 | R1 | R6 `APPROVE` on `d9cc868` | Yes | n/a — docs; merged as-is rather than repairing two dated cells, because a repair would have cost a review round to change a snapshot the file already labels as dated |
| #11 | R2 | R6 `APPROVE` at `0dbf995` | Yes | Clause (a) and (b): R6 re-probed all four comparator paths and re-ran both recipes rather than reading the author's report |
| #14 | R2 | R1 `APPROVE` at `32df864` | Yes | Clause (a): both recipes run by the reviewer, 178 rows audited for duplicates and no-ops, the strongest upstream claim re-derived from `Engine.cpp` |
| #10 | FE | R1 `APPROVE` at `fa431c3` | Yes | n/a — ADR + one register line. Merged `74dab98` at a head whose ADR was byte-identical to `fa431c3` |
| #13 | R1 | R6 `REQUEST CHANGES` at `24aed64` → `APPROVE` at `6f98328` | Yes | n/a — docs; three factual repairs, each re-verified by the reviewer from the primary source |

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
3. **Five deliverables have no backlog row** (§4). Recorded as a proposal, not a
   change.
4. **`docs/m0/` did not exist on `main` until #9.** #1 also created files there,
   so the two PRs were merge-order independent by construction — no shared files.
5. **#12 was lost to a merge-order artefact.** It was stacked on #11's branch;
   deleting that branch on merge closed #12 irrecoverably, and the work had to be
   re-issued as #14. Stacked PRs are incompatible with delete-branch-on-merge
   (DL-024) and one-PR-per-task: the second PR should have been based on `main`
   after the first merged. No turn was lost that could be avoided, but one was
   spent twice.
6. **A verdict is pinned to a commit, so a stale ref reads as a finding.** Twice
   this round a check compared against a local ref that had not been fetched, and
   once it reported a fix as a revert. The rule now: compare against the head the
   API reports, never against a tracking ref. The same failure mode as a `\b`
   grep that matches nothing — a confident result from a check whose input was
   wrong (`WORKFLOW.md` §13.1).
