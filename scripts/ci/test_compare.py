#!/usr/bin/env python3
"""Tests for the parity judge (ADR-0008 §1, scripts/parity/compare.py).

The comparator's dangerous failure mode is a false pass: it is the gate's only
verdict on the oracle, so a path that returns success without comparing
anything turns "parity is green" into a claim nobody re-checks. Every test
below is one of the runs a reviewer did by hand on PR #11; the four failing
paths are pinned here so they cannot quietly regress.

Run with the standard library only: `python3 -m unittest discover -s scripts/ci`
"""

from __future__ import annotations

import contextlib
import importlib.util
import io
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

MODULE_PATH = Path(__file__).resolve().parents[1] / "parity" / "compare.py"
_spec = importlib.util.spec_from_file_location("compare", MODULE_PATH)
if _spec is None or _spec.loader is None:
    raise SystemExit(f"cannot load {MODULE_PATH}")
compare = importlib.util.module_from_spec(_spec)
sys.modules["compare"] = compare
_spec.loader.exec_module(compare)

# The harness writes `id\tstep\t...\tdocument`; only the final line matters
# here, and only its first, second and seventh fields.
HEADER = "# id\tstep\tkey\tsignal\tbackspaces\tchars\tdocument"


class Judge(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.dir = Path(self._tmp.name)
        self.script = self.dir / "case.tsv"
        self.results = self.dir / "case.out"

    def run_judge(self, *extra: str) -> tuple[int, str, str]:
        argv = ["compare.py", str(self.script), str(self.results), *extra]
        out, err = io.StringIO(), io.StringIO()
        with (
            mock.patch.object(sys, "argv", argv),
            contextlib.redirect_stdout(out),
            contextlib.redirect_stderr(err),
        ):
            code = compare.main()
        return code, out.getvalue(), err.getvalue()

    def write_script(self, *rows: str) -> None:
        self.script.write_text("\n".join(rows) + "\n", encoding="utf-8")

    def write_results(self, *rows: tuple[str, str]) -> None:
        lines = [HEADER]
        for case_id, document in rows:
            lines.append(f"{case_id}\tfinal\t-\t-\t-\t-\t{document}")
        self.results.write_text("\n".join(lines) + "\n", encoding="utf-8")


class Verdicts(Judge):
    def test_agreement_exits_zero(self) -> None:
        self.write_script("a\ttelex\t-\taa\tâ")
        self.write_results(("a", "â"))
        code, out, err = self.run_judge()
        self.assertEqual((code, err), (0, ""))
        self.assertIn("1/1 cases agree", out)

    def test_wrong_baseline_exits_one_and_names_the_code_point(self) -> None:
        self.write_script("a\ttelex\t-\taa\tâ")
        self.write_results(("a", "a"))
        code, out, err = self.run_judge()
        self.assertEqual(code, 1)
        self.assertEqual(err, "")
        self.assertIn("FAIL a", out)
        self.assertIn("U+00E2 != U+0061", out)

    def test_truncated_results_exit_two(self) -> None:
        self.write_script("a\ttelex\t-\taa\tâ", "b\ttelex\t-\tb\tb")
        self.write_results(("a", "â"))
        code, _, err = self.run_judge()
        self.assertEqual(code, 2)
        self.assertIn("did not report 1 case(s): b", err)

    def test_unasked_case_exits_two(self) -> None:
        self.write_script("a\ttelex\t-\taa\tâ")
        self.write_results(("a", "â"), ("ghost", "x"))
        code, _, err = self.run_judge()
        self.assertEqual(code, 2)
        self.assertIn("not in the script: ghost", err)


class NothingCompared(Judge):
    def test_empty_script_exits_two(self) -> None:
        self.write_script("# nothing here yet")
        self.write_results()
        code, _, err = self.run_judge()
        self.assertEqual(code, 2)
        self.assertIn("nothing was compared", err)

    def test_all_skipped_exits_two_even_with_the_flag(self) -> None:
        self.write_script("a\ttelex\t-\taa", "b\ttelex\t-\tb")
        self.write_results(("a", "â"), ("b", "b"))
        code, _, err = self.run_judge("--allow-missing-expected")
        self.assertEqual(code, 2)
        self.assertIn("nothing was compared", err)


class MissingExpectedColumn(Judge):
    def test_one_column_short_is_an_error_not_a_skip(self) -> None:
        # Dropping the fifth field is a one-column editing slip; letting it
        # through removes the case from the gate and keeps the run green.
        self.write_script("a\ttelex\t-\taa\tâ", "b\ttelex\t-\tbb")
        self.write_results(("a", "â"), ("b", "b"))
        code, out, err = self.run_judge()
        self.assertEqual(code, 2)
        self.assertEqual(out, "")
        self.assertIn("b (line 2)", err)
        self.assertIn("--allow-missing-expected", err)

    def test_the_flag_makes_the_skip_explicit_and_visible(self) -> None:
        self.write_script("a\ttelex\t-\taa\tâ", "b\ttelex\t-\tbb")
        self.write_results(("a", "â"), ("b", "b"))
        code, out, err = self.run_judge("--allow-missing-expected")
        self.assertEqual((code, err), (0, ""))
        self.assertIn("SKIP b", out)
        self.assertIn("1/1 cases agree, 1 skipped", out)


class Escapes(Judge):
    def test_unescape_is_single_pass(self) -> None:
        # File text `a\\nb` is a, backslash, backslash, n, b: one literal
        # backslash followed by an `n`. A replace() chain decodes it to a
        # newline, because the second replace re-reads the first one's output.
        self.assertEqual(compare.unescape("a\\\\nb", "x"), "a\\nb")

    def test_expected_column_decodes_escapes(self) -> None:
        # The backslash passthrough case could not be written before this.
        self.write_script("bs\ttelex\t-\taa\\\\tb\ta\\\\tb")
        self.write_results(("bs", "a\\\\tb"))
        code, out, err = self.run_judge()
        self.assertEqual((code, err), (0, ""))
        self.assertIn("1/1 cases agree", out)

    def test_backslash_is_not_confused_with_a_tab(self) -> None:
        self.write_script("bs\ttelex\t-\taa\\\\tb\ta\\\\tb")
        self.write_results(("bs", "a\\tb"))
        code, out, _ = self.run_judge()
        self.assertEqual(code, 1)
        self.assertIn("FAIL bs", out)

    def test_newline_in_expected_matches_a_real_newline(self) -> None:
        self.write_script("nl\ttelex\t-\taa\tâ\\n")
        self.write_results(("nl", "â\\n"))
        code, out, _ = self.run_judge()
        self.assertEqual(code, 0)
        self.assertIn("1/1 cases agree", out)

    def test_dash_is_the_empty_document_on_both_sides(self) -> None:
        self.write_script("del\ttelex\t-\taa\\b\t-")
        self.write_results(("del", "-"))
        code, _, err = self.run_judge()
        self.assertEqual((code, err), (0, ""))

    def test_unknown_escape_exits_two(self) -> None:
        self.write_script("bad\ttelex\t-\taa\ta\\qb")
        self.write_results(("bad", "a"))
        code, _, err = self.run_judge()
        self.assertEqual(code, 2)
        self.assertIn("unknown escape \\q", err)


class MalformedInput(Judge):
    def test_duplicate_case_id_exits_two(self) -> None:
        self.write_script("a\ttelex\t-\taa\tâ", "a\ttelex\t-\tb\tb")
        self.write_results(("a", "â"))
        code, _, err = self.run_judge()
        self.assertEqual(code, 2)
        self.assertIn("duplicate case id 'a'", err)

    def test_short_result_line_exits_two_instead_of_raising(self) -> None:
        self.write_script("a\ttelex\t-\taa\tâ")
        self.results.write_text(HEADER + "\na\tfinal\t-\t-\t-\t-\n", encoding="utf-8")
        code, _, err = self.run_judge()
        self.assertEqual(code, 2)
        self.assertIn("at least 7 tab-separated fields", err)


if __name__ == "__main__":
    unittest.main()
