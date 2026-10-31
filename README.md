# GoKey

**GoKey** (Vietnamese: "Gõ Key") is a Rust rewrite of
[OpenKey](https://github.com/tuyenvm/OpenKey) — an open-source Vietnamese input
method currently shipped as two separate native apps (Objective-C/AppKit on
macOS, Win32 on Windows), with no working Linux build.

GPL-3.0. OpenKey and its author Mai Vu Tuyen are credited in `NOTICE.md`; the
product name is ours only in the sense that we do not use theirs (DL-010).

Maintained by `@centopw`. The project is planned, routed and verified by a
Waker program manager; every published artifact carries `centopw` (DL-020).

**Status: M0 — bootstrap.** No typing logic is implemented yet. This repository
holds the plan, role roster, workflow, parity contracts and a compiling crate
skeleton.

## Roadmap

| Phase | Scope | Status |
| --- | --- | --- |
| 1.0 | macOS only: full typing parity, menu bar, settings, universal dmg (ad-hoc signed; one-time Gatekeeper bypass documented) | Next |
| Post-1.0 | Windows, then Linux, on the same unchanged core | Planned |
| Later | AI assist: inline correction of the word at the caret (Tab accept / Esc dismiss), ghost completion, next-word prediction | Seams only in 1.0 |

Cross-platform readiness is a hard constraint even though 1.0 ships on macOS:
the engine stays OS-free, every adapter crate keeps compiling, and CI builds all
three targets. AI features are post-1.0, but the extension seams (context
provider, caret geometry, caret-anchored suggestion span, accept/dismiss events)
ship inert in 1.0 so the engine is never retrofitted. See DL-003, DL-014,
DL-018 and `docs/ARCHITECTURE.md` Section 10.

Work is planned in **turns with a verification recipe per task**, not in weeks
(DL-019): see `docs/PLAN.md` and `docs/WORKFLOW.md` Section 13.

## Why port it

Upstream duplicates its engine across two platform apps, compiles platform
keycodes into the engine, keeps options in process globals, and has no automated
tests. The port aims for one pure Rust core, thin native adapters, and a
differential test harness that proves behaviour matches the C++ engine — plus a
Linux build that upstream never finished.

Full rationale and measures: `docs/CHARTER.md`.

## Layout

```
crates/
  openkey-core/      pure engine: keys, options, charsets, macros (no I/O, no unsafe)
  openkey-platform/  Host and KeySource traits, AppId, capability model
  openkey-cli/       headless harness: replay scripts, convert text
  openkey-macos/     CGEventTap / InputMethodKit adapter + menu bar
  openkey-windows/   WH_KEYBOARD_LL + SendInput adapter + tray (TSF in v2)
  openkey-linux/     IBus engine (Fcitx5 in v2)
tests/corpus/        golden parity cases shared with the C++ reference build
docs/                charter, architecture, plan, team, workflow, risks, decisions
backlog.csv          task backlog (IDs match the issue tracker)
```

Adapter crates are empty on other platforms, so the workspace builds everywhere.
Crate names keep the `openkey-` prefix to stay traceable to upstream; the
product is GoKey.

## Quickstart

```sh
rustup show                 # toolchain pinned by rust-toolchain.toml
cargo check
cargo test --workspace
cargo run -p openkey-cli
```

CI (`.github/workflows/ci.yml`) runs the same commands on Ubuntu, macOS and
Windows, plus `cargo deny` against the GPL-compatible license allowlist.

Note: this scaffold has not been compiled locally on the machine that generated
it (no Rust toolchain installed there). Task K-001 makes CI the source of truth
for the first green build.

## Documents

| File | Purpose |
| --- | --- |
| `docs/CHARTER.md` | Objective, scope, constraints, stakeholders, success measures |
| `docs/ARCHITECTURE.md` | Crate graph, core contract, platform mechanics, capability matrix |
| `docs/PLAN.md` | Milestones M0–M6, turn budgets, gates, critical path |
| `docs/TEAM.md` | Roles, ownership, kickoff roster, RACI, decision rights |
| `docs/WORKFLOW.md` | Branching, review, CI gates, cadence, verification recipes, release checklist |
| `docs/RISKS.md` | Risk register with owners and triggers |
| `docs/DECISIONS.md` | Decision log, open items blocking milestones |
| `backlog.csv` | 69 tasks with owners, dependencies, turn estimates and verification recipes |
| `tests/corpus/README.md` | Parity corpus format and review rules |

## Licensing

GPL-3.0, inherited from upstream. See `LICENSE` and `NOTICE.md`. Any distributed
build must ship both and must make source available for that exact version.

## Next action

Start M0 against its 20-turn budget: staff R1, R2, R3, R4, R5, R6 and the macOS
spike (R7); the first three deliverables are the reference harness (K-003),
corpus v0 (K-004), and the macOS adapter decision (K-008). Each task closes only
when its verification recipe has been run and recorded.

Decisions confirmed: GoKey as the public name with a public repository (DL-010),
macOS-only 1.0 (DL-003, DL-014), suggest-don't-auto-correct AI interaction
(DL-018), turns as the planning unit (DL-019), `centopw` as public owner
(DL-020). Still open: AI feature order (DL-015), settings UI stack (DL-009),
macOS integration (DL-004).
