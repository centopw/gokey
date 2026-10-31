#!/usr/bin/env python3
"""Tests for the core-purity scanner (ADR-0008 §6).

The scanner's dangerous failure mode is a false *pass*: a gate that reports
success while the property it asserts is false is worse than no gate at all,
because it is a guarantee nobody checks again. Each test below pins one shape
that a line-at-a-time matcher silently missed, plus a negative case so the suite
cannot pass vacuously if expansion ever starts flagging everything.

Run with the standard library only: `python3 -m unittest discover -s scripts/ci`
"""

from __future__ import annotations

import importlib.util
import sys
import unittest
from pathlib import Path

MODULE_PATH = Path(__file__).with_name("check-core-purity.py")
_spec = importlib.util.spec_from_file_location("check_core_purity", MODULE_PATH)
if _spec is None or _spec.loader is None:
    raise SystemExit(f"cannot load {MODULE_PATH}")
scanner = importlib.util.module_from_spec(_spec)
sys.modules["check_core_purity"] = scanner
_spec.loader.exec_module(scanner)


def flagged(source: str) -> set[str]:
    return {
        token
        for _lineno, line in scanner.logical_lines(source)
        for token in scanner.offending_tokens(line)
    }


class UseTreeExpansion(unittest.TestCase):
    def test_grouped_import_is_expanded(self) -> None:
        # The idiomatic form rustfmt emits: neither path appears verbatim.
        self.assertEqual(flagged("use std::{fs, net};\n"), {"std::fs", "std::net"})

    def test_aliased_import_is_caught(self) -> None:
        # ADR-0008 previously claimed this was the blind spot; it never was.
        self.assertIn("std::fs", flagged("use std::fs as f;\n"))

    def test_alias_inside_group_is_caught(self) -> None:
        self.assertIn("std::fs", flagged("use std::{fs as f, io};\n"))

    def test_nested_group_is_expanded(self) -> None:
        self.assertEqual(
            flagged("use std::{fs::File, net::TcpStream};\n"),
            {"std::fs", "std::net"},
        )

    def test_self_import_keeps_the_prefix(self) -> None:
        self.assertIn("std::fs", flagged("use std::fs::{self, File};\n"))

    def test_multiline_tree_is_joined(self) -> None:
        source = "use std::{\n    fs,\n    net,\n};\n"
        self.assertEqual(flagged(source), {"std::fs", "std::net"})

    def test_visibility_prefix_is_handled(self) -> None:
        self.assertIn("std::env", flagged("pub(crate) use std::{env, fmt};\n"))

    def test_fully_qualified_path_in_body(self) -> None:
        source = 'fn f() {\n    let _ = std::process::exit(1);\n}\n'
        self.assertIn("std::process", flagged(source))

    def test_unrelated_std_use_is_clean(self) -> None:
        self.assertEqual(flagged("use std::{fmt, io, ops::Range};\n"), set())


class ScopeRules(unittest.TestCase):
    def test_cfg_test_items_are_excluded(self) -> None:
        source = "#[cfg(test)]\nmod tests {\n    use std::fs;\n\n    #[test]\n    fn t() {}\n}\n"
        self.assertEqual(flagged(source), set())

    def test_cfg_test_does_not_hide_the_next_item(self) -> None:
        source = "#[cfg(test)]\nmod tests {\n    use std::fs;\n}\n\nuse std::net;\n"
        self.assertEqual(flagged(source), {"std::net"})

    def test_comment_only_lines_are_ignored(self) -> None:
        self.assertEqual(flagged("// std::fs is banned here\n"), set())


class BannedConstructs(unittest.TestCase):
    def test_unsafe_is_caught_without_matching_the_identifier(self) -> None:
        self.assertIn("unsafe", flagged("pub unsafe fn f() {}\n"))
        self.assertEqual(flagged("fn unsafely() {}\n"), set())

    def test_static_mut_is_caught(self) -> None:
        self.assertIn("static mut", flagged("static mut COUNTER: u32 = 0;\n"))

    def test_clean_module_is_clean(self) -> None:
        source = "use std::fmt;\n\npub fn render(out: &mut String) {\n    out.push('x');\n}\n"
        self.assertEqual(flagged(source), set())


class WordEncapsulation(unittest.TestCase):
    """ADR-0002 D3: `Word::letters` is indexed only in `word.rs`."""

    def test_direct_indexing_is_flagged(self) -> None:
        self.assertEqual(scanner.word_index_offences("self.letters[i] = l;\n"), [1])
        self.assertEqual(scanner.word_index_offences("let l = w.letters [ 0 ];\n"), [1])

    def test_comment_is_not_code(self) -> None:
        self.assertEqual(
            scanner.word_index_offences("// letters[0] lives in word.rs\n"), []
        )

    def test_field_access_without_indexing_is_out_of_scope(self) -> None:
        # Only indexing is banned. Reading the length cannot smuggle a letter
        # out, and flagging it would make the rule about the field name rather
        # than about the layout escaping its owner.
        self.assertEqual(scanner.word_index_offences("let n = self.letters.len();\n"), [])

    def test_test_modules_are_scanned_too(self) -> None:
        # `logical_lines()` drops `#[cfg(test)]` items for the I/O rules; this
        # scan must not, because a test outside `word.rs` is equally unentitled
        # to index the field (the scanner's docstring says so).
        source = (
            "#[cfg(test)]\nmod tests {\n    fn t(w: &Word) { let _ = w.letters[0]; }\n}\n"
        )
        self.assertEqual(scanner.word_index_offences(source), [3])

    def test_line_numbers_are_reported(self) -> None:
        source = "fn a() {}\n\nfn b(w: &Word) -> Letter { w.letters[1] }\n"
        self.assertEqual(scanner.word_index_offences(source), [3])

    def test_this_repository_is_clean(self) -> None:
        # The gate against the real tree: a future commit that indexes the
        # field outside `word.rs` fails here with its file and line.
        self.assertEqual(scanner.scan_word_encapsulation(), [])


if __name__ == "__main__":
    unittest.main()
