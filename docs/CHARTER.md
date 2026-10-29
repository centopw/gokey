# Charter — GoKey

**Project name: GoKey** (Vietnamese copy: "Gõ Key"). Public owner: Hiep Nguyen,
`@centopw` — every published artifact names him. Operating owner: the Project
Administrator (Waker R1), who runs plan, risk, routing and verification
(DL-020). GPL-3.0 derivative of [OpenKey](https://github.com/tuyenvm/OpenKey);
upstream is credited in `NOTICE.md` and the About dialog (DL-010).

## 1. Objective

Rebuild the OpenKey Vietnamese input method as one Rust engine with thin native
adapters for macOS, Windows and Linux — including a real Linux build, which
upstream does not have.

One sentence: **same typing behaviour as OpenKey on every desktop, from a single
testable core.**

## 2. Problem being solved

| Today (upstream) | Consequence |
| --- | --- |
| Engine logic duplicated across an Objective-C/AppKit app and a Win32 app | Every fix and rule change is implemented twice |
| Engine compiled against per-platform keycode headers | Behaviour drifts per OS; tests cannot be shared |
| Option state held in `extern int v*` globals | Not testable, not embeddable, not thread-safe |
| No automated test suite; ~148 open issues | Regressions surface only through user reports |
| Linux folder is a "coming soon" stub | No Linux support at all |
| Typing engine is C++ plus ObjC++ | Hard to attract contributors, unsafe to refactor |

## 3. Objectives and success measures

| ID | Objective | Measure | Target |
| --- | --- | --- | --- |
| O1 | Behavioural parity | Parity corpus (>=2000 cases) vs reference C++ build | >=99.5% agree; 100% of documented examples |
| O2 | One core | Platform-specific code share | Core contains zero OS calls; adapters < 20% of LOC |
| O3 | Cross-platform by design | 1.0 platforms / post-1.0 platforms | 1.0 ships macOS 13+ (arm64 + x86_64). Windows 10/11 and Linux (GNOME + KDE) follow on the same unchanged core |
| O4 | Latency | Keystroke decision time, p99 | <=2 ms end-to-end, <=0.5 ms in core |
| O5 | Stability | Crash-free sessions in beta | >=99.9% across at least 1,000 dogfood sessions |
| O6 | Privacy | Typed content logged or persisted | Zero, by design and review |
| O7 | Maintainability | New input method or charset | Implementable in one crate, behind tests |
| O8 | AI-ready, not AI-shipping | Extension seams present in 1.0 | Context provider, caret geometry, suggestion channel with a caret-anchored span, and accept/dismiss events defined and compile-tested, with no AI dependency in `openkey-core`; zero network surface in the 1.0 binary |

## 4. In scope for 1.0

- Input methods: Telex, VNI, Simple Telex 1, Simple Telex 2.
- Charsets: Unicode (precomposed), Unicode Compound, TCVN3/ABC, VNI-Windows, CP1258.
- Spelling check, grammar check, restore-key-on-invalid-word.
- Modern orthography, quick Telex, quick start/end consonant, `z f w j` initial consonants.
- Macros (including auto-capitalisation and macro use in English mode).
- Smart mode switching per application; per-app charset memory.
- Auto uppercase first character of a sentence.
- Convert tool (transcode text between charsets).
- Settings UI, menu bar control, global enable hotkey.
- Packaging, code signing/notarisation, signed dmg, vi/en user docs — **macOS only for 1.0**.
- Architecture seams for future AI assist: text-context provider, caret geometry,
  suggestion channel carrying a caret-anchored `range` + `replacement`, accept
  and dismiss events, `ai` settings section — all inert and off in 1.0.

## 5. Out of scope for 1.0

- Mobile (iOS, Android), browser extension, web demo.
- Cloud sync, accounts, user profiles.
- Windows and Linux implementations in 1.0 — they are post-1.0 milestones (M4, M5) on the same core.
- AI features in 1.0: inline correction of the word at the caret, ghost
  completion, next-word prediction, rewrite. The seams ship, the features do
  not (M6).
- Windows TSF-native implementation (v2 candidate, ADR-0005).
- Fcitx5 addon (v2 candidate, ADR-0006).
- Telemetry/analytics of any kind (privacy stance is a product feature).
- Linux distributions beyond the two supported reference images.
- Any change in the typing algorithm's user-visible behaviour, except where
  upstream documents a bug and the sponsor approves the divergence.

## 6. Constraints

| Type | Constraint | Impact |
| --- | --- | --- |
| License | GPL-3.0-only inherited from upstream | Whole port is GPL-3.0; no closed-source reuse; must ship LICENSE/NOTICE |
| Naming | Public name is GoKey (DL-010); "OpenKey" is not ours to use | Upstream credited in NOTICE; package/trademark clearance by R13 before the first public binary |
| Planning | Work is estimated and tracked in turns with recorded verification (DL-019) | No calendar estimate is a commitment; progress is turns spent vs turns budgeted, per gate |
| macOS | Event taps / accessibility APIs need user-granted Accessibility permission | Onboarding UX + security review required |
| Windows | Keyboard hooks raise antivirus false positives; distributables need Authenticode | Signing budget + AV submissions |
| Linux | Wayland forbids global key capture; must be an input-method engine | IBus/Fcitx5 integration, not a hook |
| Privacy | No network, no keystroke persistence | Rules out hosted spellcheck, crash auto-upload without opt-in |
| AI later | Assist features need text context, which upstream never reads | Context reading is a separate trust boundary: opt-in only, on-device default, never in secure fields; the 1.0 core must not depend on it |
| Keycodes | Upstream bakes platform keycodes into the engine | Adapters must normalise keys; parity harness must normalise too |

## 7. Stakeholders

| Role | Who | Interest |
| --- | --- | --- |
| Sponsor and public owner | Hiep Nguyen, `@centopw` (`hiep@hce.vn`) | Scope, staffing, release approval; every published artifact names him |
| Operating owner / program manager | Project Administrator (Waker R1) | Plan, risks, decisions, routing, verification evidence |
| Upstream author | Mai Vu Tuyen (courtesy contact) | Attribution, direction |
| End users | Vietnamese typists on macOS/Windows/Linux | Same behaviour, plus Linux |
| Existing macOS/Windows users | OpenKey install base | No regressions, migration of settings |
| Security reviewer | R12 (Waker) | Permission model, no keylogging surface |
| Community / contributors | Open-source Rust + Vietnamese IME community | Buildable, documented, welcoming |

Ownership rule (DL-020): the repository, releases, package names and any public
page carry `centopw`. The Waker identity appears only in internal routing, never
in a published document or commit author.

## 8. Assumptions (must be confirmed)

1. Staffing described in `TEAM.md` is available; the five-Waker working group
   (R1–R7 lane coverage) is in place at kickoff.
2. macOS is the first adapter, then Windows, then Linux (DL-003).
3. Upstream author is reachable and cooperative about attribution.
4. A reference C++ build of the engine can be compiled and driven headlessly.
5. Effort is planned in turns with a verification gate (DL-019); no external
   deadline forces a calendar commitment. Windows/Linux (M4–M5) and AI (M6) sit
   outside the 1.0 turn budget.
6. Settings migration from upstream is "best effort", not guaranteed lossless.

## 9. Governance

- Scope, schedule and staffing changes go through the decision log (`DECISIONS.md`)
  and require sponsor sign-off; the PM records them, the tech lead assesses impact.
- Milestone exit criteria in `PLAN.md` are the only definition of "done" for a phase.
- Anything that touches licensing, public naming, permissions, or user data
  requires the security/licensing reviewer before release.
