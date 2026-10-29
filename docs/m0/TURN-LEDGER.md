# GoKey — M0 turn ledger

Owner: PM (R1) · Opened 2026-10-29 · Updated by PM at each gate checkpoint

The ledger is the record of turns spent, not of turns planned. It exists because
DL-019 made the turn the unit of plan and `PLAN.md` §2 gave M0 a budget of 20:
without a per-turn record, the budget is discovered by overrun.

Rules this file enforces, all from existing decisions:

- **One row per turn, priced by artefact.** A turn counts only with its artefact —
  a branch or PR and the recipe output — per `WORKFLOW.md` §6 ("an unevidenced turn
  does not count"). Two artefacts are countable without judgement: a merge (a task
  turn, §1) and a review whose body begins `VERDICT:` (a review turn, §4 U9).
- **A `REQUEST CHANGES` then an `APPROVE` is two turns.** Two published verdicts,
  and the second follows work. One cycle is priced the same way on every lane and
  every PR; the first revision of this file priced the same cycle at 2 on one lane
  and 1 on another, which is why its lane totals could not be added. A comment with
  no `VERDICT:` line is not a turn — including PM's read on #6, by design.
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
| T4 | R1 PM | — *(governance register)* | #6 merged `22887b0` | V-DOC | DL-023 (CI artefacts internal-only), DL-024 (enforced merge policy), DL-025 (reviewer independence); `RISKS.md` RK-01 dispositioned. **Priced in §4 U1, not as task spend** | 4 |
| T5 | R1 PM | — *(this ledger)* | #9 merged `3127a75` | V-DOC | Turn log opened with turns as spent, not as estimated. **Priced in §4 U3**, not as a task turn | 5 |
| T6 | R2 Backend | K-003 (est 2) | #11 merged `6cd6eeb` | V-PARITY (11/11 smoke at `46b33e0`), unit (39) | Harness + comparator against pin `89c2fd3b`; four false-pass paths converted to exit 2. **#12 was lost with its stacked base** when #11's branch was deleted and could not be reopened, so it was re-issued as #14 — a repair round spent on a merge-order artefact, not on the task | 6 |
| T7 | R2 Backend | K-004 (est 2) | #14 merged `e08016d` | V-PARITY (178 corpus + 11 smoke, 189/189 replayed), unit (39) | 178 cases in nine files, plus the 11 smoke cases that predate them. **178 of the 300 corpus cases G0 reads — 122 to go**; the 189 is what replays, not what the gate counts. Six rows record upstream's behaviour where our expectation differed; all six are reproduction under DL-013 | 7 |
| T8 | FE | K-008 (est 3) | #10 merged `74dab98` | V-DOC | ADR-0004: active HID tap, seven-type mask, 16 `extern` functions + 1 `extern static`. The trusted run is still open and is the only thing between DL-004 and MAC-002 | 8 |

Three kinds of turn appear above and they do not all spend task estimate. Rows
**T1–T3 and T6–T8** are attributable to a backlog row: six of M0's task turns.
**T4 and T5 carry `—`**, so by the rule in the header neither can be task spend:
T4 is the governance register (§4 U1) and T5 is this ledger (§4 U3), and each is
counted once, in §4. K-009 has no §1 row at all — the kickoff produced decisions
rather than a merge, so its turn is a task turn without a row and is counted in
§4.1(a). That gives seven task turns. Everything else in M0 is either delivered
inside another row (K-007 inside T2) or has no row anywhere (K-006, §2).

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
| K-009 Kickoff | 1 | 1 | **Done** — scope, staffing and platform order confirmed at seq 7–39 (DL-003, DL-014). No merge artefact, so no §1 row; the turn is counted in §4.1(a) |

Four task rows are closed (K-001, K-002, K-003, K-009), one was delivered
inside another row (K-007 in T2), and one shipped its artefacts with no turn row
at all (K-006). The three that remain open — K-004, K-005, K-008 — are the three
the milestone's gate actually reads: a 300-case corpus, a replay command, and an
adapter decision with a trusted run behind it. On turns, though, M0 is already
overspent: **7 task turns and 31 unbudgeted ones, 38 of 20**, with 5 more of task
estimate still outstanding (§4.1(a)).

## 3. Turns drawn from other milestones during M0

M1 work started before M0 closed. That is allowed; what is not allowed is
charging it to M0.

| Lane | Task | Milestone | Artefact | Recipes | Result |
|---|---|---|---|---|---|
| R2 | C-001 (est 2) | M1 | #1 merged `f7bda19` | V-DOC | Porting map (66 symbols), pinned oracle, Rust word model. PM `REQUEST CHANGES` 09:58Z on `7bdad6a` (two label strings); repaired in `7de616e`, `APPROVE` 10:08Z. The map's Q3 now resolves to DL-026 |
| R2 | C-013 (est 2) | M1 | #2 merged `a59f7b6` | V-CORE (DL-025 comment-only certification) | Comment and task-ID correction in `options.rs`; 8 added / 4 removed, no executable logic changed |

Both rows are charged to M1's 51, not to M0's 20. Reviewing #1 is the exception
and is charged to M0: the work is M1's, but the turn is PM's and PM turns are M0's.
It is one of the verdicts in §4 U9, which is where every review turn now sits.

## 4. Unbudgeted turns — the gap this ledger exposes

| # | Work | Lane | Turns | Why it has no row |
|---|---|---|---|---|
| U1 | Governance register: DL-019…DL-025, `WORKFLOW.md` §1/§2/§6, `RISKS.md` RK-01, `TEAM.md` gate correction, ADR-0008 gate rename. Includes #6, which is row T4 | R1 | 3 (seq 70, seq 113, #6) | No backlog row covers process governance |
| U2 | Kickoff detours: repo access, `docs/` restore, `~/.gitignore_global` root cause | R1 + R2 | 2 (1 each) | Same; the work is real, the plan has no line for it |
| U3 | This ledger (#9, row T5) | R1 | 1 | A process artefact cannot carry a task ID |
| U5 | Merging the backlog: #9, #6, #8 onto refreshed heads, plus the gate-naming audit that found `PLAN.md`'s `G6` | R1 | 1 | Merge and audit work; the audit is what made #13 necessary |
| U7 | #13 authoring: DL-026, the DL-013 form, PG-01…PG-03, the evidence-grep rule, then three factual repairs after `REQUEST CHANGES` | R1 | 1 | Governance and decision recording |
| U8 | #10 branch refresh after `main` moved and the branch fell behind | FE | 1 | Author work on its own PR; not a verdict, so U9 does not price it |
| U9 | **Review verdicts by non-authoring lanes — one turn per `VERDICT:` review.** R1: #1 ×2, #8 ×2, #10 ×2, #14 ×1. R6: #2, #5 ×2, #6 ×2, #9 ×2, #11 ×2, #13 ×2. R2: #3, #4 ×2. R10: #3 | R1 7, R6 11, R2 3, R10 1 | 22 | DL-021 made review a recorded artefact with a cost; nothing budgets it. Counted from the API, not from memory (command in §4.1(c)) |

U4 and U6 held the #1 and #14 reviews; both are now inside U9 so that one rule
prices them. The identifiers were not renumbered, so references to this file from
#13 and `WORKFLOW.md` still resolve. The turn that used to be counted twice — #6 in
T4 *and* in U1 — is counted once.

### 4.1 The three numbers the sponsor asked for

**(a) Turns per lane, M0, against the budget of 20**

| Lane | Task turns | Unbudgeted — non-review | Review verdicts (U9) | Total |
|---|---|---|---|---|
| R1 PM | 1 (K-009 kickoff) | 7 (U1 3, U2 1, U3 1, U5 1, U7 1) | 7 | **15** |
| R2 Backend | 3 (T3, T6, T7) | 1 (U2) | 3 | **7** |
| R6 DevOps | 2 (T1, T2) | 0 | 11 | **13** |
| FE | 1 (T8) | 1 (U8) | 0 | **2** |
| R10 | 0 | 0 | 1 | **1** |
| **Total** | **7** | **9** | **22** | **38 of 20** |

Every column is checkable. Task turns come from §1 plus K-009's recorded decisions;
the merges in U1 and U3 are in `git log` on `main`. Review turns come from one API
call per PR (§4.1(c)) and are exact — the arithmetic is "count the lines beginning
`VERDICT:`". The nine non-review unbudgeted turns are R1's count of working rounds
and are ±1 each; that is 24% of the total, so a mistake there moves the headline by
at most one.

Remaining task estimate: K-004 1 (122 cases) + K-005 2 + K-008 2 = 5, against
**−18 turns of budget left**. K-006's 2 estimated turns are unrecorded and are not
counted here. If they cost their estimate, the totals become 40 and the projection 45.

M0 is **eighteen turns over budget with a third of its task estimate still
unspent**, and the projection is 43 of 20 before the fix reserve is considered — 45
if K-006 cost its estimate. The first revision of this file put the same milestone
at 24 turns. The fourteen-turn difference is U9's 22 review turns, less the six that
U4, U6 and U8 already carried in part, less the one turn #6 was counted twice for.
**The plan priced delivery and never priced review.** Twenty-two of the 38 turns are
a lane reading another lane's work. They were never free — DL-021 is what made them
*recorded*, and a record can be counted. The tasks themselves were estimated at 17
and are running at 7 spent plus 5 outstanding, close to plan.

The distribution matters more than the total. R1 carries 15 of 38 (39%), R6 13
(34%), R2 7, FE 2, R10 1. My own lane is the largest single spender, and its seven
non-review turns are the ones a K-010 row would have covered.

**(b) Parity corpus against the 300-case gate**

`find tests/corpus -name '*.tsv' -exec cat {} + | grep -vc '^#\|^$'` → **178**.
`grep -vc '^#\|^$' scripts/parity/smoke.tsv` → 11. These are two different numbers
and G0 reads only the first: **178 of 300 corpus cases, 122 to go**, against
`PLAN.md` §2 G0's ">=300 corpus cases". The 189 that the first revision printed is
what `parity` *replays* today — 178 corpus plus 11 smoke — and replaying eleven
smoke cases does not advance a 300-case gate. The 150-case staffing cliff is
cleared, one turn into K-004's estimate of two.

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

No unverdicted merges to name. The distribution is worth reading too, and the
first revision read it wrong: it said "four PRs needed two rounds" and then listed
five. The correct count, straight from the same command, is **nine repair rounds**
— nine of the twelve merged PRs published a `REQUEST CHANGES` and then an `APPROVE`:

| Reviewer lane | PRs where it blocked and then approved | Rounds |
|---|---|---|
| R1 | #1, #8, #10 | 3 |
| R6 | #5, #6, #9, #11, #13 | 5 |
| R2 | #4 | 1 |

#3 is the one multi-review PR that is **not** a repair round: R2 approved at 08:52
without a `VERDICT:` line, R10 issued the signed verdict at 08:56, and R2 signed
again at 09:05 — two lanes agreeing, nobody blocking. #2 and #14 were approved on
first review. That is nine of twelve PRs requiring a repair cycle, which is the
substantive finding: the first review catches something three times in four, and
the review cost is a delivery cost, not a review-team luxury.

**Proposal to the sponsor, not applied by me.** One backlog row,
`K-010 Process governance and planning artefacts — R1, M0–M3, no fixed estimate`,
with turns charged to the milestone in which they occur and reported at each gate
checkpoint. I am not adding it: it changes the M0 estimate, and the estimate is
the sponsor's. The case for it is no longer the 10 turns of the first revision —
it is 31, against 7 of task spend, and the reserve it is eating is 3 turns.

## 5. Merge records — DL-021 and DL-025

`Required GitHub reviews` are off because every agent pushes as `centopw`
(DL-024). The verdict in a `COMMENTED` review is the review record (DL-021), and
a non-authoring lane may issue it under the compensating control in DL-025.

| PR | Author role | Verdict | Independent? | DL-025 control |
|---|---|---|---|---|
| #3 | R6 | R2 `APPROVE` 08:52Z — **no `VERDICT:` line, so not a signed record**; R10 `APPROVE` 08:56Z; R2 `APPROVE` 09:05Z | Yes, two lanes | n/a — build/manifest change, `WORKFLOW.md` §2. Three reviews, no block: the only merged PR where a lane reviewed without signing |
| #4 | R6 | R2 `REQUEST CHANGES` 09:12Z → `APPROVE` 09:20Z on `1578a0a` | Yes | Genuine repair cycle: both required changes are in the first verdict |
| #2 | R2 | R6 `APPROVE` 09:51Z | Yes | Comment-only certification: comments and doc text, no executable logic |
| #5 | R2 | R6 `REQUEST CHANGES` 09:52:02Z → `APPROVE` 09:59:03Z | Yes | Genuine repair cycle; merged 10:16:08Z as `7194789` |
| #1 | R2 | R1 `REQUEST CHANGES` 09:58:12Z on `7bdad6a` → `APPROVE` 10:08:15Z on `7de616e` | Yes | n/a — docs-only, V-DOC; 11+/7- in two files, no executable logic |
| #6 | R1 | R6 `REQUEST CHANGES` 09:56:38Z → `APPROVE` 10:05:22Z on `da43f03` | Yes | n/a — R6's own verdict; PM's 09:58:24Z read is a comment without a verdict line, by DL-025, and is not a turn. Merged `22887b0` at a head verified byte-identical to `da43f03` |
| #8 | R6 | R1 `REQUEST CHANGES` 09:56:52Z → `APPROVE` 10:01:43Z on `e468b4f` | Yes | n/a — docs and CI job names. Merged `b672281` on the same byte-identical rule |
| #9 | R1 | R6 `REQUEST CHANGES` 10:13:41Z → `APPROVE` 10:19:13Z on `d9cc868` | Yes | n/a — docs; one repair round, merged `3127a75` at the approved head |
| #10 | FE | R1 `REQUEST CHANGES` 10:12:23Z (mask two event types short) → `APPROVE` 10:17:41Z at `fa431c3` | Yes | n/a — ADR + one register line. Merged `74dab98` at a head whose ADR was byte-identical to `fa431c3` |
| #11 | R2 | R6 `REQUEST CHANGES` 10:16:28Z → `APPROVE` 10:23:46Z at `0dbf995` | Yes | Clause (a) and (b): R6 re-probed all four comparator paths and re-ran both recipes rather than reading the author's report |
| #13 | R1 | R6 `REQUEST CHANGES` at `24aed64` → `APPROVE` at `6f98328` | Yes | n/a — docs; three factual repairs, each re-verified by the reviewer from the primary source |
| #14 | R2 | R1 `APPROVE` at `32df864` | Yes | Clause (a): both recipes run by the reviewer, 178 rows audited for duplicates and no-ops, the strongest upstream claim re-derived from `Engine.cpp` |

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
3. **Seven deliverables have no backlog row** (§4). Recorded as a proposal, not a
   change.
4. **`docs/m0/` was created by #1, not by #9.** The first revision said #9 brought
   the directory and that #1 "also created files there"; `git log --diff-filter=A
   -- docs/m0` shows #1 (`f7bda19`) adding it with `PORTING-MAP.md`,
   `porting-map.tsv` and `ORACLE-PIN.json`, and #9 (`3127a75`) adding only
   `TURN-LEDGER.md`. The conclusion survives the correction — the two PRs share no
   file, so they were merge-order independent — but the premise was wrong, and a
   wrong premise that reaches a right conclusion is still a defect in the record.
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
7. **The first revision priced one review cycle two ways.** U1 charged R1's #8
   review at 2 turns for a `REQUEST CHANGES`→`APPROVE` pair; U8 charged R6's #11
   at 1 for the same pair; #2, #5, #6 and #9 were charged in no row at all; and #6
   was counted twice, once as task spend (T4) and once in U1. R6 caught it by
   adding the rows up rather than by reading them. The rule at the top of this file
   is the fix — one turn per signed verdict, counted from the API, applied to every
   lane — and it moves the milestone from 24 to 38. The failure is the one in item
   6: a plausible number, from a check applied unevenly.
