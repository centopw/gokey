# ADR-0011 — Mouse and session entry: `Engine::session_break`, not `Phase`, not `reset()`

- **Status**: accepted (2026-10-31) — `VERDICT: APPROVE` at `8a26b67`, review
  5722226254 (https://github.com/centopw/gokey/pull/33#pullrequestreview-5722226254)
- **Date**: 2026-10-31
- **Author**: R2 (backend)
- **Task**: C-002 (engine), consumed by MAC-002 (macOS adapter)
- **Reviewers**: R2 for the C-002 verdict; FE as required input (advisory, no turn)
- **Decided by**: R2, under the C-002 review that signs this ADR with the engine
- **Supersedes**: nothing. Settles the question ADR-0004 §1.1 deliberately left open
  ("C-002 must choose — a third `Phase` arm, a mouse discriminator on `KeyEvent`,
  or a dedicated session entry point").
- **Related decisions**: DL-004, DL-013, DL-026

## Context

Upstream starts a new word on a click or drag, not on a keystroke. The tap masks four
mouse types — `kCGEventLeftMouseDown`, `kCGEventRightMouseDown`, `kCGEventLeftMouseDragged`,
`kCGEventRightMouseDragged` — and each one calls `RequestNewSession()`
(`OpenKey.mm:688-691`), which does two things (`OpenKey.mm:147-153`):

```objc
vKeyHandleEvent(vKeyEvent::Mouse, vKeyEventState::MouseDown, 0);
if (IS_DOUBLE_CODE(vCodeTable)) { //VNI
    _syncKey.clear();
}
```

The first line is **engine input**: the engine's break branch runs
(`Engine.cpp:1314-1341`; `isWordBreak` returns true for `vKeyEvent::Mouse`,
`Engine.cpp:145-147`), which can run the quick-consonant check, the
restore-if-wrong-spelling check, save or clear the word history, and start a new
session. The second line is **adapter state**: `_syncKey` counts characters already
emitted per keystroke, and it is cleared only for the double-byte output charsets
(`IS_DOUBLE_CODE(code) == (code == 2 || code == 3)`, `DataType.h:149` — VNI-Windows and
Unicode Compound). For Telex on Unicode one keystroke emits one character, so there is
nothing to clear.

ADR-0004 §1.1 recorded the consequence: calling `Engine::reset()` from the adapter would
collapse the engine half with the adapter half and lose the restore and quick-consonant
behaviour, so the entry point had to be decided before MAC-002 depends on it. It also
fixed the alternatives: a third `Phase` arm, a mouse discriminator on `KeyEvent`, or a
dedicated entry point.

Two further facts bound the decision:

- **The reference harness cannot press a mouse.** Corpus inputs are characters
  (`tests/corpus/README.md`); the oracle is driven per keystroke. Whatever shape is
  chosen, the mouse path has no parity case and must be pinned by unit tests in the
  adapter instead.
- **Key-up is already excluded.** The mask omits `kCGEventKeyUp` and upstream only
  dispatches on `kCGEventKeyDown` (`OpenKey.mm:713`); `Phase::Up` is carried so the
  adapter can honour that, not so the engine can act on it.

## Decision

**A dedicated method: `Engine::session_break() -> Outcome`** (`engine.rs:179-191`). It
sets `is_caps = false` (the mouse dispatch passes `capsStatus = 0`, so `_isCaps` is
false after `Engine.cpp:1319`) and calls `dispatch(Source::Mouse, 0, 0, false)`, then
returns the outcome. `Phase` keeps exactly `Down` and `Up`; `KeyEvent` still describes a
keystroke and nothing else. `Engine::reset()` remains the public alias of
`start_new_session()` (`engine.rs:168-177`) and is **not** the adapter entry.

### The contract MAC-002 must implement

1. **Call `session_break()` on exactly those four types, and pass the event through.**
   Upstream returns the original event unmodified after `RequestNewSession()`
   (`OpenKey.mm:690`); the click must reach the application. `otherMouse*` is not in the
   mask and must not be added — ADR-0004 §1.1 records that as a divergence, not a fix.
2. **Discard the returned `Outcome` on this path.** Upstream discards the return value of
   `vKeyHandleEvent` here, so even an `Action::Restore` produced by
   `restore_if_invalid` is dropped: the click ends the word, it does not retype it. The
   return value exists for tests and diagnostics, not for emission.
3. **Do not call `reset()` or `start_new_session()` instead.** `reset()` skips
   `break_branch`: no quick-consonant check, no `restore_if_invalid` check, no
   `special_char`/`typing_states` handling, no macro-break bookkeeping. The two are not
   interchangeable, and the difference is typing behaviour, which DL-013 forbids shipping.
4. **Keep the sync key adapter-side, and clear it only for the double-byte charsets.**
   `session_break()` does not touch it. MAC-002 clears its `_syncKey` equivalent for
   `Charset::VniWindows | Charset::UnicodeCompound` — those two variants and no others,
   exactly as `IS_DOUBLE_CODE` does — and leaves it alone for Telex/Simple Telex on
   Unicode. The predicate is written by variant, not by number (`Charset`'s declaration
   order makes VNI-Windows `2` and Unicode Compound `3` today, but a later enum edit
   would silently break a numeric match). MAC-002 pins the rule with the variant
   predicate, not an end-to-end typing case: only Unicode is ported until C-005.
5. **Do not restore caps across the click.** `session_break()` clears `is_caps`; the
   adapter must not re-apply a remembered shift state to the first character after the
   click.
6. **The engine-fault gate still applies.** A mouse event on a faulted engine passes
   through without calling it, the same rule `EngineHost` already applies to keystrokes
   (`engine_host.rs:112-114`). A click is never the event that retries a crashed engine.

### Caller rules that ride with the same call site

These are MAC-002's emission rules, pinned here because they are decided in the same
review and would otherwise be reinvented per adapter:

- **`Restore` / `RestoreAndBreak`:** emit the edit, then re-send the original key event
  when the key has no character mapping, and type `Outcome::passthrough` otherwise
  (`PORTING-MAP` §4.6, citing `OpenKey.mm:480-488`). `passthrough` is always after the
  edit.
- **`Process` with zero backspaces and empty text is a no-op, not a swallow**
  (`PORTING-MAP` §4.6).
- **Forward delete is a documented deviation.** `Key::Delete` is mapped from keycode 117
  deliberately — `mac.h` has no row for it, because upstream's `KEY_DELETE` is 51,
  backspace (`keymap.rs:106-110`). Upstream never names the key: keycode 117 falls past
  the break table and the space/delete branches into the generic "start and check key"
  branch (`Engine.cpp:1466-1484`), where it is inserted into the typing word as an
  unknown letter. The port instead treats it as a break
  (`engine.rs:243-256`): nothing is consumed, the caches are cleared, and the session
  starts over. The key is passed through either way; the difference is what the *next*
  keystroke emits, and upstream's choice corrupts a word buffer with a keycode the word
  model cannot represent. No corpus case can cover it (the harness cannot press 117), so
  the deviation is deliberate, bounded to this key, and recorded here rather than fixed
  in the port's main path.
- **`Phase::Up` returns `Action::Pass` with no edit** (`engine.rs:218-225`), matching the
  fact that upstream never dispatches key-up. MAC-002 does not need to filter key-up
  itself if it forwards it; it must not expect an edit.

## Alternatives rejected

- **A third `Phase` arm (`Phase::Mouse`).** `Phase` is the phase of one keystroke; a
  mouse has none. Every exhaustive match over `Phase` would grow an arm no key path can
  produce, and the replay harness — which turns corpus characters into `KeyEvent`s —
  would need a rule for a pseudo-key that is not a key. It also separates the two halves
  of `RequestNewSession` even less than `reset()` does, because it puts the adapter half
  back into the type the engine reads.
- **A mouse discriminator on `KeyEvent` (e.g. `key: Key::Mouse`).** Same problem, but
  worse: `Key` indexes the keycode tables (`word.rs::key_code`), and a variant that has
  no keycode makes every table lookup a special case. The word model would gain a letter
  it can never store.
- **`reset()` as the entry.** Loses the restore and quick-consonant half of the break
  branch, and hides the `_syncKey` rule at a call site that reads like "start over" —
  the collapse ADR-0004 §1.1 warned about.

## Consequences

- MAC-002 adds one method to the engine surface it holds (`KeystrokeEngine` or a narrow
  companion, its choice) and dispatches `NativeEvent::Mouse` to it. `EngineHost::handle`
  currently returns `Reaction::Pass` for mouse events (`engine_host.rs:103-110`); MAC-002
  replaces that arm with the call and keeps the pass-through.
- Tests owed by MAC-002, not by C-002: one click calls the entry exactly once and swallows
  nothing; a click after a wrong spelling produces no edit; a faulted engine is not
  called; the sync-key clear happens for charsets 2 and 3 only.
- C-002 owes the engine half only: `session_break` sets caps false, runs the mouse break
  branch, and returns an outcome — covered by
  `session_break_discards_the_word_and_starts_a_new_one` in `engine.rs`.
- `docs/ARCHITECTURE.md` section 6 (the adapter contract) is unchanged: the entry point is
  an engine API, not a change to the four quantities the parity harness diffs.
