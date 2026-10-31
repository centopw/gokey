//! Headless harness: replay parity corpus scripts against `openkey-core`.
//!
//! K-005. The command is:
//!
//! ```text
//! openkey replay --corpus tests/corpus [--diff ref-engine] [--ref .oracle/build/harness]
//! ```
//!
//! It reports one status per case (pass, fail, pending, skip, invalid) plus a
//! summary. It never prints an agreement percentage while `Engine::apply` is a
//! stub: a percentage over a mostly-pending run would describe nothing.
//!
//! Exit status: 0 every case passed, 1 at least one case failed, 2 the run could
//! not produce a verdict (pending or invalid cases, a corpus/ref error, or an
//! empty run).

mod corpus;
mod flags;
mod refengine;
mod replay;
mod report;

use std::path::PathBuf;
use std::process::ExitCode;

use refengine::RefEngine;
use report::{Row, Summary, escape, first_difference};

const USAGE: &str = "\
openkey — headless harness for the GoKey port

USAGE:
    openkey --version
    openkey replay --corpus <path> [--diff ref-engine] [--ref <path>]
                   [--allow-missing-expected]

OPTIONS:
    --corpus <path>             A .tsv file or a directory searched recursively.
    --diff ref-engine           Compare every document with the pinned reference
                                harness as well as with the corpus expected column.
    --ref <path>                Reference harness binary. Defaults to
                                $OPENKEY_REF_ENGINE, then .oracle/build/harness.
    --allow-missing-expected    Treat a row without a fifth column as a deliberate
                                skip. CI never passes this: a dropped tab would
                                otherwise remove a case from the gate silently.

EXIT STATUS:
    0  every case passed
    1  at least one case failed
    2  no verdict could be produced (engine pending, invalid row, corpus or
       reference-engine error, or an empty run)";

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(command) = args.next() else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };

    match command.as_str() {
        "--version" | "-V" => {
            println!("openkey {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        "--help" | "-h" => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        "replay" => match replay_command(args.collect()) {
            Ok(code) => ExitCode::from(code),
            Err(message) => {
                eprintln!("error: {message}");
                ExitCode::from(2)
            }
        },
        other => {
            eprintln!("error: unknown command {other:?}\n\n{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn replay_command(args: Vec<String>) -> Result<u8, String> {
    let mut corpus_path: Option<PathBuf> = None;
    let mut diff_ref = false;
    let mut ref_path: Option<PathBuf> = None;
    let mut allow_missing = false;

    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--corpus" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--corpus needs a path".to_string())?;
                corpus_path = Some(PathBuf::from(value));
            }
            "--diff" => {
                let value = args.next().ok_or_else(|| {
                    "--diff needs a value; only `ref-engine` is supported".to_string()
                })?;
                if value != "ref-engine" {
                    return Err(format!(
                        "unknown --diff value {value:?}; only `ref-engine` is supported"
                    ));
                }
                diff_ref = true;
            }
            "--ref" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--ref needs a path".to_string())?;
                ref_path = Some(PathBuf::from(value));
            }
            "--allow-missing-expected" => allow_missing = true,
            "--help" | "-h" => {
                println!("{USAGE}");
                return Ok(0);
            }
            other => return Err(format!("unknown replay argument {other:?}")),
        }
    }

    let corpus_path = corpus_path.ok_or_else(|| "replay needs --corpus <path>".to_string())?;
    let cases = corpus::load(&corpus_path).map_err(|err| err.to_string())?;

    let (ref_engine, reference) = if diff_ref {
        let engine = RefEngine::resolve(ref_path.as_deref())?;
        let finals = engine.finals_by_source(&cases)?;
        (Some(engine), Some(finals))
    } else {
        (None, None)
    };

    let mut rows = Vec::with_capacity(cases.len());
    let mut ref_checked = 0usize;
    for case in &cases {
        // A missing expected column is a corpus defect, not an engine result.
        // Checking it before replaying keeps a dropped tab visible even while
        // every case is engine-pending.
        if case.expected.is_none() && !allow_missing {
            rows.push(Row::invalid(
                case.id.clone(),
                format!(
                    "{}:{}: no expected column; write the fifth field or pass \
                     --allow-missing-expected",
                    case.source.display(),
                    case.line
                ),
            ));
            continue;
        }

        let reference_document = reference
            .as_ref()
            .and_then(|finals| finals.get(&case.id))
            .map(String::as_str);

        let row = match replay::run(case) {
            Ok(actual) => judge(case, &actual, reference_document, allow_missing),
            Err(err) => match err {
                replay::ReplayError::Invalid(message) => Row::invalid(
                    case.id.clone(),
                    format!("{}:{}: {message}", case.source.display(), case.line),
                ),
                replay::ReplayError::Pending(message) => {
                    Row::pending(case.id.clone(), format!("engine-pending: {message}"))
                }
                replay::ReplayError::Panic(message) => {
                    Row::fail(case.id.clone(), "engine-panic", message)
                }
            },
        };
        if reference_document.is_some()
            && !matches!(
                row.status,
                report::Status::Pending | report::Status::Invalid
            )
        {
            ref_checked += 1;
        }
        rows.push(row);
    }

    let mut summary = Summary::from_rows(&rows);
    summary.ref_checked = ref_checked;
    let ref_label = ref_engine
        .as_ref()
        .map(|engine| engine.binary().display().to_string());
    print!("{}", report::render(&rows, &summary, ref_label.as_deref()));
    Ok(summary.exit_code() as u8)
}

fn judge(case: &corpus::Case, actual: &str, reference: Option<&str>, allow_missing: bool) -> Row {
    let Some(expected) = case.expected.as_deref() else {
        if !allow_missing {
            return Row::invalid(
                case.id.clone(),
                format!(
                    "{}:{}: no expected column; write the fifth field or pass \
                     --allow-missing-expected",
                    case.source.display(),
                    case.line
                ),
            );
        }
        if let Some(reference) = reference {
            if reference != actual {
                return Row::fail(
                    case.id.clone(),
                    "reference-engine",
                    format!(
                        "reference {} vs port {}; {}",
                        escape(reference),
                        escape(actual),
                        first_difference(reference, actual)
                    ),
                );
            }
        }
        return Row::skip(case.id.clone());
    };

    if actual != expected {
        let mut detail = format!(
            "input {:?}; expected {}, actual {}; {}",
            case.input_raw,
            escape(expected),
            escape(actual),
            first_difference(expected, actual)
        );
        if let Some(reference) = reference {
            if reference == actual {
                detail.push_str("; the reference engine agrees with the port here");
            }
        }
        return Row::fail(case.id.clone(), "expected", detail);
    }

    if let Some(reference) = reference {
        if reference != actual {
            return Row::fail(
                case.id.clone(),
                "reference-engine",
                format!(
                    "reference {} vs port {}; {}",
                    escape(reference),
                    escape(actual),
                    first_difference(reference, actual)
                ),
            );
        }
    }

    Row::pass(case.id.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use report::Status;

    fn case(expected: Option<&str>) -> corpus::Case {
        corpus::Case {
            id: "case".to_string(),
            method: "telex".to_string(),
            options: "-".to_string(),
            input: "vieejt".to_string(),
            input_raw: "vieejt".to_string(),
            expected: expected.map(str::to_string),
            source: PathBuf::from("t.tsv"),
            line: 1,
        }
    }

    #[test]
    fn a_matching_expected_column_passes() {
        let row = judge(&case(Some("việt")), "việt", None, false);
        assert_eq!(row.status, Status::Pass);
    }

    #[test]
    fn a_mismatch_names_the_class_and_the_reference_verdict() {
        let row = judge(&case(Some("việt")), "viet", Some("viet"), false);
        assert_eq!(row.status, Status::Fail);
        assert_eq!(row.class, Some("expected"));
        let detail = row.detail.unwrap();
        assert!(detail.contains("first difference"), "{detail}");
        assert!(
            detail.contains("reference engine agrees with the port"),
            "{detail}"
        );
    }

    #[test]
    fn a_reference_mismatch_fails_even_when_expected_matches() {
        let row = judge(&case(Some("việt")), "việt", Some("viêt"), false);
        assert_eq!(row.status, Status::Fail);
        assert_eq!(row.class, Some("reference-engine"));
    }

    #[test]
    fn a_missing_expected_column_is_invalid_by_default() {
        let row = judge(&case(None), "việt", None, false);
        assert_eq!(row.status, Status::Invalid);
        assert!(row.detail.unwrap().contains("no expected column"));
    }

    #[test]
    fn a_missing_expected_column_is_a_skip_or_a_reference_failure() {
        let row = judge(&case(None), "việt", Some("việt"), true);
        assert_eq!(row.status, Status::Skip);

        let row = judge(&case(None), "viet", Some("việt"), true);
        assert_eq!(row.status, Status::Fail);
        assert_eq!(row.class, Some("reference-engine"));
    }
}
