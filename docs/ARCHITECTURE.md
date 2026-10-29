# Architecture

Status: sections 2–4 are ratified by `adr/0002-crate-graph-core-api-adapter-contract.md`
(K-002, proposed 2026-10-29); the rest is a working proposal. Owner: technical
lead (R2).

## 1. Goals

1. **One engine.** All Vietnamese typing logic in a pure Rust crate with no I/O,
   no globals, no platform keycodes.
2. **Thin adapters.** Each OS front-end only captures keys, tracks focus,
   emits edits, and draws UI.
3. **Provable parity.** The core is diffable, case by case, against the upstream
   C++ engine.
4. **Bounded blast radius.** Unsafe code and platform APIs confined to adapter
   crates, one module per OS concern.

## 2. Crate graph

```
                 +------------------+
                 |  openkey-core    |   pure, #![forbid(unsafe_code)]
                 |  engine, charsets|   no OS calls, no globals
                 |  macros, options |
                 +---------+--------+
                           ^
              +------------+-------------+
              |                          |
     +--------+--------+        +--------+---------+
     | openkey-platform|        |  openkey-cli     |
     | Host/KeySource  |        |  replay, convert |
     | capability model |       +------------------+
     +--------+--------+
              ^
   +----------+-----------+-----------+
   |                      |           |
+--+--------+     +-------+------+ +--+-----------+
| openkey-  |     | openkey-     | | openkey-     |
| macos     |     | windows      | | linux        |
| CGEventTap|     | LL hook      | | IBus engine  |
| status item|    | tray + dialog| | (Fcitx5 v2)  |
+-----------+     +--------------+ +--------------+
```

Rules:

- `openkey-core` never depends on `openkey-platform` or an adapter.
- Adapters may not reimplement typing rules; if an adapter needs a rule, it
  belongs in core with a corpus case.
- `openkey-cli` links only core, so engine work can proceed without any OS
  integration and can run on Linux CI.

Post-1.0 only, and never linked into the 1.0 binary:

```
  openkey-ai  ->  openkey-core (types) + openkey-platform (context/caret traits)
                  suggestion providers, model runtime, evaluation harness
```

1.0 ships the *traits* in `openkey-platform`; it ships no AI implementation, no
model, no network calls. The dependency direction is enforced in CI: core must
never depend on `openkey-ai`.

## 3. Core contract

```rust
// openkey-core
pub struct Engine { /* options + bounded 32-char word state */ }

impl Engine {
    pub fn new(options: Options) -> Self;
    pub fn set_options(&mut self, options: Options);
    pub fn reset(&mut self);                       // upstream startNewSession
    pub fn apply(&mut self, ev: &KeyEvent) -> Outcome;
}

pub struct Outcome {
    pub action: Action,          // Pass | Process | BreakWord | Restore | Macro | RestoreAndBreak
    pub ext: Ext,                // Normal | Break | Delete | SuppressEmpty (upstream extCode)
    pub edit: Option<Edit>,      // backspaces + text, already charset-encoded
    pub passthrough: Option<KeyEvent>,
}

pub struct Edit {
    pub backspaces: u8,          // upstream backspaceCount
    pub text: Encoded,           // Utf16(Vec<u16>) | Bytes(Vec<u8>) — see ADR-0002
    pub beep: bool,
}
```

The word model (`Word`/`Letter`), `Encoded`, `Ext` and the `Options` membership
rule are fixed in `adr/0002-crate-graph-core-api-adapter-contract.md` (K-002).

This is a deliberate change from upstream:

| Upstream | Port | Why |
| --- | --- | --- |
| `extern int vLanguage`, `vInputType`, `vCodeTable`, ... | `Options` passed in / owned by `Engine` | Testable, multi-instance safe, serialisable |
| `void vKeyHandleEvent(...)` mutating `vKeyHookState` out-param | `apply(&mut self, &KeyEvent) -> Outcome` | Pure function of state; diffable |
| Platform keycodes compiled in (`KEY_A` = 0 or 0x41) | `Key` enum normalised by adapter | One engine binary path for all OSes |
| Tone/mark/caps packed into `Uint32` high bits | Typed word model, `legacy` shim only if parity demands | Removes bit-mask bugs |
| 32-char `charData[MAX_BUFF]` | `Vec<char>` bounded to `MAX_BUFFER` | Same bound, no UB |

`Action` mirrors upstream `HoolCodeState` one-to-one so parity output stays
comparable.

## 4. Adapter contract

```rust
pub trait Host {
    fn app_id(&self) -> AppId;        // bundle id / exe / desktop id
    fn caps_lock(&self) -> bool;
    fn emit(&mut self, edit: Edit);   // backspace, then Unicode text
    fn beep(&mut self);
    fn capabilities(&self) -> Capabilities;
}
```

Event loop per platform (all adapters own their native run loop; core is unaware):

1. Capture native key event.
2. Normalise keycode -> `Key`, collect modifiers, caps lock, layout character.
3. Call `engine.apply`.
4. `Pass` -> let the OS deliver; `Process`/`Macro`/`Restore` -> suppress and emit
   `Edit`; `RestoreAndBreak` -> emit edit, then re-inject the break key.
5. On focus change / app switch -> new session (`engine.reset()`), load per-app
   mode and charset.

### Platform mechanics

| Concern | macOS | Windows | Linux |
| --- | --- | --- | --- |
| Capture | CGEventTap at `kCGHIDEventTap` (Accessibility) | `WH_KEYBOARD_LL` | IBus engine key handler |
| Suppress | tap callback returns NULL | hook returns 1 | engine consumes event |
| Focus / app id | NSWorkspace frontmost + AX observer | `GetForegroundWindow` -> exe path | IBus focus-in/out signal |
| Emit text | `CGEventKeyboardSetUnicodeString` | `SendInput` with `KEYEVENTF_UNICODE` | `commit_text` (no synthesis) |
| Emit backspace | synthetic key events | `SendInput` VK_BACK | edit the preedit buffer, not the document |
| Permission | Accessibility (TCC) | none, but AV/signing risk | none; must be installed as IME |
| Integration | menu bar item (NSStatusItem) | tray icon + Win32 dialog | IBus engine + GNOME/KDE settings |

Linux note: because the engine is embedded in the IME, "backspace technique"
becomes preedit manipulation plus commit. This is a behavioural adapter concern
(ADR-0006), not an engine change.

## 5. Capability matrix

Platform differences are visible in the product, so they are data, not surprises.

| Capability | macOS | Windows | Linux (IBus) |
| --- | --- | --- | --- |
| Global key capture | yes | yes | only inside focused client |
| Key suppression | yes | yes | yes (IME scope) |
| Focus/app identity | yes | yes | yes |
| Read text near caret | yes (AX) | partial (UIA) | preedit only |
| Caret geometry for overlay | yes (AX `kAXBoundsForRange`) | partial | no |
| Replace span left of caret (backspace + type) | yes | yes | preedit only |
| Secure-field detection | yes (AX secure text) | partial | n/a |
| Per-key emission fallback | yes | yes | not needed |
| System input-source switching | partial | partial | required |
| Marked text / preedit | avoid (upstream bug) | not used | native mechanism |
| Requires elevated permission | yes | no | no |

## 6. Porting strategy

**Phase A — faithful transliteration.** Port `Engine.cpp`, `Vietnamese.cpp`,
`Macro.cpp`, `ConvertTool.cpp`, `SmartSwitchKey.cpp` structure first, keeping
function boundaries recognisable. This maximises parity and makes the C++ and
Rust code reviewable side by side. GPL permits it; we credit it in NOTICE.

**Phase B — refactor behind tests.** Only after parity: replace bit-mask flags
with typed model, split modules, simplify. The corpus protects the behaviour.

**Parity harness (the core quality mechanism).**

1. Build upstream engine as a headless CLI (`tools/ref-engine`, task K-003).
2. Replay the same keystroke script through the reference and through
   `openkey-cli replay`.
3. Diff final text, action, backspace count, and beep per keystroke.
4. Nightly: randomised differential fuzzing over Telex/VNI grammar.
5. Gate: PR-level smoke (<=500 cases), milestone-level full corpus (>=2000).

Known source of false diffs: upstream output depends on which platform's
keycode table was compiled in. The harness normalises both sides to logical keys.

## 7. State model

- One `Engine` instance per focus session.
- `reset()` on: focus change, navigation keys, enter, word-break keys, option change.
- Per-app memory (mode + charset) lives in `openkey-platform`, keyed by `AppId`.
- Buffer bounded at 32 chars; exceeding it forces a word break, matching upstream.

## 8. Settings and persistence

- `Options` is the single source of truth, `serde`-serialised as JSON.
- Locations: `~/Library/Application Support/<name>/settings.json` (macOS),
  `%APPDATA%\<name>\settings.json` (Windows), `$XDG_CONFIG_HOME/<name>/` (Linux).
- Migration from upstream plist / registry is one-way, best effort, and never
  modifies the original keys (task C-013).
- Schema is versioned; unknown fields are rejected at release, tolerated in dev.

## 9. Budgets and safety

| Concern | Budget / rule |
| --- | --- |
| Key decision latency | p99 <= 0.5 ms core, <= 2 ms including emission |
| Memory | <= 60 MB RSS for the whole app; word state <= 1 KB |
| Allocation | No allocation on the `Pass` fast path; prefer stack buffers |
| Unsafe | Only in adapter crates, each block with `// SAFETY:` and a named owner |
| Network | None at runtime; update check is explicit, signed, opt-in-visible |
| Key content | Never logged, never persisted; session buffer is in-memory only |
| Panics | `catch_unwind` at the FFI/event boundary; engine must be panic-free |

## 10. AI assist extension points (seams ship in 1.0, features post-1.0)

AI is not in the keystroke path. The engine stays deterministic; assist features
live in a separate crate that observes text and proposes edits.

```rust
// openkey-platform (shipped in 1.0, unused)
pub struct CaretRect { pub x: f64, pub y: f64, pub height: f64 }

pub struct TextContext {
    pub before_caret: String,   // bounded, e.g. 512 chars; caret is the end of this
    pub app: AppId,
    pub secure: bool,
}

pub trait ContextProvider {
    fn context(&self, max_chars: usize) -> Option<TextContext>;
    fn caret_rect(&self) -> Option<CaretRect>;
    fn is_secure_field(&self) -> bool;
}

// openkey-ai (post-1.0, never linked into the 1.0 binary)
pub struct Suggestion {
    /// Byte range inside `TextContext::before_caret`. Must end at the caret.
    pub range: Range<usize>,
    pub replacement: String,
    pub kind: SuggestionKind,   // Correction | Completion
    pub confidence: f32,
}

pub trait SuggestionEngine {
    fn suggest(&self, ctx: &TextContext) -> Option<Suggestion>; // worker thread
}
```

### 10.1 Correction lifecycle (DL-018)

The user-facing behaviour, decided:

1. The user types; the engine keeps its normal word buffer and never waits.
2. After the word settles (no keystroke for 250 ms, or a word-break key), the
   host asks for a suggestion using the text immediately before the caret.
3. If the anchor word is wrong, the host highlights the span and paints the
   replacement inline — the original text stays in the document, unmodified.
4. `Tab` accepts: backspace over the span, type the replacement, one undo unit,
   caret to the end of the inserted text, engine buffer reset.
5. `Esc` dismisses: nothing is emitted, and that span is not offered again.
6. If the user keeps typing, the suggestion is dropped and the text is
   untouched. A new suggestion appears only for a different anchor word.

State machine (one suggestion at a time, per focus session):

```
Idle --word settled--> Query --engine reply--> Offered
Offered --Tab--> Accepted --> Idle
Offered --Esc--> Dismissed --> Idle (span remembered)
Offered --any keystroke--> Dropped --> Idle
Query --caret moved / focus lost / secure field--> Idle
```

Binding constraints:

- **Caret-anchored only.** `range.end == before_caret.len()`. Accepting is then
  always "delete backwards, type forwards", which every adapter can do with the
  primitives it already has. Corrections that would need a selection or a
  forward deletion are out of scope until there is a selection API (ADR-0007).
- **No auto-correct.** A false positive that moves the caret mid-word is worse
  than a missed correction.
- **One suggestion per anchor.** A dismissed or dropped anchor is remembered, so
  the same error is not re-offered until the text around it changes.
- **Secure and non-text fields never reach the provider.** The block is in the
  host, not in the engine or the AI crate.

### 10.2 Rules for both correction and completion

1. **Never block typing.** Suggestions run off the key path, are cancelled by
   the next keystroke, and are dropped if older than 150 ms.
2. **Accept is an ordinary edit.** Accepting sends backspaces plus text through
   the same `emit` path, as a single undo unit; rejecting sends nothing.
3. **Determinism boundary.** `openkey-core` has no AI types, no async runtime,
   no network. CI fails if core depends on `openkey-ai`.
4. **Privacy.** Context is read only after explicit opt-in; secure fields and
   password managers return `None`; nothing is logged or persisted.
5. **Graceful absence, not degradation.** If a platform cannot supply caret
   geometry or nearby text, assist features are hidden entirely. This is a
   second reason macOS leads.
6. **Rendering.** The overlay draws the highlighted span plus the replacement
   and is non-activating; it is never marked text (DL-017).

## 11. Open architecture decisions

- ADR-0004: macOS CGEventTap vs InputMethodKit (spike K-008).
- ADR-0005: Windows hook v1 vs TSF (report WIN-007).
- ADR-0006: Linux IBus first vs Fcitx5 first; preedit strategy (LNX-002/LNX-006).
- ADR-0007: whether a selection/range-replace primitive is worth the risk, and
  what it unlocks for correction spans that do not end at the caret.
- GUI stack for settings: native per platform vs a shared Rust UI toolkit.
- Whether the core exposes a stable C ABI for embedders beyond our adapters.
