//! Charset encoding. Port of upstream `getCharacterCode()` (`Engine.cpp:501-556`)
//! and the character lookup the hook uses to turn a `charData` entry into a
//! character (`keyCodeToCharacter`, `Vietnamese.cpp:508-576`).
//!
//! Only the Unicode code table (`_codeTable[0]`, `Vietnamese.cpp:411-430`) is
//! ported here; the remaining four tables (Unicode Compound, TCVN3/ABC,
//! VNI-Windows, Vietnamese locale CP1258) are task C-005. Until then a
//! non-Unicode [`Charset`] panics with `not yet implemented`, which the parity
//! replay reports as `engine-pending` rather than as a disagreement.
//!
//! The engine works on abstract letters; encoding happens at this edge, so the
//! same keystroke logic serves every output charset.

use crate::options::Charset;
use crate::tables::{
    self, CAPS_MASK, CHAR_CODE_MASK, CHAR_MASK, KEY_A, KEY_E, KEY_O, KEY_U, MARK_MASK, MARK1_MASK,
    MARK2_MASK, MARK3_MASK, MARK4_MASK, MARK5_MASK, PURE_CHARACTER_MASK, TONE_MASK, TONEW_MASK,
};

/// Upstream `GET(...)`: encode one `charData`/`TypingWord` entry for `charset`.
pub(crate) fn encode(bits: u32, charset: Charset) -> u32 {
    match charset {
        Charset::Unicode => encode_unicode(bits),
        _ => todo!("charset tables — task C-005"),
    }
}

/// `getCharacterCode` for `_codeTable[0]` (Unicode, precomposed).
///
/// Returns `bits` unchanged when the table has no row for the key — upstream's
/// `if (_codeTable[...].find(key) == end()) return data;` — which is what leaves
/// a plain keycode for [`character`] to resolve through `keyCodeToCharacter`.
pub(crate) fn encode_unicode(data: u32) -> u32 {
    let caps_elem: usize = if data & CAPS_MASK != 0 { 0 } else { 1 };
    let mut key = data & CHAR_MASK;

    if data & MARK_MASK != 0 {
        let mut mark_elem: i32 = match data & MARK_MASK {
            MARK1_MASK => 0,
            MARK2_MASK => 2,
            MARK3_MASK => 4,
            MARK4_MASK => 6,
            MARK5_MASK => 8,
            // `MARK_MASK` is exactly the five mark bits; a combination cannot be
            // produced by the word model (`Mark::from_bits` refuses it).
            _ => return data,
        };
        mark_elem += caps_elem as i32;

        // A marked vowel without `^`/`w` uses the plain-mark columns.
        if matches!(key, k if k == u32::from(KEY_A) || k == u32::from(KEY_O) || k == u32::from(KEY_U) || k == u32::from(KEY_E))
            && data & TONE_MASK == 0
            && data & TONEW_MASK == 0
        {
            mark_elem += 4;
        }

        if data & TONE_MASK != 0 {
            key |= TONE_MASK;
        } else if data & TONEW_MASK != 0 {
            key |= TONEW_MASK;
        }

        match tables::unicode_code(key, mark_elem as usize) {
            Some(code) => u32::from(code) | CHAR_CODE_MASK,
            None => data,
        }
    } else {
        if tables::unicode_code(key, 0).is_none() {
            return data;
        }
        if data & TONE_MASK != 0 {
            tables::unicode_code(key, caps_elem)
                .map(|code| u32::from(code) | CHAR_CODE_MASK)
                .unwrap_or(data)
        } else if data & TONEW_MASK != 0 {
            tables::unicode_code(key, caps_elem + 2)
                .map(|code| u32::from(code) | CHAR_CODE_MASK)
                .unwrap_or(data)
        } else {
            // No tone bits: upstream returns the entry unchanged, so a plain
            // letter stays a keycode until the hook resolves it.
            data
        }
    }
}

/// The character a `charData` entry stands for, mirroring the hook's
/// `SendNewCharString` (`OpenKey.mm:423-480`) and the reference harness's
/// `emitUnits`.
///
/// `None` when a bare keycode has no character (upstream would send a NUL,
/// `OpenKey.mm:449`); the Unicode table and `key_character` never produce that.
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
    use crate::tables::{KEY_D, KEY_I, KEY_O, KEY_U, KEY_Y};

    fn marked(key: u16, mark: u32, tone: bool, tonew: bool, caps: bool) -> u32 {
        u32::from(key)
            | mark
            | if tone { TONE_MASK } else { 0 }
            | if tonew { TONEW_MASK } else { 0 }
            | if caps { CAPS_MASK } else { 0 }
    }

    #[test]
    fn plain_letters_stay_keycodes_for_the_hook_to_resolve() {
        assert_eq!(encode_unicode(u32::from(KEY_A)), u32::from(KEY_A));
        assert_eq!(character(encode_unicode(u32::from(KEY_A))), Some('a'));
        assert_eq!(
            character(encode_unicode(u32::from(KEY_A) | CAPS_MASK)),
            Some('A')
        );
    }

    #[test]
    fn marks_and_tone_modifiers_pick_the_precomposed_row() {
        // á, ấ, ă, ắ — the three modifier columns of the A row.
        assert_eq!(
            character(encode_unicode(marked(
                KEY_A, MARK1_MASK, false, false, false
            ))),
            Some('á')
        );
        assert_eq!(
            character(encode_unicode(marked(
                KEY_A, MARK1_MASK, true, false, false
            ))),
            Some('ấ')
        );
        assert_eq!(
            character(encode_unicode(marked(KEY_A, MARK1_MASK, false, true, true))),
            Some('Ắ')
        );
        // Huyền/nặng on I and Y are in the table too.
        assert_eq!(
            character(encode_unicode(marked(
                KEY_I, MARK2_MASK, false, false, false
            ))),
            Some('ì')
        );
        assert_eq!(
            character(encode_unicode(marked(
                KEY_Y, MARK5_MASK, false, false, false
            ))),
            Some('ỵ')
        );
    }

    #[test]
    fn unmodified_tone_and_horn_are_encoded_without_a_mark() {
        assert_eq!(
            character(encode_unicode(u32::from(KEY_A) | TONE_MASK)),
            Some('â')
        );
        assert_eq!(
            character(encode_unicode(u32::from(KEY_O) | TONEW_MASK)),
            Some('ơ')
        );
        assert_eq!(
            character(encode_unicode(u32::from(KEY_U) | TONEW_MASK)),
            Some('ư')
        );
        assert_eq!(
            character(encode_unicode(u32::from(KEY_D) | TONE_MASK)),
            Some('đ')
        );
    }

    #[test]
    fn a_key_outside_the_table_returns_the_entry_unchanged() {
        // `s` has no row in `_codeTable`; upstream returns `data`.
        assert_eq!(
            encode_unicode(u32::from(tables::KEY_S)),
            u32::from(tables::KEY_S)
        );
        assert_eq!(
            character(encode_unicode(u32::from(tables::KEY_S))),
            Some('s')
        );
    }

    #[test]
    fn non_unicode_charsets_are_not_ported_yet() {
        for charset in [
            Charset::Tcvn3,
            Charset::VniWindows,
            Charset::UnicodeCompound,
            Charset::Cp1258,
        ] {
            let err = std::panic::catch_unwind(|| encode(u32::from(KEY_A), charset));
            assert!(err.is_err(), "{charset:?} must stay pending until C-005");
        }
    }
}
