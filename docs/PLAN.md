# Plan

Approach: parity first, then platforms. The core engine is proved against the
C++ reference before any native front-end is built, so platform work never
becomes the place where typing bugs are found.

## 1. Unit of plan: the turn

Work is not scheduled in weeks (DL-019). One **turn** is one agent run that
starts from a prompt and ends in a **verified artifact increment**: code that
builds, tests that pass, and the evidence recorded on the task. A milestone is
done when its gate evidence exists, not when a date arrives.

Rules:

- Every task in `backlog.csv` carries `estimate_turns` and a `verify` recipe.
- A task closes when the recipe output is attached to the PR (see
  `WORKFLOW.md` Section 13). "Code written" is not closure.
- Two consecutive turns without a measurable improvement on the same task
  escalates to R2 (design) or R1 (scope), rather than a third attempt.

## 2. Milestones and turn budgets

Budgets include a 15% fix reserve. They are estimates, revised at each gate from
actual turns spent.

| Milestone | Turns | Content | Exit criteria (gate) |
| --- | --- | --- | --- |
| **M0 Bootstrap** | 20 | Repo, CI, reference harness, corpus v0, ADRs, adapter spike, licensing | G0: builds on 3 OSes; ref harness replays a script; >=300 corpus cases; macOS adapter decision; LICENSE/NOTICE in repo; charter approved |
| **M1 Core parity** | 51 | Port Telex, VNI, Simple Telex, charsets, spelling/grammar, macros, smart switch, convert, options; define AI seams with a caret-anchored span | G1: >=99.5% agreement on >=2000 cases, 100% documented examples; core p99 <= 0.5 ms; API frozen; zero `unsafe` in core; context/suggestion traits compile with no AI dependency |
| **M2 macOS alpha** | 24 | Event tap adapter, focus tracking, menu bar, settings, notarised dmg | G2: installs on a clean macOS 13+ machine; >=1,000 dogfood sessions; compat matrix documented |
| **M3 macOS 1.0** | 21 | Hardening, external beta, auto-update, docs, release | G3: 1.0.0 published for macOS; crash-free >=99.9% over 1,000 dogfood sessions; security sign-off; zero network calls outside the update check |
| **M4 Windows** | 21 | Hook capture, SendInput emission, tray + dialog, signed MSI | G4: signed installer; compat matrix (Office, browsers, IDEs, terminals); no AV false positive on the reference set |
| **M5 Linux** | 18 | IBus engine, preedit strategy, packaging, desktop matrix | G5: types Vietnamese in GTK and Qt apps on GNOME and KDE; deb + rpm install cleanly on Ubuntu 24.04 and Fedora |
| **M6 AI assist** | 49 | Inline correction (caret-anchored span, Tab/Esc), suggestion overlay, ghost completion, next-word prediction, evaluation, privacy gate | G6: correction precision >=95% with dismissal behaviour measured; completion acceptance rate measured; privacy review passed; no core or keystroke-path dependency |

1.0 is M0–M3: **116 turns**. M4–M6 are post-1.0 and run outside that budget.

Calendar dates are derived for humans when asked; they are never tracked. If a
date and a gate disagree, the gate wins.

## 2. Critical path

```
K-003 ref harness -> K-004 corpus v0 -> K-005 replay CLI
        -> C-001 word model -> C-002 Telex -> C-006 spelling/restore
        -> C-014 corpus 2000 + fuzz -> G1
        -> (parallel) K-008 macOS spike -> MAC-001 capture -> MAC-002 emission
        -> MAC-006 packaging -> REL-006 1.0 -> G3
```

Risk-retiring work starts in the first turns, not "later": the macOS adapter
spike (K-008) runs in parallel with core work because it decides the product's
permission model, and the reference harness (K-003) is on the critical path from
day one. Windows, Linux and AI stay off the 1.0 critical path.

## 3. Parallel workstreams

| Stream | Milestones | Owner |
| --- | --- | --- |
| Core engine | M1 | R3, R4 |
| Parity/QA harness | M0–M5 | R5 |
| macOS | M0 spike, M2–M3 | R7 |
| Windows | M4 (post-1.0) | R8 (joins at M4) |
| Linux | M5 (post-1.0) | R9 (joins at M5) |
| AI assist | M6 (post-1.0) | R14 (joins at M6) |
| Build/release/packaging | M0–M5 | R6 |
| UX | M2–M3 | R10 |
| Licensing/community | M0, M3 | R13 |

## 4. Task tracking

- Backlog lives in `backlog.csv` and mirrors the issue tracker (GitHub Projects).
- ID scheme: `K-*` bootstrap, `C-*` core, `MAC-*`, `WIN-*`, `LNX-*`, `REL-*`,
  `AI-*` assist.
- Each task has: owning role, dependencies, `estimate_turns`, a `verify` recipe
  (defined in `WORKFLOW.md` Section 13), and an objective acceptance statement.
- Turn estimates are planning values, not commitments. Actual turns spent are
  recorded per task, and the delta is reviewed at each gate.
- Anyone may add tasks; only the PM re-sequences a milestone.

## 5. Capacity and rework

- The working group is five Wakers: R1 program manager (Project Administrator),
  plus PM, backend, front-end and DevOps lanes; macOS platform work (R7) shares
  the backend lane until M2.
- Post-1.0 phases add one platform lane each; AI adds R14.
- Each milestone budget already carries a 15% fix reserve. A milestone that
  spends more than 25% of its budget on rework is re-planned at the gate, not
  silently extended.

## 6. Definition of a milestone demo

Every milestone ends with a scripted demo on a clean machine:

1. Fresh OS install or fresh user profile.
2. Install from the artifact produced by CI.
3. Type a fixed 20-sentence Vietnamese script (Telex and VNI), including tone
   edge cases, a macro, and an app switch.
4. Show latency numbers and parity report.
5. Show the known-limitations list.

A milestone without a demo is not complete.

## 7. Out of scope for this plan

Windows and Linux implementations (M4, M5), AI features (M6), and the v2
integration candidates (TSF, Fcitx5) are post-1.0. They must not enter M0–M3
without a scope change; only the inert seams defined in `ARCHITECTURE.md` ship
in 1.0.
