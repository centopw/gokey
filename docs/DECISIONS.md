# Decision log

Open decisions block the milestone they affect. Owners chase; the PM records
outcomes and links the ADR.

| ID | Decision | Options | Recommendation | Status | Owner | Needed by |
| --- | --- | --- | --- | --- | --- | --- |
| DL-001 | License | GPL-3.0-only vs GPL-3.0-or-later | GPL-3.0-only: inherit exactly what upstream grants; do not widen it unilaterally | Proposed | Sponsor + R13 | Before first public push |
| DL-002 | Toolchain and MSRV | Stable with MSRV 1.85 | Edition 2024, stable channel, MSRV pinned in `Cargo.toml`; bump only with an ADR | Proposed | R2 | M0 |
| DL-003 | Platform order | macOS first / Windows first / all in parallel | macOS (dogfood + reference behaviour), Windows, then Linux. Parallel adapters would multiply unknowns before the core is proven | Proposed | Sponsor | M0 gate |
| DL-004 | macOS integration | CGEventTap vs InputMethodKit vs hybrid | Decide after spike K-008; CGEventTap reproduces upstream behaviour, IMK integrates with the input-source switcher | Open | R7 | End of W1 |
| DL-005 | Windows integration | Low-level hook v1 / TSF now / hook then TSF | Hook v1 for parity, TSF v2 after user validation | Open | R8 | M2 |
| DL-006 | Linux integration | IBus / Fcitx5 / both | IBus first (GNOME default), Fcitx5 addon v2; use preedit, not backspace synthesis | Open | R9 | M3 |
| DL-007 | Core state model | Keep upstream globals / owned `Engine` + `Options` | Owned struct; globals make parity testing and multi-instance use impossible | Proposed | R2 | M0 |
| DL-008 | Settings format | JSON / TOML / platform-native | JSON via serde in per-OS config dir; one-way migration from plist and registry | Proposed | R4 | M1 |
| DL-009 | Settings UI stack | Per-platform native / Slint / Tauri / egui | Undecided; weigh bundle size, accessibility, and menu-bar/tray integration. Do not block core work | Open | R10 + R2 | M1 |
| DL-010 | Public name and trademark | Keep "OpenKey" with permission / new name | New name unless the upstream author grants use in writing; GPL does not grant marks | Open | Sponsor + R13 | Before any public binary |
| DL-011 | Parity definition | 100% / >=99.5% + documented exceptions | >=99.5% on the corpus, 100% on documented examples, linguistics sign-off on every exception | Proposed | R2 + R11 | G1 |
| DL-012 | Auto-update in 1.0 | Ship / defer to 1.1 | Ship a signed, opt-in-visible update check in 1.0; it is a security feature for an input method | Proposed | Sponsor + R6 | M4 |
| DL-013 | Divergence policy for known upstream bugs | Reproduce / fix | Reproduce by default; fix only with sponsor approval, corpus note and changelog entry | Proposed | R2 | M1 |
