# Risk register

Severity = impact x likelihood. Reviewed weekly; owners update status, PM
chases. A risk without a trigger is not a risk, it is a worry.

| ID | Risk | Impact | Likelihood | Sev | Owner | Mitigation | Trigger / early warning |
| --- | --- | --- | --- | --- | --- | --- | --- |
| RK-01 | GPL-3.0 obligations not met (missing source, wrong license notice) or "OpenKey" name used without permission | High | Medium | High | R13 | LICENSE/NOTICE in repo from day one; name decision DL-010 before any public build; licence check in CI | Any public artifact or store listing planned |
| RK-02 | macOS input integration dead end: InputMethodKit forces marked text (the underline bug upstream avoids), CGEventTap needs Accessibility and may be restricted | High | Medium | High | R7 | Timeboxed spike K-008 in W1 with both prototypes; fallback defined before M2 | Spike shows >2 weeks of work or unstable typing in Electron apps |
| RK-03 | Windows hook approach blocked by antivirus or corporate policy; TSF is much larger than estimated | Medium | Medium | Med-High | R8 | Ship hook-based parity first; TSF scoped as v2 with a hard decision point (WIN-007) | AV false positives or blocker reports from >2 testers |
| RK-04 | Wayland forbids global capture; IBus/Fcitx5 integration turns out to be a different product shape (preedit, not backspace) | High | High | High | R9 | Accept that Linux must be an IME; prototype preedit strategy in M3, before M4 commitment; declare supported desktops | GNOME/KDE behave differently or apps ignore preedit |
| RK-05 | Silent behavioural divergence from the C++ engine (undocumented quirks in 52 KB of engine code) | High | High | High | R5 | Reference harness + >=2000-case corpus + differential fuzzing; parity gate blocks merge | Any unexplained diff in nightly fuzz |
| RK-06 | Platform specialists unavailable or single-person knowledge (bus factor 1) | High | Medium | High | R1 | Named secondary reviewer per adapter; ADRs for decisions; contractor bench identified before M3 | R7/R8/R9 unstaffed 2 weeks before milestone |
| RK-07 | Accessibility permission refused by users (macOS) or seen as spyware | Medium | High | High | R10 | In-product explanation, no telemetry, open source, visible off switch | Install-to-activation drop-off in beta |
| RK-08 | Rust/native interop build complexity: universal binaries, notarisation, FFI crashes | Medium | Medium | Medium | R6 | Build/release engineer from W1; unsigned artifacts in CI from W2; FFI boundary guarded | Any CI job taking >20 min or requiring manual signing steps |
| RK-09 | Scope creep: auto-update, sync, mobile, extra input methods | Medium | High | Medium | R1 | Explicit non-goals in charter; change control via sponsor; v2 ADR for each | New feature appears in a milestone without an approved change |
| RK-10 | Performance regression vs the C++ engine | Low | Medium | Low | R2 | Criterion benches in CI; allocation-free fast path; p99 budget | p99 > 0.5 ms in core or > 2 ms end-to-end |
| RK-11 | Application compatibility matrix is larger than expected (Electron, Office, CAD, terminals, Citrix) | Medium | Medium | Medium | R5 | Fixed compatibility matrix per platform, tested each milestone; document exceptions | More than 3 exceptions per platform |
| RK-12 | Upstream relationship turns negative (naming dispute, perceived takeover) | Medium | Low | Medium | R13 | Contact early, credit prominently, offer findings upstream, do not ship a binary until name is agreed | No reply within 3 weeks of first contact |
| RK-13 | Settings migration from upstream loses user configuration | Medium | Medium | Medium | R4 | Best-effort migration, never modify source config, show what was migrated | Reported lost macro or charset after upgrade |
| RK-14 | Keyboard layout assumptions break non-QWERTY users | Medium | Medium | Medium | R3 | Use layout-provided characters where available; test AZERTY/QWERTZ; document limits | Bug reports from non-QWERTY users |
