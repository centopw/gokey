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
./scripts/parity.sh --full                # same file set; selects streams, not scope
.oracle/build/harness some.tsv            # replay one script by hand
python3 scripts/parity/compare.py some.tsv <( \
    .oracle/build/harness some.tsv)       # ...and judge it
cargo run -p openkey-cli -- replay \
    --corpus tests/corpus/telex-basic.tsv --scope parity/scope.txt \
    --diff ref-engine --ref .oracle/build/harness   # judge it with the Rust port
```

`parity.sh` verifies the pin first (`scripts/ci/check-oracle-pin.py`), fetches the
upstream tree by **commit hash** at `parity/oracle/pin.json → upstream.commit`
if `.oracle/src` is absent, builds with plain `clang++ -std=c++17 -O2 -w`, then
replays `scripts/parity/smoke.tsv` and every `tests/corpus/*.tsv` it finds. A case
whose `expected` column is absent is replayed but not judged; a case whose
`expected` column disagrees fails the script. After that oracle half, it judges
the same scripts with the Rust port — see [The Rust judge](#the-rust-judge-k-005).

The oracle half always replays the whole file set. The **judged** set is
`parity/scope.txt` (see [Scope](#scope-k-005-turn-3)): a block that is defined
but not active is named and skipped, and an in-scope case that is not judged
keeps the job non-green. `--full` selects the file/stream set and never the
judged set — the nightly passes it, and an inactive block stays skipped there.

`parity/oracle/pin.json` is the machine-read copy of the record the decisions are
argued from, `docs/m0/ORACLE-PIN.json` (PORTING-MAP §5). The two must be the same
bytes, and the checker is what enforces that: it prints both sha256 digests and
exits 1 if they differ, and exits 2 if either file is missing, before it fetches
anything. A copy that has drifted is not a second opinion — it is a second truth,
so refresh it from the record (`cp docs/m0/ORACLE-PIN.json parity/oracle/pin.json`)
rather than editing it.

The build adds `-include algorithm` because upstream `ConvertTool.cpp:169` calls
`std::find` without including `<algorithm>`: libc++ pulls the header in
transitively, libstdc++ (the CI runner's) does not. Forcing the header keeps the
pinned tree byte-identical — patching it would break the digests the step above
just verified — and picks the same `std::find`.

## Pin and platform

- The normative platform is **macOS** (`parity/oracle/pin.json`, i.e.
  `docs/m0/ORACLE-PIN.json`, DL-026). The
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
  harness uses for empty output). It uses the same escape table as `input`
  (`\b \n \t \r \s \\`), decoded in a single pass, so a document that really
  holds a backslash followed by `n` is written `\\n` and is not a newline.
  Four fields mean "replay only"; the judge refuses that shape unless it is
  asked for with `--allow-missing-expected`, because one dropped tab removes
  the case from the gate and leaves the run green.

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

`compare.py <script> <results>` judges a run against the `expected` column:
0 every case agrees, 1 a case disagrees (the report names the case and the
first differing code point), 2 it could not judge — the two files do not line
up, a case lost its `expected` column, or nothing was compared at all. A script
whose cases all skip is exit 2, not a green run: the judge is the gate's only
verdict on the oracle, so "0/0 cases agree" must never be a pass.

## The Rust judge (K-005)

The oracle half above judges the *baselines*: it says the `.tsv` expected
column matches the reference engine. It says nothing about the port. `parity.sh`
therefore runs a second judge over the same scripts:

```
cargo run -q --locked -p openkey-cli -- replay \
    --corpus <script> --scope parity/scope.txt \
    --diff ref-engine --ref .oracle/build/harness
```

`openkey replay` reports one status per case — `PASS`, `FAIL`, `PENDING`,
`SKIP`, `INVALID` — then a summary line and a scope line. With `--diff
ref-engine` the reference harness is a second, independent judge: a case whose
expected column agrees but whose Rust output differs from the oracle fails with
class `reference-engine`, and the detail names the first differing code point.
The gate does not pass `--allow-missing-expected`, so a row that lost its fifth
field is `INVALID` (a corpus defect, visible), not a silent skip.

Exit status, and why pending is separate from pass:

- `0` — every **in-scope** case was judged and all pass. With `--scope`, a run
  whose every case is in an inactive block is also `0`: nothing declared is
  unjudged. That is not parity, and the counts line says so.
- `1` — at least one judged case fails (expected column or reference engine).
- `2` — no verdict: an in-scope case was left unjudged (pending, invalid, or a
  skipped in-scope row), or the build, corpus, scope or reference engine could
  not run. `parity.sh` prints the state by name
  (`::error::parity judge: no verdict — …`) instead of folding it into green.

`PENDING` exists because `Engine::apply` is `todo!()` until C-002: the replay
catches that panic and reports `engine-pending` rather than inventing a
document. Pending **inside** the declared scope keeps the job at 2 — folding
unjudged in-scope cases into a green job is the false green this gate is built
to prevent. Pending **outside** it is `SKIP`, named per case and counted. The
same contract binds C-014's nightly gate.

## Scope (K-005 turn 3)

`parity/scope.txt` declares which task owns each case. Grammar:

```
block: <name> <selector>[,<selector>...]
active: <name>[,<name>...]
```

Selectors, most specific first: an exact id, `id:<glob>` (`*`, `?`), and
`file:<path>` (a path suffix, so a fixture may use a bare file name). Every case
must resolve to exactly one block; two matches at the same specificity are
ambiguous.

Definition and activation are separate. All five blocks are defined at K-005
(`telex`, `vni`, `simple`, `edit`, `opt`) so every case resolves and an
unclassified case is always an error; `active:` starts empty and grows one line
per task (C-002 `telex`, C-006 `edit`, C-003 `vni`, C-004 `simple`, C-008
`opt`). At C-014 `active:` lists all five, so no skip path remains.

- A case in an inactive block is `SKIP` with the block name, and is neither
  replayed nor validated; the oracle half still replays every script, so a
  malformed row is caught there.
- An unknown block name or an active block that matches no case is exit 2: a
  selector that stopped matching must not look like a clean run.
- A green job prints `judged N / in-scope M / skipped K` twice — the
  `::warning::` line and `$GITHUB_STEP_SUMMARY` — so green reads as "nothing
  declared is unjudged", never as parity.

`scripts/parity.sh` propagates the oracle comparator's 2 as "could not judge"
rather than folding it into disagreement, and treats any judge exit other than
0/1/2 (a build failure, cargo exit 101) as "could not start".
