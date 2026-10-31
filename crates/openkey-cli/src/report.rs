//! Per-case report and exit status (K-005).
//!
//! K-005's acceptance is a command plus a per-case report, not an agreement
//! percentage: `Engine::apply` is a stub until C-002, so a percentage over a
//! mostly-pending run would be a number with no meaning. The report therefore
//! carries one status per case and the summary counts statuses.
//!
//! Statuses:
//! - `PASS` — document equals the corpus `expected` column (and the reference
//!   engine, when `--diff ref-engine` was requested).
//! - `FAIL` — document differs, with a machine-readable class and the first
//!   differing code point.
//! - `PENDING` — the engine is not implemented for this key yet (`todo!()`).
//! - `SKIP` — the row has no expected column and `--allow-missing-expected` was
//!   passed.
//! - `INVALID` — the row asks for something the replay cannot model.
//!
//! Exit status: 0 every case passed, 1 at least one case failed, 2 the run
//! could not produce a verdict (pending or invalid cases, or an empty run).
//! Pending and failing are kept apart for the same reason `parity.sh` keeps
//! "could not start" apart from "cases disagree".

use std::fmt::Write as _;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Pass,
    Fail,
    Pending,
    Skip,
    Invalid,
}

#[derive(Debug, Clone)]
pub struct Row {
    pub id: String,
    pub status: Status,
    /// Machine-readable class for failures: `expected`, `reference-engine`,
    /// `engine-panic`.
    pub class: Option<&'static str>,
    pub detail: Option<String>,
}

impl Row {
    pub fn pass(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            status: Status::Pass,
            class: None,
            detail: None,
        }
    }

    pub fn fail(id: impl Into<String>, class: &'static str, detail: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            status: Status::Fail,
            class: Some(class),
            detail: Some(detail.into()),
        }
    }

    pub fn pending(id: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            status: Status::Pending,
            class: None,
            detail: Some(detail.into()),
        }
    }

    pub fn skip(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            status: Status::Skip,
            class: None,
            detail: Some("no expected column (--allow-missing-expected)".to_string()),
        }
    }

    pub fn invalid(id: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            status: Status::Invalid,
            class: None,
            detail: Some(detail.into()),
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Summary {
    pub pass: usize,
    pub fail: usize,
    pub pending: usize,
    pub skip: usize,
    pub invalid: usize,
    /// Cases whose document was compared against the reference engine.
    pub ref_checked: usize,
    /// Cases whose document differed from the reference engine.
    pub ref_differ: usize,
}

impl Summary {
    pub fn from_rows(rows: &[Row]) -> Self {
        let mut summary = Self::default();
        for row in rows {
            match row.status {
                Status::Pass => summary.pass += 1,
                Status::Fail => {
                    summary.fail += 1;
                    if row.class == Some("reference-engine") {
                        summary.ref_differ += 1;
                    }
                }
                Status::Pending => summary.pending += 1,
                Status::Skip => summary.skip += 1,
                Status::Invalid => summary.invalid += 1,
            }
        }
        summary
    }

    pub fn exit_code(&self) -> i32 {
        if self.fail > 0 {
            1
        } else if self.pending > 0 || self.invalid > 0 || self.pass == 0 {
            2
        } else {
            0
        }
    }
}

/// Render one document the way the harness does (`-` for empty).
pub fn escape(document: &str) -> String {
    if document.is_empty() {
        return "-".to_string();
    }
    let mut out = String::with_capacity(document.len());
    for ch in document.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\u{8}' => out.push_str("\\b"),
            _ => out.push(ch),
        }
    }
    out
}

/// Name the first difference between two documents.
pub fn first_difference(expected: &str, actual: &str) -> String {
    for (index, (left, right)) in expected.chars().zip(actual.chars()).enumerate() {
        if left != right {
            return format!(
                "first difference at code point {index}: expected U+{:04X} {:?}, actual U+{:04X} {:?}",
                left as u32, left, right as u32, right
            );
        }
    }
    format!(
        "one is a prefix of the other: expected {} code point(s), actual {}",
        expected.chars().count(),
        actual.chars().count()
    )
}

pub fn render(rows: &[Row], summary: &Summary, ref_engine: Option<&str>) -> String {
    let mut out = String::new();
    for row in rows {
        match row.status {
            Status::Pass => {
                let _ = writeln!(out, "PASS {}", row.id);
            }
            Status::Fail => {
                let class = row.class.unwrap_or("unknown");
                let _ = writeln!(out, "FAIL {} [{}]", row.id, class);
                if let Some(detail) = &row.detail {
                    let _ = writeln!(out, "  {detail}");
                }
            }
            Status::Pending => {
                let _ = writeln!(out, "PENDING {}", row.id);
                if let Some(detail) = &row.detail {
                    let _ = writeln!(out, "  {detail}");
                }
            }
            Status::Skip => {
                let _ = writeln!(out, "SKIP {}", row.id);
            }
            Status::Invalid => {
                let _ = writeln!(out, "INVALID {}", row.id);
                if let Some(detail) = &row.detail {
                    let _ = writeln!(out, "  {detail}");
                }
            }
        }
    }

    let _ = writeln!(
        out,
        "summary: {} pass, {} fail, {} pending, {} skip, {} invalid",
        summary.pass, summary.fail, summary.pending, summary.skip, summary.invalid
    );
    match ref_engine {
        Some(binary) => {
            let _ = writeln!(
                out,
                "reference engine ({binary}): {} compared, {} differ",
                summary.ref_checked, summary.ref_differ
            );
        }
        None => {
            let _ = writeln!(out, "reference engine: not run (--diff ref-engine)");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_exit_codes_keep_pending_and_failure_apart() {
        let pass = Summary {
            pass: 1,
            ..Summary::default()
        };
        assert_eq!(pass.exit_code(), 0);

        let pending = Summary {
            pending: 1,
            ..Summary::default()
        };
        assert_eq!(pending.exit_code(), 2);

        let fail = Summary {
            pass: 1,
            fail: 1,
            ..Summary::default()
        };
        assert_eq!(fail.exit_code(), 1);

        let empty = Summary::default();
        assert_eq!(empty.exit_code(), 2, "an empty run is not a pass");
    }

    #[test]
    fn reference_differences_are_counted_from_the_class() {
        let rows = vec![
            Row::fail("a", "reference-engine", "diff"),
            Row::fail("b", "expected", "diff"),
        ];
        let summary = Summary::from_rows(&rows);
        assert_eq!(summary.fail, 2);
        assert_eq!(summary.ref_differ, 1);
    }

    #[test]
    fn escape_matches_the_harness_vocabulary() {
        assert_eq!(escape(""), "-");
        assert_eq!(escape("a b\n\t\\"), "a b\\n\\t\\\\");
        assert_eq!(escape("việt"), "việt");
    }

    #[test]
    fn first_difference_names_the_code_point() {
        let message = first_difference("việt", "viet");
        assert!(message.contains("code point 2"), "{message}");
        let prefix = first_difference("vi", "việt");
        assert!(prefix.contains("prefix"), "{prefix}");
    }

    #[test]
    fn render_reports_each_status_and_the_summary() {
        let rows = vec![
            Row::pass("a"),
            Row::fail("b", "expected", first_difference("x", "y")),
            Row::pending("c", "not yet implemented"),
            Row::skip("d"),
            Row::invalid("e", "bad flag"),
        ];
        let summary = Summary::from_rows(&rows);
        let text = render(&rows, &summary, Some(".oracle/build/harness"));
        assert!(text.contains("PASS a"));
        assert!(text.contains("FAIL b [expected]"));
        assert!(text.contains("PENDING c"));
        assert!(text.contains("SKIP d"));
        assert!(text.contains("INVALID e"));
        assert!(text.contains("1 pass, 1 fail, 1 pending, 1 skip, 1 invalid"));
        assert!(text.contains("0 compared, 0 differ"));
    }
}
