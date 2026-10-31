# Spelling check and the restore key

Implemented by C-006 in `crates/openkey-core/src/engine.rs` (`check_spelling`,
`check_restore_if_wrong_spelling`, and the `space`/`break` branches that call
them). This document is C-006's "rules documented" acceptance artefact. Upstream
references are to the pinned oracle `89c2fd3` and default to `Engine.cpp` unless
another file is named.

## What the check decides

`checkSpelling` (`Engine.cpp:176-288`) computes one bit, `tempDisableKey`
(`:284`): "the current word is not a valid Vietnamese syllable". The bit is
consumed in `main_branch`:

```cpp
if (!IS_SPECIALKEY(data) || tempDisableKey) {   // Engine.cpp:1486
    insertKey(data, _isCaps);                   // the key is literal, no tone/mark
} else {
    handleMainKey(data, _isCaps);               // tone/mark keys transform the word
}
```

So while `tempDisableKey` is set, `s`, `f`, `r`, `x`, `j`, `z`, `w` and the
other dedicated keys are inserted as-is instead of modifying the word. The port
mirrors this gate in `main_branch`.

## The stages, in order

`checkSpelling(forceCheckVowel = false)`:

1. **Reset and trim.** `_spellingOK = false`, `_spellingVowelOK = true`,
   `_spellingEndIndex = _index` (`:177-179`). A trailing `]` is excluded from the
   span (`:181-183`), so `]` can close a word without joining the check.
2. **Empty span is valid.** If `_spellingEndIndex == 0` (nothing to judge, or
   only a `]`), `_spellingOK = true` (`:281-282`). This is the only `else` of the
   `_spellingEndIndex > 0` block.
3. **Onset scan.** If the first letter is a consonant, walk `_consonantTable`;
   a row matches when every in-span letter equals it. Two option masks are
   cleared before comparing: `END_CONSONANT_MASK` under `vQuickStartConsonant`
   (quick start allows `f`/`j`/`w` onsets) and `CONSONANT_ALLOW_MASK` under
   `vAllowConsonantZFWJ` (allow `z`/`f`/`w`/`j`) (`:188-206`). The loop leaves
   `j` at the row length when it matches, or at the first mismatched column.
4. **The "d" case.** If `j == _spellingEndIndex` the whole span is an onset
   consonant, so `_spellingOK = true` (`:208-210`).
5. **`qu` and `gìn` corrections.** `CHR(k) == u`, `k > 0`, `k < end - 1` and
   `CHR(k-1) == q` advances `k` and `j` (`:213-216`); `g i <consonant>` rewinds
   `k = j = 1` (`:217-219`).
6. **Vowel scan.** Up to three leading non-consonants advance `k`
   (`:220-226`); `k > j` means the span has a vowel.
7. **Vowel-combination table** — consulted **only** when
   `forceCheckVowel` **and** `k - j > 1` (`:230-245`); otherwise a single
   non-consonant at `j` is accepted on its own (`:246-248`).
8. **End-consonant table.** Match the tail from `k` against `_endConsonantTable`,
   clearing `END_CONSONANT_MASK` under `vQuickEndConsonant`; a row that ends at
   the span makes `_spellingOK = true` (`:253-270`).
9. **`ch` / `t` restriction.** A word ending `ch` or `t` cannot carry `~`, `` ` ``
   or `?`, so `_spellingOK` is cleared when the preceding vowel lacks the allowed
   marks (`:272-279`).

If `k > j` never holds (no vowel was found), upstream leaves `_spellingOK` at
whatever stages 3 and 8 decided — for a lone consonant cluster that is `false`.
The port reproduces this; the defect C-006 fixed is described below.

## What changes the verdict

| Option (port name) | Upstream flag | Effect |
| --- | --- | --- |
| `check_spelling` | `vCheckSpelling` | Master switch; `false` skips the check entirely (`no-spelling`). |
| `temp_off_spelling` | `vTempOffSpellChecking` | Runtime toggle of `vCheckSpelling` (`Engine.cpp:1223-1227`); `vSetCheckSpelling` snapshots the pre-session value (`:1229-1231`). |
| `allow_consonant_zfwj` | `vAllowConsonantZFWJ` | Clears `CONSONANT_ALLOW_MASK` in the onset scan, letting `z`/`j` start a syllable. |
| `quick_start_consonant` | `vQuickStartConsonant` | Clears `END_CONSONANT_MASK` in the onset scan. |
| `quick_end_consonant` | `vQuickEndConsonant` | Clears `END_CONSONANT_MASK` in the end-consonant scan. |
| `force_check_vowel` (argument) | `forceCheckVowel` | Enables the vowel-combination table; the space and break branches pass `true`. |

`Options::default()` is the macOS first-run profile: the check is on, and the
restore flag is off.

## Where the check runs

- `insertKey` after each literal insertion, non-forced (`Engine.cpp:361-362`).
- `restoreLastTypingState` when a saved special-character word is replayed,
  non-forced (`Engine.cpp:435-455`, call at `:446`).
- The space branch, forced (`Engine.cpp:1385-1386`).
- The break branch, forced (`Engine.cpp:1336-1337`).
- The delete branch, non-forced (`Engine.cpp:1449`).

The port calls `check_spelling(false)` from `insert_key`, `delete_branch` and
`restore_last_typing_state`, and `check_spelling(true)` from `space_branch` and
`break_branch`.

## The restore key

`checkRestoreIfWrongSpelling(handleCode)` (`Engine.cpp:1204-1221`) walks the
word and, if any **non-consonant** letter carries `MARK_MASK`, `TONE_MASK` or
`TONEW_MASK`, replaces the word with its raw keystrokes:

```
hCode = handleCode;  hBPC = _index;  hNCC = _stateIndex;
for (i = 0; i < _stateIndex; i++) {
    TypingWord[i] = KeyStates[i];          // raw keys, no transformation
    hData[_stateIndex - 1 - i] = TypingWord[i];  // charData is last-char-first
}
_index = _stateIndex;
```

It is gated by `vRestoreIfWrongSpelling` (`restore_if_invalid` in the port,
**default false**) and by `tempDisableKey`: only a word the spelling check has
already rejected is restored. Two call sites:

| Branch | Handle code | Upstream | Port |
| --- | --- | --- | --- |
| Break key (`.`, `,`, `;`, space via `isWordBreak`, …) | `vRestoreAndStartNewSession` (5) | `Engine.cpp:1335-1341` | `break_branch` |
| Space key | `vRestore` (3) | `Engine.cpp:1384-1397` | `space_branch` |

The edit is `hBPC` backspaces followed by the raw `KeyStates` (the break key
itself is re-posted by the platform layer, not by the engine). Because the flag
defaults false, this path is dead under the macOS first-run profile; the corpus
exercises it with `restore-if-wrong-spelling`.

## Two behaviours the port carries rather than fixes

1. **A break restore leaves the raw keys in the engine word.** Upstream sets
   `_index = _stateIndex` and does not call `startNewSession` for
   `vRestoreAndStartNewSession`; starting the new session is the platform's job
   (`OpenKey.mm:492-507`). The port preserves this: after `asd.` the engine word
   is `asd`, and the document is `asd.`.
2. **`charData` is stored last-character-first.** `hData[_stateIndex - 1 - i]`
   puts the first typed key at the highest index, and the consumer walks
   `charData` backwards (`OpenKey.mm:423-480`). `Engine::edit_text` renders it in
   reading order; the raw ordering is part of the hook contract, not an accident.

## The defect C-006 fixed

The port had an extra `else { spelling_ok = true; }` on the `k == j` path — the
no-vowel case. Upstream leaves `_spellingOK` false there (`Engine.cpp:229-280`),
and only the `_spellingEndIndex == 0` branch sets it true (`:281-282`). Deleting
the extra `else` restores the upstream verdicts: `zzas` stays `zzas` and `xq`
stays `xq` with the check on, while `no-spelling` still tones `zzas` to `zzá`.
The three red `edit` cases (`sp-on-zzas`, `sp-on-xq`, `sp-on-hoc`) were this
symptom, not corpus defects.

## Corpus

`tests/corpus/spelling-boundaries.tsv` has 17 rows. The restore rows cover the
flag itself (`sp-restore-asdf`, `-viet`, `-hi`, `-xyz`, harness baseline), the
break branch (`sp-restore-break`, `asd.`), the space branch
(`sp-restore-space`, `asd` + space), and the flag-off control
(`sp-restore-off`, `asd` + space → `ád `). With the `edit` block active the
parity run judges 193 cases; every in-scope case must be judged.
