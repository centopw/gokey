# Reference harness (K-003)

`harness.cpp` links the pinned upstream C++ engine unmodified and replays a
keystroke script against it. Its output is the **behaviour oracle**: corpus
baselines are produced by this binary, so nothing here may encode a typing rule
of its own. The only upstream logic it restates is the small part of the macOS
hook that lives outside the engine — the part that turns a `vKeyHookState` into
document edits — and each such rule carries a line reference to the code it
copies.

```
./scripts/parity.sh                       # verify the pin, build, replay everything
.oracle/build/harness some.tsv            # replay one script by hand
python3 scripts/parity/compare.py some.tsv <( \
    .oracle/build/harness some.tsv)       # ...and judge it
```

`parity.sh` verifies the pin first (`scripts/ci/check-oracle-pin.py`), fetches the
upstream tree by **commit hash** at `docs/m0/ORACLE-PIN.json → upstream.commit`
if `.oracle/src` is absent, builds with plain `clang++ -std=c++17 -O2 -w`, then
replays `scripts/parity/smoke.tsv` and every `tests/corpus/*.tsv` it finds. A case
whose `expected` column is absent is replayed but not judged; a case whose
`expected` column disagrees fails the script.

## Pin and platform

- The normative platform is **macOS** (`docs/m0/ORACLE-PIN.json`, DL-026). The
  harness is therefore compiled **without** `-DLINUX` / `-D_WIN32`; a compile-time
  `#error` enforces it. `DataType.h:84-90` picks the keycode header by ifdef, so
  defining `LINUX` would change `KEY_A` from 0 to 38 and reorder the
  keycode-keyed `std::map` tables (DEV-02) — the oracle would then disagree with
  the port for reasons that are not typing behaviour.
- Settings are upstream's macOS **first-run** values from
  `AppDelegate.m:loadDefaultConfig` (`:289-315`), not the C initialisers at
  `:28-60`. They disagree (`vUseModernOrthography` is 1 at `:32` and 0 in
  `loadDefaultConfig`) and the app reads `NSUserDefaults` on launch
  (`OpenKey.mm:91-105`), so `loadDefaultConfig` is what a fresh install runs. A
  baseline taken from the initialisers would describe an app nobody has.
- Engine state is reset per case with `vKeyInit()` (`Engine.cpp:135-143`), the
  same call the app makes at a word boundary.

## What the document model implements

| Rule | Upstream |
|---|---|
| `vDoNothing` → the system types the key | `OpenKey.mm:713-737` |
| `vWillProcess` / `vRestore` / `vRestoreAndStartNewSession` → `backspaceCount` backspaces, then the emitted text | `:764-769`, `:423-480` |
| Backspaces applied only when `0 < backspaceCount < MAX_BUFF` (32) — upstream's own condition, not a safety check | `:764-769` |
| `charData` emitted from `newCharCount-1` **down to 0** | `:430-432` |
| `charData` entries truncated to `UniChar` (16-bit), so mask bits above U+FFFF are dropped | `:453-454` |
| `vRestore` also re-sends the pressed key as a character when it has one, else re-posts the control key | `:483-491`, `:492-507` |
| Emitted UTF-16 units are decoded as code points (surrogate pairs preserved) | `:423` |

Keycodes come from upstream's own `_characterMap` (`Vietnamese.cpp:508+`,
exported in `Vietnamese.h`) rather than a transcribed table, so the harness cannot
drift from the engine it links.

## What it deliberately does **not** model

A case that would need any of these is refused with a non-zero exit rather than
guessed at. A baseline that encodes a guess is worse than a missing case.

- `vSendKeyStepByStep` — the option `send-key-step-by-step` is rejected; only the
  default `0` path is implemented (`OpenKey.mm:770-782`).
- Code tables other than Unicode — `vCodeTable != 0` is an error (`:447-454`).
- `vReplaceMaro` — requires a macro table; a fresh install has none, so the case
  cannot occur and is refused.
- Control keys other than Delete/Return/Tab — no modelled plain-text-field effect.
- Per-application behaviour (browser recommendation, Spotlight, terminals),
  focus changes, and the window manager's own handling of re-posted keys.
- Grapheme clusters wider than one code point: `SendBackspace` deletes one
  grapheme on macOS, and the model assumes NFC, where one code point is one
  grapheme. MAC-002 is the check that proves this equivalence on a real host.

## Script and output format

Scripts are the corpus format from `tests/corpus/README.md`:

```
id <TAB> method <TAB> options <TAB> input [<TAB> expected]
```

- `method`: `telex`, `vni`, `simple_telex_1`, `simple_telex_2`.
- `options`: comma-separated, `-` for defaults. Accepted:
  `spelling`, `no-spelling`, `modern-orthography`, `quick-telex`, `free-mark`,
  `restore-if-wrong-spelling`, `allow-zfwj`, `quick-start-consonant`,
  `quick-end-consonant`, `smart-switch-key`, `no-smart-switch-key`, `macro`,
  `no-macro`, `macro-in-english`, `auto-caps-macro`, `upper-case-first`,
  `temp-off-spelling`. Unknown flags are errors, not ignored — a typo must not
  silently produce a default-configuration baseline.
- `input`: typed characters, with `\b` backspace, `\n` enter, `\t` tab, `\s`
  space, `\\` backslash. Any other escape is an error.
- `expected`: the final document, or `-` for an empty one (the same spelling the
  harness uses for empty output). Four fields mean "replay only, do not judge".

The harness prints one line per key, then one summary line per case:

```
id <TAB> step <TAB> key <TAB> signal <TAB> backspaces <TAB> chars <TAB> document
id <TAB> final <TAB> - <TAB> - <TAB> - <TAB> - <TAB> document
```

`signal` is the engine's `vKeyHookState::code`; `chars` lists the code points the
key appended, in emission order, comma-separated hex; `document` is the text
after the key, with `\\`, `\t`, `\n`, `\r` escaped and `-` for empty. The per-key
lines exist so a disagreement is a diff at a keystroke, not just a different
final string.

Exit status: 0 replayed, 2 bad invocation or malformed script, 3 a case needs
something the model refuses.

`compare.py <script> <results>` judges a run against the `expected` column and
exits 1 on disagreement, 2 if the two do not line up.
