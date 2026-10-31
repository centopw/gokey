//! Run the pinned reference harness and read its per-case final documents
//! (K-005).
//!
//! The harness (`scripts/parity/harness.cpp`) produces; this module only reads
//! its output. It never re-implements a typing rule and never judges a case: the
//! corpus `expected` column and the reference document are two separate inputs to
//! the report, so a wrong baseline stays visible as a diff.
//!
//! The harness reports one line per key plus a `final` line per case
//! (`scripts/parity/README.md`). Only `final` lines are read here. A script whose
//! ids the harness did not all report is an error, not a skip: a missing case in
//! the oracle run would otherwise turn into a silent pass.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::corpus::{Case, decode};

#[derive(Debug, Clone)]
pub struct RefEngine {
    binary: PathBuf,
}

impl RefEngine {
    /// Resolve the harness from `--ref`, then `OPENKEY_REF_ENGINE`, then the
    /// default build path `scripts/parity.sh` writes (`.oracle/build/harness`).
    pub fn resolve(explicit: Option<&Path>) -> Result<Self, String> {
        if let Some(path) = explicit {
            return Self::at(path);
        }
        if let Some(path) = std::env::var_os("OPENKEY_REF_ENGINE") {
            return Self::at(Path::new(&path));
        }
        Self::at(Path::new(".oracle/build/harness"))
    }

    fn at(path: &Path) -> Result<Self, String> {
        if !path.is_file() {
            return Err(format!(
                "reference engine {} not found; build it with scripts/parity.sh, \
                 set OPENKEY_REF_ENGINE, or pass --ref <path>",
                path.display()
            ));
        }
        Ok(Self {
            binary: path.to_path_buf(),
        })
    }

    pub fn binary(&self) -> &Path {
        &self.binary
    }

    /// Replay one script through the harness and return `id -> final document`.
    pub fn finals(&self, script: &Path) -> Result<BTreeMap<String, String>, String> {
        let output = Command::new(&self.binary)
            .arg(script)
            .output()
            .map_err(|err| format!("could not run {}: {err}", self.binary.display()))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stderr = stderr.trim();
            return Err(format!(
                "{} {} exited with {}{}",
                self.binary.display(),
                script.display(),
                output.status,
                if stderr.is_empty() {
                    String::new()
                } else {
                    format!(": {stderr}")
                }
            ));
        }
        let stdout = String::from_utf8(output.stdout)
            .map_err(|err| format!("{}: output is not UTF-8: {err}", self.binary.display()))?;
        parse_finals(&stdout, script)
    }

    /// Load the finals for every distinct source file and check that each one
    /// reported exactly the ids that file contains.
    pub fn finals_by_source(&self, cases: &[Case]) -> Result<BTreeMap<String, String>, String> {
        let mut by_source: BTreeMap<&Path, Vec<&Case>> = BTreeMap::new();
        for case in cases {
            by_source
                .entry(case.source.as_path())
                .or_default()
                .push(case);
        }

        let mut finals = BTreeMap::new();
        for (source, cases) in by_source {
            let reported = self.finals(source)?;
            let expected: Vec<&str> = cases.iter().map(|case| case.id.as_str()).collect();
            let missing: Vec<&str> = expected
                .iter()
                .copied()
                .filter(|id| !reported.contains_key(*id))
                .collect();
            let extra: Vec<&str> = reported
                .keys()
                .map(String::as_str)
                .filter(|id| !expected.contains(id))
                .collect();
            if !missing.is_empty() {
                return Err(format!(
                    "{}: the reference engine did not report {} case(s): {}",
                    source.display(),
                    missing.len(),
                    missing.join(", ")
                ));
            }
            if !extra.is_empty() {
                return Err(format!(
                    "{}: the reference engine reported {} case(s) that are not in the script: {}",
                    source.display(),
                    extra.len(),
                    extra.join(", ")
                ));
            }
            finals.extend(reported);
        }
        Ok(finals)
    }
}

/// Parse the harness's stdout. Exposed for tests.
pub fn parse_finals(text: &str, script: &Path) -> Result<BTreeMap<String, String>, String> {
    let mut finals = BTreeMap::new();
    for (index, line) in text.lines().enumerate() {
        let line_no = index + 1;
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 7 {
            return Err(format!(
                "{}:{}: expected at least 7 tab-separated fields, got {}",
                script.display(),
                line_no,
                fields.len()
            ));
        }
        if fields[1] != "final" {
            continue;
        }
        let id = fields[0];
        let document = decode(
            fields[6],
            &format!("{}:{}: ref document", script.display(), line_no),
        )
        .map_err(|err| err.to_string())?;
        if finals.insert(id.to_string(), document).is_some() {
            return Err(format!(
                "{}:{}: two final lines for {:?}",
                script.display(),
                line_no,
                id
            ));
        }
    }
    Ok(finals)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn script() -> PathBuf {
        PathBuf::from("tests/corpus/example.tsv")
    }

    #[test]
    fn reads_final_lines_and_decodes_the_document() {
        let text = "\
# id\tstep\tkey\tsignal\tbackspaces\tchars\tdocument
case-1\t0\t0x00\t1\t0\t-\tv
case-1\tfinal\t-\t-\t-\t-\tviệt
case-2\tfinal\t-\t-\t-\t-\t-
case-3\tfinal\t-\t-\t-\t-\ta\\sb\\nc
";
        let finals = parse_finals(text, &script()).unwrap();
        assert_eq!(finals.get("case-1").map(String::as_str), Some("việt"));
        assert_eq!(finals.get("case-2").map(String::as_str), Some(""));
        assert_eq!(finals.get("case-3").map(String::as_str), Some("a b\nc"));
    }

    #[test]
    fn rejects_short_lines_and_duplicate_finals() {
        assert!(parse_finals("case-1\tfinal\t-\n", &script()).is_err());
        let duplicate = "case-1\tfinal\t-\t-\t-\t-\ta\ncase-1\tfinal\t-\t-\t-\t-\tb\n";
        assert!(parse_finals(duplicate, &script()).is_err());
    }

    #[test]
    fn a_missing_binary_is_reported_with_the_build_hint() {
        let err = RefEngine::resolve(Some(Path::new("/nonexistent/harness"))).unwrap_err();
        assert!(err.contains("scripts/parity.sh"));
        assert!(err.contains("--ref"));
    }
}
