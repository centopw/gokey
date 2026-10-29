# Parity corpus

Golden cases shared by the Rust engine and the reference C++ engine.

Format: tab-separated, one case per line, `#` starts a comment.

```
id <TAB> method <TAB> options <TAB> input <TAB> expected
```

- `method`: `telex`, `vni`, `simple_telex_1`, `simple_telex_2`
- `options`: comma-separated flags, or `-` for defaults. Rendered as JSON in the
  harness before it reaches the engine so cases stay readable.
- `input`: characters typed literally, with escapes:
  - `\b` backspace
  - `\n` enter
  - `\t` tab
  - `\s` space
  - `\\` literal backslash
- `expected`: final text in the document, Unicode NFC, no quoting, using the
  same escapes as `input` (`\b \n \t \s \\`) — decoded in one pass, so a real
  backslash followed by `n` is written `\\n`. `-` means the empty document. A
  five-column row is required; the judge refuses a four-column one unless it
  is asked for with `--allow-missing-expected`.

Example:

```
telex-basic	telex	-	vieejt	việt
telex-d-stroke	telex	-	dd	đ
vni-basic	vni	-	vie65t	việt
```

Rules:

1. Every published example in upstream README/CHANGELOG must appear here
   (100% pass required).
2. Cases that the engine gets "wrong" but upstream also gets wrong are recorded
   with an `xfail:` prefix on the id and a comment pointing at the upstream
   behaviour. Do not "fix" them silently.
3. The corpus is executed both against `openkey-cli replay` and against the
   reference C++ harness; a case only counts as agreed when both produce the
   same output.

Status of rules 2 and 3 in v0:

- `compare.py` has no `xfail:` semantics yet (a prefix would have to be
  stripped before the harness sees the id), so the cases where upstream is
  "wrong" — unbroken English words, `w` never reaching the `qu` shortcut,
  Simple Telex 1's tone placement — are pinned with the engine's actual output
  as `expected` and a comment naming the upstream behaviour. They are recorded,
  not silenced; adding `xfail:` support is a K-004 follow-up.
- `openkey-cli replay` does not exist yet (K-005), so today the corpus is
  judged against the C++ harness only. Rule 3 starts holding when K-005 lands,
  and `scripts/parity.sh` is where both runs will meet.

Files: `telex-basic`, `telex-tones`, `telex-words`, `telex-syllables`,
`vni-basic`, `vni-words`, `simple-telex`, `editing`, `options`.

Owned by: QA/parity engineer (R5). Reviewed by: linguistics reviewer (R11).
