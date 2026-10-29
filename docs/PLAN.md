# Plan

Approach: parity first, then platforms. The core engine is proved against the
C++ reference before any native front-end is built, so platform work never
becomes the place where typing bugs are found.

## 1. Phases and milestones

Schedule is expressed in weeks from kickoff (W1 = first week of November 2026).
Dates are provisional until the sponsor signs off (DL-003).

| Milestone | Weeks | Content | Exit criteria (gate) |
| --- | --- | --- | --- |
| **M0 Bootstrap** | W1–W2 | Repo, CI, reference harness, corpus v0, ADRs, adapter spikes, licensing | G0: builds on 3 OSes; ref harness replays a script; >=300 corpus cases; macOS adapter decision; LICENSE/NOTICE in repo; sponsor approves charter |
| **M1 Core parity** | W3–W7 | Port Telex, VNI, Simple Telex, charsets, spelling/grammar, macros, smart switch, convert, options | G1: >=99.5% agreement on >=2000 cases, 100% documented examples; core p99 <= 0.5 ms; API frozen; zero `unsafe` in core |
| **M2 macOS alpha** | W8–W12 | Event tap adapter, focus tracking, menu bar, settings, notarised dmg | G2: installs on a clean macOS 13+ machine; two weeks of daily dogfooding by the team; compat matrix documented |
| **M3 Windows alpha** | W13–W17 | Hook capture, SendInput emission, tray + dialog, signed MSI | G3: signed installer; compat matrix (Office, browsers, IDEs, terminals); no AV false positive on VirusTotal reference set |
| **M4 Linux alpha** | W18–W22 | IBus engine, preedit strategy, packaging, desktop matrix | G4: types Vietnamese in GTK and Qt apps on GNOME and KDE; deb + rpm install cleanly on Ubuntu 24.04 and Fedora |
| **M5 Beta and 1.0** | W23–W28 | External beta, security review, docs, release | G5: release checklist in `WORKFLOW.md` complete; crash-free >=99.9% over 3 weeks; security sign-off |

## 2. Critical path

```
K-003 ref harness -> K-004 corpus v0 -> K-005 replay CLI
        -> C-001 word model -> C-002 Telex -> C-006 spelling/restore
        -> C-014 corpus 2000 + fuzz -> G1
        -> (parallel) K-008 macOS spike -> MAC-001 capture -> MAC-002 emission
        -> MAC-006 packaging -> G2 -> G3 -> G4 -> G5
```

Risk-retiring work that must not wait: the macOS adapter spike (K-008) starts in
W1, in parallel with core work, because it decides the product's permission
model. The reference harness (K-003) is on the critical path and is staffed from
day one.

## 3. Parallel workstreams

| Stream | Milestones | Owner |
| --- | --- | --- |
| Core engine | M1 | R3, R4 |
| Parity/QA harness | M0–M5 | R5 |
| macOS | M0 spike, M2 | R7 |
| Windows | M3 | R8 (joins M2) |
| Linux | M4 | R9 (joins M3) |
| Build/release/packaging | M0–M5 | R6 |
| UX | M2–M3 | R10 |
| Licensing/community | M0, M5 | R13 |

## 4. Task tracking

- Backlog lives in `backlog.csv` and mirrors the issue tracker (GitHub Projects).
- ID scheme: `K-*` bootstrap, `C-*` core, `MAC-*`, `WIN-*`, `LNX-*`, `REL-*`.
- Each task has: owning role, dependencies, estimate in person-days, and an
  objective acceptance statement. Estimates are planning values, not commitments.
- Anyone may add tasks; only the PM re-sequences a milestone.

## 5. Capacity and contingency

- Kickoff team: ~5.9 FTE; peak in M3–M4: ~7.5 FTE.
- Planned engineering effort in the backlog: roughly 240 person-days.
- Milestone windows carry roughly 35% contingency for platform API surprises;
  that contingency is the primary schedule buffer. Do not spend it early.

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

Windows TSF-native integration, Fcitx5 addon, mobile, sync, telemetry. Each has
a v2 candidate ADR and must not enter a 1.0 milestone without a scope change.
