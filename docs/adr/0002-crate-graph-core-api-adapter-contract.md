# ADR-0002 — Crate graph, core API and adapter contract

- **Status**: proposed
- **Date**: 2026-10-29
- **Author**: R2 (technical lead)
- **Task**: K-002 (`backlog.csv`)
- **Acceptance**: boundary review by R3 and R4 (K-002 acceptance cell)
- **Ratifies**: `docs/ARCHITECTURE.md` sections 2–4, which have carried a
  "pending ADR-0002 sign-off" status line since bootstrap (`ARCHITECTURE.md:3`)
- **Related decisions**: DL-002, DL-007, DL-008, DL-011, DL-013, DL-014, DL-018, DL-019, DL-021

## Context

1. **Two tasks are blocked on this one.** `backlog.csv` declares
   `C-001 depends_on=K-002` and `C-013 depends_on=K-002`: the word model and the
   settings schema cannot be written against an API that is still a proposal.
   C-017 then freezes the public surface, so every type decided here is
   expensive to change afterwards.
2. **The parity gate defines what the API must be able to express.** The
   harness diffs four quantities per keystroke — rendered text, action,
   backspace count, beep (`ARCHITECTURE.md` section 6) — and parity is the merge
   gate (DL-011, `WORKFLOW.md` section 7). A core type that cannot represent one
   of those four is untestable by the gate; a type that leaks a platform detail
   moves the decision into three adapters and out of the corpus.
3. **Three scaffold types cannot express the port.** `Engine::buffer: Vec<char>`
   discards pending tone/mark state that `insertW`, `handleModernMark` and
   `checkSpelling` read before rendering; `Edit.text: String` cannot carry
   byte-oriented charsets; `Action` has no `ext` axis, so the Windows
   autocomplete workaround and per-app line-length bookkeeping cannot be
   expressed at all. Sections D3–D5 record the evidence.
4. **One interface, three adapters.** 1.0 is macOS only (DL-003, DL-014), but
   `openkey-platform` and every adapter compile against this surface from day
   one (`ARCHITECTURE.md` section 2), so it is decided once, now, rather than
   rediscovered per platform.
5. **The AI boundary is negative for this ADR.** AI seams ship inert in 1.0
   (DL-018, `WORKFLOW.md` section 12). What this ADR must guarantee is that
   `openkey-core` stays free of suggestion and context types; the caret-anchored
   `Suggestion` type itself belongs to AI-001 and ADR-0007.

Upstream references in this ADR are to the pinned OpenKey source
(`89c2fd3bf258562f2349f89b49d81e2f140c3fc3`).

## Decision

### D1 — Crate graph and dependency direction

Accept `ARCHITECTURE.md` section 2 as normative:

- `openkey-core` depends on nothing in this workspace and never on
  `openkey-platform` or an adapter.
- `openkey-platform` depends on core; adapters and `openkey-cli` depend on both.
- `openkey-ai` is post-1.0, is never a dependency of core, and is never linked
  into the 1.0 binary.

Enforcement available today: `#![forbid(unsafe_code)]` in core plus V-CORE's
"no `unsafe`, no `cfg(target_os)` in core" pass condition (`WORKFLOW.md`
section 13), and review. A CI check on core's resolved dependency list is named
as a follow-up for C-017, not claimed as existing.

### D2 — The public surface of `openkey-core`

After this ADR the crate exposes exactly:

```rust
pub use engine::{Engine, Letter, Mark, Word};
pub use event::{Action, Edit, Encoded, Ext, Key, KeyEvent, Mods, Outcome, Phase};
pub use options::{Charset, InputMethod, Options};
```

Rules:

- Anything not on this list is private. `Engine` exposes state through
  accessors (`options()`, `buffer()`), never through fields.
- `Word`/`Letter`/`Mark` are public because C-001's acceptance is unit tests
  over the typed model and K-005's diff report benefits from pre-render state;
  they are not an invitation for adapters to manipulate the buffer directly.
- This is the surface C-017 freezes. Adding to it later is a semver event.

### D3 — R-API-1: typed word model (C-001)

```rust
pub const MAX_LETTERS: usize = 32;             // MAX_BUFF, DataType.h:19

pub struct Letter {
    pub key: Key,          // CHAR_MASK        0x0000_FFFF (DataType.h)
    pub caps: bool,        // CAPS_MASK        0x0001_0000 (:92)
    pub circ: bool,        // TONE_MASK        ^ : â ê ô
    pub w_mod: bool,       // TONEW_MASK       w : ă ơ ư
    pub mark: Mark,        // MARK_MASK        sắc huyền hỏi ngã nặng
    pub standalone: bool,  // STANDALONE_MASK  0x0100_0000 (:117)
    pub resolved: bool,    // CHAR_CODE_MASK   0x0200_0000 (:120)
}

pub struct Word {
    letters: [Letter; MAX_LETTERS],   // TypingWord[32]
    len: u8,                          // _index
    overflow: Vec<Letter>,            // _longWordHelper, dropped from the front
}
```

- `circ` and `w_mod` stay **two booleans, not one enum**, because upstream sets
  them independently (`insertAOE` clears `TONEW` at `Engine.cpp:838`, `insertW`
  clears `TONE` at `:877`) and `getCharacterCode` gives `TONE` precedence
  (`:536-539`). Two bools make that ordering explicit instead of implicit in a
  bit layout.
- Overflow semantics are reproduced exactly: pushing at `len == 32` drops the
  **front** letter into `overflow` and shifts left (`Engine.cpp:350-356`);
  delete shifts the last overflow letter back in at the front
  (`Engine.cpp:1436-1447`). Kept as a shift until the corpus is green
  (`ARCHITECTURE.md` section 6, Phase A), with the API stable either way.
- `Word` is the only type that indexes `letters`; everything else reads through
  `get`/`span`/`tail_matches`. This is grep-able and is part of C-001's
  acceptance.
- `to_bits()`/`from_bits()` may exist for reasoning, but **bit equality is not
  an oracle**: the low 16 bits hold an OS keycode upstream (`platforms/*.h`
  differ per OS), so cross-platform bit comparison is invalid. The harness
  compares rendered text.

### D4 — R-API-2: `Edit.text` is encoded, not `String` (C-005)

```rust
pub enum Encoded {
    Utf16(Vec<u16>),
    Bytes(Vec<u8>),
}

pub struct Edit {
    pub backspaces: u8,      // upstream backspaceCount
    pub text: Encoded,
    pub beep: bool,
}
```

- `Tcvn3`, `VniWindows` and `Cp1258` are byte-oriented and do not round-trip
  through UTF-8; `UnicodeCompound` needs two code units per vowel (base plus
  combining). C-005 owns the tables, but this field decides whether they can be
  carried at all, and changing it after C-005 lands is a breaking API change.
- Encoding happens in `openkey-core::charset`, driven by `Options.charset`, on
  the way out of `apply`. The engine works on abstract letters; **no adapter may
  hold a charset table**, which is what stops three adapters from disagreeing
  about TCVN3.
- `backspaces` keeps upstream's meaning: the count of deletion events the
  adapter must emit. It is deliberately *not* reinterpreted as grapheme or byte
  counts; platforms that delete by grapheme translate at the adapter, and the
  parity risk that creates is recorded under Consequences.

### D5 — R-API-3: the `ext` axis (C-002)

```rust
pub enum Ext { Normal, Break, Delete, SuppressEmpty }

pub struct Outcome {
    pub action: Action,               // Pass | Process | BreakWord | Restore | Macro | RestoreAndBreak
    pub ext: Ext,
    pub edit: Option<Edit>,
    pub passthrough: Option<KeyEvent>,
}
```

- One-to-one with `DataType.h:69-75` (1 word break, 2 delete, 3 normal,
  4 should not send empty character), assigned on the same paths as upstream's
  `hExt` (`Engine.cpp:63`; e.g. `hExt = 3` at `:1500`).
- It is a **separate axis, not more `Action` variants**: upstream keeps `code`
  (`DataType.h:65`) and `extCode` (`:75`) independent, and collapsing them needs
  24 combinations and destroys the per-axis diff in the replay report.
- `Action` stays a 1:1 mirror of `HoolCodeState` (`DataType.h:47-55`), which is
  what keeps the parity output comparable.

### D6 — R-API-4: `check_grammar` is deleted; `free_mark` replaces it

- **Delete `Options::check_grammar` now.** There is no `vCheckGrammar` upstream.
  Grammar repositioning is gated by `if (!vFreeMark && !IS_KEY_D(data))`
  (`Engine.cpp:1504`), and a field named `check_grammar` defaults to the
  *opposite* of upstream behaviour, so no corpus case could ever be written
  against it.
- **Add `free_mark: bool`, default `false`** (`vFreeMark` is 0 on both
  platforms: `AppDelegate.m:30`, `win32/OpenKey/OpenKey/OpenKey.cpp:83`).
- **The gate is ported faithfully**, so a corpus case can set `free_mark: true`
  and exercise it. That is why the field exists rather than being deleted: the
  behaviour needs a case, not a silent omission.
- **Persistence refuses it.** Upstream never reads `vFreeMark` back
  (`OpenKey.mm:91` assigns 0 with the read commented out; `OpenKey.cpp:83` never
  reads it). GoKey's loader therefore rewrites `false` on load and 1.0 ships no
  UI toggle. Per C-013's acceptance this is *persistence refused*, not new
  behaviour, and it is DL-013 (reproduce upstream) applied to a dead control.
- C-007 is **re-scoped, not deleted**: `backlog.csv` now holds "Corpus coverage for
  grammar repositioning (free_mark gate); no standalone feature" (R5, depends on
  C-002, V-PARITY), and its acceptance already requires that no `vCheckGrammar`
  option exists in `Options`. The gate itself is ported with C-002, ON by
  default.

### D7 — R-API-5 and option placement: `Options` holds only what the engine reads

- **Membership rule:** a field may be added to `Options` only together with core
  code that reads it. A field no core path reads is host state and belongs to
  `openkey-platform`.
- **Immediate consequence:** `send_key_step_by_step` is read only by platform
  glue (`OpenKey.mm:558,771`) and never by `Engine.cpp`; it moves to
  `openkey-platform::HostSettings`. **Timing: with C-013**, alongside the schema
  version — moving it twice would cost two migrations for zero behaviour change
  today. Until then the field stays where it is and **no engine code may read
  it**; adapters read it from `Engine::options()`.
- `ShowUIOnStartup` (`AppDelegate.m:168`) and `vSwitchKeyStatus`
  (`AppDelegate.m:397-400`) are host state from the start; neither enters
  `Options`. Recording them as omissions would be wrong.
- Net field count does not grow: one rename (`check_grammar` → `free_mark`) and
  one deferred move.

### D8 — Adapter contract

- `Host` and `KeySource` as they exist in `crates/openkey-platform/src/lib.rs` are
  accepted unchanged; this ADR adds no methods.
- `Edit` is the **single output channel**. No adapter invents output, and no
  suggestion, macro or restore path bypasses `emit`.
- `Capabilities` gates the UI, not behaviour: an option a platform cannot honour
  is hidden or disabled, never silently ignored. `supports_per_key_emission` is
  how `send_key_step_by_step` becomes visible per platform once D7 lands.
- Adapters may not reimplement typing rules (`ARCHITECTURE.md` section 2); a
  rule an adapter needs goes into core with a corpus case (DL-011,
  `WORKFLOW.md` section 7).
- The platform keycode → `Key` normalisation table is the adapter's contract
  and is reviewed by R2 as part of the M0 deliverable list (`TEAM.md`).

### D9 — AI boundary

- `openkey-core` carries no `Suggestion`, no `TextContext`, no caret geometry,
  no async runtime and no network (`ARCHITECTURE.md` section 10.2.3, DL-018).
- The caret-anchored `Suggestion` type, its `range.end == before_caret.len()`
  debug assert and the offered/dismissed/dropped lifecycle are AI-001 and
  ADR-0007. This ADR fixes only the direction: platform and AI depend on core,
  never the reverse.

### D10 — Change control

- C-017 freezes the surface in D2 at the end of M1.
- Until C-017, changing a symbol in D2 requires an amendment section appended to
  this ADR (dated, not a new file) plus a corpus note if behaviour moves
  (DL-011). After C-017, it is a semver decision, not an ADR amendment.

## Alternatives considered

| Alternative | Why rejected |
| --- | --- |
| Keep `Vec<char>` and transliterate the `Uint32` bit-packing literally | Faithful transliteration (`ARCHITECTURE.md` section 6) applies to control flow and function boundaries, not to the C bit layout. Pending tone/mark state is unrepresentable as `char`, and bit equality is not a valid oracle across platforms. `to_bits`/`from_bits` keeps the C mapping reviewable without adopting it. |
| Keep `text: String` and encode per adapter | Four of five charsets do not round-trip through UTF-8, so the tables would live in three adapters and disagree. |
| Model `ext` as extra `Action` variants | Loses the independence upstream keeps between `code` and `extCode`; 24 combinations, and the replay report can no longer diff the axes separately. |
| Keep `check_grammar` and only fix its default | No upstream control exists, so no corpus case can test it, and the name keeps asserting the opposite of what the engine does. |
| Move `send_key_step_by_step` in this turn | Two schema migrations (now and at C-013) for no behaviour change; the target is recorded instead. |
| Define a stable C ABI now | Still an open architecture question (`ARCHITECTURE.md` section 11); no embedder exists, and it would freeze types before parity proves them. |
| Put `Suggestion`/`TextContext` in core for convenience | Violates DL-018 and `ARCHITECTURE.md` section 10.2.3; the keystroke path must stay free of AI types and async. |

## Consequences

**Positive**

- C-001, C-005 and C-006 can start against types that can express what the port
  must reproduce, before the corpus grows around them.
- The replay diff has four well-defined axes (text, action, ext, backspaces,
  beep), so a disagreement can be attributed instead of merely detected.
- Charset knowledge is in one crate, so adapters cannot drift.

**Cost**

- C-005 grows: encoding at the edge plus round-trip tests per table.
- `Edit` is no longer `Copy`, and the CLI and every adapter need a decode step.
- `Word` carries the overflow deque that `Vec<char>` hid.
- Deleting `check_grammar` is a breaking API change, taken deliberately while
  C-017 has not frozen the surface and no external consumer exists.

**Residual risks**

- **Backspace semantics differ by platform** (grapheme on macOS, code unit on
  Windows). Parity counts upstream's number; the translation is adapter-side.
  MAC-002 must prove it with the 20-sentence script (V-MAC) in Compound/TCVN3.
  If it fails, the fix is in the adapter and does not reopen this ADR.
- **`Ext` is exhaustively matched**, so a future upstream ext value is a
  breaking core change. Accepted because upstream's four values have been stable
  and C-017 reviews the surface before freeze.

## Follow-up tasks

| Task | Owner | What this ADR requires |
| --- | --- | --- |
| C-001 | R3 | Implement D3; unit tests per mark; overflow push/delete round-trip test |
| C-002 | R5 (interim) | Port Telex with the `free_mark` gate (D6) and `hExt` assignment (D5); corpus cases for ext 1–4 and `free_mark` true/false |
| C-005 | R4 | Implement D4; round-trip tests per charset table |
| C-013 | R3 with Backend | Five default fixes, `check_grammar` → `free_mark`, loader refuses `free_mark=true`, `HostSettings` move of `send_key_step_by_step`, schema version |
| C-017 | R2 | Freeze the D2 surface; add the CI dependency-direction check |
| AI-001 | R2 | Platform-side AI types with the caret-anchored `Suggestion`; ADR-0007 for selection/range-replace |
| R1 (PM) | R1 | Flip DL-007 to Decided, linking this ADR, once the boundary review lands |
| Boundary review | R3, R4 | Review D1–D10 against their lanes; record in the K-002 thread |

## Amendment log

- 2026-10-29 — initial version.
