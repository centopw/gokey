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
    let owned: Vec<String> = args.iter().map(|arg| arg.to_string()).collect();
    invoke_owned(&owned)
}

fn invoke_owned(args: &[String]) -> Run {
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

// --- scope (K-005 turn 3) -------------------------------------------------
//
// These assert the split PA asked for: a run with nothing declared in scope is
// green (`judged 0 / in-scope 0 / skipped N`), while an in-scope case left
// unjudged — pending, invalid, or a skipped in-scope row — stays at exit 2.
// Green means "nothing declared is unjudged", never parity.

fn scoped(args: &[&str], scope: &str) -> Run {
    let mut owned: Vec<String> = args.iter().map(|arg| arg.to_string()).collect();
    owned.push("--scope".to_string());
    owned.push(corpus(scope));
    invoke_owned(&owned)
}

#[test]
fn an_all_inactive_scope_skips_every_case_and_is_green() {
    let run = scoped(
        &["replay", "--corpus", &corpus("basic.tsv")],
        "scope-inactive.txt",
    );
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(status_of(&run.stdout, "case-one"), Some("SKIP"));
    assert_eq!(status_of(&run.stdout, "case-two"), Some("SKIP"));
    assert!(
        run.stdout
            .contains("scope: judged 0 / in-scope 0 / skipped 2"),
        "{}",
        run.stdout
    );
}

#[test]
fn an_in_scope_pending_case_is_not_green() {
    let run = scoped(
        &["replay", "--corpus", &corpus("basic.tsv")],
        "scope-active.txt",
    );
    assert_eq!(run.code, 2, "{}", run.stdout);
    assert_eq!(status_of(&run.stdout, "case-one"), Some("PENDING"));
    assert_eq!(status_of(&run.stdout, "case-two"), Some("PENDING"));
    assert!(
        run.stdout
            .contains("scope: judged 0 / in-scope 2 / skipped 0"),
        "{}",
        run.stdout
    );
}

#[test]
fn only_active_blocks_are_judged() {
    let run = scoped(
        &["replay", "--corpus", &corpus("basic.tsv")],
        "scope-mixed.txt",
    );
    assert_eq!(run.code, 2, "{}", run.stdout);
    assert_eq!(status_of(&run.stdout, "case-one"), Some("PENDING"));
    assert_eq!(status_of(&run.stdout, "case-two"), Some("SKIP"));
    assert!(
        run.stdout
            .contains("scope: judged 0 / in-scope 1 / skipped 1"),
        "{}",
        run.stdout
    );
}

#[test]
fn an_in_scope_skip_is_still_unjudged() {
    let run = scoped(
        &[
            "replay",
            "--corpus",
            &corpus("no-expected.tsv"),
            "--allow-missing-expected",
        ],
        "scope-no-expected.txt",
    );
    assert_eq!(
        run.code, 2,
        "an in-scope skip is not a verdict:\n{}",
        run.stdout
    );
    // While the engine is a stub this row is PENDING; once C-002 lands it is the
    // in-scope SKIP judge() produces. Either way it is unjudged, not green.
    let status = status_of(&run.stdout, "case-three");
    assert!(
        status == Some("PENDING") || status == Some("SKIP"),
        "expected PENDING or SKIP, got {status:?}:\n{}",
        run.stdout
    );
    assert!(
        run.stdout
            .contains("scope: judged 0 / in-scope 1 / skipped 0"),
        "{}",
        run.stdout
    );
}

#[test]
fn an_unclassified_case_is_exit_two_with_nothing_replayed() {
    let run = scoped(
        &["replay", "--corpus", &corpus("basic.tsv")],
        "scope-unclassified.txt",
    );
    assert_eq!(run.code, 2);
    assert!(run.stderr.contains("not classified"), "{}", run.stderr);
    assert!(run.stdout.is_empty(), "{}", run.stdout);
}

#[test]
fn an_active_block_that_matches_nothing_anywhere_is_exit_two() {
    let run = invoke(&[
        "scope",
        "--scope",
        &corpus("scope-zero-match.txt"),
        "--corpus",
        &corpus("basic.tsv"),
    ]);
    assert_eq!(run.code, 2);
    assert!(run.stderr.contains("matches no case"), "{}", run.stderr);
    assert!(run.stdout.is_empty(), "{}", run.stdout);
}

#[test]
fn an_active_block_may_match_only_some_corpora() {
    // The global check is what makes this legal: `telex` has a case in
    // basic.tsv and none in second.tsv, and replaying second.tsv alone must not
    // call that a broken selector (that per-file error is the regression this
    // fix removes).
    let run = invoke(&[
        "scope",
        "--scope",
        &corpus("scope-global.txt"),
        "--corpus",
        &corpus("basic.tsv"),
        "--corpus",
        &corpus("second.tsv"),
    ]);
    assert_eq!(run.code, 0, "{}", run.stderr);

    let replay = scoped(
        &["replay", "--corpus", &corpus("second.tsv")],
        "scope-global.txt",
    );
    assert_eq!(replay.code, 0, "{}", replay.stdout);
    assert_eq!(status_of(&replay.stdout, "case-four"), Some("SKIP"));
}

#[test]
fn a_case_matching_two_blocks_at_one_specificity_is_exit_two() {
    let run = scoped(
        &["replay", "--corpus", &corpus("basic.tsv")],
        "scope-ambiguous.txt",
    );
    assert_eq!(run.code, 2);
    assert!(run.stderr.contains("exactly one block"), "{}", run.stderr);
    assert!(run.stdout.is_empty(), "{}", run.stdout);
}

fn played_scripts() -> (PathBuf, Vec<PathBuf>) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let scope = root.join("parity").join("scope.txt");
    let mut scripts = vec![root.join("scripts/parity/smoke.tsv")];
    let corpus_dir = root.join("tests/corpus");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&corpus_dir)
        .expect("the corpus directory exists")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "tsv"))
        .collect();
    files.sort();
    scripts.extend(files);
    (scope, scripts)
}

#[test]
fn the_declared_scope_classifies_every_case_at_the_turn_three_commit() {
    let (scope, scripts) = played_scripts();
    assert_eq!(scripts.len(), 19, "18 corpus files + smoke");

    // One global pass over the played set: zero unclassified, exactly one block
    // per case, and the five-block split this commit expects.
    let mut args = vec![
        "scope".to_string(),
        "--scope".to_string(),
        scope.display().to_string(),
    ];
    for script in &scripts {
        args.push("--corpus".to_string());
        args.push(script.display().to_string());
    }
    let run = invoke_owned(&args);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert!(
        run.stdout
            .contains("scope: 311 case(s) over 19 corpus file(s)"),
        "{}",
        run.stdout
    );
    assert!(
        run.stdout
            .contains("scope blocks: edit 49, opt 39, simple 27, telex 141, vni 55"),
        "{}",
        run.stdout
    );
    assert!(run.stdout.contains("active: none"), "{}", run.stdout);

    // And per file, every case is skipped by name while `active:` is empty.
    let mut skipped = 0usize;
    for script in &scripts {
        let replay = invoke_owned(&[
            "replay".to_string(),
            "--corpus".to_string(),
            script.display().to_string(),
            "--scope".to_string(),
            scope.display().to_string(),
        ]);
        assert_eq!(
            replay.code,
            0,
            "{} is not fully classified by parity/scope.txt: {}",
            script.display(),
            replay.stderr
        );
        assert!(
            replay
                .stdout
                .contains("scope: judged 0 / in-scope 0 / skipped "),
            "{}: every block is inactive at turn 3:\n{}",
            script.display(),
            replay.stdout
        );
        skipped += replay
            .stdout
            .lines()
            .filter(|line| line.starts_with("SKIP "))
            .count();
    }
    assert_eq!(
        skipped, 311,
        "300 corpus cases + 11 smoke cases at #29's head (bf5ffcc)"
    );
}
