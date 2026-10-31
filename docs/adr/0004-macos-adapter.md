# ADR-0004 — macOS adapter: an active event tap in Rust, no ObjC++ host

- Status: proposed (DL-004 records the mechanism as decided 2026-10-29; this ADR becomes
  `accepted` when R2 signs it)
- Date: 2026-10-29
- Owner: R7 (macOS platform engineer; staffed interim by the R10 lane)
- Reviewers: R2 (technical lead)
- Supersedes: nothing. Answers K-008.
- Backlog: K-008 (spike), MAC-002 (adapter), DL-004, DL-013, DL-017

## Context

Upstream macOS is an Objective-C++ host around the C++ engine. `ModernKey/OpenKey.mm` holds a
`CGEventTapProxy` (`:70`), builds replacement key events and writes the character with
`CGEventKeyboardSetUnicodeString` (`:256-257`), and posts them with `CGEventTapPostEvent(_proxy, …)`
(`:258-259`). The typing model is therefore: **swallow the physical key at the tap, emit finished
text, and never show composition to the client app.** No marked text, no preedit, no underline.

The port has a C++-free core (`openkey-core`, pure by gate) and a macOS crate. Three questions had
to be answered before MAC-002 could be planned, and none of them could be answered from
documentation:

1. Does tap creation need Accessibility trust, and does it fail loudly or silently when it is absent?
2. Can a plain Rust process without a bundle, an `NSApplication`, or an ObjC++ shell own the tap?
3. How much FFI does that actually cost — is a host language the only way to keep it small?

K-008 answered them with a probe: `crates/openkey-macos/src/bin/k008_spike.rs`, 452 lines, one
commit (`6700aaa`), one file, **no manifest change and no new crates** — `std` plus raw
CoreGraphics/CoreFoundation/IOKit declarations. The spike is evidence; per `WORKFLOW.md` §1 spikes
are never merged, and the branch `spike/k-008-cgeventtap-20261029` stays as the re-runnable record.

## Decision

### 1. Mechanism: an active event tap at the HID location, owned by our process

The adapter installs a `CGEventTap` with:

| Parameter | Value | Why |
| --- | --- | --- |
| location | `kCGHIDEventTap` (0) | earliest point; also the only one where a synthetic self-test is indistinguishable from a real keystroke |
| place | `kCGHeadInsertEventTap` (0) | our decision is made before any other tap's |
| options | `kCGEventTapOptionDefault` (0), active | listen-only cannot swallow, and swallowing is the model |
| mask | key-down, flags-changed and four mouse-down/drag types — **not** key-up | the engine's input is not only typing; the mask is upstream's, and the two groups the spike omitted carry a toggle and a reset (§1.1) |

**§1.1 — The mask is the adapter's contract with upstream, and two thirds of it is not typing.**
Upstream masks seven event types (`OpenKeyManager.m:45-51`): `kCGEventKeyDown`, `kCGEventKeyUp`,
`kCGEventFlagsChanged`, `kCGEventLeftMouseDown`, `kCGEventRightMouseDown`, `kCGEventLeftMouseDragged`,
`kCGEventRightMouseDragged`. The spike masked one. That is a behaviour gap, not a simplification,
because the two missing groups carry engine input:

- **`kCGEventFlagsChanged` (`1 << 12`) is where the language switch lives.** Modifier keys never
  generate a key-down, so a modifier hotkey is invisible to a key-down-only tap: the toggle never
  fires, and `_lastFlag` / `_hasJustUsedHotKey` never update. Upstream also uses the release branch
  for the temporary switches — `vTempOffSpellChecking()` at `OpenKey.mm:653`, `vTempOffEngine()` at
  `:656` — so a ⌘ or ⌃ tap-and-release is a behaviour, not a no-op. A single *unmodified* hotkey
  still works through the key-down branch (`:615-633`), which is exactly why the gap reads as
  complete in a smoke test.
- **The four mouse types (`1 << 1`, `1 << 3`, `1 << 6`, `1 << 7`) start a new word.**
  `OpenKey.mm:689` calls `RequestNewSession()`, which sends a `Mouse / MouseDown` event into the
  engine (`:149`) and additionally clears `_syncKey` for VNI (`:151-153`). Without it, `a`, click,
  `s` yields `á` where upstream yields `s` — a typing-behaviour difference with no corpus case, which
  DL-013 forbids shipping. The set is exactly upstream's: `otherMouse*` is **not** masked, and adding
  it would be as much a divergence as omitting these four.

Key-up stays out. Upstream masks it (`1 << 11`) and then only reaches it in the type filter
(`:664-666`); the one dispatch to the engine is `kCGEventKeyDown` (`:713`), so nothing reads a
key-up. Excluding it costs no behaviour and one callback per keystroke.

The mouse path is **engine input, not an adapter-side flush**. `Engine::reset()` exists
(`crates/openkey-core/src/engine.rs:46`) and is documented as upstream `startNewSession`, but
upstream's mouse handler does more: the `_syncKey` clear is table-specific (`OpenKey.mm:151-153`).
Calling `reset()` from the adapter would collapse the two and lose half the behaviour. C-002 must
therefore choose — a third `Phase` arm, a mouse discriminator on `KeyEvent`, or a dedicated session
entry point — and because that changes the `openkey-core` API it needs its own ADR before MAC-002
depends on it. This ADR deliberately does not decide it.

**§1.2 — The location is a decision, not a description.** Upstream installs at
`kCGSessionEventTap` (`OpenKeyManager.m:53`); this ADR takes `kCGHIDEventTap`. It is the earliest
point, so a swallow there is final for every consumer downstream, including other input methods; a
synthetic event posted at HID level is indistinguishable from hardware, which is what makes the
self-test a fair test rather than a test of the session queue; and it is the location where an active
tap's swallow semantics are unambiguous. The cost is on the permission side: an active tap needs
Accessibility at either location, and HID-level listening may additionally require Input
Monitoring on current macOS. That is one of the things the trusted run measures —
`AXIsProcessTrusted()` printed next to `IOHIDCheckAccess(listen)` — and if HID turns out to need a
second grant for no behavioural gain, the fallback is upstream's session location with nothing else
changed: the mask, the dispatch and the guard are identical at both.

The SDK header still says what the permission checks cannot. `CGEvent.h:269-270` states that a tap
may be placed at `kCGHIDEventTap` only by a process running as root, and that `NULL` is returned for
other users; the Accessibility and Input Monitoring grants are the supported route past that
requirement. `CGEvent.h:273-279` adds the second fact that shapes the acceptance test: when the tap
is not permitted to monitor key events, the relevant mask bits are cleared at creation, and `NULL`
is returned only if that leaves an empty mask. A non-`NULL` tap therefore does **not** prove that
keystrokes are being seen — the trusted run's typed sentence does, with `key_events` as the
observable.

Listen-only is not a fallback. A listen-only tap would let the physical key through *and* emit the
converted character, producing `as` → `aá`. The tap is active or the IME is not running; there is no
configuration in which a partially working adapter is preferable to a visible "not permitted" state.

### 2. The adapter is Rust with raw FFI; there is no ObjC++ host

13 `extern` functions and one `extern static` (`kCFRunLoopDefaultMode`) in three
`unsafe extern "C"` blocks cover the whole platform surface: 6 functions from ApplicationServices,
6 from CoreFoundation plus that static, and 1 from IOKit (`IOHIDCheckAccess`, used only for
reporting trust state). Counted at `d56b160` with
`awk '/^mod ffi/,/^}/' crates/openkey-macos/src/tap.rs | grep -cE 'pub fn'` (13) and the same range
with `pub static` (1). All of the FFI lives in this one file; `key_from_keycode` and the callback are
the only logic that knows it is on macOS.

An ObjC++ host exists upstream because the UI is ObjC++ — not because the tap needs it. Keeping the
adapter in Rust means the callback, the keycode table and the engine call are in the same language as
the engine, the whole path is covered by `cargo clippy`, and the macOS build cannot drift into a
second toolchain. It also means the eventual Windows and Linux adapters have a same-shaped
counterpart instead of a platform-specific shell that must be re-derived.

### 3. The emit path

Inside the callback we emit through the tap **proxy**, as upstream does
(`CGEventTapPostEvent(_proxy, event)`), because that delivers downstream of our own tap and is
therefore immune to re-entry by construction. The spike used `CGEventPost(kCGSessionEventTap, …)` and
stamped every emission anyway: with a HID-level tap that pair happens not to re-enter, but with
`--session` the tap and the post share a level, and a HID-level post re-enters directly. The guard in
§5 has to hold for every posting location, so it was written before the trusted run rather than after
a loop was observed. `CGEventPost` at the HID location remains only where an event must enter at the
top — the self-test, and nothing in production.

Characters are written as UTF-16 with `CGEventKeyboardSetUnicodeString` on a keycode-`0` event, not
as keycode-plus-modifier combinations. That is layout-independent: a US layout still produces `ữ`,
and we are not obliged to own a second table mapping Vietnamese characters to keystrokes the user's
layout may not have.

### 4. Keycode tables stay adapter-owned

`openkey-core` never sees a macOS keycode. `Key::A` … `Key::Other(0x7e)` is the core vocabulary;
`0x00 → Key::A` lives in `openkey-macos` next to the callback that uses it. This is the same split
upstream makes (`platforms/mac.h` vs the engine) and it is what keeps the `core-purity` gate honest:
a core that knew `0x00` is `A` would be a core that only compiles on macOS, which is the state
ADR-0008 exists to prevent.

The table must cover the full ANSI set plus the keys the engine's hotkey matching reads, and it needs
a unit test per row — a wrong digit in a 62-row table is silent, and the symptom (one dead key) looks
like an engine bug.

### 5. The `kCGEventSourceUserData` guard is load-bearing

Every synthetic event we create is stamped with `kCGEventSourceUserData = 0x474B3031` ("GK01")
and the callback returns such events untouched before it reads anything else. (The spike used
`0x4B303038` "K008"; the adapter's value is `capture::ECHO_MAGIC` and is what the guard must keep in
one place.)

```rust
if CGEventGetIntegerValueField(event, FIELD_USER_DATA) == MAGIC {
    return event;
}
```

This is not defensive decoration. Any emission that reaches HID level re-enters our own tap, and the
consequence is not a duplicated glyph: the tap would swallow its own output, feed it back into the
engine's buffer, and corrupt composition state from inside the callback. The paths that can reach HID
level are real — the self-test, and any future replay (backspace bursts for inline correction,
key-repeat) — so the guard is what makes the hazard impossible rather than unlikely. It must survive
the engine swap and every later refactor of `emit`.

## Evidence (K-008)

Environment: macOS 26.6.2 (25G83), arm64, rustc 1.99.0, Apple clang 21.0.0. Branch
`spike/k-008-cgeventtap-20261029` at `6700aaa`; gates on that commit were `fmt --check` 0,
`clippy -D warnings` 0, `core-purity` ok, `no-network` ok, `openkey-core` untouched.

### 1. The trust state was measured, not assumed

```
AXIsProcessTrusted() = false  (Accessibility)
IOHIDCheckAccess(post)   = 1 denied
IOHIDCheckAccess(listen) = 2 unknown
```

`IOHIDCheckAccess` returns `0` granted, `1` denied, `2` unknown (never asked). Listening access was
never requested, so it reads `unknown`; posting access is `denied`, which is the axis that matters for
emission.

### 2. `CGEventTapCreate` returned `NULL` in all four combinations

```
hold HID active    (args: [])
  tap option = active      tap location = 0   CGEventTapCreate -> NULL   RESULT=no-tap
HID listen-only    (args: [--listen-only])
  tap option = listen-only tap location = 0   CGEventTapCreate -> NULL   RESULT=no-tap
session active     (args: [--session])
  tap option = active      tap location = 1   CGEventTapCreate -> NULL   RESULT=no-tap
session listen-only(args: [--session --listen-only])
  tap option = listen-only tap location = 1   CGEventTapCreate -> NULL   RESULT=no-tap
```

Two things make this a finding rather than a shrug: the symbol resolved and the call returned
cleanly, so `NULL` is a refusal, not an ABI mistake; and the same result on a listen-only tap shows
the refusal is about trust, not about the active option.

**Ad-hoc signing with the hardened runtime changes none of it**, and the reason is worth recording:
the copy re-signed `codesign --force -s - --options runtime` reports
`flags=0x10002(adhoc,runtime)` and still `AXIsProcessTrusted() = false`, still `NULL`. A signature
gives TCC a stable subject to grant to; it is not the grant.

### 3. The emit ABI was proven without a tap

`--emit-probe` builds an event, stamps the guard, writes the string, and reads it back through the
same getter the callback uses:

```
emit-probe "á" -> "á" units=1 guard=0x4b303038 ok
emit-probe "ơ" -> "ơ" units=1 guard=0x4b303038 ok
emit-probe "đ" -> "đ" units=1 guard=0x4b303038 ok
emit-probe "ữ" -> "ữ" units=1 guard=0x4b303038 ok
RESULT=emit-abi-ok
```

This is deliberately separate from the tap result. It proves the declarations, the UTF-16 handling and
the `kCGEventKeyboardSetUnicodeString` argument order are right, so a later failure is a trust
problem and not a suspected FFI bug. It does **not** prove that posting works — nothing is delivered
in this probe — which is exactly what the open verification below is for.

### What this evidence does not establish

- Which grant is required. Accessibility was absent and Input Monitoring was never granted, so the
  four `NULL`s cannot separate the two. The trusted run separates them by reading the same three
  lines with the grant in place.
- That swallowing and re-emission work end to end. The code path exists and the self-test injects at
  HID level, but the tap never installed.
- Behaviour in a real app, in secure input mode, or with a stale trust record after a rebuild.
- Anything about the six event types the probe did not mask. It watched key-down only, so none of
  these runs say whether flags-changed or mouse events are delivered under trust. §1.1 comes from
  upstream's mask and callback (`OpenKeyManager.m:45-51`, `OpenKey.mm:615-700`), not from the probe,
  and it is the correction the probe's scope made necessary.

## Open verification: the trusted run

This is the one measurement K-008 could not take, because it needs a human at the Accessibility pane.
It is recorded here so it is not lost with the branch. **It is the last step between the decision and
MAC-002.**

```
# 1. build the probe from the spike branch (it is not and will not be on main)
git worktree add /tmp/k008 spike/k-008-cgeventtap-20261029   # or: git checkout 6700aaa
cd /tmp/k008 && cargo build -p openkey-macos --bin k008_spike

# 2. grant Accessibility to the binary, once:
#    System Settings → Privacy & Security → Accessibility → "+" → target/debug/k008_spike
#    (or drag the binary in; add the binary, not the terminal)

# 3. run it for 3 seconds
./target/debug/k008_spike 3
```

Expected, on a machine where step 2 took effect:

```
AXIsProcessTrusted() = true  (Accessibility)
IOHIDCheckAccess(post)   = 0 granted           # Accessibility covers posting; 1 here means the grant did not take
CGEventTapCreate -> 0x…                        # a live mach port pointer
swallowed=3 emitted=3
RESULT=swallow-and-reemit-ok
```

`swallowed=3 emitted=3` is the claim: three synthetic HID-level keystrokes (`a`, `s`, `space`) were
swallowed by our tap, the third one's engine stand-in produced `á`, and all three were re-emitted.
If `RESULT=tap-installed-no-traffic` appears instead, the tap installed but the self-test did not
reach it; if `RESULT=no-tap` persists after the grant, that is a finding about the grant subject (path
vs binary identity) and belongs in this ADR, not in the backlog.

The record to paste back into this ADR: macOS version, the five printed lines, and the two counts.

## Why InputMethodKit is rejected for 1.0

IMK is the supported way to be a macOS input method, and it is the wrong mechanism for this port.

1. **It forces marked text.** An IMK input method communicates through `NSTextInputClient`; anything
   the user has not committed is *marked text* — drawn by the client app, underlined, with its own
   selection semantics (`setMarkedText:selectedRange:replacementRange:`). That is the composition
   model upstream deliberately does not have. Adopting it would change what the user sees mid-word on
   every keystroke, which is a typing behaviour change with no corpus case behind it (DL-013), and it
   contradicts DL-017. The parity budget is not spent on an implementation convenience.
2. **It changes how the app is activated.** IMK requires an input method bundle in
   `~/Library/Input Methods`, registration (logout or a re-login for the first install), and selection
   in the input-source switcher. Upstream is a background agent whose toggle is global. Under IMK the
   IME sees keystrokes only while it is the selected source per app, so the "always installed, enabled
   by hotkey" behaviour would have to be redefined — the second, quieter parity break.
3. **It does not remove the hard parts.** Marked text would still have to be committed into the
   client; the settings UI, the menu bar, the trust prompt for any non-IMK path and the AI overlay all
   remain. What it buys is Apple's insertion path, for apps that implement IMK correctly — and the
   ones that do not (a share of Electron and Java apps) are exactly where a tap still wins.
4. **Hybrid (IMK front-end, tap for the rest)** was considered and rejected: two mechanisms for one
   behaviour means two failure modes, two code paths to test, and the marked-text artefact still
   appears wherever IMK is used. It would not be parity with upstream anywhere; it would be parity in
   some apps and a different product in others.

The cost of rejecting IMK is the Accessibility/Input Monitoring grant, and that cost is now a
measured, visible product requirement rather than a surprise — see the first two consequences.

## Alternatives considered

1. **Listen-only tap plus post** — cannot swallow; produces doubled characters. §1.
2. **`CGEventPost(kCGHIDEventTap)` for emission instead of the proxy form** — re-enters the tap and
   makes the §5 guard the only thing between us and a state-corrupting loop. The proxy form is what
   upstream uses; use it.
3. **ObjC++ adapter crate around the Rust core** (the upstream shape) — second toolchain, second
   lint surface, and the only thing gained is ObjC++ that we do not need. §2.
4. **InputMethodKit** — §"Why InputMethodKit is rejected".
5. **Accessibility API instead of a tap** — `AXUIElement` cannot intercept keystrokes; it reads UI
   state. Not an option for an IME, only for the future overlay's caret lookup.

## Consequences

**Positive**

- The typing model is upstream's: swallow, convert, emit finished text. No marked text, no preedit,
  no client cooperation required, works in any app that accepts keystrokes.
- The whole adapter is one Rust file plus a table. Clippy, `fmt`, and the purity gates see all of it.
- The failure mode is measurable and fundable: the trust state is three integers we can print.

**Negative / accepted — these are product requirements, not notes**

1. **TCC is a first-class UI state.** The app has at least three states — not determined, denied,
   granted — and they belong in S-09's banner, not in a one-shot modal at launch. Not determined is
   the state that matters: it is what a new user sees, and the banner must offer the deep link into
   System Settings → Privacy & Security → Accessibility and then re-check, without restarting. The
   menu bar reflects the same state (S-08), because a menu-bar-only app has no other place to say
   "I am not working".
2. **The app must fail loudly rather than install and do nothing.** A denied active tap returns
   `NULL` with no dialog and no error. Treating that as an ordinary error path produces the worst
   possible outcome: an IME that appears installed, appears enabled, and types plain ASCII. Startup
   must detect `NULL` and enter the denied state before it claims to be running.
3. **The mask is upstream's, and two thirds of it is not typing.** §1.1. Flags-changed carries the
   language toggle and both temporary switches; the four mouse types start a new word. A
   key-down-only tap passes a smoke test and fails the first hotkey and the first mid-word click, in
   that order of discovery. The callback dispatches on event type from the first commit, not after a
   bug report.
4. **The keycode table is adapter-owned and tested.** §4. Core stays layout-neutral; the gate that
   enforces it (`core-purity`) is the reason not to take the shortcut.
5. **The `kCGEventSourceUserData` guard is load-bearing.** §5. It is the only thing that stops our
   own emission from being reprocessed, and the corruption it prevents is silent.
6. **The Accessibility grant is per build until M2.** TCC keys the grant to the bundle identity,
   signing identity and path, so an ad-hoc-signed rebuild is a new subject and the user re-grants.
   This is the dogfood cost ADR-0008 §2 already pays for the artefact job; it is the reason M2 brings
   the stable self-signed identity, and DL-022's rename window is the M3 re-grant.
7. **The adapter must re-enable a tap the system disables.** `kCGEventTapDisabledByTimeout` and
   `…ByUserInput` arrive *as events* in the callback, not as errors. Ignoring them leaves an IME that
   worked minutes ago and now types plain English with no visible cause. MAC-002 owns this; it is
   listed here because an ADR that says "tap" and omits it would be describing a prototype.
8. **Secure input is the one place we must be silent.** When a password field enables secure event
   input, the system stops delivering key events to taps. That is the correct behaviour (and the
   suggestion overlay's secure-field rule assumes it), but it must be *verified* rather than assumed,
   and the engine buffer must be dropped on the transition so no stale composition leaks into the
   first character typed after the field loses focus.

**Neutral**

- `IOHIDCheckAccess` is the only IOKit symbol, and it exists for reporting. If it ever becomes
  load-bearing for behaviour, that is a signal the trust model is being guessed at instead of read.

## Follow-up tasks

- **MAC-002** — the adapter: tap installation with the seven-type mask of §1.1 and dispatch on event
  type (key-down → engine key event; flags-changed → toggle and temporary switches on release; mouse
  down/drag → new session), `DisabledByTimeout`/`ByUserInput` recovery, proxy-form emission, keycode
  table with per-row tests, trust-state machine emitting S-09's banner state.
- **The trusted run** above, recorded back into this ADR. Blocks MAC-002's acceptance, not its start.
- **C-002** — `Engine::apply`: the spike's stand-in is one call site and is deleted when the real
  engine lands. Its mouse/session half is a core-API decision (§1.1) and needs its own ADR if it
  changes `Phase` or `KeyEvent`; MAC-002 depends on there being an entry point, not on its shape.
- **S-09 / S-08** — banner and menu-bar rendering of the three trust states (the UI half of
  consequence 1). Owned by the same lane.
- **Secure-field verification** — one manual case: type in a password field, return to a text field,
  assert no stale buffer and no suggestion offer. Feeds the AI-001 seam tests.
