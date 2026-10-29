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
- `expected`: final text in the document, Unicode NFC, no quoting.

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

Owned by: QA/parity engineer (R5). Reviewed by: linguistics reviewer (R11).
