//! Macro table and matching.
//!
//! Port of upstream `Sources/OpenKey/engine/Macro.cpp`: `convert`
//! (`:29-65`), `modifyCaseUnicode` (`:132-153`), `findMacro` (`:155-197`),
//! `addMacro` (`:216-232`) and `onTableCodeChange` (`:242-246`).
//!
//! Upstream keeps the table in the process-global `macroMap` and the
//! auto-caps scratch state in file-static variables. Here the table is an
//! owned [`MacroTable`] and every scratch value is local, so the engine stays a
//! pure function of (`Options`, session state, `KeyEvent`) — the parity
//! harness can hold two engines side by side.
//!
//! A macro key is stored as the *keycodes* the user types, produced by
//! `convert`: an ASCII character becomes its keycode (plus `CAPS_MASK` for
//! the shifted form), a character that appears in the Unicode code table
//! becomes that cell with `CHAR_CODE_MASK`, and anything else is
//! `PURE_CHARACTER_MASK | code unit`. Matching is an exact lookup of that
//! vector, then — when `auto_caps_macro` is on — a second lookup of the
//! lower-cased key, whose content is re-cased to match.

use std::collections::BTreeMap;

use crate::charset;
use crate::options::Charset;
use crate::tables::{self, CAPS_MASK, CHAR_CODE_MASK, CHAR_MASK, PURE_CHARACTER_MASK};

/// One user macro, as stored by the settings layer: the text the user types
/// and the text it expands to (`MacroData::macroText`/`macroContent`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Macro {
    pub name: String,
    pub content: String,
}

impl Macro {
    pub fn new(name: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            content: content.into(),
        }
    }
}

/// The macro table, keyed by the converted key vector (`macroMap`,
/// `Macro.cpp:23`).
///
/// The table remembers the charset its entries were converted for: a macro's
/// key and content are converted at the time it is added, so switching
/// `vCodeTable` leaves the keys stale and re-converts only the content
/// (`onTableCodeChange`, `Macro.cpp:242-246`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MacroTable {
    charset: Charset,
    entries: BTreeMap<Vec<u32>, Entry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Entry {
    /// Original UTF-8 content, kept so [`MacroTable::rebuild`] can re-convert
    /// it the way `onTableCodeChange` does.
    content: String,
    content_code: Vec<u32>,
}

impl Default for MacroTable {
    fn default() -> Self {
        Self::new(Charset::default())
    }
}

impl MacroTable {
    /// An empty table whose entries will be converted for `charset`
    /// (`initMacroMap(nullptr, 0)` with `vCodeTable` already set).
    pub fn new(charset: Charset) -> Self {
        Self {
            charset,
            entries: BTreeMap::new(),
        }
    }

    /// Build a table from user macros through [`MacroTable::add`], which is
    /// what `addMacro` (`Macro.cpp:216-232`) does for every configured row.
    pub fn from_macros(charset: Charset, macros: &[Macro]) -> Self {
        let mut table = Self::new(charset);
        for entry in macros {
            table.add(&entry.name, &entry.content);
        }
        table
    }

    pub fn charset(&self) -> Charset {
        self.charset
    }

    /// `addMacro` (`Macro.cpp:216-232`): convert the name, then insert the
    /// converted content, or replace the content when the key already exists.
    pub fn add(&mut self, name: &str, content: &str) {
        let mut key = Vec::new();
        convert(name, self.charset, &mut key);
        let mut content_code = Vec::new();
        convert(content, self.charset, &mut content_code);
        self.entries.insert(
            key,
            Entry {
                content: content.to_string(),
                content_code,
            },
        );
    }

    /// Remove every entry (`initMacroMap` starts from an empty `macroMap`).
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// `onTableCodeChange` (`Macro.cpp:242-246`): the charset changed, so
    /// re-convert every content for the new code table. Keys are deliberately
    /// not re-converted — upstream leaves the stored key vectors stale.
    pub fn rebuild(&mut self, charset: Charset) {
        if self.charset == charset {
            return;
        }
        self.charset = charset;
        for entry in self.entries.values_mut() {
            convert(&entry.content, charset, &mut entry.content_code);
        }
    }
}

/// `convert` (`Macro.cpp:29-65`).
///
/// The input arrives as UTF-8 and is read as UTF-16 code units, exactly as
/// upstream's `utf8ToWideString` (`Engine.cpp:127-129`) produces them, so a
/// non-BMP character becomes two `PURE_CHARACTER` entries and the render path
/// puts the surrogate pair back together.
fn convert(text: &str, charset: Charset, out: &mut Vec<u32>) {
    out.clear();
    for unit in text.encode_utf16() {
        let t = u32::from(unit);

        // "find normal character first" (`Macro.cpp:37-41`).
        if let Some(code) = char::from_u32(t).and_then(tables::character_key) {
            out.push(code);
            continue;
        }

        // "find character which has tone/mark" (`Macro.cpp:43-58`): scan the
        // Unicode table — `_codeTable[0]`, which is why the scan order is a
        // `std::map`'s ascending key order — and emit the *active* charset's
        // cell at the same column.
        let mut found = false;
        for (row_key, cells) in tables::code_rows(Charset::Unicode) {
            if let Some(k) = cells
                .iter()
                .position(|&cell| cell == (t & CHAR_MASK) as u16)
            {
                let value = tables::code_value(charset, row_key, k)
                    .expect("every code table has the same rows and columns");
                out.push(u32::from(value) | CHAR_CODE_MASK);
                found = true;
                break;
            }
        }
        if found {
            continue;
        }

        // "find other character" (`Macro.cpp:61`).
        out.push(t | PURE_CHARACTER_MASK);
    }
}

/// `modifyCaseUnicode` (`Macro.cpp:132-153`): force one entry to upper or
/// lower case, returning whether it changed.
///
/// A plain keycode carries its case in `CAPS_MASK`. Upstream masks the value
/// (`code &= isUpperCase ? CAPS_MASK : ~CAPS_MASK`, `Macro.cpp:135-137`), which
/// on the upper arm would drop the keycode itself; that arm is unreachable from
/// [`find_macro`] — only a `CHAR_CODE` entry is ever re-cased upward — so the
/// port sets the bit and records the difference here rather than reproduce a
/// masking step no caller can observe.
///
/// A `CHAR_CODE` entry is looked up in the active table: rows interleave
/// caps/normal cells in pairs, so an even index matched while lower-casing moves
/// one cell right, and an odd index matched while upper-casing moves one cell
/// left.
fn modify_case_unicode(code: &mut u32, upper: bool, charset: Charset) -> bool {
    let before = *code;

    if *code & CHAR_CODE_MASK == 0 {
        if upper {
            *code |= CAPS_MASK;
        } else {
            *code &= !CAPS_MASK;
        }
        return *code != before;
    }

    for (_, cells) in tables::code_rows(charset) {
        for (k, &cell) in cells.iter().enumerate() {
            if (*code & CHAR_MASK) as u16 == cell {
                let k = if k % 2 == 0 && !upper {
                    k + 1
                } else if k % 2 != 0 && upper {
                    k - 1
                } else {
                    k
                };
                *code = u32::from(cells[k]) | CHAR_CODE_MASK;
                return *code != before;
            }
        }
    }
    false
}

/// `findMacro` (`Macro.cpp:155-197`): look `key` up in `table`, returning the
/// content code to send.
///
/// `key` is converted in place first (`:156-158`), and — as upstream does —
/// left converted (and, on the auto-caps path, partly lower-cased) even when
/// the lookup fails (`:165-196`). The engine stores this vector as its
/// `hMacroKey`, so the mutation is part of the observable behaviour.
///
/// The auto-caps path mirrors upstream's asymmetry exactly: `_macroFlag` is
/// set only when the *second* entry was upper case (`:166-171`); the first
/// entry is lower-cased last (`:173`); and only content entry 0 is re-cased
/// when `_macroFlag` stayed false (`:176`).
pub fn find_macro(
    key: &mut Vec<u32>,
    table: &MacroTable,
    auto_caps_macro: bool,
) -> Option<Vec<u32>> {
    for entry in key.iter_mut() {
        *entry = charset::encode(*entry, table.charset);
    }

    if let Some(entry) = table.entries.get(key) {
        return Some(entry.content_code.clone());
    }

    if !auto_caps_macro {
        return None;
    }

    let mut macro_flag = false;
    if key.len() > 1 && modify_case_unicode(&mut key[1], false, table.charset) {
        macro_flag = true;
        for entry in key.iter_mut().skip(2) {
            modify_case_unicode(entry, false, table.charset);
        }
    }

    if key.is_empty() || !modify_case_unicode(&mut key[0], false, table.charset) {
        return None;
    }

    let entry = table.entries.get(key)?;
    let mut content = entry.content_code.clone();
    for (index, value) in content.iter_mut().enumerate() {
        if index != 0 && !macro_flag {
            continue;
        }
        match tables::key_character(*value) {
            // `keyCodeToCharacter` + `toupper` + `_characterMap` (`:182-186`).
            Some(character) => {
                *value = tables::character_key(character.to_ascii_uppercase()).unwrap_or_default();
            }
            // A `CHAR_CODE` entry is re-cased in the active table (`:187-189`).
            None if *value & CHAR_CODE_MASK != 0 => {
                modify_case_unicode(value, true, table.charset);
            }
            None => {}
        }
    }
    Some(content)
}

/// Whether `bits` carries a `PURE_CHARACTER` payload (macro content that came
/// from outside the code tables). Only the macro tests need the predicate;
/// rendering branches on the flag directly.
#[cfg(test)]
pub(crate) fn is_pure(bits: u32) -> bool {
    bits & PURE_CHARACTER_MASK != 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tables::{KEY_A, KEY_B, KEY_D, KEY_E, KEY_K, KEY_O, KEY_SPACE};

    fn table(macros: &[(&str, &str)]) -> MacroTable {
        let macros: Vec<Macro> = macros
            .iter()
            .map(|(name, content)| Macro::new(*name, *content))
            .collect();
        MacroTable::from_macros(Charset::Unicode, &macros)
    }

    #[test]
    fn convert_reads_ascii_as_keycodes_and_unknown_text_as_pure_units() {
        let mut out = Vec::new();
        convert("Ko", Charset::Unicode, &mut out);
        assert_eq!(out, vec![u32::from(KEY_K) | CAPS_MASK, u32::from(KEY_O)]);

        convert("🙂", Charset::Unicode, &mut out);
        assert_eq!(out.len(), 2, "one entry per UTF-16 code unit");
        assert!(out.iter().all(|&bits| is_pure(bits)));
        // The surrogate pair survives unchanged, so rendering puts it back.
        assert_eq!(
            out.iter()
                .map(|&bits| (bits & CHAR_MASK) as u16)
                .collect::<Vec<u16>>(),
            "🙂".encode_utf16().collect::<Vec<u16>>()
        );
    }

    #[test]
    fn convert_maps_a_tabled_character_to_the_active_charsets_cell() {
        // `ấ` is the A|TONE row's sắc column of the active table.
        let mut unicode = Vec::new();
        convert("ấ", Charset::Unicode, &mut unicode);
        assert_eq!(
            unicode,
            vec![0x1EA5 | CHAR_CODE_MASK],
            "Unicode: the precomposed character"
        );

        let mut tcvn3 = Vec::new();
        convert("ấ", Charset::Tcvn3, &mut tcvn3);
        assert_eq!(
            tcvn3,
            vec![0x00CA | CHAR_CODE_MASK],
            "TCVN3: the same column, the TCVN3 byte"
        );
    }

    #[test]
    fn modify_case_unicode_walks_the_caps_normal_pairs() {
        // Lower-casing the upper `Â` cell (index 0) moves to index 1.
        let mut code = 0x00C2 | CHAR_CODE_MASK;
        assert!(modify_case_unicode(&mut code, false, Charset::Unicode));
        assert_eq!(code, 0x00E2 | CHAR_CODE_MASK);

        // Upper-casing the lower `â` cell moves back.
        assert!(modify_case_unicode(&mut code, true, Charset::Unicode));
        assert_eq!(code, 0x00C2 | CHAR_CODE_MASK);

        // Already in the requested case: no change, and the value is rewritten
        // to the same cell as upstream does.
        let mut lower = 0x00E2 | CHAR_CODE_MASK;
        assert!(!modify_case_unicode(&mut lower, false, Charset::Unicode));
        assert_eq!(lower, 0x00E2 | CHAR_CODE_MASK);

        // A plain keycode only carries the caps bit.
        let mut key = u32::from(KEY_B);
        assert!(!modify_case_unicode(&mut key, false, Charset::Unicode));
        assert!(modify_case_unicode(&mut key, true, Charset::Unicode));
        assert_eq!(key, u32::from(KEY_B) | CAPS_MASK);
    }

    #[test]
    fn a_plain_key_is_matched_and_converted() {
        let mut key = vec![u32::from(KEY_K), u32::from(KEY_O)];
        let content = find_macro(&mut key, &table(&[("ko", "không")]), false).unwrap();
        // `không`: k h ô n g.
        assert_eq!(content.len(), 5);
        assert_eq!(content[0], u32::from(KEY_K));
        assert_eq!(content[1], u32::from(tables::KEY_H));
        assert_eq!(content[2], 0x00F4 | CHAR_CODE_MASK, "ô");
        assert_eq!(content[3], u32::from(tables::KEY_N));
        assert_eq!(content[4], u32::from(tables::KEY_G));
    }

    #[test]
    fn the_key_is_mutated_in_place_even_when_the_lookup_fails() {
        let table = table(&[("ka", "không")]);
        // `key[0]` and `key[1]` are lower-cased by the auto-caps attempt; the
        // vector keeps that change after the miss, exactly as `hMacroKey` does
        // upstream (`Macro.cpp:165-173`).
        let mut key = vec![u32::from(KEY_K) | CAPS_MASK, u32::from(KEY_O) | CAPS_MASK];
        assert_eq!(find_macro(&mut key, &table, true), None);
        assert_eq!(key, vec![u32::from(KEY_K), u32::from(KEY_O)]);
    }

    #[test]
    fn auto_caps_matches_a_capitalised_key_and_capitalises_the_content() {
        let table = table(&[("ko", "không")]);
        let mut key = vec![u32::from(KEY_K) | CAPS_MASK, u32::from(KEY_O)];
        let content = find_macro(&mut key, &table, true).unwrap();
        // Only entry 0 is upper-cased: `_macroFlag` is false because `o` was
        // already lower case (`Macro.cpp:165-176`).
        assert_eq!(content[0], u32::from(KEY_K) | CAPS_MASK, "K");
        assert_eq!(content[1], u32::from(tables::KEY_H), "h stays lower case");
    }

    #[test]
    fn auto_caps_of_a_two_letter_capitalised_key_cases_the_whole_content() {
        let table = table(&[("ko", "không")]);
        // `KO`: the second entry changes case, so `_macroFlag` is true and
        // every content entry is re-cased.
        let mut key = vec![u32::from(KEY_K) | CAPS_MASK, u32::from(KEY_O) | CAPS_MASK];
        let content = find_macro(&mut key, &table, true).unwrap();
        assert_eq!(content[0], u32::from(KEY_K) | CAPS_MASK);
        assert_eq!(content[1], u32::from(tables::KEY_H) | CAPS_MASK, "H");
        assert_eq!(content[2], 0x00D4 | CHAR_CODE_MASK, "Ô");
        assert_eq!(content[3], u32::from(tables::KEY_N) | CAPS_MASK, "N");
        assert_eq!(content[4], u32::from(tables::KEY_G) | CAPS_MASK, "G");
    }

    #[test]
    fn auto_caps_is_off_by_default() {
        let table = table(&[("ko", "không")]);
        let mut key = vec![u32::from(KEY_K) | CAPS_MASK, u32::from(KEY_O)];
        assert_eq!(find_macro(&mut key, &table, false), None);
    }

    #[test]
    fn the_last_added_content_wins_for_a_duplicate_key() {
        let table = table(&[("ko", "một"), ("ko", "hai")]);
        let mut key = vec![u32::from(KEY_K), u32::from(KEY_O)];
        let content = find_macro(&mut key, &table, false).unwrap();
        // `hai`: h a i.
        assert_eq!(
            content,
            vec![
                u32::from(tables::KEY_H),
                u32::from(KEY_A),
                u32::from(tables::KEY_I)
            ]
        );
        assert_eq!(table.len(), 1);
    }

    #[test]
    fn an_empty_table_never_matches() {
        let table = MacroTable::new(Charset::Unicode);
        let mut key = vec![u32::from(KEY_K), u32::from(KEY_O)];
        assert_eq!(find_macro(&mut key, &table, true), None);
        assert!(table.is_empty());
    }

    #[test]
    fn convert_handles_space_and_punctuation() {
        let mut out = Vec::new();
        convert("a b", Charset::Unicode, &mut out);
        assert_eq!(
            out,
            vec![u32::from(KEY_A), u32::from(KEY_SPACE), u32::from(KEY_B)]
        );
    }

    #[test]
    fn rebuild_reconverts_content_but_not_keys() {
        let mut table = table(&[("ko", "đ")]);
        // `đ` is `KEY_D|TONE_MASK` in Unicode, the VNI cell in VNI-Windows.
        let mut key = vec![u32::from(KEY_K), u32::from(KEY_O)];
        let before = find_macro(&mut key, &table, false).unwrap();
        assert_eq!(before, vec![0x0111 | CHAR_CODE_MASK], "Unicode đ");

        table.rebuild(Charset::VniWindows);
        let mut key = vec![u32::from(KEY_K), u32::from(KEY_O)];
        let after = find_macro(&mut key, &table, false).unwrap();
        assert_eq!(after, vec![0x00F1 | CHAR_CODE_MASK], "VNI-Windows đ");
    }

    #[test]
    fn rebuild_with_the_same_charset_keeps_the_table() {
        let mut table = table(&[("ko", "không")]);
        let before = table.clone();
        table.rebuild(Charset::Unicode);
        assert_eq!(table, before);
    }

    #[test]
    fn a_macro_name_with_a_tabled_character_is_matched_after_conversion() {
        // A Vietnamese name: `ấ` is a CHAR_CODE entry, not a keycode.
        let table = table(&[("ấ", "x")]);
        let mut key = vec![0x1EA5 | CHAR_CODE_MASK];
        let content = find_macro(&mut key, &table, false).unwrap();
        assert_eq!(content, vec![u32::from(tables::KEY_X)]);
    }

    #[test]
    fn clear_empties_the_table() {
        let mut table = table(&[("ko", "không")]);
        table.clear();
        assert!(table.is_empty());
        let mut key = vec![u32::from(KEY_K), u32::from(KEY_O)];
        assert_eq!(find_macro(&mut key, &table, false), None);
    }

    #[test]
    fn is_pure_recognises_the_flag_only() {
        assert!(is_pure(0x202F | PURE_CHARACTER_MASK));
        assert!(!is_pure(u32::from(KEY_E)));
        assert!(!is_pure(0x1EA5 | CHAR_CODE_MASK));
        assert!(!is_pure(u32::from(KEY_D)));
    }
}
