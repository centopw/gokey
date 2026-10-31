//! Parity corpus loader (K-005).
//!
//! The format is the one owned by `tests/corpus/README.md`: one case per line,
//! `id <TAB> method <TAB> options <TAB> input [<TAB> expected]`, with `#` starting
//! a comment. Escapes are decoded exactly once here, with the same vocabulary the
//! reference harness and `scripts/parity/compare.py` use (`\b \n \t \r \s \\`),
//! and `-` means the empty document.
//!
//! The loader is strict on purpose: a line with fewer than four fields, an
//! unknown escape, or a duplicate id is an error, not a skip. A case that
//! silently disappears from a run is a green gate that measured nothing.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

/// A decoded corpus case.
#[derive(Debug, Clone)]
pub struct Case {
    pub id: String,
    pub method: String,
    /// Raw options column, mapped to [`openkey_core::Options`] by `flags`.
    pub options: String,
    /// Input column with escapes decoded (backspace is U+0008).
    pub input: String,
    /// Input column as written, for failure reports.
    pub input_raw: String,
    /// Expected column decoded, or `None` for a replay-only row.
    pub expected: Option<String>,
    pub source: PathBuf,
    pub line: usize,
}

#[derive(Debug)]
pub struct Error {
    message: String,
}

impl Error {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

/// Load every `.tsv` under `path` (a file or a directory, recursively).
///
/// Files are read in sorted path order so a run's output is stable. Case ids
/// must be unique across the whole load, not just within one file: the replay
/// report and the reference-engine diff both key on the id.
pub fn load(path: &Path) -> Result<Vec<Case>, Error> {
    let mut files = Vec::new();
    if path.is_dir() {
        collect_tsv(path, &mut files)?;
        files.sort();
    } else if path.is_file() {
        files.push(path.to_path_buf());
    } else {
        return Err(Error::new(format!(
            "corpus path {} does not exist",
            path.display()
        )));
    }
    if files.is_empty() {
        return Err(Error::new(format!(
            "no .tsv files under {}",
            path.display()
        )));
    }

    let mut cases = Vec::new();
    let mut seen: BTreeMap<String, (PathBuf, usize)> = BTreeMap::new();
    for file in files {
        let text = fs::read_to_string(&file)
            .map_err(|err| Error::new(format!("{}: {err}", file.display())))?;
        for case in parse(&text, &file)? {
            if let Some((first_file, first_line)) = seen.get(&case.id) {
                return Err(Error::new(format!(
                    "{}:{}: duplicate case id {:?} (first at {}:{})",
                    file.display(),
                    case.line,
                    case.id,
                    first_file.display(),
                    first_line
                )));
            }
            seen.insert(case.id.clone(), (file.clone(), case.line));
            cases.push(case);
        }
    }
    if cases.is_empty() {
        return Err(Error::new(format!(
            "{}: no cases (comments and blank lines only)",
            path.display()
        )));
    }
    Ok(cases)
}

fn collect_tsv(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), Error> {
    let entries =
        fs::read_dir(dir).map_err(|err| Error::new(format!("{}: {err}", dir.display())))?;
    for entry in entries {
        let entry = entry.map_err(|err| Error::new(format!("{}: {err}", dir.display())))?;
        let file_type = entry
            .file_type()
            .map_err(|err| Error::new(format!("{}: {err}", entry.path().display())))?;
        if file_type.is_dir() {
            collect_tsv(&entry.path(), out)?;
        } else if file_type.is_file() && entry.path().extension().is_some_and(|ext| ext == "tsv") {
            out.push(entry.path());
        }
    }
    Ok(())
}

/// Parse one script. Exposed for tests; [`load`] is the filesystem entry point.
pub fn parse(text: &str, source: &Path) -> Result<Vec<Case>, Error> {
    let mut cases = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line_no = index + 1;
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 4 {
            return Err(Error::new(format!(
                "{}:{}: expected at least 4 tab-separated fields, got {}",
                source.display(),
                line_no,
                fields.len()
            )));
        }
        let id = fields[0].trim();
        if id.is_empty() {
            return Err(Error::new(format!(
                "{}:{}: empty case id",
                source.display(),
                line_no
            )));
        }
        let input_raw = fields[3].to_string();
        let input = decode(
            fields[3],
            &format!("{}:{}: input", source.display(), line_no),
        )?;
        let expected = if fields.len() > 4 {
            Some(decode(
                fields[4],
                &format!("{}:{}: expected", source.display(), line_no),
            )?)
        } else {
            None
        };
        cases.push(Case {
            id: id.to_string(),
            method: fields[1].to_string(),
            options: fields[2].to_string(),
            input,
            input_raw,
            expected,
            source: source.to_path_buf(),
            line: line_no,
        });
    }
    Ok(cases)
}

/// Decode one field with the corpus escape vocabulary.
pub fn decode(raw: &str, where_: &str) -> Result<String, Error> {
    if raw == "-" {
        return Ok(String::new());
    }
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('b') => out.push('\u{8}'),
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('s') => out.push(' '),
            Some('\\') => out.push('\\'),
            Some(other) => {
                return Err(Error::new(format!(
                    "{where_}: unknown escape \\{other} (allowed: \\b \\n \\t \\r \\s \\\\)"
                )));
            }
            None => return Err(Error::new(format!("{where_}: trailing backslash"))),
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path() -> PathBuf {
        PathBuf::from("tests/corpus/example.tsv")
    }

    #[test]
    fn decodes_the_escape_table_once() {
        assert_eq!(decode("-", "x").unwrap(), "");
        assert_eq!(decode(r"a\sb", "x").unwrap(), "a b");
        assert_eq!(decode(r"a\nb", "x").unwrap(), "a\nb");
        assert_eq!(decode(r"a\\nb", "x").unwrap(), "a\\nb");
        assert_eq!(decode(r"\b\t\r", "x").unwrap(), "\u{8}\t\r");
    }

    #[test]
    fn rejects_unknown_and_trailing_escapes() {
        assert!(
            decode(r"a\qb", "x")
                .unwrap_err()
                .to_string()
                .contains("unknown escape")
        );
        assert!(
            decode("a\\", "x")
                .unwrap_err()
                .to_string()
                .contains("trailing")
        );
    }

    #[test]
    fn parses_comments_blanks_and_optional_expected() {
        let text = "# comment\n\ncase-1\ttelex\t-\taa\tâ\ncase-2\tvni\t-\tvie65t\n";
        let cases = parse(text, &path()).unwrap();
        assert_eq!(cases.len(), 2);
        assert_eq!(cases[0].id, "case-1");
        assert_eq!(cases[0].input, "aa");
        assert_eq!(cases[0].expected.as_deref(), Some("â"));
        assert_eq!(cases[1].expected, None);
        assert_eq!(cases[0].line, 3);
    }

    #[test]
    fn rejects_short_rows_and_empty_ids() {
        assert!(parse("case-1\ttelex\t-\n", &path()).is_err());
        assert!(parse("\ttelex\t-\taa\tâ\n", &path()).is_err());
    }

    #[test]
    fn later_fields_do_not_create_a_case() {
        // A comment marker after a tab is still data; only a leading `#` comments.
        let cases = parse("case-1\ttelex\t-\taa\tâ\textra\n", &path()).unwrap();
        assert_eq!(cases.len(), 1);
    }
}
