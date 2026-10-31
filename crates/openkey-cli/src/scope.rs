//! Declared parity scope (K-005 turn 3).
//!
//! `parity/scope.txt` answers one question for every case: which block owns it,
//! and is that block active yet? The corpus is written ahead of the port, so a
//! case whose task has not started is *skipped by name* rather than judged into
//! a red gate or counted into a green one. The load-bearing rule is the other
//! direction: a case in an **active** block that is not judged keeps the run
//! non-green, so green can only mean "nothing declared is unjudged".
//!
//! Definition and activation are separate lines. All five blocks are defined from
//! day one — so every case resolves and an unclassified case is always an error,
//! never a quiet skip — while `active:` starts empty and grows one line per task
//! (C-002 `telex`, C-006 `edit`, C-003 `vni`, C-004 `simple`, C-008 `opt`). At
//! C-014 `active:` lists all five, so no skip path remains.
//!
//! Grammar (one file, `#` comments, blank lines ignored):
//!
//! ```text
//! block: <name> <selector>[,<selector>...]
//! active: <name>[,<name>...]
//! ```
//!
//! Selectors, most specific first:
//! - `telex-basic` — an exact id, as written in the corpus;
//! - `id:case-vni-*` — a glob over ids (`*` any run, `?` one character);
//! - `file:tests/corpus/telex-words.tsv` — a corpus file, matched as a path
//!   suffix so a fixture can use a bare file name.
//!
//! The most specific match decides. Two blocks matching at the same specificity
//! is ambiguous, and a case with no match is unclassified; both are errors,
//! because a selector that stopped matching must not look like a clean run.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Selector {
    Exact(String),
    Glob(String),
    File(String),
}

impl Selector {
    fn specificity(&self) -> u8 {
        match self {
            Self::Exact(_) => 3,
            Self::Glob(_) => 2,
            Self::File(_) => 1,
        }
    }

    fn matches(&self, id: &str, source: &Path) -> bool {
        match self {
            Self::Exact(value) => value == id,
            Self::Glob(pattern) => glob_match(pattern, id),
            Self::File(path) => file_matches(path, source),
        }
    }
}

#[derive(Debug, Clone)]
struct Block {
    name: String,
    selectors: Vec<Selector>,
}

/// The parsed scope file.
#[derive(Debug, Clone)]
pub struct Scope {
    blocks: Vec<Block>,
    active: Vec<String>,
}

impl Scope {
    /// Read and parse a scope file.
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = fs::read_to_string(path).map_err(|err| {
            format!(
                "could not read scope file {}: {err}; scripts/parity.sh always passes \
                 parity/scope.txt",
                path.display()
            )
        })?;
        Self::parse(&text, &path.display().to_string())
    }

    fn parse(text: &str, origin: &str) -> Result<Self, String> {
        let mut blocks: Vec<Block> = Vec::new();
        let mut active: Vec<String> = Vec::new();

        for (index, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let at = format!("{origin}:{}", index + 1);

            if let Some(rest) = line.strip_prefix("block:") {
                let rest = rest.trim();
                let mut parts = rest.splitn(2, char::is_whitespace);
                let name = parts.next().unwrap_or_default();
                let selector_list = parts.next().unwrap_or_default();
                if name.is_empty() {
                    return Err(format!("{at}: `block:` needs a name"));
                }
                if blocks.iter().any(|block| block.name == name) {
                    return Err(format!("{at}: block {name:?} is defined twice"));
                }
                if selector_list.trim().is_empty() {
                    return Err(format!(
                        "{at}: block {name:?} has no selectors; it would match nothing"
                    ));
                }
                let mut selectors = Vec::new();
                for raw_selector in selector_list.split(',') {
                    let selector = raw_selector.trim();
                    if selector.is_empty() {
                        return Err(format!(
                            "{at}: block {name:?} has an empty selector; a dangling comma \
                             must not read as a wildcard"
                        ));
                    }
                    selectors.push(parse_selector(selector, &at)?);
                }
                if selectors.is_empty() {
                    return Err(format!(
                        "{at}: block {name:?} has no selectors; it would match nothing"
                    ));
                }
                blocks.push(Block {
                    name: name.to_string(),
                    selectors,
                });
            } else if let Some(rest) = line.strip_prefix("active:") {
                for name in rest.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                    if !active.iter().any(|seen| seen == name) {
                        active.push(name.to_string());
                    }
                }
            } else {
                return Err(format!(
                    "{at}: unknown directive {line:?}; expected `block:` or `active:`"
                ));
            }
        }

        if blocks.is_empty() {
            return Err(format!(
                "{origin}: no `block:` lines; a scope file that classifies nothing cannot \
                 distinguish skipped work from broken selectors"
            ));
        }
        for name in &active {
            if !blocks.iter().any(|block| &block.name == name) {
                return Err(format!(
                    "{origin}: `active:` names unknown block {name:?}; the block must be \
                     defined before a task can activate it"
                ));
            }
        }

        Ok(Self { blocks, active })
    }

    pub fn is_active(&self, block: &str) -> bool {
        self.active.iter().any(|name| name == block)
    }

    /// Resolve one case to its block, or explain why it cannot be resolved.
    pub fn classify(&self, id: &str, source: &Path) -> Result<&str, String> {
        let mut best = 0u8;
        let mut matched: Vec<&str> = Vec::new();
        for block in &self.blocks {
            let mut top = 0u8;
            for selector in &block.selectors {
                if selector.matches(id, source) {
                    top = top.max(selector.specificity());
                }
            }
            if top == 0 {
                continue;
            }
            if top > best {
                best = top;
                matched.clear();
                matched.push(&block.name);
            } else if top == best {
                matched.push(&block.name);
            }
        }

        match matched.len() {
            0 => Err(format!(
                "case {id:?} in {} is not classified by parity/scope.txt; add a selector \
                 for it rather than widening a block",
                source.display()
            )),
            1 => Ok(matched[0]),
            _ => Err(format!(
                "case {id:?} in {} matches blocks {} at the same specificity; the scope \
                 requires exactly one block per case",
                source.display(),
                matched.join(", ")
            )),
        }
    }

    /// Refuse an activated block that matched no case.
    ///
    /// An activation line and a selector that no longer match is a broken gate,
    /// not a green run: the task that appended the line believes its cases are
    /// judged, and nothing would say otherwise.
    pub fn check_activation(&self, counts: &BTreeMap<String, usize>) -> Result<(), String> {
        for name in &self.active {
            if counts.get(name).copied().unwrap_or(0) == 0 {
                return Err(format!(
                    "active block {name:?} matches no case; a declared block with no cases \
                     is a broken selector, not a clean run"
                ));
            }
        }
        Ok(())
    }
}

impl fmt::Display for Scope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let blocks: Vec<&str> = self
            .blocks
            .iter()
            .map(|block| block.name.as_str())
            .collect();
        write!(f, "blocks: {}", blocks.join(", "))?;
        if self.active.is_empty() {
            write!(f, "; active: none")
        } else {
            write!(f, "; active: {}", self.active.join(", "))
        }
    }
}

fn parse_selector(text: &str, at: &str) -> Result<Selector, String> {
    if text.chars().any(char::is_whitespace) {
        return Err(format!("{at}: selector {text:?} contains whitespace"));
    }
    if let Some(path) = text.strip_prefix("file:") {
        if path.is_empty() {
            return Err(format!("{at}: `file:` needs a path"));
        }
        return Ok(Selector::File(path.to_string()));
    }
    if let Some(pattern) = text.strip_prefix("id:") {
        if pattern.is_empty() {
            return Err(format!("{at}: `id:` needs a pattern"));
        }
        return Ok(Selector::Glob(pattern.to_string()));
    }
    Ok(Selector::Exact(text.to_string()))
}

fn file_matches(selector: &str, source: &Path) -> bool {
    let source = normalize(&source.to_string_lossy());
    let selector = normalize(selector);
    source == selector || source.ends_with(&format!("/{selector}"))
}

fn normalize(path: &str) -> String {
    let mut path = path.replace('\\', "/");
    while let Some(rest) = path.strip_prefix("./") {
        path = rest.to_string();
    }
    path
}

/// `*` matches any run of characters, `?` matches one. Iterative with
/// backtracking: no recursion over an attacker-controlled id.
fn glob_match(pattern: &str, text: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let text: Vec<char> = text.chars().collect();
    let (mut p, mut t) = (0usize, 0usize);
    let mut star: Option<(usize, usize)> = None;

    while t < text.len() {
        if p < pattern.len() && (pattern[p] == '?' || pattern[p] == text[t]) {
            p += 1;
            t += 1;
        } else if p < pattern.len() && pattern[p] == '*' {
            star = Some((p, t));
            p += 1;
        } else if let Some((star_p, star_t)) = star {
            p = star_p + 1;
            t = star_t + 1;
            star = Some((star_p, star_t + 1));
        } else {
            return false;
        }
    }
    while p < pattern.len() && pattern[p] == '*' {
        p += 1;
    }
    p == pattern.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn parse(text: &str) -> Scope {
        Scope::parse(text, "scope.txt").expect("scope parses")
    }

    fn classify(scope: &Scope, id: &str, file: &str) -> Result<String, String> {
        scope.classify(id, &PathBuf::from(file)).map(str::to_string)
    }

    #[test]
    fn exact_glob_and_file_selectors_resolve() {
        let scope = parse(
            "block: telex file:tests/corpus/telex-words.tsv\n\
             block: vni id:vni-*\n\
             block: simple simple-telex-1\n",
        );
        assert_eq!(
            classify(&scope, "telex-a", "tests/corpus/telex-words.tsv").unwrap(),
            "telex"
        );
        assert_eq!(classify(&scope, "vni-basic", "smoke.tsv").unwrap(), "vni");
        assert_eq!(
            classify(&scope, "simple-telex-1", "smoke.tsv").unwrap(),
            "simple"
        );
    }

    #[test]
    fn the_most_specific_match_wins() {
        let scope = parse(
            "block: telex file:tests/corpus/case-mixed.tsv\n\
             block: vni id:case-vni-*\n",
        );
        assert_eq!(
            classify(&scope, "case-caps", "tests/corpus/case-mixed.tsv").unwrap(),
            "telex"
        );
        assert_eq!(
            classify(&scope, "case-vni-viet", "tests/corpus/case-mixed.tsv").unwrap(),
            "vni",
            "an id glob outranks the file selector that also matches"
        );
    }

    #[test]
    fn an_exact_id_outranks_a_glob() {
        let scope = parse("block: a id:telex-*\nblock: b telex-aa\n");
        assert_eq!(classify(&scope, "telex-aa", "x.tsv").unwrap(), "b");
    }

    #[test]
    fn a_case_with_no_match_is_unclassified() {
        let scope = parse("block: telex id:telex-*\n");
        let err = classify(&scope, "vni-basic", "x.tsv").unwrap_err();
        assert!(err.contains("not classified"), "{err}");
    }

    #[test]
    fn two_blocks_at_the_same_specificity_are_ambiguous() {
        let scope = parse("block: a id:telex-*\nblock: b id:telex-a*\n");
        let err = classify(&scope, "telex-aa", "x.tsv").unwrap_err();
        assert!(err.contains("exactly one block"), "{err}");
    }

    #[test]
    fn an_inactive_block_still_classifies() {
        let scope = parse("block: telex id:telex-*\nactive:\n");
        assert_eq!(classify(&scope, "telex-aa", "x.tsv").unwrap(), "telex");
        assert!(!scope.is_active("telex"));
    }

    #[test]
    fn active_names_a_defined_block() {
        let scope = parse("block: telex id:telex-*\nactive: telex, vni\nblock: vni id:vni-*\n");
        // `active: telex, vni` before vni is defined is fine: the check runs at end.
        assert!(scope.is_active("telex") && scope.is_active("vni"));

        let err = Scope::parse("block: telex id:telex-*\nactive: vni\n", "scope.txt").unwrap_err();
        assert!(err.contains("unknown block"), "{err}");
    }

    #[test]
    fn an_activating_block_that_matches_nothing_is_an_error() {
        let scope = parse("block: telex id:telex-*\nactive: telex\n");
        let counts = BTreeMap::from([("telex".to_string(), 0)]);
        let err = scope.check_activation(&counts).unwrap_err();
        assert!(err.contains("matches no case"), "{err}");

        let counts = BTreeMap::from([("telex".to_string(), 3)]);
        scope.check_activation(&counts).unwrap();
    }

    #[test]
    fn parsing_is_strict() {
        let err = Scope::parse("block: telex\n", "scope.txt").unwrap_err();
        assert!(err.contains("no selectors"), "{err}");

        let err = Scope::parse("block: telex id:telex-*,\n", "scope.txt").unwrap_err();
        assert!(err.contains("empty selector"), "{err}");

        let err = Scope::parse(
            "block: telex id:telex-*\nblock: telex id:t-*\n",
            "scope.txt",
        )
        .unwrap_err();
        assert!(err.contains("defined twice"), "{err}");

        let err = Scope::parse("telex id:telex-*\n", "scope.txt").unwrap_err();
        assert!(err.contains("unknown directive"), "{err}");

        let err = Scope::parse("# only a comment\n", "scope.txt").unwrap_err();
        assert!(err.contains("no `block:` lines"), "{err}");

        let err = Scope::parse("block: telex file:\n", "scope.txt").unwrap_err();
        assert!(err.contains("`file:` needs a path"), "{err}");

        let err = Scope::parse("block: a b c\n", "scope.txt").unwrap_err();
        assert!(err.contains("whitespace"), "{err}");
    }

    #[test]
    fn file_selectors_match_a_path_suffix_on_either_separator() {
        assert!(file_matches(
            "tests/corpus/telex-words.tsv",
            Path::new("/repo/tests/corpus/telex-words.tsv")
        ));
        assert!(file_matches(
            "telex-words.tsv",
            Path::new("tests/corpus/telex-words.tsv")
        ));
        assert!(file_matches(
            "tests/corpus/telex-words.tsv",
            Path::new("./tests/corpus/telex-words.tsv")
        ));
        assert!(file_matches(
            "tests/corpus/telex-words.tsv",
            Path::new(r"tests\corpus\telex-words.tsv")
        ));
        assert!(!file_matches(
            "tests/corpus/telex-words.tsv",
            Path::new("tests/corpus/telex-word.tsv")
        ));
    }

    #[test]
    fn glob_matches_ids_without_reading_them_as_regex() {
        assert!(glob_match("case-*", "case-vni-viet"));
        assert!(glob_match("telex-?a", "telex-aa"));
        assert!(glob_match("*", "anything"));
        assert!(glob_match("a*b*c", "aXXbYYc"));
        assert!(!glob_match("telex-*", "vni-basic"));
        assert!(!glob_match("a?c", "ac"));
        assert!(!glob_match("telex-aa", "telex-aa2"));
    }
}
