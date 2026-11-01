//! CLI contract tests for `openkey convert` (C-012).
//!
//! The machine and the codecs are pinned in `openkey-core::convert`; these
//! tests cover the shell: argument parsing, stdin/stdout versus file I/O, the
//! five charset names, the five caps/mark flags, and the exit codes — 0 on
//! success, 1 on a usage/encoding/I-O error, never 2.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

fn run(args: &[&str], stdin_bytes: &[u8]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_openkey"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the openkey binary runs");
    // `--input <file>` makes the child exit without reading stdin, so a
    // broken pipe here is expected, not a test failure.
    match child
        .stdin
        .take()
        .expect("stdin is piped")
        .write_all(stdin_bytes)
    {
        Err(err) if err.kind() == std::io::ErrorKind::BrokenPipe => {}
        other => other.expect("the test writes stdin"),
    }
    child.wait_with_output().expect("the binary exits normally")
}

fn code(output: &Output) -> i32 {
    output
        .status
        .code()
        .expect("the binary exits normally, not on a signal")
}

fn stdout(output: &Output) -> &[u8] {
    &output.stdout
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr is UTF-8")
}

fn convert(args: &[&str], input: &str) -> Output {
    run(args, input.as_bytes())
}

#[test]
fn stdin_to_stdout_applies_the_default_normal_form() {
    // No caps flag: the converter lowercases, as `ConvertTool.cpp:159-160` does.
    let output = convert(
        &["convert", "--from", "unicode", "--to", "unicode"],
        "Việt Nam\n",
    );
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    assert_eq!(stdout(&output), "việt nam\n".as_bytes());
}

#[test]
fn file_in_file_out() {
    let directory = std::env::temp_dir().join(format!("openkey-convert-{}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("the temp directory is created");
    let input: PathBuf = directory.join("in.txt");
    let output: PathBuf = directory.join("out.txt");
    std::fs::write(&input, "Đường\n").expect("the fixture is written");

    let result = run(
        &[
            "convert",
            "--from",
            "unicode",
            "--to",
            "unicode",
            "--input",
            &input.display().to_string(),
            "--output",
            &output.display().to_string(),
        ],
        b"ignored",
    );
    assert_eq!(code(&result), 0, "{}", stderr(&result));
    assert_eq!(stdout(&result), b"", "nothing goes to stdout with --output");
    assert_eq!(
        std::fs::read(&output).expect("the output file exists"),
        "đường\n".as_bytes()
    );

    std::fs::remove_dir_all(&directory).expect("the temp directory is removed");
}

#[test]
fn every_charset_name_is_accepted_on_both_sides() {
    let names = ["unicode", "tcvn3", "vni", "unicode-compound", "cp1258"];
    for name in names {
        let output = run(&["convert", "--from", name, "--to", name], b"");
        assert_eq!(code(&output), 0, "{name}: {}", stderr(&output));
        assert_eq!(stdout(&output), b"", "{name}: empty in, empty out");
    }
}

#[test]
fn every_flag_is_applied() {
    for (flag, input, expected) in [
        ("--all-caps", "việt nam", "VIỆT NAM"),
        ("--all-non-caps", "VIỆT NAM", "việt nam"),
        ("--caps-first-letter", "việt nam", "Việt nam"),
        ("--caps-each-word", "việt nam", "Việt Nam"),
        ("--remove-mark", "việt nam", "viet nam"),
    ] {
        let output = convert(
            &["convert", "--from", "unicode", "--to", "unicode", flag],
            input,
        );
        assert_eq!(code(&output), 0, "{flag}: {}", stderr(&output));
        assert_eq!(stdout(&output), expected.as_bytes(), "{flag}");
    }
}

#[test]
fn a_vni_target_is_written_as_bytes_not_text() {
    // `việt` -> VNI is `vie` 0xE4 `t` (charset.rs pins the same word). A
    // String-based comparison would hide a Latin-1/UTF-8 accident.
    let output = convert(&["convert", "--from", "unicode", "--to", "vni"], "việt");
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    assert_eq!(stdout(&output), &[0x76, 0x69, 0x65, 0xE4, 0x74]);
}

#[test]
fn a_compound_target_normalises_the_last_unit() {
    // D2: the cell that the machine leaves bare at the last position
    // (`0xA0EA`) is decomposed at the file boundary into `ê` + U+0323.
    let output = convert(
        &["convert", "--from", "unicode", "--to", "unicode-compound"],
        "việ",
    );
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    assert_eq!(stdout(&output), &[0x76, 0x69, 0xC3, 0xAA, 0xCC, 0xA3]);
}

#[test]
fn an_unknown_charset_is_a_usage_error() {
    let output = convert(&["convert", "--from", "latin1", "--to", "unicode"], "a");
    assert_eq!(code(&output), 1);
    assert!(
        stderr(&output).contains("unknown charset"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn a_missing_input_file_is_an_io_error() {
    let path = "/nonexistent/openkey-convert-input";
    let output = run(
        &[
            "convert", "--from", "unicode", "--to", "unicode", "--input", path,
        ],
        b"",
    );
    assert_eq!(code(&output), 1);
    assert!(stderr(&output).contains(path), "{}", stderr(&output));
}

#[test]
fn invalid_utf8_input_is_rejected_not_replaced() {
    // A truncated two-byte sequence: the converter must fail, not emit U+FFFD.
    let output = run(
        &["convert", "--from", "unicode", "--to", "unicode"],
        &[0xC3],
    );
    assert_eq!(code(&output), 1);
    assert!(
        stderr(&output).contains("not valid UTF-8"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn empty_input_is_success_and_empty_output() {
    for to in ["unicode", "tcvn3", "vni", "unicode-compound", "cp1258"] {
        let output = run(&["convert", "--from", "unicode", "--to", to], b"");
        assert_eq!(code(&output), 0, "{to}: {}", stderr(&output));
        assert_eq!(stdout(&output), b"", "{to}");
    }
}

#[test]
fn the_required_charset_flags_are_named_in_the_error() {
    let output = convert(&["convert", "--to", "unicode"], "a");
    assert_eq!(code(&output), 1);
    assert!(
        stderr(&output).contains("convert needs --from"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn help_prints_the_command_and_its_charsets() {
    let output = run(&["convert", "--help"], b"");
    assert_eq!(code(&output), 0);
    let usage = String::from_utf8(output.stdout).expect("USAGE is UTF-8");
    assert!(usage.contains("openkey convert --from <charset> --to <charset>"));
    for name in ["unicode", "tcvn3", "vni", "unicode-compound", "cp1258"] {
        assert!(usage.contains(name), "USAGE names {name}");
    }
}
