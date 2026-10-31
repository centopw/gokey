# macOS Accessibility — the permission flow

Implemented by MAC-001 (`crates/openkey-macos`). Decisions: ADR-0004
consequences 1, 2 and 6. This document is the MAC-001 acceptance artefact
"permission flow documented"; the code it describes is
`crates/openkey-macos/src/trust.rs` (states and copy) and
`crates/openkey-macos/src/tap.rs` (the read and the refusal).

## What is being asked for

GoKey converts keystrokes by reading them before the application does and
replacing the text. macOS allows that only through an **active `CGEventTap`**,
and an active tap requires the app to be trusted for **Accessibility** in
TCC. There is no prompt that can be triggered from code for this pane: the
system shows no dialog, `CGEventTapCreate` simply returns `NULL`.

Two axes are reported by the system:

| Axis | Symbol | Read by |
| --- | --- | --- |
| Accessibility | `AXIsProcessTrusted()` | the active tap, the only one that may swallow |
| Input listening | `IOHIDCheckAccess(kIOHIDRequestTypeListenEvent)` | a HID tap may additionally need Input Monitoring |
| Event posting | `IOHIDCheckAccess(kIOHIDRequestTypePostEvent)` | only relevant to emitting synthetic events (MAC-002) |

The trusted run recorded in ADR-0004 is what decides whether the HID location
needs the second axis as well. Until it runs, the app reports all three and
offers both panes; it does not claim to know which is sufficient.

## The three states

```text
accessibility granted                       -> granted
not granted, either HID axis denied         -> denied
not granted, both HID axes unknown          -> not determined
```

`not determined` is a **proxy**, and the code says so where it derives it:
Accessibility has no "has the user been asked?" query, so the HID answer is the
closest observable. The two states install no tap and open the same pane; they
differ only in what the banner says, which matters because `not determined` is
what every new user sees.

## What the user sees (S-09)

There is no modal at launch. The banner is persistent while the state is not
`granted`:

| State | Title | Link |
| --- | --- | --- |
| not determined | GoKey needs Accessibility access | Accessibility pane, plus Input Monitoring |
| denied | GoKey cannot convert keystrokes | Accessibility pane, plus Input Monitoring |
| granted | *no banner* | — |

Both non-granted banners state two things the user needs and one thing the app
promises:

- typing is unaffected — nothing is intercepted, so every key reaches the app;
- the exact pane, as a deep link:
  `x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility`;
- after granting, **no restart**: coming back to the window re-reads the state.

`trust::banner(state)` returns the copy and the links, so the UI layer renders
rather than rewrites them. The menu bar (S-08) shows the same state; a
menu-bar-only app has no other place to say "I am not working".

## Re-checking without a restart

`tap::access_report()` is three C calls and can be invoked as often as the UI
likes. The menu bar and the banner call it when they are shown; the adapter
reads it again on every tap installation. There is no cached boolean: a
revocation while the app runs must become visible the next time anything asks.

## The refusal is loud

`EventTap::install` returns `Err(TapError::NotTrusted)` when creation returns
`NULL` and Accessibility is not granted. The message names the pane and the
deep link, and the app must enter the denied state **before** it claims to be
running. The failure this rule exists to prevent is the worst possible one: an
IME that appears installed and enabled and types plain ASCII.

If creation returns `NULL` while Accessibility *is* granted, that is
`TapError::CreateFailed` — a different message, because it is not a permission
state and the user cannot fix it.

## Per-build grant, and what changes at M2

TCC keys the grant to the bundle identity, signing identity and install path. An
ad-hoc-signed rebuild is a new subject, so the user re-grants after each
dogfood build. This is expected until M2, which brings the stable self-signed
identity (ADR-0008 §2); the M3 rename is the next re-grant (DL-022). It is the
reason "grant disappears after updating" is not treated as a bug in M0/M1.

## Secure input

When a password field enables secure event input, the system stops delivering
key events to taps. That is correct behaviour, and it means the app is silent
exactly where silence is required. The verification case belongs to the AI-001
seam (MAC-007): type in a password field, return to a text field, assert no
stale buffer and no suggestion offer. It is listed here because it is a property
of this permission model, not of the suggestion feature.

## How to verify (MAC-001 recipe)

V-MAC is "build + install on a clean macOS 13+ machine, type the 20-sentence
script". MAC-001 covers capture, so the observable is different from MAC-002's:

1. `cargo run -p openkey-macos --bin mac001_probe -- 4` **without** the grant.
   Expected: `trust: denied` (or `not_determined` on a fresh profile),
   `RESULT=no-tap`, exit code 2, and typing in the focused app is unaffected.
2. Grant Accessibility to the probe binary in System Settings → Privacy &
   Security → Accessibility, run again.
   Expected: `tap installed`, then
   `captured=3 key_events=3 ... swallowed=0` and
   `RESULT=capture-passthrough-ok`.
   `key_events` is 3 and not 6 because key-up is deliberately unmasked
   (ADR-0004 §1.1) — that line is the mask check.
3. With the C-002 stub, `engine_faults` equals the key-down count: `Engine::apply`
   is `todo!()`. The three characters still reach the focused app. That is the
   panic-passthrough rule observed in a real process, not a unit test.
4. Capture in the matrix apps (TextEdit, Safari or Chrome, Terminal, VS Code,
   Excel if installed): the probe's `captured` and `key_events` counters rise
   while typing in each; nothing is swallowed. Capture is app-independent at the
   HID location, which is why this checks the *tap*, not the app.
5. Revoke Accessibility while the app is running: the next re-check reports
   `denied` and the banner returns. No restart.

The trusted run in ADR-0004 (swallow and re-emit) remains open and is what
closes DL-004; it is not replaced by step 2, which only proves capture.
