//! Charset encoding. Port of upstream `getCharacterCode()` (`Engine.cpp:501-556`)
//! and of the rendering the macOS hook performs on `charData`
//! (`SendNewCharString`, `OpenKey.mm:423-480`), plus the character lookup that
//! turns a bare keycode into a character (`keyCodeToCharacter`,
//! `Vietnamese.cpp:508-576`).
//!
//! All five code tables are ported (`_codeTable[0..5]`, `Vietnamese.cpp:411-491`).
//! The engine works on abstract letters; encoding happens at this edge, so the
//! same keystroke logic serves every output charset and no adapter holds a
//! table.
//!
//! The tables themselves live in [`crate::tables`] and are diffed cell by cell
//! against the upstream source text by the tests at the bottom of this file —
//! PORTING-MAP §4.7 item 6, "no table is ported by eye".

use crate::event::Encoded;
use crate::options::Charset;
use crate::tables::{
    self, CAPS_MASK, CHAR_CODE_MASK, CHAR_MASK, KEY_A, KEY_E, KEY_O, KEY_U, MARK_MASK, MARK1_MASK,
    MARK2_MASK, MARK3_MASK, MARK4_MASK, MARK5_MASK, PURE_CHARACTER_MASK, TONE_MASK, TONEW_MASK,
};

/// `getCharacterCode` (`Engine.cpp:501-556`): encode one `charData`/`TypingWord`
/// entry for `charset`.
///
/// Returns `bits` unchanged when the charset's table has no row (or no column)
/// for the key — upstream's `if (_codeTable[...].find(key) == end()) return data;`
/// — which is what leaves a plain keycode for [`render`] to resolve through
/// `keyCodeToCharacter`.
pub(crate) fn encode(bits: u32, charset: Charset) -> u32 {
    let caps_elem: usize = if bits & CAPS_MASK != 0 { 0 } else { 1 };
    let mut key = bits & CHAR_MASK;

    if bits & MARK_MASK != 0 {
        let mut mark_elem: i32 = match bits & MARK_MASK {
            MARK1_MASK => 0,
            MARK2_MASK => 2,
            MARK3_MASK => 4,
            MARK4_MASK => 6,
            MARK5_MASK => 8,
            // `MARK_MASK` is exactly the five mark bits; a combination cannot be
            // produced by the word model (`Mark::from_bits` refuses it).
            _ => return bits,
        };
        mark_elem += caps_elem as i32;

        // A marked vowel without `^`/`w` uses the plain-mark columns.
        if matches!(key, k if k == u32::from(KEY_A) || k == u32::from(KEY_O) || k == u32::from(KEY_U) || k == u32::from(KEY_E))
            && bits & TONE_MASK == 0
            && bits & TONEW_MASK == 0
        {
            mark_elem += 4;
        }

        if bits & TONE_MASK != 0 {
            key |= TONE_MASK;
        } else if bits & TONEW_MASK != 0 {
            key |= TONEW_MASK;
        }

        match tables::code_value(charset, key, mark_elem as usize) {
            Some(code) => u32::from(code) | CHAR_CODE_MASK,
            None => bits,
        }
    } else {
        if tables::code_value(charset, key, 0).is_none() {
            return bits;
        }

        if bits & TONE_MASK != 0 {
            tables::code_value(charset, key, caps_elem)
                .map(|code| u32::from(code) | CHAR_CODE_MASK)
                .unwrap_or(bits)
        } else if bits & TONEW_MASK != 0 {
            tables::code_value(charset, key, caps_elem + 2)
                .map(|code| u32::from(code) | CHAR_CODE_MASK)
                .unwrap_or(bits)
        } else {
            // No tone bits: upstream returns the entry unchanged, so a plain
            // letter stays a keycode until the hook resolves it.
            bits
        }
    }
}

/// Render `charData[0..entries.len()]` the way `SendNewCharString` sends it
/// (`OpenKey.mm:423-480`): last entry first, each one expanded to code units in
/// `charset`.
///
/// The variant follows the charset, not the content: Unicode and Unicode
/// Compound produce UTF-16 units (a Compound vowel is a base unit plus a
/// combining mark); TCVN3, VNI-Windows and CP1258 produce bytes. The one
/// exception is a `PURE_CHARACTER` entry above `0xFF` — macro data, which this
/// port has no table for — and it keeps the edit in `Utf16` rather than
/// truncating to a byte that would be a different character.
///
/// Panics when a bare keycode has no character: upstream would send a NUL
/// (`OpenKey.mm:449`).
pub(crate) fn render(entries: &[u32], charset: Charset) -> Encoded {
    let mut units: Vec<u16> = Vec::with_capacity(entries.len());
    for (index, &bits) in entries.iter().enumerate().rev() {
        push_units(&mut units, bits, charset, index);
    }
    match charset {
        Charset::Unicode | Charset::UnicodeCompound => Encoded::Utf16(units),
        Charset::Tcvn3 | Charset::VniWindows | Charset::Cp1258 => {
            if units.iter().all(|&unit| unit <= u16::from(u8::MAX)) {
                let bytes = units
                    .iter()
                    .map(|&unit| u8::try_from(unit).expect("every unit is <= u8::MAX"))
                    .collect();
                Encoded::Bytes(bytes)
            } else {
                Encoded::Utf16(units)
            }
        }
    }
}

/// One `charData` entry, appended to `units` as the units `charset` sends it in.
fn push_units(units: &mut Vec<u16>, bits: u32, charset: Charset, index: usize) {
    if bits & PURE_CHARACTER_MASK != 0 {
        // `_newCharString` is `UniChar`, so only the low 16 bits travel
        // (`OpenKey.mm:440-443`).
        units.push((bits & CHAR_MASK) as u16);
        return;
    }

    if bits & CHAR_CODE_MASK == 0 {
        let character = character(bits).unwrap_or_else(|| {
            panic!(
                "charData[{index}] = 0x{bits:x} is a keycode with no character; \
                 the hook would emit a NUL (OpenKey.mm:449)"
            )
        });
        // `keyCodeToCharacter` returns a `Uint16` and only maps ASCII.
        units.push(character as u16);
        return;
    }

    // Upstream `_newChar` is a `Uint16`, so the entry is truncated before the
    // branches below read its high byte or its mark bits (`OpenKey.mm:451-476`).
    let value = (bits & CHAR_MASK) as u16;
    match charset {
        Charset::Unicode => units.push(value),
        Charset::Tcvn3 | Charset::VniWindows | Charset::Cp1258 => {
            units.push(value & 0x00FF);
            let high = value >> 8;
            if high > 32 {
                units.push(high);
            }
        }
        Charset::UnicodeCompound => {
            // Bits 13-15 hold the mark index; the rest is the base character.
            let mark = value >> 13;
            units.push(value & 0x1FFF);
            if mark > 0 {
                units.push(tables::UNICODE_COMPOUND_MARKS[usize::from(mark) - 1]);
            }
        }
    }
}

/// The character a `charData` entry stands for, mirroring the hook's
/// `SendNewCharString` (`OpenKey.mm:423-480`, `:447-450`).
///
/// `None` when a bare keycode has no character (upstream would send a NUL);
/// [`render`] turns that into a panic naming the entry.
pub(crate) fn character(bits: u32) -> Option<char> {
    if bits & PURE_CHARACTER_MASK != 0 {
        char::from_u32(bits & CHAR_MASK)
    } else if bits & CHAR_CODE_MASK == 0 {
        tables::key_character(bits)
    } else {
        char::from_u32(bits & CHAR_MASK)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tables::{KEY_D, KEY_I, KEY_Y};

    /// The five charsets in `_codeTable[]` order (`Vietnamese.cpp:405-410`).
    const CHARSETS: [Charset; 5] = [
        Charset::Unicode,
        Charset::Tcvn3,
        Charset::VniWindows,
        Charset::UnicodeCompound,
        Charset::Cp1258,
    ];

    /// The verbatim `_codeTable[]` initialiser, from the pinned oracle.
    const UPSTREAM_TABLE: &str = include_str!("../tests/data/code-tables.txt");

    fn key_bits(name: &str) -> u32 {
        let name: String = name.chars().filter(|c| !c.is_whitespace()).collect();
        let mut bits = 0;
        for part in name.split('|') {
            bits |= match part {
                "KEY_A" => u32::from(KEY_A),
                "KEY_O" => u32::from(KEY_O),
                "KEY_U" => u32::from(KEY_U),
                "KEY_E" => u32::from(KEY_E),
                "KEY_I" => u32::from(KEY_I),
                "KEY_Y" => u32::from(KEY_Y),
                "KEY_D" => u32::from(KEY_D),
                "TONE_MASK" => TONE_MASK,
                "TONEW_MASK" => TONEW_MASK,
                other => panic!("the fixture names a key the port does not know: {other}"),
            };
        }
        bits
    }

    /// Parse the upstream initialiser into five row lists of `(key, values)`.
    ///
    /// Deliberately literal: it reads the C text, not a normalised form, so a
    /// fixture edited to match a wrong port still fails (the fixture is the
    /// oracle's bytes; changing it changes what is being compared).
    fn upstream_rows() -> Vec<Vec<(String, Vec<u16>)>> {
        let mut tables: Vec<Vec<(String, Vec<u16>)>> = Vec::new();
        for line in UPSTREAM_TABLE.lines() {
            let line = line.trim();
            if line.is_empty()
                || line.starts_with('#')
                || line.starts_with("//")
                || line.starts_with("map<")
                || line == "};"
                || line == "},"
                || line == "}"
            {
                continue;
            }
            if line == "{" || line.starts_with("{   //") {
                tables.push(Vec::new());
                continue;
            }
            let row = line.strip_prefix('{').expect("a row starts with `{`");
            let (key, values) = row.split_once('{').expect("a row is `{KEY, {values}}`");
            let key = key.trim().trim_end_matches(',').to_string();
            let values = values.trim_end_matches([',', '}']).trim();
            let mut cells = Vec::new();
            for value in values.split(',') {
                let value = value.trim().trim_start_matches("0x");
                cells.push(u16::from_str_radix(value, 16).expect("a u16 hex literal"));
            }
            tables
                .last_mut()
                .expect("a row follows its table opener")
                .push((key, cells));
        }
        tables
    }

    /// The letter bits one fixture cell stands for.
    ///
    /// The column layout is the one `getCharacterCode` indexes: for a plain
    /// vowel row the first four columns are the `^`/`w` caps/normal pairs and
    /// the rest are the five marks, each caps-then-normal; `I` and `Y` have no
    /// pair columns; `D` has only caps and normal; a `TONE`/`TONEW` row carries
    /// its modifier bit and starts straight at the marks.
    fn bits_for(row: &str, elem: usize) -> u32 {
        let (letter, modifier) = match row.split_once('|') {
            Some((letter, modifier)) => (letter, Some(modifier)),
            None => (row, None),
        };
        let mut bits = key_bits(letter);
        if letter == "KEY_D" {
            // The `D` row is only reached with the `TONE` bit: upstream's
            // non-mark path returns `data` unchanged unless `TONE`/`TONEW` is
            // set, and the engine spells `đ` as `KEY_D | TONE_MASK`.
            bits |= TONE_MASK;
            if elem == 0 {
                bits |= CAPS_MASK;
            }
            return bits;
        }

        let pair_columns = match modifier {
            Some(_) => 0,
            None if letter == "KEY_I" || letter == "KEY_Y" => 0,
            None => 4,
        };
        let mut mark_elem = elem;
        match modifier {
            Some("TONE_MASK") => bits |= TONE_MASK,
            Some("TONEW_MASK") => bits |= TONEW_MASK,
            Some(other) => panic!("unexpected row modifier {other}"),
            None if elem < pair_columns => {
                bits |= if elem < 2 { TONE_MASK } else { TONEW_MASK };
                if elem % 2 == 0 {
                    bits |= CAPS_MASK;
                }
                return bits;
            }
            None => mark_elem = elem - pair_columns,
        }
        bits |= MARK1_MASK << (mark_elem / 2);
        if mark_elem % 2 == 0 {
            bits |= CAPS_MASK;
        }
        bits
    }

    /// Every `charData` entry `getCharacterCode` can be handed: the six tabled
    /// vowels with the caps, `^`/`w` and mark bits independent, plus `D`.
    fn all_letter_bits() -> Vec<u32> {
        let marks = [MARK1_MASK, MARK2_MASK, MARK3_MASK, MARK4_MASK, MARK5_MASK];
        let mut all = Vec::new();
        for key in [KEY_A, KEY_O, KEY_U, KEY_E, KEY_I, KEY_Y] {
            for caps in [0, CAPS_MASK] {
                for modifier in [0, TONE_MASK, TONEW_MASK] {
                    all.push(u32::from(key) | caps | modifier);
                    for mark in marks {
                        all.push(u32::from(key) | caps | modifier | mark);
                    }
                }
            }
        }
        for modifier in [0, TONE_MASK, TONEW_MASK] {
            all.push(u32::from(KEY_D) | modifier);
            all.push(u32::from(KEY_D) | CAPS_MASK | modifier);
        }
        all
    }

    fn marked(key: u16, mark: u32, tone: bool, tonew: bool, caps: bool) -> u32 {
        u32::from(key)
            | mark
            | if tone { TONE_MASK } else { 0 }
            | if tonew { TONEW_MASK } else { 0 }
            | if caps { CAPS_MASK } else { 0 }
    }

    /// Render a word given in reading order. `charData` is stored the other way
    /// round — index 0 is the last character typed (`OpenKey.mm:430-432`) — so
    /// this reverses first and the tests below read like the word.
    fn rendered(word: &[u32], charset: Charset) -> Encoded {
        let mut entries = word.to_vec();
        entries.reverse();
        render(&entries, charset)
    }

    #[test]
    fn plain_letters_stay_keycodes_for_the_hook_to_resolve() {
        for charset in CHARSETS {
            assert_eq!(
                encode(u32::from(KEY_A), charset),
                u32::from(KEY_A),
                "{charset:?}"
            );
        }
        assert_eq!(
            character(encode(u32::from(KEY_A), Charset::Unicode)),
            Some('a')
        );
        assert_eq!(
            character(encode(u32::from(KEY_A) | CAPS_MASK, Charset::Unicode)),
            Some('A')
        );
    }

    #[test]
    fn marks_and_tone_modifiers_pick_the_precomposed_row() {
        // á, ấ, ă, ắ — the three modifier columns of the A row.
        assert_eq!(
            character(encode(
                marked(KEY_A, MARK1_MASK, false, false, false),
                Charset::Unicode
            )),
            Some('á')
        );
        assert_eq!(
            character(encode(
                marked(KEY_A, MARK1_MASK, true, false, false),
                Charset::Unicode
            )),
            Some('ấ')
        );
        assert_eq!(
            character(encode(
                marked(KEY_A, MARK1_MASK, false, true, true),
                Charset::Unicode
            )),
            Some('Ắ')
        );
        // Huyền/nặng on I and Y are in the table too.
        assert_eq!(
            character(encode(
                marked(KEY_I, MARK2_MASK, false, false, false),
                Charset::Unicode
            )),
            Some('ì')
        );
        assert_eq!(
            character(encode(
                marked(KEY_Y, MARK5_MASK, false, false, false),
                Charset::Unicode
            )),
            Some('ỵ')
        );
    }

    #[test]
    fn unmodified_tone_and_horn_are_encoded_without_a_mark() {
        assert_eq!(
            character(encode(u32::from(KEY_A) | TONE_MASK, Charset::Unicode)),
            Some('â')
        );
        assert_eq!(
            character(encode(u32::from(KEY_O) | TONEW_MASK, Charset::Unicode)),
            Some('ơ')
        );
        assert_eq!(
            character(encode(u32::from(KEY_U) | TONEW_MASK, Charset::Unicode)),
            Some('ư')
        );
        assert_eq!(
            character(encode(u32::from(KEY_D) | TONE_MASK, Charset::Unicode)),
            Some('đ')
        );
    }

    #[test]
    fn a_key_outside_the_table_returns_the_entry_unchanged() {
        // `s` has no row in any `_codeTable` entry; upstream returns `data`.
        for charset in CHARSETS {
            assert_eq!(
                encode(u32::from(tables::KEY_S), charset),
                u32::from(tables::KEY_S),
                "{charset:?}"
            );
        }
        assert_eq!(
            character(encode(u32::from(tables::KEY_S), Charset::Tcvn3)),
            Some('s')
        );
    }

    #[test]
    fn the_five_tables_match_the_upstream_source() {
        let tables = upstream_rows();
        assert_eq!(
            tables.len(),
            5,
            "`_codeTable[]` has one row list per charset"
        );
        for (rows, charset) in tables.iter().zip(CHARSETS) {
            assert_eq!(rows.len(), 13, "{charset:?}: thirteen rows");
            for (name, cells) in rows {
                let key = key_bits(name);
                for (elem, &expected) in cells.iter().enumerate() {
                    assert_eq!(
                        tables::code_value(charset, key, elem),
                        Some(expected),
                        "{charset:?} {name} column {elem}"
                    );
                }
                assert_eq!(
                    tables::code_value(charset, key, cells.len()),
                    None,
                    "{charset:?} {name}: the row ends where upstream ends it"
                );
            }
        }
    }

    #[test]
    fn every_table_cell_round_trips_through_encode() {
        let tables = upstream_rows();
        let domain = all_letter_bits();
        for (rows, charset) in tables.iter().zip(CHARSETS) {
            for (name, cells) in rows {
                for (elem, &value) in cells.iter().enumerate() {
                    let bits = bits_for(name, elem);
                    let encoded = encode(bits, charset);
                    assert_eq!(
                        encoded,
                        u32::from(value) | CHAR_CODE_MASK,
                        "{charset:?} {name} column {elem}"
                    );
                    // The inverse is a set: TCVN3 merges marks (A sắc and A
                    // huyền are both 0xB8) and CP1258 reuses `DD`/`FD` for ư, so
                    // a value may belong to more than one letter. The cell must
                    // be one of them.
                    let decoded: Vec<u32> = domain
                        .iter()
                        .copied()
                        .filter(|&candidate| {
                            encode(candidate, charset) & CHAR_MASK == u32::from(value)
                        })
                        .collect();
                    assert!(
                        decoded.contains(&bits),
                        "{charset:?} {name} column {elem} = 0x{value:04X} decodes to {decoded:08X?}"
                    );
                }
            }
        }
    }

    #[test]
    fn the_published_vni_example_renders_as_its_documented_bytes() {
        // `vie65t` -> `việt` is the published example in `tests/corpus/README.md`
        // rule 1 and `vni-words.tsv`. In the VNI-Windows table `ệ` is the
        // `KEY_E|TONE_MASK` row's nặng column (`Vietnamese.cpp:461`), 0xE465:
        // the letter `e` followed by the 0xE4 glyph cell, which is how VNI
        // carries a stacked circumflex-plus-nặng rather than a precomposed
        // code point. The render is therefore `vie` 0xE4 `t`.
        let word = [
            u32::from(tables::KEY_V),
            u32::from(KEY_I),
            encode(
                marked(KEY_E, MARK5_MASK, true, false, false),
                Charset::VniWindows,
            ),
            u32::from(tables::KEY_T),
        ];
        assert_eq!(
            rendered(&word, Charset::VniWindows),
            Encoded::Bytes(vec![0x76, 0x69, 0x65, 0xE4, 0x74])
        );
    }

    #[test]
    fn each_charset_renders_the_same_word_its_own_way() {
        // `việt`: ệ is `KEY_E` with `^` and nặng. One word, five encodings —
        // which is the whole point of D4 (a `String` could carry only the first).
        let word = |charset: Charset| {
            let word = [
                u32::from(tables::KEY_V),
                u32::from(KEY_I),
                encode(marked(KEY_E, MARK5_MASK, true, false, false), charset),
                u32::from(tables::KEY_T),
            ];
            rendered(&word, charset)
        };
        assert_eq!(
            word(Charset::Unicode),
            Encoded::Utf16(vec![0x0076, 0x0069, 0x1EC7, 0x0074])
        );
        assert_eq!(
            word(Charset::Tcvn3),
            Encoded::Bytes(vec![0x76, 0x69, 0xD6, 0x74])
        );
        assert_eq!(
            word(Charset::VniWindows),
            Encoded::Bytes(vec![0x76, 0x69, 0x65, 0xE4, 0x74])
        );
        assert_eq!(
            word(Charset::UnicodeCompound),
            Encoded::Utf16(vec![0x0076, 0x0069, 0x00EA, 0x0323, 0x0074])
        );
        assert_eq!(
            word(Charset::Cp1258),
            Encoded::Bytes(vec![0x76, 0x69, 0xEA, 0xF2, 0x74])
        );
    }

    #[test]
    fn a_byte_charset_sends_the_high_byte_only_when_it_is_an_extra_unit() {
        // `d9uo72ng` -> `đường` is the second published VNI form. Upper-case
        // and lower-case `đ` are the two cells of the `D` row
        // (`Vietnamese.cpp:457`), 0xD1/0xF1, so the lower-case stroke-d is one
        // byte; `ư` is 0xF6; `ờ` is 0xF8F4 — low byte then high, because the
        // high byte is the tone glyph rather than padding.
        let word = [
            encode(u32::from(KEY_D) | TONE_MASK, Charset::VniWindows),
            encode(u32::from(KEY_U) | TONEW_MASK, Charset::VniWindows),
            encode(
                marked(KEY_O, MARK2_MASK, false, true, false),
                Charset::VniWindows,
            ),
            u32::from(tables::KEY_N),
            u32::from(tables::KEY_G),
        ];
        assert_eq!(
            rendered(&word, Charset::VniWindows),
            Encoded::Bytes(vec![0xF1, 0xF6, 0xF4, 0xF8, 0x6E, 0x67])
        );
    }

    #[test]
    fn a_bare_keycode_is_rendered_as_its_character_in_every_charset() {
        for charset in CHARSETS {
            assert_eq!(
                rendered(&[u32::from(KEY_A) | CAPS_MASK], charset),
                match charset {
                    Charset::Unicode | Charset::UnicodeCompound => Encoded::Utf16(vec![0x41]),
                    _ => Encoded::Bytes(vec![0x41]),
                },
                "{charset:?}"
            );
        }
    }

    #[test]
    fn an_empty_edit_is_empty_in_both_variants() {
        assert!(Encoded::default().is_empty());
        assert!(Encoded::Bytes(Vec::new()).is_empty());
        assert!(!Encoded::Utf16(vec![0x41]).is_empty());
        assert!(!Encoded::Bytes(vec![0x41]).is_empty());
    }
}
