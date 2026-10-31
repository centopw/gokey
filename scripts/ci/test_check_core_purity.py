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


class LetterIndexing(unittest.TestCase):
    def test_direct_indexing_is_flagged(self) -> None:
        self.assertTrue(scanner.indexes_letters("let first = self.letters[0];"))

    def test_space_before_the_bracket_is_flagged(self) -> None:
        self.assertTrue(scanner.indexes_letters("let first = word.letters [0];"))

    def test_get_is_clean(self) -> None:
        self.assertFalse(scanner.indexes_letters("let first = word.get(0);"))

    def test_slice_accessor_is_clean(self) -> None:
        self.assertFalse(scanner.indexes_letters("let all = word.letters();"))

    def test_comment_is_ignored(self) -> None:
        self.assertFalse(scanner.indexes_letters("// letters[ is only for word.rs"))


if __name__ == "__main__":
    unittest.main()
