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
- `openkey-cli replay` exists since K-005 and reports one status per case
  (`PASS` / `FAIL` / `PENDING` / `SKIP` / `INVALID`) plus a summary, with
  `--diff ref-engine` adding the reference harness as a second judge. While
  `Engine::apply` is a stub (`todo!()` until C-002) every case reports
  `PENDING` and the command exits 2: the port has no document to compare yet,
  so no agreement is claimed. Rule 3 starts holding when C-002 lands and
  `scripts/parity.sh` is wired to run both sides.

Files: `telex-basic`, `telex-tones`, `telex-words`, `telex-syllables`,
`vni-basic`, `vni-words`, `simple-telex`, `editing`, `options`.

Owned by: QA/parity engineer (R5). Reviewed by: linguistics reviewer (R11).
