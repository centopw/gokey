//! The standalone text converter — a port of `ConvertTool.cpp:51-179` — plus
//! the codecs the `openkey convert` command needs at the file boundary
//! (`utf8ToWideString` / `wideStringToUtf8`, `Engine.cpp:126-133`).
//!
//! The tool is off the typing path (`docs/m0/PORTING-MAP.md`): it transcribes a
//! whole document between the five charsets, it does not read `Options`, and the
//! engine never calls it. It works on UTF-16 code units, exactly like the
//! `wstring` upstream iterates, so a caller decodes the file first and encodes
//! the result afterwards; those two steps are [`decode`] and [`encode`].
//!
//! With no caps flag the converter applies upstream's **normal form**: it
//! lowercases the text (`:136-137` promotes an even cell to its odd neighbour,
//! `:159-160` lowercases everything outside the table). That is measured
//! behaviour, not a defect to repair — the macOS dialog starts with the caps
//! checkboxes off, so this path is reachable.
//!
//! Every expectation in the tests was probed against the pinned source with a
//! throw-away C++ driver before it was written down (contract §2.3), except
//! case j, which says so in place.

use std::fmt;

use crate::options::Charset;
use crate::tables::{self, UNICODE_COMPOUND_MARKS};

/// The converter's settings: the seven `convertTool*` globals at `:16-24`.
///
/// `from`/`to` are the existing [`Charset`], whose declaration order already
/// matches `vCodeTable` 0–4.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConvertOptions {
    /// `convertToolFromCode`.
    pub from: Charset,
    /// `convertToolToCode`.
    pub to: Charset,
    /// `convertToolToAllCaps`.
    pub all_caps: bool,
    /// `convertToolToAllNonCaps`.
    pub all_non_caps: bool,
    /// `convertToolToCapsFirstLetter`.
    pub caps_first_letter: bool,
    /// `convertToolToCapsEachWord`.
    pub caps_each_word: bool,
    /// `convertToolRemoveMark`.
    pub remove_mark: bool,
}

/// A conversion failure the pin would hit by throwing or reading out of bounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConvertError {
    /// [`decode`] was handed bytes that are not strict UTF-8 (Unicode or
    /// Compound input). Upstream's `codecvt_utf8_utf16` rejects these too; the
    /// port must not replace them silently.
    InvalidUtf8,
    /// [`encode`] was handed a lone UTF-16 surrogate. A Unicode target cannot
    /// write it, and UTF-8 cannot carry one.
    UnpairedSurrogate,
    /// [`encode`] was asked to write `unit` into a byte charset that has no
    /// byte for it. The port refuses rather than truncating (contract D2,
    /// rev. r2): the pin passes such text through, so silently dropping the
    /// high bits would lose a character.
    Unrepresentable(u16),
}

impl fmt::Display for ConvertError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUtf8 => formatter.write_str("input is not valid UTF-8"),
            Self::UnpairedSurrogate => formatter.write_str("unpaired UTF-16 surrogate"),
            Self::Unrepresentable(unit) => write!(
                formatter,
                "U+{unit:04X} is not representable in this charset"
            ),
        }
    }
}

impl std::error::Error for ConvertError {}

/// `findKeyCode` (`:28-40`): the first cell equal to `code` in `charset`'s
/// table, scanned in `std::map` key order.
///
/// The scan order is observable when one value sits in more than one cell —
/// TCVN3 stores Á and á as the same byte — because the first match fixes the
/// row and column the target cell is taken from.
pub fn find_key_code(code: u32, charset: Charset) -> Option<(u32, usize)> {
    for (key, row) in tables::code_rows(charset) {
        if let Some(column) = row.iter().position(|&cell| u32::from(cell) == code) {
            return Some((key, column));
        }
    }
    None
}

/// `getUnicodeCompoundMarkIndex` (`:42-49`): `(i + 1) << 13` for one of the
/// five tone marks, `0` when `mark` is not one of them.
pub fn compound_mark_index(mark: u16) -> u16 {
    UNICODE_COMPOUND_MARKS
        .iter()
        .position(|&candidate| candidate == mark)
        .map_or(0, |index| ((index as u16) + 1) << 13)
}

/// `convertUtil`'s loop (`:51-179`) over UTF-16 code units.
///
/// The input is in `options.from`'s cell space: a byte charset is one unit per
/// byte, Unicode Compound is a base unit optionally followed by a combining
/// tone mark, and Unicode is the text itself. The output is in `options.to`'s
/// file form, because both of upstream's emit sites go through [`emit_cell`]:
/// **the last unit of a cell emits exactly as the same cell does mid-string**,
/// instead of the bare `push_back(target)` at `:150`. That is the port's one
/// deviation from `convertUtil` (contract D2, rev. r2); provenance lives here,
/// where the machine knows a cell was produced, so [`encode`] never has to
/// guess from a unit's value.
pub fn convert(input: &[u16], options: &ConvertOptions) -> Vec<u16> {
    let mut output = Vec::with_capacity(input.len());
    let mut has_break = false;
    let mut should_upper = options.caps_first_letter || options.caps_each_word;
    if options.all_non_caps {
        should_upper = false;
    }

    let mut i = 0;
    while i < input.len() {
        // `:63-99`: a byte target reads a two-byte cell and a compound target
        // reads its combining mark, but only when a unit follows.
        let mut consumed = 0;
        if i + 1 < input.len() {
            let candidate = match options.from {
                Charset::VniWindows | Charset::Cp1258 => {
                    consumed = 1;
                    (input[i] & 0xFF) | ((input[i + 1] & 0xFF) << 8)
                }
                Charset::UnicodeCompound => {
                    let mark = compound_mark_index(input[i + 1]);
                    if mark > 0 {
                        consumed = 1;
                        input[i] | mark
                    } else {
                        input[i]
                    }
                }
                Charset::Unicode | Charset::Tcvn3 => input[i],
            };
            if let Some((key, column)) = find_key_code(u32::from(candidate), options.from) {
                if let Some(target) = target_cell(options, should_upper, key, column) {
                    emit_cell(target, options.to, &mut output);
                    should_upper = false;
                    has_break = false;
                    i += consumed + 1;
                    continue;
                }
            }
        }

        // `:103-152`: the primary path reads one unit of the source. Its emit
        // is the same one the pair path uses — the pin's bare `push_back`
        // (`:150`) is the deviation this port does not reproduce (D2, r2).
        let unit = input[i];
        if let Some((key, column)) = find_key_code(u32::from(unit), options.from) {
            if let Some(target) = target_cell(options, should_upper, key, column) {
                emit_cell(target, options.to, &mut output);
                should_upper = false;
                has_break = false;
                i += 1;
                continue;
            }
        }

        // `:156-160`: anything outside the table takes the C-locale case mapping.
        output.push(if options.all_caps || should_upper {
            ascii_upper(unit)
        } else if options.all_non_caps || !should_upper {
            ascii_lower(unit)
        } else {
            unit
        });

        // `:162-173`: a newline, a word break followed by a space, and the
        // punctuation in `_breakCode` (`:26`) arm `shouldUpperCase`; the break
        // itself only raises `hasBreak`, which the following space consumes.
        if unit == u16::from(b'\n') || (has_break && unit == u16::from(b' ')) {
            if options.caps_first_letter || options.caps_each_word {
                should_upper = true;
            }
        } else if unit == u16::from(b' ') && options.caps_each_word {
            should_upper = true;
        } else if matches!(unit, 0x2E | 0x3F | 0x21) {
            has_break = true;
        } else {
            should_upper = false;
            has_break = false;
        }
        i += 1;
    }
    output
}

/// The target cell for a table hit, after the case and mark rules
/// (`:90-105` on the pair path, `:132-148` on the primary path).
///
/// `None` means the target table has no such row or column. Every table has the
/// same shape (pinned by a test below), so the fall-through is unreachable;
/// upstream would read past the row instead (`Engine.cpp:542-545`).
fn target_cell(
    options: &ConvertOptions,
    should_upper: bool,
    key: u32,
    column: usize,
) -> Option<u16> {
    // Cells are stored `[UPPER, lower]`; an odd column is promoted to its even
    // neighbour under a caps mode, an even column to its odd neighbour under
    // the normal form. The untouched half already has the wanted case.
    let column = if (options.all_caps || should_upper) && column % 2 != 0 {
        column - 1
    } else if (options.all_non_caps || !should_upper) && column % 2 == 0 {
        column + 1
    } else {
        column
    };
    let mut target = tables::code_value(options.to, key, column)?;
    if options.remove_mark {
        // `keyCodeToCharacter((Uint8)j)`: the cast drops the tone bits, so the
        // base letter is always the lowercase one. `:99-104`/`:142-147` only
        // re-case it under the two all-caps flags, never under
        // `caps_first_letter`/`caps_each_word`.
        target = tables::key_character(key & 0xFF).map_or(0, |letter| letter as u16);
        if options.all_caps {
            target = ascii_upper(target);
        } else if options.all_non_caps {
            target = ascii_lower(target);
        }
    }
    Some(target)
}

/// A cell's emit, shared by both paths (`:107-123` on the pair path and the
/// primary path's `:150` after D2, rev. r2): a byte target splits a cell into
/// its low and high byte, a compound target splits the mark tag off, and the
/// Unicode and TCVN3 targets push the cell unchanged.
fn emit_cell(target: u16, charset: Charset, output: &mut Vec<u16>) {
    match charset {
        Charset::Unicode | Charset::Tcvn3 => output.push(target),
        Charset::VniWindows | Charset::Cp1258 => {
            if (target >> 8) > 32 {
                output.push(target & 0xFF);
                output.push(target >> 8);
            } else {
                output.push(target & 0xFF);
            }
        }
        Charset::UnicodeCompound => {
            let index = target >> 13;
            if index == 0 {
                output.push(target);
            } else if let Some(&mark) = UNICODE_COMPOUND_MARKS.get(usize::from(index) - 1) {
                output.push(target & 0x1FFF);
                output.push(mark);
            } else {
                // No table cell carries a tag above the five marks.
                output.push(target);
            }
        }
    }
}

/// `towupper`/`towlower` under the "C" locale (`:157-160`): only ASCII maps.
///
/// The pinned tree never calls `setlocale`, so `ñ`, `ð` and `Ñ` pass through
/// while `a`↔`A` maps. Rust's Unicode casing would be a measured divergence.
fn ascii_upper(unit: u16) -> u16 {
    if (0x61..=0x7A).contains(&unit) {
        unit - 0x20
    } else {
        unit
    }
}

fn ascii_lower(unit: u16) -> u16 {
    if (0x41..=0x5A).contains(&unit) {
        unit + 0x20
    } else {
        unit
    }
}

/// Decode file bytes into the source charset's cell space.
///
/// Unicode and Unicode Compound are UTF-8 (`utf8ToWideString`,
/// `Engine.cpp:126-129`), decoded strictly: invalid bytes are an error, never
/// U+FFFD. The byte charsets are one unit per byte — the same bytes the hook
/// writes (`OpenKey.mm:423-480`).
pub fn decode(bytes: &[u8], charset: Charset) -> Result<Vec<u16>, ConvertError> {
    match charset {
        Charset::Unicode | Charset::UnicodeCompound => {
            let text = std::str::from_utf8(bytes).map_err(|_| ConvertError::InvalidUtf8)?;
            Ok(text.encode_utf16().collect())
        }
        Charset::Tcvn3 | Charset::VniWindows | Charset::Cp1258 => {
            Ok(bytes.iter().map(|&byte| u16::from(byte)).collect())
        }
    }
}

/// Encode converted units for the target charset at the file boundary.
///
/// This is a **plain encoder**: [`convert`] has already applied the charset's
/// pair-path emit to every cell, including the last, so nothing is decomposed
/// here (contract D2, rev. r2). Unicode and Unicode Compound write UTF-8 of the
/// units (a surrogate pair combines; a lone surrogate is an error); VNI, CP1258
/// and TCVN3 write one byte per unit and **refuse** a unit above `0xFF` with
/// [`ConvertError::Unrepresentable`] instead of truncating it.
pub fn encode(units: &[u16], charset: Charset) -> Result<Vec<u8>, ConvertError> {
    match charset {
        Charset::Unicode | Charset::UnicodeCompound => utf8_units(units),
        Charset::VniWindows | Charset::Cp1258 | Charset::Tcvn3 => {
            let mut bytes = Vec::with_capacity(units.len());
            for &unit in units {
                let byte = u8::try_from(unit).map_err(|_| ConvertError::Unrepresentable(unit))?;
                bytes.push(byte);
            }
            Ok(bytes)
        }
    }
}

fn utf8_units(units: &[u16]) -> Result<Vec<u8>, ConvertError> {
    String::from_utf16(units)
        .map(String::into_bytes)
        .map_err(|_| ConvertError::UnpairedSurrogate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tables::{KEY_A, KEY_E, KEY_I, TONE_MASK};

    const CHARSETS: [Charset; 5] = [
        Charset::Unicode,
        Charset::Tcvn3,
        Charset::VniWindows,
        Charset::UnicodeCompound,
        Charset::Cp1258,
    ];

    fn units(text: &str) -> Vec<u16> {
        text.encode_utf16().collect()
    }

    fn text(units: &[u16]) -> String {
        String::from_utf16(units).expect("the converter emitted valid UTF-16")
    }

    fn options(from: Charset, to: Charset) -> ConvertOptions {
        ConvertOptions {
            from,
            to,
            ..ConvertOptions::default()
        }
    }

    fn transcode(source: &str, from: Charset, to: Charset) -> String {
        text(&convert(&units(source), &options(from, to)))
    }

    fn from_bytes(bytes: &[u8], from: Charset, to: Charset) -> String {
        let decoded = decode(bytes, from).expect("the fixture bytes are valid");
        text(&convert(&decoded, &options(from, to)))
    }

    fn from_bytes_with(bytes: &[u8], from: Charset, to: Charset, extra: ConvertOptions) -> String {
        let decoded = decode(bytes, from).expect("the fixture bytes are valid");
        let options = ConvertOptions { from, to, ..extra };
        text(&convert(&decoded, &options))
    }

    /// The sample minus the astral character: the three 8-bit charsets cannot
    /// carry it across a file, so the file-level law uses this text and the
    /// astral row gets its own converter-level test.
    fn file_sample() -> Vec<u16> {
        units("Việt Nam\nÂm Ếch Đường\nviệt nam . ? !\na. việt\ná. việt\na, việt\n")
    }

    fn seed(charset: Charset) -> Vec<u8> {
        let converted = convert(&file_sample(), &options(Charset::Unicode, charset));
        encode(&converted, charset).expect("the sample encodes in every charset")
    }

    fn transcode_bytes(bytes: &[u8], from: Charset, to: Charset) -> Vec<u8> {
        let decoded = decode(bytes, from).expect("the fixture bytes are valid");
        let converted = convert(&decoded, &options(from, to));
        encode(&converted, to).expect("the target encodes")
    }

    #[test]
    fn an_astral_character_survives_every_converter_level_round_trip() {
        // The astral row is a port-level test (contract §2.2): the pin's
        // `utf8ToWideString` cannot read one under libstdc++ (case j), but the
        // port's converter passes the two units through every charset.
        let emoji = units("😀");
        for from in CHARSETS {
            assert_eq!(convert(&emoji, &options(from, from)), emoji, "{from:?}");
            for to in CHARSETS {
                let crossed = convert(&convert(&emoji, &options(from, to)), &options(to, from));
                assert_eq!(crossed, emoji, "{from:?} -> {to:?} -> {from:?}");
            }
        }
    }

    // --- §2.1 case a -------------------------------------------------------

    #[test]
    fn default_without_caps_flags_is_a_lowercasing_normal_form() {
        // `:136-137` promotes an even cell to its odd neighbour and `:159-160`
        // lowercases everything outside the table. It is not a case-preserving
        // transcode.
        for (source, expected) in [
            ("Việt Nam", "việt nam"),
            ("Âm", "âm"),
            ("Ếch", "ếch"),
            ("Đường", "đường"),
        ] {
            assert_eq!(
                transcode(source, Charset::Unicode, Charset::Unicode),
                expected
            );
        }
    }

    // --- §2.1 case b -------------------------------------------------------

    #[test]
    fn all_caps_promotes_odd_cells_and_uppercases_the_rest() {
        // `:134-135` takes `k - 1` for an odd cell; `:157-158` is `towupper`.
        let options = ConvertOptions {
            from: Charset::Unicode,
            to: Charset::Unicode,
            all_caps: true,
            ..ConvertOptions::default()
        };
        assert_eq!(text(&convert(&units("việt nam"), &options)), "VIỆT NAM");
    }

    #[test]
    fn all_caps_alone_demotes_an_already_uppercase_accented_cell() {
        // The reader trap of `all_caps` (contract §2.1 b, rev. r2, probed):
        // only the caps-first/each flags arm `shouldUpperCase` (`:58-59`), so
        // the `!shouldUpperCase` disjunct at `:136` demotes the even
        // `[UPPER, lower]` cell. Plain ASCII letters are not table characters
        // and stay uppercase through `towupper` (`:157-158`).
        let options = ConvertOptions {
            from: Charset::Unicode,
            to: Charset::Unicode,
            all_caps: true,
            ..ConvertOptions::default()
        };
        assert_eq!(text(&convert(&units("VIỆT NAM"), &options)), "VIệT NAM");
    }

    // --- §2.1 case c -------------------------------------------------------

    #[test]
    fn all_non_caps_promotes_even_cells_and_lowercases_the_rest() {
        // Cells are stored `[UPPER, lower]`, so under `all_non_caps` an even
        // cell takes `k + 1` and an odd cell is left alone (`:93-94`).
        let options = ConvertOptions {
            from: Charset::Unicode,
            to: Charset::Unicode,
            all_non_caps: true,
            ..ConvertOptions::default()
        };
        assert_eq!(text(&convert(&units("VIỆT NAM"), &options)), "việt nam");
    }

    // --- §2.1 case d -------------------------------------------------------

    #[test]
    fn caps_first_letter_starts_armed_and_reads_the_break_off_the_next_word() {
        let first = ConvertOptions {
            from: Charset::Unicode,
            to: Charset::Unicode,
            caps_first_letter: true,
            ..ConvertOptions::default()
        };
        let convert_first = |source: &str| text(&convert(&units(source), &first));

        // `:58-59` arms the flag before the loop, so the first letter is
        // capitalised even when the document starts with a break.
        assert_eq!(convert_first(".việt"), ".Việt");
        assert_eq!(convert_first("a.việt"), "A.việt");
        assert_eq!(convert_first("a. việt"), "A. Việt");

        // `.` only raises `hasBreak` (`:26`, `:171-172`); the space that follows
        // arms the flag (`:164-168`), so the break rule is read off the second
        // word.
        assert_eq!(convert_first("á.việt"), "Á.việt");
        assert_eq!(convert_first("á. việt"), "Á. Việt");

        // `? ` and `! ` arm like `. `, and a newline arms on its own.
        assert_eq!(convert_first("a? việt"), "A? Việt");
        assert_eq!(convert_first("a! việt"), "A! Việt");
        assert_eq!(convert_first("a\nviệt"), "A\nViệt");

        // `, `, `; ` and `\r` are not in `_breakCode` and do not arm.
        assert_eq!(convert_first("a, việt"), "A, việt");
        assert_eq!(convert_first("a; việt"), "A; việt");
        assert_eq!(convert_first("a\rviệt"), "A\rviệt");
    }

    // --- §2.1 case e -------------------------------------------------------

    #[test]
    fn caps_each_word_arms_on_every_space() {
        // `:167-168` arms on a space; the word's own table hit disarms it
        // (`:151-152`), so its tail stays lowercase.
        let options = ConvertOptions {
            from: Charset::Unicode,
            to: Charset::Unicode,
            caps_each_word: true,
            ..ConvertOptions::default()
        };
        assert_eq!(text(&convert(&units("việt nam"), &options)), "Việt Nam");
    }

    // --- §2.1 case f -------------------------------------------------------

    #[test]
    fn remove_mark_writes_the_base_letter() {
        // `keyCodeToCharacter((Uint8)j)` at `:98-105` and `:141-148`.
        let remove = ConvertOptions {
            from: Charset::Unicode,
            to: Charset::Unicode,
            remove_mark: true,
            ..ConvertOptions::default()
        };
        for (source, expected) in [("việt", "viet"), ("đ", "d"), ("ơ", "o")] {
            assert_eq!(text(&convert(&units(source), &remove)), expected);
        }

        // The `towupper`/`towlower` follow-up only runs under the two all-caps
        // flags (`:100-103`).
        let caps = ConvertOptions {
            all_caps: true,
            ..remove
        };
        assert_eq!(text(&convert(&units("việt"), &caps)), "VIET");
        let non_caps = ConvertOptions {
            all_non_caps: true,
            ..remove
        };
        assert_eq!(text(&convert(&units("VIỆT"), &non_caps)), "viet");

        // Under `caps_first_letter` the removed letter is not re-cased, even
        // when it is the first one.
        let first = ConvertOptions {
            caps_first_letter: true,
            ..remove
        };
        assert_eq!(text(&convert(&units("ệ"), &first)), "e");
    }

    // --- §2.1 case g -------------------------------------------------------

    #[test]
    fn non_table_case_mapping_is_the_c_locale_ascii_one() {
        // `:157-160`: the pinned tree has no `setlocale`, so `towupper` and
        // `towlower` map ASCII only.
        assert_eq!(transcode("ñ", Charset::Unicode, Charset::Unicode), "ñ");
        let caps = ConvertOptions {
            all_caps: true,
            ..options(Charset::Unicode, Charset::Unicode)
        };
        assert_eq!(text(&convert(&units("añ"), &caps)), "Añ");
        let non_caps = ConvertOptions {
            all_non_caps: true,
            ..options(Charset::Unicode, Charset::Unicode)
        };
        assert_eq!(text(&convert(&units("AÑ"), &non_caps)), "aÑ");
        assert_eq!(text(&convert(&units("abc"), &caps)), "ABC");
    }

    // --- §2.1 case h -------------------------------------------------------

    #[test]
    fn every_cell_emits_identically_mid_string_and_last() {
        // `:107-123` splits a cell on the pair path; the last unit takes the
        // primary path, whose pin emit is the bare `push_back(target)` at
        // `:150`. The port's one deviation (D2, rev. r2): the primary path uses
        // the same `emit_cell`, so a cell emits identically in both positions.
        assert_eq!(
            convert(
                &units("việt"),
                &options(Charset::Unicode, Charset::VniWindows)
            ),
            vec![0x0076, 0x0069, 0x0065, 0x00E4, 0x0074]
        );
        assert_eq!(
            convert(
                &units("việ"),
                &options(Charset::Unicode, Charset::VniWindows)
            ),
            vec![0x0076, 0x0069, 0x0065, 0x00E4]
        );
        assert_eq!(
            convert(
                &units("việt"),
                &options(Charset::Unicode, Charset::UnicodeCompound)
            ),
            vec![0x0076, 0x0069, 0x00EA, 0x0323, 0x0074]
        );
        assert_eq!(
            convert(
                &units("việ"),
                &options(Charset::Unicode, Charset::UnicodeCompound)
            ),
            vec![0x0076, 0x0069, 0x00EA, 0x0323]
        );
        // TCVN3 is single-byte, so both paths push the same cell.
        assert_eq!(
            convert(&units("việt"), &options(Charset::Unicode, Charset::Tcvn3)),
            vec![0x0076, 0x0069, 0x00D6, 0x0074]
        );
        assert_eq!(
            convert(&units("việ"), &options(Charset::Unicode, Charset::Tcvn3)),
            vec![0x0076, 0x0069, 0x00D6]
        );

        // `encode` never re-decomposes: the pin's bare cells are now simply
        // units, so a VNI cell above 0xFF is unrepresentable and the Compound
        // `0xA0EA` is written as the code point it is.
        assert_eq!(
            encode(&[0xE465], Charset::VniWindows),
            Err(ConvertError::Unrepresentable(0xE465))
        );
        assert_eq!(
            encode(&[0xA0EA], Charset::UnicodeCompound),
            Ok("\u{A0EA}".as_bytes().to_vec())
        );
    }

    #[test]
    fn every_target_passes_non_vietnamese_text_through_the_machine() {
        // The regression at 659c3a8: keying the split on the unit value also
        // fired on ordinary BMP text. `中` is not a table character, so the
        // machine leaves it alone for every target (contract AC-1, rev. r2).
        for to in CHARSETS {
            assert_eq!(
                convert(&units("a中b"), &options(Charset::Unicode, to)),
                units("a中b"),
                "{to:?}"
            );
        }
    }

    #[test]
    fn only_the_utf8_targets_can_write_non_vietnamese_text() {
        // The machine passes `中` through; the file boundary can only carry it
        // for the two UTF-8 charsets. A byte target refuses rather than
        // truncating (D2, rev. r2).
        for charset in [Charset::Unicode, Charset::UnicodeCompound] {
            assert_eq!(
                encode(&units("中"), charset),
                Ok("中".as_bytes().to_vec()),
                "{charset:?}"
            );
        }
        for charset in [Charset::Tcvn3, Charset::VniWindows, Charset::Cp1258] {
            assert_eq!(
                encode(&units("中"), charset),
                Err(ConvertError::Unrepresentable(0x4E2D)),
                "{charset:?}"
            );
        }
    }

    // --- §2.1 case i -------------------------------------------------------

    #[test]
    fn a_byte_or_compound_source_reads_a_two_unit_cell() {
        // VNI `vie` 0xE4 `t` (`:67-71`).
        assert_eq!(
            from_bytes(
                &[0x76, 0x69, 0x65, 0xE4, 0x74],
                Charset::VniWindows,
                Charset::Unicode
            ),
            "việt"
        );

        // Compound reads the base and then one of the five tone marks
        // (`:73-82`); the pin's `_unicodeCompoundMark` is `[0x0301, 0x0300,
        // 0x0309, 0x0303, 0x0323]`.
        assert_eq!(
            from_bytes(
                "ê\u{301}".as_bytes(),
                Charset::UnicodeCompound,
                Charset::Unicode
            ),
            "ế"
        );
        assert_eq!(
            from_bytes(
                "a\u{300}".as_bytes(),
                Charset::UnicodeCompound,
                Charset::Unicode
            ),
            "à"
        );

        // TCVN3 `0xB8` sits in the Á and á cells; map order finds Á first
        // (`k = 4`), and the normal form promotes it to á. A plain `A` is not a
        // table character, so it is lowercased at `:159-160`.
        assert_eq!(
            from_bytes(&[0x41, 0xB8], Charset::Tcvn3, Charset::Unicode),
            "aá"
        );
        assert_eq!(from_bytes(&[0xB8], Charset::Tcvn3, Charset::Unicode), "á");
        // `all_caps` reaches `Aá`, not `AÁ`: the first condition needs an odd
        // column, and the second is `(all_non_caps || !shouldUpperCase)`, which
        // is true here because no earlier hit armed `shouldUpperCase`. The
        // probe returned `0041 00E1`; the contract's `AÁ` is the one case a
        // probe corrected (contract §2.3 — the expectation follows the probe).
        let caps = ConvertOptions {
            all_caps: true,
            ..ConvertOptions::default()
        };
        assert_eq!(
            from_bytes_with(&[0x41, 0xB8], Charset::Tcvn3, Charset::Unicode, caps),
            "Aá"
        );
    }

    // --- §2.1 case j -------------------------------------------------------

    #[test]
    fn surrogate_pairs_pass_unit_by_unit_macos_reference() {
        // `towupper` leaves a lone surrogate alone (`:157-158`), so an astral
        // character reaches the writer intact. Evidence limit: the pin's
        // `utf8ToWideString` (`Engine.cpp:126-129`, `codecvt_utf8_utf16`)
        // throws `std::range_error` on astral input under libstdc++ (g++ 11.4,
        // measured here), while libc++ on macOS accepts it. This is the
        // macOS-reference behaviour, not a probe-confirmed parity row.
        let emoji = units("😀");
        let converted = convert(&emoji, &options(Charset::Unicode, Charset::Unicode));
        assert_eq!(converted, emoji);
        let bytes = encode(&converted, Charset::Unicode).expect("a surrogate pair is valid UTF-16");
        assert_eq!(bytes, vec![0xF0, 0x9F, 0x98, 0x80]);
        assert_eq!(decode(&bytes, Charset::Unicode), Ok(emoji));
    }

    // --- §2.1 case k -------------------------------------------------------

    #[test]
    fn no_trailing_sentinel_reaches_the_output() {
        // Upstream pushes `0` at `:176`, but `to_bytes(str.c_str())` drops it;
        // the vector ends with the last real unit (probe: no trailing `0000`).
        let converted = convert(&units("Việt"), &options(Charset::Unicode, Charset::Unicode));
        assert_eq!(converted, vec![0x0076, 0x0069, 0x1EC7, 0x0074]);
    }

    // --- helpers -----------------------------------------------------------

    #[test]
    fn the_five_charset_tables_have_one_shape() {
        // `target_cell` looks the same `(row, column)` up in the target table,
        // so every table must carry the same rows with the same widths.
        let reference: Vec<(u32, usize)> = tables::code_rows(Charset::Unicode)
            .map(|(key, row)| (key, row.len()))
            .collect();
        for charset in CHARSETS {
            let shape: Vec<(u32, usize)> = tables::code_rows(charset)
                .map(|(key, row)| (key, row.len()))
                .collect();
            assert_eq!(shape, reference, "{charset:?}");
        }
    }

    #[test]
    fn find_key_code_scans_in_map_order_and_takes_the_first_cell() {
        assert_eq!(
            find_key_code(0x00E1, Charset::Unicode),
            Some((u32::from(KEY_A), 5))
        );
        // 0xB8 is Á (column 4) and á (column 5); the earlier cell wins.
        assert_eq!(
            find_key_code(0xB8, Charset::Tcvn3),
            Some((u32::from(KEY_A), 4))
        );
        assert_eq!(
            find_key_code(0xE465, Charset::VniWindows),
            Some((u32::from(KEY_E) | TONE_MASK, 9))
        );
        assert_eq!(find_key_code(u32::from(KEY_I), Charset::Unicode), None);
        assert_eq!(find_key_code(u32::from(b'v'), Charset::Unicode), None);
    }

    #[test]
    fn compound_mark_index_tags_the_five_tone_marks() {
        assert_eq!(compound_mark_index(0x0301), 0x2000);
        assert_eq!(compound_mark_index(0x0300), 0x4000);
        assert_eq!(compound_mark_index(0x0309), 0x6000);
        assert_eq!(compound_mark_index(0x0303), 0x8000);
        assert_eq!(compound_mark_index(0x0323), 0xA000);
        assert_eq!(compound_mark_index(0x0302), 0);
    }

    // --- §2.2 round trips --------------------------------------------------

    #[test]
    fn every_identity_pair_is_idempotent() {
        // The default normal form is a fixed point: a second pass, file
        // boundary included, equals the first.
        for charset in CHARSETS {
            let once = transcode_bytes(&seed(charset), charset, charset);
            let twice = transcode_bytes(&once, charset, charset);
            assert_eq!(once, twice, "{charset:?}");
        }
    }

    #[test]
    fn every_round_trip_is_the_same_normalisation_as_a_single_pass() {
        // 20 ordered pairs plus the five identity pairs through
        // decode -> convert -> encode, the path the CLI runs. The law is
        // stability, not identity: the normal form lowercases, and TCVN3
        // stores Á and á in one cell, so `A -> B -> A` is `A -> A` rather than
        // the input. The machine's emit is position-independent (case h), which
        // is what makes the byte representation stable across the trip.
        for from in CHARSETS {
            for to in CHARSETS {
                let direct = transcode_bytes(&seed(from), from, from);
                let there = transcode_bytes(&seed(from), from, to);
                let back = transcode_bytes(&there, to, from);
                assert_eq!(back, direct, "{from:?} -> {to:?} -> {from:?}");
            }
        }
    }

    #[test]
    fn only_the_utf8_targets_can_write_an_astral_character() {
        // The converter-level test above keeps an astral character for every
        // pair; at the *file* boundary only the two UTF-8 charsets can write
        // it. A byte target refuses instead of truncating (D2, rev. r2).
        let emoji = units("😀");
        for charset in [Charset::Unicode, Charset::UnicodeCompound] {
            let converted = convert(&emoji, &options(Charset::Unicode, charset));
            let bytes = encode(&converted, charset).expect("a surrogate pair is valid UTF-16");
            assert_eq!(decode(&bytes, charset), Ok(emoji.clone()), "{charset:?}");
        }

        for charset in [Charset::Tcvn3, Charset::VniWindows, Charset::Cp1258] {
            let converted = convert(&emoji, &options(Charset::Unicode, charset));
            assert_eq!(
                encode(&converted, charset),
                Err(ConvertError::Unrepresentable(0xD83D)),
                "{charset:?}"
            );
        }
    }

    // --- §2.2 boundaries ---------------------------------------------------

    #[test]
    fn empty_input_is_empty_output_in_every_charset() {
        for from in CHARSETS {
            for to in CHARSETS {
                assert_eq!(
                    convert(&[], &options(from, to)),
                    Vec::<u16>::new(),
                    "{from:?} -> {to:?}"
                );
            }
        }
    }

    #[test]
    fn a_single_unit_converts() {
        assert_eq!(transcode("A", Charset::Unicode, Charset::Unicode), "a");
        let caps = ConvertOptions {
            all_caps: true,
            ..options(Charset::Unicode, Charset::Unicode)
        };
        assert_eq!(text(&convert(&units("a"), &caps)), "A");
    }

    #[test]
    fn a_trailing_break_does_not_look_ahead() {
        for source in ["a.", "a. ", "a?", "a!", "a\n"] {
            let converted = convert(&units(source), &options(Charset::Unicode, Charset::Unicode));
            assert_eq!(text(&converted), source, "{source:?}");
        }
    }

    #[test]
    fn every_error_path_is_reachable_and_named() {
        // Invalid UTF-8 is rejected for the two UTF-8 sources.
        assert_eq!(
            decode(&[0xFF], Charset::Unicode),
            Err(ConvertError::InvalidUtf8)
        );
        assert_eq!(
            decode(&[0xC3], Charset::UnicodeCompound),
            Err(ConvertError::InvalidUtf8)
        );
        // A lone surrogate cannot be written as UTF-8 by either UTF-8 target.
        assert_eq!(
            encode(&[0xD83D], Charset::Unicode),
            Err(ConvertError::UnpairedSurrogate)
        );
        assert_eq!(
            encode(&[0xD83D], Charset::UnicodeCompound),
            Err(ConvertError::UnpairedSurrogate)
        );
        // The byte charsets take any byte and refuse any unit above 0xFF,
        // naming the code point so a user can find it in the document.
        assert_eq!(decode(&[0xFF], Charset::VniWindows), Ok(vec![0xFF]));
        assert_eq!(encode(&[0xFF], Charset::Tcvn3), Ok(vec![0xFF]));
        for charset in [Charset::Tcvn3, Charset::VniWindows, Charset::Cp1258] {
            assert_eq!(
                encode(&[0x4E2D], charset),
                Err(ConvertError::Unrepresentable(0x4E2D)),
                "{charset:?}"
            );
        }
        assert_eq!(
            ConvertError::Unrepresentable(0x4E2D).to_string(),
            "U+4E2D is not representable in this charset"
        );
    }
}
