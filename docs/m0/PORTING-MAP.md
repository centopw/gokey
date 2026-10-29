# GoKey — M0 Backend: function→module porting map + Rust word model (C-001)

Owner: Backend Engineer (R3, + R5 accepted as interim — see §7b) · Created 2026-10-29
Status: **proposal for review — M0 exit criteria 4 and 5, plus PG-01**
Behaviour oracle: `github.com/tuyenvm/OpenKey` @ `89c2fd3bf258562f2349f89b49d81e2f140c3fc3` → `ORACLE-PIN.json`
Target repo: `github.com/centopw/gokey` @ `c64f8d6` (`openkey-rs` workspace)
Scope: analysis + design. **No Telex implementation** (per kickoff). No behaviour code in this delivery.

---

## 0. What I read, and what changed since my first draft

| Source | Lines | Role |
|---|---|---|
| `Sources/OpenKey/engine/Engine.cpp` | 1558 | State machine, spelling, tone placement, transforms |
| `Sources/OpenKey/engine/Vietnamese.cpp` | 576 | Phonotactic + code-conversion tables |
| `Sources/OpenKey/engine/DataType.h` | 156 | `TypingWord` bit layout, hook-state protocol, key-role macros |
| `Sources/OpenKey/engine/Engine.h` | 245 | 22 config globals, public entry points |
| `Sources/OpenKey/engine/platforms/{mac,win32,linux}.h` | 77/78/77 | Per-OS keycodes compiled into the engine |
| `Sources/OpenKey/engine/{Macro,SmartSwitchKey,ConvertTool}.cpp` | 293/73/180 | Macros, per-app memory, text converter |
| `Sources/OpenKey/macOS/ModernKey/OpenKey.mm`, `win32/.../OpenKey.cpp` | — | Glue: how `HookState` is applied to the field |
| `gokey` @ c64f8d6 | — | Scaffold, `backlog.csv`, `tests/corpus/README.md`, `ci.yml` |

Three corrections against my first draft, now that the repo exists:

1. **Crate names are `openkey-core`, `openkey-platform`, `openkey-cli`, `openkey-{macos,windows,linux}`** — not the `gokey-*` names I sketched. §2 is rewritten against the scaffold.
2. **C-001 is defined** in `backlog.csv`: *“Word model: letters, tones, marks, caps, replacing upstream bit masks”*, R3, depends on K-002, acceptance *“Typed model covers all upstream marks; unit tests”*. That is exactly what §4 specifies.
3. **The scaffold already has a core API** (`Key`, `KeyEvent`, `Action`, `Edit`, `Outcome`, `Options`, `Engine::apply`) and the repo rule is *“do not change the core API without an ADR”*. So §4 is now framed as **deltas to that API**, each needing ADR-0002 (K-002) before C-001/C-005 code lands. This is the most actionable part of the document.

---

## 1. Five findings that shape the port

1. **The word is a bit-packed array of “key identity + pending modifiers”, not characters.**
   `TypingWord[32]` (`Engine.cpp:83`) holds `u32`: bits 0–15 = OS keycode of the base letter; then
   caps, `^`, `w`, one of 5 tone marks, standalone, resolved-to-character (`DataType.h:92-122`).
   Characters exist only at render time (`getCharacterCode`, `Engine.cpp:501`). This is *why*
   `Engine::buffer: Vec<char>` cannot survive C-001: a `char` cannot hold “`o` with a pending horn
   and no tone mark yet”, which is exactly the state `insertW` (871-984) and the tone-placement
   rules (629-751) operate on.
2. **30 of the ~35 file statics in `Engine.cpp` are scratch, not state.**
   `i, j, k, ii, iii, l, kk, isCorect, isChanged, capsElem, key, markElem, keyForAEO,
   isCheckedGrammar` (`Engine.cpp:96-111`) are loop counters and temporaries. Only ~12 carry
   state. The scaffold's `Engine` struct is therefore already the right shape; it just needs the
   right field types.
3. **Output is a diff protocol with two non-obvious rules.** `charData` is filled **in reverse**
   (consumer iterates `for (i = newCharCount-1; i >= 0; i--)`, `OpenKey.cpp:632`), and on
   `vRestore` the adapter must **re-inject the physical key** after the patch, and cannot re-inject
   anything when `keyCodeToCharacter(keycode) == 0` (`OpenKey.mm:480-488`). The scaffold's
   `Outcome.passthrough` covers the second rule; §4.6 pins its semantics.
4. **Code-table decoding lives in the platform glue, four times.**
   `OpenKey.mm:302-330` + `:455-478`, `OpenKey.cpp:250-270` + `:345-360` each implement: `vCodeTable==2`
   (VNI) packs two bytes hi-then-lo; `vCodeTable==3` (Unicode Compound) packs `hi = code >> 13`
   as an index into `_unicodeCompoundMark` (U+0301/0300/0309/0303/0323) appended to `code & 0x1FFF`.
   This is behaviour of the code table, not of the adapter. It belongs in
   `openkey-core/src/charset.rs`, once. It also means `Edit.text: String` is the wrong type (R-API-2).
5. **Table iteration order is load-bearing and platform-dependent.** `_vowel`, `_vowelForMark`,
   `_vowelCombine` are `std::map` keyed by *keycode*, and `handleMainKey` (1105-1135) takes the
   first row that matches. macOS order is A,E,Y,O,U,I; Windows is A,E,I,O,U,Y (`mac.h:31-52` vs
   `win32.h:33-54`). Rust must use ordered `const` arrays, never `HashMap` (§6 R1).

---

## 2. Module map against the actual scaffold

The scaffold's boundaries are sound; the mapping below only names where each upstream symbol lands.
No new crates are proposed — `openkey-core` is already documented as “keys, options, charsets,
macros (no I/O, no unsafe)”, which covers what I had split across four crates.

| Upstream | Lands in | Notes |
|---|---|---|
| `DataType.h` bit masks and `vKeyHookState` | `openkey-core::{event, engine}` | Typed as `Letter`/`Word`/`Outcome`/`Edit` (§4) |
| `Engine.cpp` state machine + transforms | `openkey-core::engine` | One module, but internally: `word`, `spelling`, `phonotactics`, `tone`, `ops`, `quick`, `history`, `restore` |
| `Vietnamese.cpp` tables | `openkey-core::{engine, charset}` | Phonotactics in `engine::tables`, code tables in `charset` |
| `getCharacterCode` + `_codeTable[5]` + glue decoding | `openkey-core::charset` *(currently a stub: 8 lines)* | Move the glue decoding here (§1.4) |
| `Engine.h` `v*` globals | `openkey-core::options::Options` *(exists, 22 fields)* | Two defects: §4.7, §4.8 |
| `platforms/*.h` keycodes | `openkey-{macos,windows,linux}` → `openkey_core::KeyEvent` | Engine stays keycode-free, as `lib.rs` already requires |
| `Macro.cpp` | `openkey-core` (macros) + `openkey-platform` (persistence) | lib.rs places macros in core; file format is a product decision |
| `SmartSwitchKey.cpp` | `openkey-platform` (`AppId` already exists) | Per-app language |
| `ConvertTool.cpp` | `openkey-cli` | Off the typing path |
| Hook glue (`OpenKey.mm` / `OpenKey.cpp`) | `openkey-*` adapters | The `Edit` contract is the seam |

Function-level map with line references: `porting-map.tsv` (66 rows, one per symbol, with a
`slice` column and a `(new)` marker on modules that do not exist yet). Gaps check: every function
defined in the five engine `.cpp` files and every table in `Vietnamese.cpp` is listed, except the
three debug `cout` blocks (286-287, 1554-1557), which are deleted rather than ported.

---

## 3. Core API deltas (input to ADR-0002 / K-002)

These are the only changes I am asking for, all inside `openkey-core`. Each is a *design* delta, not
a behaviour change.

| ID | Current scaffold | Proposed | Why it cannot wait |
|---|---|---|---|
| R-API-1 | `Engine::buffer: Vec<char>` | `Word { letters: [Letter; 32], len: u8, overflow: Vec<Letter> }` | C-001's own acceptance criterion. Pending modifiers and the tone slot are unrepresentable as `char`; `insertW`, `handleModernMark`, `checkSpelling` all read pre-render state. |
| R-API-2 | `Edit { backspaces: u8, text: String, beep: bool }` | `text: Encoded` where `Encoded = Utf16(Vec<u16>) \| Bytes(Vec<u8>)` | `Charset::Tcvn3/VniWindows/Cp1258/UnicodeCompound` are byte-oriented and do not round-trip through UTF-8; Unicode Compound needs two code units (base + combining). C-005 lands on this field. Changing it after C-005 is a breaking API change. |
| R-API-3 | `Action` has 6 variants, no `extCode` | add the ext dimension: `1` break / `2` delete / `3` normal / `4` suppress-empty (`DataType.h:69-75`) | The Windows autocomplete workaround (`vFixRecommendBrowser`) and the per-app line-length bookkeeping (`OpenKey.cpp:580-608, 611-617`) are driven by `extCode`. Without it those behaviours cannot be expressed at all, and they are user-visible. |
| R-API-4 | `Options::check_grammar: bool = false` | `free_mark: bool = false` (grammar projection ON) | There is **no** `vCheckGrammar` upstream. Grammar positioning is gated by `if (!vFreeMark && !IS_KEY_D(data))` (`Engine.cpp:1504`) and `vFreeMark` defaults to `0` on both platforms (`AppDelegate.m:30`, `AppDelegate.cpp:21`). A field named `check_grammar` defaults to the **opposite** of upstream behaviour. |
| R-API-5 | `Options` has 22 fields | add nothing / document `vSwitchKeyStatus` as platform-owned | Hotkey config is per-OS; `openkey-platform` already owns key capture. Just record the decision so it is not “missing”. |

R-API-3 is the one I would not defer: it is cheap now and it changes the meaning of every adapter
that will ever be written.

### 3.1 Q4 defaults — acknowledged as a deviation by decision, not an omission

PM's Q4 decision (one GoKey-wide defaults set for fresh installs: `UseMacro=0`,
`RestoreIfInvalidWord=0`, `SendKeyStepByStep=0`, `ShowUIOnStartup=0`, four input types on every OS)
is **accepted and recorded here as a deliberate divergence from the upstream default profile**.
Checked against the scaffold at `c64f8d6` (`options.rs:72-97`):

| Agreed default | Scaffold now | Action | Parity consequence |
|---|---|---|---|
| `UseMacro=0` | `use_macro: true` | **must change** | Upstream macOS/Windows ship macros on; GoKey ships off. |
| `RestoreIfInvalidWord=0` | `restore_if_invalid: true` | **must change** | Upstream restores raw keys on an invalid word; GoKey will not. Largest behaviour delta of the four. |
| `SendKeyStepByStep=0` | `send_key_step_by_step: false` | matches | — |
| `ShowUIOnStartup=0` | not in `Options` | none — UI/prefs-owned (`openkey-platform`) | Not a core concern; must not appear in `Options`. |
| 4 input types per OS | `InputMethod` has exactly 4 | matches | — |

The non-obvious consequence, and the reason this is recorded rather than just coded: **the corpus
baseline must not be generated with oracle defaults.** `options.rs:34` says *"Defaults must match
upstream defaults; the parity corpus is generated with these defaults"* — that sentence is now wrong
for the two rows above. Every baseline case must carry the GoKey defaults **and** the harness must set
the corresponding upstream `v*` flags to the same values before replaying, or the first diff will be
attributable to configuration, not to the port. The corpus `options` column already allows this; the
requirement is that it is never omitted. Two cases need explicit opt-in coverage (`use_macro=1`,
`restore_if_invalid=1`) so the disabled features are still parity-tested.

### 3.2 DM-04 — technical sign-off, and two caveats

I sign the technical call: **`UseMacro=0` and four input types on every OS are both correct**, and
neither harms corpus generation. Specifically:

- `UseMacro=0` removes no typing behaviour. Macros are a lookup on a break key; with the feature off,
  every upstream macro branch simply cannot fire, so the Telex/VNI corpus is unaffected. What it does
  mean is that **`Macro.cpp` has zero default coverage**, so a macro block must be written with
  `use_macro=1` or the whole file (S6 in the porting map) merges unverified.
- Four input types per OS is not a divergence at all from the engine's point of view: all four
  (`Telex`, `VNI`, `SimpleTelex1`, `SimpleTelex2`) are engine-level `vInputType` values. It only
  means every OS ships the same adapter-to-option mapping, which makes the corpus *portable* — one
  case set, four methods, three platforms.

Two caveats, both worth recording before the corpus is generated:

1. **`SendKeyStepByStep` is not engine state, so it should not be in `Options`.** Upstream confirms
   it is read only by the platform glue — `OpenKey.mm:558,771` and `OpenKey.cpp:443,627` — and never
   by `Engine.cpp`. It does not change *what* the engine decides, only *how* the adapter emits the
   same `Edit` (one string vs per-key). Corpus cases cannot observe it, which means a field for it in
   `Options` is untestable by the parity gate and will drift. Put it in the adapter/host settings and
   let `Edit` be the contract. (It is also platform-split upstream: Windows defaults it to **1**,
   `AppDelegate.cpp:33`; macOS to **0**, `AppDelegate.m:42` — so "one value everywhere" is itself a
   divergence from *both*.)
2. **`RestoreIfInvalidWord=0` is the one that needs deliberate coverage.** Upstream restores the raw
   keystrokes when a word turns out invalid; with the feature off, that entire branch
   (`checkRestoreIfWrongSpelling`, `restore::from_raw_keys`) is dead by default. Off-by-default is
   fine as a product decision, but the acceptance for C-013 should say so explicitly, or a whole
   restore path will be reviewed as "ported" while never having executed.



---

## 4. Rust word model for C-001

Matches C-001 exactly: *“letters, tones, marks, caps, replacing upstream bit masks”*.

### 4.1 `Letter`

```rust
pub const MAX_LETTERS: usize = 32;              // MAX_BUFF, DataType.h:19

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Letter {
    pub key: Key,          // CHAR_MASK 0x0000_FFFF — base identity, semantic
    pub caps: bool,        // CAPS_MASK         0x0001_0000
    pub circ: bool,        // TONE_MASK         0x0002_0000  (^ : â ê ô)
    pub w_mod: bool,       // TONEW_MASK        0x0004_0000  (w : ă ơ ư)
    pub mark: Mark,        // MARK_MASK         0x00F8_0000  (sắc huyền hỏi ngã nặng)
    pub standalone: bool,  // STANDALONE_MASK   0x0100_0000
    pub resolved: bool,    // CHAR_CODE_MASK    0x0200_0000
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Mark { #[default] None, Sac, Huyen, Hoi, Nga, Nang }
```

`Key` already exists in `event.rs` and is the right type — no new key enum. Two notes:

- `circ` and `w_mod` are **independent bits**, not one enum: upstream can and does set both
  (`insertAOE` clears `TONEW` at 846-848, `insertW` clears `TONE` at 876-879), and `getCharacterCode`
  gives `TONE` precedence (535-539). Keeping them as two `bool`s preserves that ordering explicitly.
- `Letter::to_bits()/from_bits()` reproduce the C layout for reasoning and debugging, but **bit
  equality is only meaningful inside the Rust world**: in C the low 16 bits hold an *OS keycode*
  (`KEY_A` is `0` on macOS, `0x41` on Windows), so cross-checking raw bits is not a valid oracle.
  The oracle compares rendered text.

### 4.2 `Word`

```rust
pub struct Word {
    letters: [Letter; MAX_LETTERS],   // TypingWord[32]
    len: u8,                          // _index
    overflow: Vec<Letter>,            // _longWordHelper: entries dropped off the FRONT
}

impl Word {
    pub fn set(&mut self, i: usize, l: Letter);        // setKeyData
    pub fn push(&mut self, l: Letter);                  // insertKey (no spelling)
    pub fn pop(&mut self);                              // backspace, incl. overflow right-shift
    pub fn get(&self, i: usize) -> Option<Letter>;
    pub fn len(&self) -> usize;
    pub fn clear(&mut self);
    pub fn span(&self, scan: Scan) -> VowelSpan;        // findAndCalculateVowel
    pub fn tail_matches(&self, row: &Row, from: usize) -> bool;  // checkCorrectVowel
}

#[derive(Clone, Copy, Default)]
pub struct VowelSpan { pub start: u8, pub end: u8, pub count: u8, pub mark_slot: u8 }
```

Overflow semantics to reproduce exactly: on push at `len == 32` the **front** letter is dropped into
`overflow` and the buffer shifts left (`Engine.cpp:350-356`); on delete the last overflow letter is
shifted back in at the front (`1439-1447`). It is a head-drop deque written as a shift — keep it as
a shift until the corpus is green, then consider a ring buffer behind the same API.

`Word` is the only type allowed to index `letters`; everything else reads through `get()`. That is a
grep-able invariant (acceptance criterion 5).

`Vec<char>` in the current scaffold becomes `Word`; `Engine.buffer()` can keep returning something
renderable for tests, e.g. `Vec<char>` produced through `charset`.

### 4.3 `Engine` (session state)

```rust
pub struct Engine {
    options: Options,      // already exists
    word: Word,            // replaces buffer: Vec<char>
    raw: KeyHistory,       // KeyStates[32] + _stateIndex      (restore-if-wrong-spelling)
    log: WordLog,          // _typingStates, _specialChar, _spaceCount
    rt: Runtime,           // temp_disable_key, has_handled_macro, has_handle_quick_consonant,
                           //   upper_case_status, will_temp_off_engine, is_restored_w,
                           //   check_spelling (effective), spelling_before
    span: VowelSpan,       // VSI/VEI/vowelCount/vowelWillSetMark — cached like upstream
}
```

Only ~12 of the C statics become fields. `Engine::apply(&KeyEvent) -> Outcome` keeps the scaffold
signature; `reset()` maps to `startNewSession` (`Engine.cpp:457`, also called by adapters on focus
change); `MAX_BUFFER` stays 32.

### 4.4 Config vs runtime split

`vCheckSpelling` is a **setting** that `vTempOffSpellChecking` flips at **runtime**
(`Engine.cpp:1223-1231`). Keep `Options::check_spelling` (setting) and
`Runtime::check_spelling` (effective) + `Runtime::spelling_before` (saved setting). Same observable
behaviour as upstream, no shared mutable global. This is what makes `Engine` `Send` and lets the
harness run configurations in parallel.

### 4.5 Transform signature

Every upstream transform mutates the word *and* hand-writes the hook state (`hCode/hBPC/hNCC/hData`
via the macros at `Engine.cpp:60-64`). In Rust:

```rust
struct Patch { backspaces: u8, letters: Vec<Letter> }   // letters in forward order

fn apply_mark(word: &mut Word, mark: Mark, cfg: &Options) -> Patch;
fn toggle_w(word: &mut Word, cfg: &Options) -> Patch;
// ... one Patch-returning function per upstream insert*/remove* function
```

The reversal convention then exists in exactly one place (`Patch → Edit`), instead of ~15.

### 4.6 `Outcome` semantics to pin (ADR text)

- `Outcome.action` mirrors `HoolCodeState` 1:1 — keep the variants as shipped.
- `Outcome.edit` is always in **forward** order; `Edit.backspaces` counts characters to delete.
- `Outcome.passthrough` is emitted **after** the edit, and only for `Action::Restore` /
  `RestoreAndBreak`; the adapter must skip it when the key has no character mapping, and instead
  re-send the original key event (`OpenKey.mm:480-488`). Upstream has no field for this decision —
  the adapter recomputes it — so the rule has to be written down or it will be reinvented three times.
- `Action::Process` with `Edit.backspaces == 0 && edit.text.is_empty()` is a no-op, not a swallow.

### 4.7 Accepted C-001 criteria (restating the backlog, testable)

1. `openkey-core` still passes `#![forbid(unsafe_code)]`, `cargo clippy -D warnings`, and builds on
   all three CI targets with no platform imports.
2. Round-trip: `Letter::to_bits`/`from_bits` over the whole `Key` domain.
3. `Word` unit tests keyed to C line refs:
   - push at `len == 32` drops the front letter into `overflow` (Engine.cpp:350-356);
   - delete with non-empty overflow right-shifts it back in (1439-1447);
   - `span()` reproduces the `qu`/`gi` cases and the `u`-after-`q` correction (560-585);
   - `tail_matches()` reproduces the `C`/`T` mark restriction and the `qu` guard (468-499).
4. Every upstream mark is representable: 5 tone marks × `^` × `w` × caps × standalone, asserted
   by a table test that enumerates `DataType.h:96-122` (this is C-001's stated acceptance).
5. No `letters[` indexing outside `word.rs` (CI grep, same style as the existing `forbid` rules).
6. No table is “ported by eye”: a C dumper and a Rust dumper emit identical normalised JSON for
   `_vowel`, `_vowelCombine`, `_vowelForMark`, `_consonantTable`, `_endConsonantTable`,
   `_consonantD`, `_codeTable[0..5]`.

### 4.8 Known upstream quirk to carry, not fix

`_longWordHelper` is unbounded within a word (it only clears on break/`startNewSession`), and
`MAX_BUFF` is both the word cap and the output cap, so the consumer silently drops the whole patch
when `backspaceCount >= MAX_BUFF` (`OpenKey.cpp:620`). Parity says preserve for M1 and record a case;
do not “fix while porting”.

---

## 5. PG-01 — pinned oracle and corpus baseline

`ORACLE-PIN.json` is the record. It ships as `docs/m0/ORACLE-PIN.json` in this PR; K-003 moves the
machine-read copy to `parity/oracle/pin.json` and `scripts/parity.sh` fails if the two disagree.
How the harness consumes it, using the artefacts the scaffold already has:

### 5.1 Pin

- repo + commit + commit date + `version.json` (macOS 2.0.3/47, Windows 2.0.2/131074) + GPL-3.0
  `LICENSE` sha256;
- `sha256` of the ten engine files and three platform headers — a mismatch means the pin moved or a
  vendored copy was edited, and both must fail loudly;
- retrieval method and its caveat: the clone was shallow (`--depth 1`), so CI must fetch **by hash**,
  not by branch, and fail if the resolved commit differs.

### 5.2 Baseline keyed by oracle hash

The scaffold already defines the file format (`tests/corpus/README.md`: TSV,
`id / method / options / input / expected`) and the CI hook (`scripts/parity.sh`, currently a
warning). Completion of PG-01 is therefore:

```
parity/oracle/pin.json              # committed copy of ORACLE-PIN.json
tests/corpus/*.tsv                  # human-authored cases (format already specified)
parity/baseline/<commit>/<id>.txt   # rendered document text per case, from the oracle
parity/baseline/<commit>/MANIFEST.json  # case list + per-case sha256 + oracle pin hash
scripts/parity.sh                   # builds the oracle driver, replays, diffs, exits non-zero
```

CI rule: pin commit ≠ baseline directory name ⇒ **fail**. Regenerating a baseline is then a
reviewable commit whose diff *is* the behaviour change — which is what makes PG-02 enforceable
mechanically rather than by memory.

Two gaps in the current corpus format worth closing while the corpus is still v0 (owner: R5/PM, not
a unilateral change): `\b` models Backspace but there is no Delete key, and there is no way to
express “event must pass through untouched” (`Action::Pass`) or a modifier-bearing event. Both are
needed for `restore_if_invalid`, `temp_off_spelling` (ctrl) and the `extCode` rules in R-API-3.
Suggest an optional sixth column (`expect`), not a format rewrite.

### 5.3 Oracle without a GUI (recommended, and it fits K-003's wording)

Two oracles, in this order:

1. **Headless C driver (primary, hermetic).** Compile the pinned `Engine.cpp` unmodified into a
   driver that feeds `vKeyHandleEvent` and applies `HookState` to an in-memory buffer using the
   *same* rules as `OpenKey.mm` (backspace N; emit `charData` reversed; on `vRestore` re-inject the
   key), then prints per-key output. This is literally K-003's acceptance: *“Reference binary
   replays a keystroke script and prints per-key output.”* It runs on the `macos-latest` runner the
   CI already has.
2. **Real-app spot check (calibration, not CI).** Drive the built app with synthetic CGEvents on a
   corpus sample and diff against (1). If they diverge, the driver is wrong — and that divergence is
   itself a finding.

Stated risk: the oracle is only as good as the driver's apply logic. That is exactly why §4.6 pins
`Outcome` semantics to the driver's behaviour rather than inventing a new contract.

---

## 6. Risks

**R1 — Table iteration order is platform-dependent (highest).** `std::map` keyed by keycode; macOS
order A,E,Y,O,U,I vs Windows A,E,I,O,U,Y. Rust: ordered `const` arrays, order pinned to the macOS
oracle, with a comment citing `Engine.cpp:1105-1135` so nobody “tidies” it into a `HashMap`.

**R2 — Crossed Windows keycodes upstream.** `KEY_BACKQUOTE=0xDC` (that is `VK_OEM_5`, backslash),
`KEY_BACK_SLASH=0xDE` (`VK_OEM_7`, quote), `KEY_QUOTE=0xC0` (`VK_OEM_3`, backquote)
(`win32.h:71-75`). The Windows build’s own word-break and `[`/`]` paths therefore run on the wrong
physical keys. The scaffold’s design (semantic `Key` in core, native tables in adapters) does not
inherit this. Log as a deliberate deviation with a corpus case.

**R3 — Global mutable state.** 30+ statics, single instance, no reentrancy. The scaffold’s owned
`Engine` already avoids this; the risk is only if a ported function reaches for a static “because
the C version had one”.

**R4 — The reversed-buffer / restore contract lives in adapter code.** Every multi-char edit goes
through `OpenKey.mm:744-776` / `OpenKey.cpp:611-642`. Core-only tests cannot catch a mistake there.
`Edit`-level integration tests are required, not just `Engine` unit tests.

**R5 — `MAX_BUFF` is both caps.** See §4.8. Preserve for M1, log a case.

**R6 — `_longWordHelper` unbounded.** Parity default: no cap; add a configurable cap (off by
default) for the fuzz harness.

**R7 — `findMacro` mutates its argument** (`Macro.cpp:155-157` rewrites `key` in place to character
codes). Port as a pure lookup; internal-only, no user-visible change.

**R8 — Table rows carry flags in the key field.** `END_CONSONANT_MASK 0x4000` and
`CONSONANT_ALLOW_MASK 0x8000` are OR-ed into the keycode *inside table rows*
(`Vietnamese.cpp:22, 365-367`). With a semantic `Key` these move to a typed `RowFlags` per row.
Mechanical, but it touches every phonotactic table — do it in the transcription commit and let the
dump-diff test (4.7.6) prove equivalence.

**R9 — GPL-3.0 is structural, not packaging.** A Rust port is a derivative work; the word model in
§4 is a translation of upstream data structures. The workspace already declares `GPL-3.0-only`, so
this is consistent — but it cannot be relaxed later, and it is why the licence text / source offer
are M0 inputs (PG-03) rather than release polish.

---

## 7. Defects I found in the scaffold (not fixed by me — they need their owners)

Reported, not patched, because each belongs to a task ID:

1. **`docs/` is missing from the repo** at `c64f8d6`, although `README.md`, `openkey-platform/src/lib.rs:38`
   and `backlog.csv` (K-002, K-009) all reference it. The bootstrap message said `docs/` was included.
   **Root cause found (evidence, not speculation):** `git check-ignore -v docs/m0/PORTING-MAP.md`
   on the bootstrap machine resolves to `~/.gitignore_global:6:docs/` — a *machine-global* excludes
   file, not the repo's own `.gitignore`. The bootstrap commit was made on a machine carrying that
   global rule, so `git add docs/` skipped the directory **silently**. Nothing was deleted; the files
   were never staged.
   - Fix on the bootstrap machine: `git add -f docs/` (or remove `docs/` from the global ignore),
     then commit. This PR force-adds only its own three files and does not change anyone's git config.
   - Anyone re-creating the workspace needs this, or K-009 will fail the same way a second time.
   Owner: whoever owns the bootstrap commit.
2. **`Options::check_grammar` has no upstream counterpart and defaults to the inverted behaviour**
   (R-API-4). Worth catching before C-007.
3. **`charset.rs` says the code tables live in `Engine.cpp`;** they are in `Vietnamese.cpp`
   (`:411-494`), and `getCharacterCode` is in `Engine.cpp:501`. Doc-only, but it will mislead C-005.
4. **`openkey-core` crate is named after the upstream product** while DL-010 is unresolved and the
   README calls the project `openkey-rs`. Crate/package names are internal, so this is not a blocker —
   but published artefact names (binary, dmg/MSI/deb identifiers, package registry) must not use
   “OpenKey”, and that decision should be recorded before DevOps freezes packaging paths.
5. **`K-003/K-004/K-005` are assigned to R5** (QA/parity, per `tests/corpus/README.md`) while the
   kickoff assigned me the reference harness and corpus baseline. **Resolved 2026-10-29:** PM
   confirmed R5 is unstaffed and proposed Backend takes R5 as interim. **Accepted**, with the risk
   stated in §7b.
6. **Two `Options` defaults contradict the agreed GoKey defaults** (`use_macro: true`,
   `restore_if_invalid: true`, `options.rs:82,87` vs Q4). Recorded in §3.1; the change belongs to
   C-013 (defaults matrix), not to this docs-only delivery.
7. **`Options` is missing `show_ui_on_startup`** — correct as-is: it is a UI preference, not engine
   state (§3.1). Listing it here only so the absence reads as a decision.

### 7a. C-007 "Grammar check" — verdict from `Engine.cpp` (asked by PM)

**There is no grammar-check feature.** There is grammar *behaviour*, and it is unconditional.

Evidence (all upstream, pinned commit):

| Fact | Location |
|---|---|
| `void checkGrammar(const int& deltaBackSpace)` exists | `Engine.cpp:290` |
| Called on every normal key: `checkGrammar(-1)` when nothing was processed, `checkGrammar(0)` otherwise | `Engine.cpp:1506`, `:1508` |
| Called on backspace: `checkGrammar(1)` | `Engine.cpp:1463` |
| The **only** gate is `if (!vFreeMark && !IS_KEY_D(data))` | `Engine.cpp:1504` |
| `vFreeMark` has **no `vCheckGrammar` sibling anywhere** — one `extern int` and one default | `Engine.h:60`; `AppDelegate.m:30 = 0`, `AppDelegate.cpp:21 = 0` |
| `vFreeMark` is exposed in the UI, i.e. it is a user setting | `ViewController.m:184`, `MainControlDialog` |

So: **FE is right** — neither upstream app has a "grammar check" toggle, because the feature it would
toggle is always on. **`backlog.csv` C-007 is unfounded as written** (a setting with acceptance
"matches upstream on grammar corpus cases" — there is no setting to match). **PLAN.md M1 "spelling/
grammar" is correct as behaviour**, and the scaffold's `Options::check_grammar` is the invented
middle ground that produced the confusion (wrong name, inverted default: R-API-4).

Recommendation, as a plan change for the sponsor rather than something I can settle alone:

- Delete C-007 as a standalone feature, and move its acceptance sentence onto the engine-port task
  (C-002): *grammar repositioning is part of the port, covered by corpus cases, ON unless
  `free_mark` is set*.
- The real option is `free_mark` (upstream `vFreeMark`). If the product wants it, it is one boolean
  in `Options` with default `false`, and its corpus block is "marks stay where the user put them".
- Do **not** keep a field called `check_grammar`: it cannot be mapped to any upstream control, so no
  corpus case could ever be written against it.

### 7b. R5 accepted as interim — the risk this creates, stated plainly

With R5 folded into R3, one agent holds **K-002, K-003, K-004, K-005, K-008** plus C-001/C-002. The
serialisation is real: K-003 (headless reference build) blocks K-004 (corpus v0 ≥300) which blocks
K-005 (replay/diff), and C-001's word model is validated *by* K-005's diff. If I am the bottleneck,
M0 exit criteria 2 and 3 slip together rather than independently — there is no longer a second person
who can take one of the three while I take another.

Two mitigations I will apply, and one I need from PM:

- Land K-003 as a **committed C driver + pin check** first, so the oracle is reproducible even if the
  corpus authoring lags behind.
- Keep corpus authoring and replay as separate PRs (K-004, K-005), so a delay in one does not block
  review of the other.
- **From PM:** name the hand-off condition now, not at M0 exit — e.g. *"if corpus v0 is not at 150
  cases by <date>, QA is staffed"*. An interim role with no expiry is a permanent role that nobody
  resourced.

---

## 8. Open questions

| # | Question | Owner | Why it matters |
|---|---|---|---|
| Q1 | ~~Confirm R-id → person mapping~~ **Answered:** R5 unstaffed → Backend interim (§7b) | PM | Closed; only the hand-off condition is open |
| Q2 | Adopt R-API-1..5 in ADR-0002 before C-001/C-005 code lands | PM (ADR-0002, K-002) | Core-API change rule; R-API-3 changes every adapter |
| Q3 | macOS build at the pinned hash is normative where macOS and Windows disagree (R1, R2) | PM to record | Decides whether the pinned order/quirks are the spec |
| Q4 | Output text: precomposed NFC, no decomposition? | Backend proposal → PM records | Upstream emits precomposed code points; macOS may normalise downstream. Affects every baseline |
| Q5 | Macro persistence: read OpenKey’s byte format, or a new one? | Backend + PM | Decides whether a legacy parser ships |
| Q6 | ~~Will `docs/` be restored~~ **Answered:** absent from `c64f8d6`; K-009 cannot close. Corpus-format extension (§5.2) still needs an owner decision | Admin / R5 | The format extension is now mine by default (§7b) — flagging rather than assuming |
| Q7 | Does the agreed Q4 defaults set apply to the **corpus baseline** too, i.e. is parity measured against upstream defaults or GoKey defaults? (§3.1) | PM to record | If unrecorded, the first parity diff is ambiguous between a port bug and a config difference |

---

## 9. What I need, one line each

- **PM**: gate on exit criteria 4 and 5; record R-API-1..5, Q2, Q3, Q4, Q7; set the R5 hand-off
  condition (§7b); state the ADR-0002 decision so C-001 can start.
- **Admin/Hiep**: `docs/` in the repo — `TEAM.md` is what unblocks Q1.
- **DevOps**: confirm the `macos-latest` job can build the headless C driver (`scripts/parity.sh` hook already exists in CI). If it cannot, DL-003’s revisit trigger fires.
