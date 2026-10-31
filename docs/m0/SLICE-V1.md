# Slice V1 — "type Vietnamese, toggle with one key"

Owner: R1 (Project Administrator). Opened 2026-10-29. Decides when the sponsor's
minimum requirement is met; does not replace M1 parity.

## Why a slice inside a milestone

M1 is 44 task turns and delivers full parity. The sponsor's minimum — *type
Vietnamese and switch VI↔EN quickly* — does not need all 44. Standing that path up
early means the macOS adapter, the toggle and the permissions onboarding are
tested against a real engine while the rest of parity is still being ported, and
it means a wrong assumption about the event tap surfaces in week one rather than
at M2. The slice reorders work; it does not add it.

## Acceptance — run by hand, once, and recorded in this file

1. In TextEdit, typing `vieejt` produces `việt`.
2. Press **⌥Z**. The menu-bar item changes from **V** to **E**.
3. Typing `vieejt` again produces the literal `vieejt`: nothing transformed,
   nothing swallowed.
4. Press **⌥Z** again. `vieejt` produces `việt`.
5. The same toggle works in a second application (Terminal or Safari) without a
   restart.
6. With the tap denied (Accessibility not granted), the app shows the onboarding
   banner and does not eat keystrokes.
7. With the engine forced to panic on one keystroke, that keystroke passes
   through instead of disappearing.

**⌥Z is the upstream default, not a choice:** `DEFAULT_SWITCH_STATUS 0x7A000206`
(`Sources/OpenKey/macOS/ModernKey/AppDelegate.m:35`) is keycode `0x06` = Z with
bit 9 = Option (`Engine.h:69-74`).

## What each acceptance point proves

| # | Proves | Failure it rules out |
| --- | --- | --- |
| 1 | Engine produces the right `Edit` for a Telex word | A port that compiles but mistypes |
| 2, 4 | Adapter intercepts the hotkey and the engine reads language state | A toggle that only moves a menu icon |
| 3 | English mode is a true passthrough | A keyboard that mangles ordinary typing — the worst failure for an IME |
| 5 | Focus/state is process-global, not window-global | Toggle that dies on app switch |
| 6 | Denial is a UI state, not a silent no-op | "Installed and does nothing" (ADR-0004) |
| 7 | Engine bugs are survivable | A panic that eats the user's keystroke |

## Work, owners, and the turns already budgeted in M1

| Item | Owner | Backlog | Turns | Depends on |
| --- | --- | --- | --- | --- |
| Word model | R3 / Backend lane | C-001 | 2 | K-002 |
| Telex engine: `dd`, doubled vowels, `w`, five tones, removal cycle, restore | R3 / Backend lane | C-002 | 4 | C-001 |
| Language/enabled state in `Options`, testable headlessly | R3 / Backend lane | C-002, C-013 | (inside C-002) | C-001 |
| Event tap, key normalisation, Accessibility onboarding, re-entrancy guard | R7 / FE lane | MAC-001 | 3 | K-008 |
| Edit emission: backspaces + Unicode | R7 / FE lane | MAC-002 | 3 | MAC-001 |
| Menu bar item, ⌥Z, state indicator | R7 / FE lane | MAC-004 | 2 | MAC-001, MAC-002 |
| Corpus cases for every V1 rule | R5 / PM + Backend | K-004, C-007 | (inside above) | — |

**V1 is done when all seven acceptance points are recorded here with the date and
the lane that ran them.** A point that cannot be run (no Accessibility grant) is
recorded as blocked with the reason, not as passed.

## Ordering rule

MAC-001 begins now against the stub engine — the tap, the keycodes, the onboarding
and the guard do not depend on what the engine returns. MAC-002 and MAC-004 wire to
the real engine in the same turn C-002 lands. If C-002 lands first, that is fine;
the slice order exists to keep two lanes busy, not to gate one on the other.

## What this does not cover

Full parity (C-003…C-017), settings UI (MAC-005), smart switch per app (MAC-003),
macros (C-009), charsets beyond Unicode (C-005). Those remain M1/M2 work with their
own acceptance in `backlog.csv`.
