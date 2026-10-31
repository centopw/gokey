//! CLI contract tests for `openkey replay` (K-005).
//!
//! The engine is a stub until C-002, so these tests assert the report's shape
//! and the error paths, not a parity verdict. The status token for a case is
//! allowed to change from `PENDING` to `PASS`/`FAIL` when C-002 lands; the
//! per-case line and the summary must keep existing.

use std::path::PathBuf;
use std::process::Command;

struct Run {
    code: i32,
    stdout: String,
    stderr: String,
}

fn invoke(args: &[&str]) -> Run {
    let output = Command::new(env!("CARGO_BIN_EXE_openkey"))
        .args(args)
        .output()
        .expect("openkey binary runs");
    Run {
        code: output
            .status
            .code()
            .expect("the binary exits normally, not on a signal"),
        stdout: String::from_utf8(output.stdout).expect("stdout is UTF-8"),
        stderr: String::from_utf8(output.stderr).expect("stderr is UTF-8"),
    }
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn corpus(name: &str) -> String {
    fixture(name).display().to_string()
}

fn status_of(stdout: &str, id: &str) -> Option<&'static str> {
    for token in ["PASS", "FAIL", "PENDING", "SKIP", "INVALID"] {
        let prefix = format!("{token} {id}");
        if stdout
            .lines()
            .any(|line| line == prefix || line.starts_with(&format!("{prefix} [")))
        {
            return Some(token);
        }
    }
    None
}

#[test]
fn replay_reports_one_status_per_case_and_a_summary() {
    let run = invoke(&["replay", "--corpus", &corpus("basic.tsv")]);
    assert!(
        run.code == 0 || run.code == 1 || run.code == 2,
        "unexpected exit {}: {}",
        run.code,
        run.stderr
    );
    assert!(
        status_of(&run.stdout, "case-one").is_some(),
        "no status line for case-one:\n{}",
        run.stdout
    );
    assert!(
        status_of(&run.stdout, "case-two").is_some(),
        "no status line for case-two:\n{}",
        run.stdout
    );
    assert!(
        run.stdout.contains("summary: ") && run.stdout.contains("reference engine: not run"),
        "summary missing:\n{}",
        run.stdout
    );
    assert!(
        !run.stderr.contains("panicked"),
        "engine panics must be reported as statuses, not traces:\n{}",
        run.stderr
    );
}

#[test]
fn a_missing_corpus_path_is_an_error() {
    let run = invoke(&["replay", "--corpus", "/nonexistent/corpus"]);
    assert_eq!(run.code, 2);
    assert!(run.stderr.contains("does not exist"), "{}", run.stderr);
}

#[test]
fn a_row_without_expected_is_refused_by_default() {
    let run = invoke(&["replay", "--corpus", &corpus("no-expected.tsv")]);
    assert_eq!(run.code, 2);
    assert_eq!(status_of(&run.stdout, "case-three"), Some("INVALID"));
    assert!(run.stdout.contains("no expected column"), "{}", run.stdout);
}

#[test]
fn a_row_without_expected_is_a_skip_only_when_asked() {
    let run = invoke(&[
        "replay",
        "--corpus",
        &corpus("no-expected.tsv"),
        "--allow-missing-expected",
    ]);
    // Replay-only rows are still replayed, so while the engine is a stub the
    // case reports PENDING; once C-002 lands it reports SKIP. Neither may count
    // as a pass.
    let status = status_of(&run.stdout, "case-three");
    assert!(
        status == Some("SKIP") || status == Some("PENDING"),
        "expected SKIP or PENDING, got {status:?}:\n{}",
        run.stdout
    );
    assert!(
        run.stdout.contains("0 pass"),
        "an all-skipped run must not look like a pass:\n{}",
        run.stdout
    );
    assert_eq!(run.code, 2);
}

#[test]
fn unknown_arguments_are_refused() {
    let run = invoke(&["replay", "--corpus", "x", "--diff", "oracle"]);
    assert_eq!(run.code, 2);
    assert!(
        run.stderr.contains("unknown --diff value"),
        "{}",
        run.stderr
    );

    let run = invoke(&["replay", "--corpus", "x", "--wat"]);
    assert_eq!(run.code, 2);
    assert!(
        run.stderr.contains("unknown replay argument"),
        "{}",
        run.stderr
    );

    let run = invoke(&["frobnicate"]);
    assert_eq!(run.code, 2);
    assert!(run.stderr.contains("unknown command"), "{}", run.stderr);
}

#[test]
fn a_missing_reference_engine_stops_before_replaying() {
    let run = invoke(&[
        "replay",
        "--corpus",
        &corpus("basic.tsv"),
        "--diff",
        "ref-engine",
        "--ref",
        "/nonexistent/harness",
    ]);
    assert_eq!(run.code, 2);
    assert!(
        run.stderr.contains("reference engine") && run.stderr.contains("scripts/parity.sh"),
        "{}",
        run.stderr
    );
    assert!(
        run.stdout.is_empty(),
        "nothing was replayed:\n{}",
        run.stdout
    );
}
