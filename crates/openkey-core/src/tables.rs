//! The upstream lookup tables, transcribed from `Vietnamese.cpp` at the pin
//! (89c2fd3b, macOS normative — DL-026), plus the macOS keycodes the engine
//! compares against.
//!
//! Every table here is data, not policy: the ported functions in `engine.rs`
//! carry the line citations. `tests` at the bottom ties the keycode constants
//! back to [`crate::word`] so the two cannot drift apart silently.
//!
//! Keycodes are macOS virtual keycodes because upstream's *reference* build is
//! the macOS hook. The engine compares keycodes, so a Windows adapter must
//! present this same domain; that mapping is the adapter's job (ADR-0002 D3).

use crate::event::Key;
use crate::options::{Charset, InputMethod};

// `DataType.h:92-126`.
pub(crate) const CAPS_MASK: u32 = 0x0001_0000;
pub(crate) const TONE_MASK: u32 = 0x0002_0000;
pub(crate) const TONEW_MASK: u32 = 0x0004_0000;
pub(crate) const MARK1_MASK: u32 = 0x0008_0000;
pub(crate) const MARK2_MASK: u32 = 0x0010_0000;
pub(crate) const MARK3_MASK: u32 = 0x0020_0000;
pub(crate) const MARK4_MASK: u32 = 0x0040_0000;
pub(crate) const MARK5_MASK: u32 = 0x0080_0000;
pub(crate) const MARK_MASK: u32 = 0x00F8_0000;
pub(crate) const CHAR_MASK: u32 = 0x0000_FFFF;
pub(crate) const STANDALONE_MASK: u32 = 0x0100_0000;
pub(crate) const CHAR_CODE_MASK: u32 = 0x0200_0000;
pub(crate) const PURE_CHARACTER_MASK: u32 = 0x8000_0000;
pub(crate) const END_CONSONANT_MASK: u16 = 0x4000;
pub(crate) const CONSONANT_ALLOW_MASK: u16 = 0x8000;

// `platforms/mac.h`.
pub(crate) const KEY_ESC: u16 = 53;
pub(crate) const KEY_DELETE: u16 = 51;
pub(crate) const KEY_TAB: u16 = 48;
pub(crate) const KEY_ENTER: u16 = 76;
pub(crate) const KEY_RETURN: u16 = 36;
pub(crate) const KEY_SPACE: u16 = 49;
pub(crate) const KEY_LEFT: u16 = 123;
pub(crate) const KEY_RIGHT: u16 = 124;
pub(crate) const KEY_DOWN: u16 = 125;
pub(crate) const KEY_UP: u16 = 126;

pub(crate) const KEY_A: u16 = 0;
pub(crate) const KEY_B: u16 = 11;
pub(crate) const KEY_C: u16 = 8;
pub(crate) const KEY_D: u16 = 2;
pub(crate) const KEY_E: u16 = 14;
pub(crate) const KEY_F: u16 = 3;
pub(crate) const KEY_G: u16 = 5;
pub(crate) const KEY_H: u16 = 4;
pub(crate) const KEY_I: u16 = 34;
pub(crate) const KEY_J: u16 = 38;
pub(crate) const KEY_K: u16 = 40;
pub(crate) const KEY_L: u16 = 37;
pub(crate) const KEY_M: u16 = 46;
pub(crate) const KEY_N: u16 = 45;
pub(crate) const KEY_O: u16 = 31;
pub(crate) const KEY_P: u16 = 35;
pub(crate) const KEY_Q: u16 = 12;
pub(crate) const KEY_R: u16 = 15;
pub(crate) const KEY_S: u16 = 1;
pub(crate) const KEY_T: u16 = 17;
pub(crate) const KEY_U: u16 = 32;
pub(crate) const KEY_V: u16 = 9;
pub(crate) const KEY_W: u16 = 13;
pub(crate) const KEY_X: u16 = 7;
pub(crate) const KEY_Y: u16 = 16;
pub(crate) const KEY_Z: u16 = 6;

pub(crate) const KEY_1: u16 = 18;
pub(crate) const KEY_2: u16 = 19;
pub(crate) const KEY_3: u16 = 20;
pub(crate) const KEY_4: u16 = 21;
pub(crate) const KEY_5: u16 = 23;
pub(crate) const KEY_6: u16 = 22;
pub(crate) const KEY_7: u16 = 26;
pub(crate) const KEY_8: u16 = 28;
pub(crate) const KEY_9: u16 = 25;
pub(crate) const KEY_0: u16 = 29;

pub(crate) const KEY_LEFT_BRACKET: u16 = 33;
pub(crate) const KEY_RIGHT_BRACKET: u16 = 30;
pub(crate) const KEY_DOT: u16 = 47;
pub(crate) const KEY_BACKQUOTE: u16 = 50;
pub(crate) const KEY_MINUS: u16 = 27;
pub(crate) const KEY_EQUALS: u16 = 24;
pub(crate) const KEY_BACK_SLASH: u16 = 42;
pub(crate) const KEY_SEMICOLON: u16 = 41;
pub(crate) const KEY_QUOTE: u16 = 39;
pub(crate) const KEY_COMMA: u16 = 43;
pub(crate) const KEY_SLASH: u16 = 44;

/// An adapter key as the engine's keycode domain.
///
/// `None` for keys upstream's macOS tables do not name: a forward delete
/// (macOS 117) is not in `_breakCode` and has no constant at the pin, so it is
/// left to the adapter rather than guessed.
pub(crate) fn platform_code(key: Key) -> Option<u16> {
    Some(match key {
        Key::A => KEY_A,
        Key::B => KEY_B,
        Key::C => KEY_C,
        Key::D => KEY_D,
        Key::E => KEY_E,
        Key::F => KEY_F,
        Key::G => KEY_G,
        Key::H => KEY_H,
        Key::I => KEY_I,
        Key::J => KEY_J,
        Key::K => KEY_K,
        Key::L => KEY_L,
        Key::M => KEY_M,
        Key::N => KEY_N,
        Key::O => KEY_O,
        Key::P => KEY_P,
        Key::Q => KEY_Q,
        Key::R => KEY_R,
        Key::S => KEY_S,
        Key::T => KEY_T,
        Key::U => KEY_U,
        Key::V => KEY_V,
        Key::W => KEY_W,
        Key::X => KEY_X,
        Key::Y => KEY_Y,
        Key::Z => KEY_Z,
        Key::Digit(0) => KEY_0,
        Key::Digit(1) => KEY_1,
        Key::Digit(2) => KEY_2,
        Key::Digit(3) => KEY_3,
        Key::Digit(4) => KEY_4,
        Key::Digit(5) => KEY_5,
        Key::Digit(6) => KEY_6,
        Key::Digit(7) => KEY_7,
        Key::Digit(8) => KEY_8,
        Key::Digit(9) => KEY_9,
        Key::Space => KEY_SPACE,
        Key::Enter => KEY_RETURN,
        Key::Tab => KEY_TAB,
        Key::Escape => KEY_ESC,
        Key::Backspace => KEY_DELETE,
        Key::Delete => return None,
        Key::ArrowLeft => KEY_LEFT,
        Key::ArrowRight => KEY_RIGHT,
        Key::ArrowUp => KEY_UP,
        Key::ArrowDown => KEY_DOWN,
        Key::LeftBracket => KEY_LEFT_BRACKET,
        Key::RightBracket => KEY_RIGHT_BRACKET,
        Key::Semicolon => KEY_SEMICOLON,
        Key::Quote => KEY_QUOTE,
        Key::Comma => KEY_COMMA,
        Key::Dot => KEY_DOT,
        Key::Slash => KEY_SLASH,
        Key::BackSlash => KEY_BACK_SLASH,
        Key::Minus => KEY_MINUS,
        Key::Equals => KEY_EQUALS,
        Key::Backquote => KEY_BACKQUOTE,
        Key::Other(code) => code,
        Key::Digit(_) => return None,
    })
}

/// `IS_CONSONANT` (`DataType.h:130`): everything that is not a vowel letter.
pub(crate) fn is_consonant(code: u16) -> bool {
    !matches!(code, KEY_A | KEY_E | KEY_U | KEY_Y | KEY_I | KEY_O)
}

/// `IS_NUMBER_KEY` (`DataType.h:154`).
pub(crate) fn is_number_key(code: u16) -> bool {
    matches!(
        code,
        KEY_1 | KEY_2 | KEY_3 | KEY_4 | KEY_5 | KEY_6 | KEY_7 | KEY_8 | KEY_9 | KEY_0
    )
}

/// `_charKeyCode` (`Engine.cpp:15-19`).
pub(crate) fn is_char_key_code(code: u16) -> bool {
    matches!(
        code,
        KEY_BACKQUOTE
            | KEY_1
            | KEY_2
            | KEY_3
            | KEY_4
            | KEY_5
            | KEY_6
            | KEY_7
            | KEY_8
            | KEY_9
            | KEY_0
            | KEY_MINUS
            | KEY_EQUALS
            | KEY_LEFT_BRACKET
            | KEY_RIGHT_BRACKET
            | KEY_BACK_SLASH
            | KEY_SEMICOLON
            | KEY_QUOTE
            | KEY_COMMA
            | KEY_DOT
            | KEY_SLASH
    )
}

/// `_breakCode` (`Engine.cpp:21-28`).
pub(crate) fn is_break_code(code: u16) -> bool {
    matches!(
        code,
        KEY_ESC
            | KEY_TAB
            | KEY_ENTER
            | KEY_RETURN
            | KEY_LEFT
            | KEY_RIGHT
            | KEY_DOWN
            | KEY_UP
            | KEY_COMMA
            | KEY_DOT
            | KEY_SLASH
            | KEY_SEMICOLON
            | KEY_QUOTE
            | KEY_BACK_SLASH
            | KEY_MINUS
            | KEY_EQUALS
            | KEY_BACKQUOTE
    )
}

/// `_macroBreakCode` (`Engine.cpp:30-32`).
pub(crate) fn is_macro_break_code(code: u16) -> bool {
    matches!(
        code,
        KEY_RETURN
            | KEY_COMMA
            | KEY_DOT
            | KEY_SLASH
            | KEY_SEMICOLON
            | KEY_QUOTE
            | KEY_BACK_SLASH
            | KEY_MINUS
            | KEY_EQUALS
    )
}

/// `IS_SPECIALKEY` (`DataType.h:133-150`).
///
/// VNI treats the ten digit keys as its transformation keys. Simple Telex 1/2
/// are refused before this function is reached (C-004); their arm here reuses
/// the Telex set, which upstream narrows by dropping the bracket keys
/// (`DataType.h:141-145`).
pub(crate) fn is_special_key(method: InputMethod, code: u16) -> bool {
    match method {
        InputMethod::Vni => is_number_key(code),
        InputMethod::Telex | InputMethod::SimpleTelex1 | InputMethod::SimpleTelex2 => {
            matches!(
                code,
                KEY_W
                    | KEY_E
                    | KEY_R
                    | KEY_O
                    | KEY_LEFT_BRACKET
                    | KEY_RIGHT_BRACKET
                    | KEY_A
                    | KEY_S
                    | KEY_D
                    | KEY_F
                    | KEY_J
                    | KEY_Z
                    | KEY_X
            )
        }
    }
}

/// `IS_KEY_DOUBLE` (`Engine.cpp:45-46`): `6` in VNI, the vowel itself in Telex.
pub(crate) fn is_key_double(method: InputMethod, code: u16) -> bool {
    match method {
        InputMethod::Vni => code == KEY_6,
        InputMethod::Telex | InputMethod::SimpleTelex1 | InputMethod::SimpleTelex2 => {
            matches!(code, KEY_A | KEY_O | KEY_E)
        }
    }
}

/// `IS_KEY_D` (`Engine.cpp:42`): `9` in VNI.
pub(crate) fn is_key_d(method: InputMethod, code: u16) -> bool {
    match method {
        InputMethod::Vni => code == KEY_9,
        InputMethod::Telex | InputMethod::SimpleTelex1 | InputMethod::SimpleTelex2 => code == KEY_D,
    }
}

/// `IS_KEY_Z` (`Engine.cpp:41`): `0` in VNI (remove the mark).
pub(crate) fn is_key_z(method: InputMethod, code: u16) -> bool {
    match method {
        InputMethod::Vni => code == KEY_0,
        InputMethod::Telex | InputMethod::SimpleTelex1 | InputMethod::SimpleTelex2 => code == KEY_Z,
    }
}

/// `IS_KEY_W` (`Engine.cpp:43-44`): `7`/`8` in VNI (horn, breve).
pub(crate) fn is_key_w(method: InputMethod, code: u16) -> bool {
    match method {
        InputMethod::Vni => matches!(code, KEY_7 | KEY_8),
        InputMethod::Telex | InputMethod::SimpleTelex1 | InputMethod::SimpleTelex2 => code == KEY_W,
    }
}

/// The `MARK*_MASK` a mark key applies (`Engine.cpp:1119-1131`), or `None` for
/// a key that is not a mark key in this method (`IS_MARK_KEY`, `:53-54`).
///
/// `ProcessingChar` orders the keys S/F/R/X/J, i.e. `1/2/3/4/5` in VNI.
pub(crate) fn mark_mask(method: InputMethod, code: u16) -> Option<u32> {
    let index = match method {
        InputMethod::Vni => match code {
            KEY_1 => 0,
            KEY_2 => 1,
            KEY_3 => 2,
            KEY_4 => 3,
            KEY_5 => 4,
            _ => return None,
        },
        InputMethod::Telex | InputMethod::SimpleTelex1 | InputMethod::SimpleTelex2 => match code {
            KEY_S => 0,
            KEY_F => 1,
            KEY_R => 2,
            KEY_X => 3,
            KEY_J => 4,
            _ => return None,
        },
    };
    Some([MARK1_MASK, MARK2_MASK, MARK3_MASK, MARK4_MASK, MARK5_MASK][index])
}

/// `IS_KEY_F`, `IS_KEY_R` or `IS_KEY_X` for this method (`Engine.cpp:47-50`):
/// these tones may not sit on a `c`/`t` ending (`Engine.cpp:488-493`).
pub(crate) fn is_restricted_end_mark(method: InputMethod, code: u16) -> bool {
    match method {
        InputMethod::Vni => matches!(code, KEY_2 | KEY_3 | KEY_4),
        InputMethod::Telex | InputMethod::SimpleTelex1 | InputMethod::SimpleTelex2 => {
            matches!(code, KEY_F | KEY_R | KEY_X)
        }
    }
}

/// `IS_QUICK_TELEX_KEY` (`DataType.h:151-152`).
pub(crate) fn is_quick_telex_key(index: usize, code: u16, previous: impl FnOnce() -> u16) -> bool {
    index > 0
        && matches!(code, KEY_C | KEY_G | KEY_K | KEY_N | KEY_Q | KEY_P | KEY_T)
        && previous() == code
}

/// `_vowel` (`Vietnamese.cpp:19-97`).
pub(crate) fn vowel(code: u16) -> &'static [&'static [u16]] {
    match code {
        KEY_A => &[
            &[KEY_A, KEY_N, KEY_G],
            &[KEY_A, KEY_G | END_CONSONANT_MASK],
            &[KEY_A, KEY_N],
            &[KEY_A, KEY_M],
            &[KEY_A, KEY_U],
            &[KEY_A, KEY_Y],
            &[KEY_A, KEY_T],
            &[KEY_A, KEY_P],
            &[KEY_A],
            &[KEY_A, KEY_C],
        ],
        KEY_O => &[
            &[KEY_O, KEY_N, KEY_G],
            &[KEY_O, KEY_G | END_CONSONANT_MASK],
            &[KEY_O, KEY_N],
            &[KEY_O, KEY_M],
            &[KEY_O, KEY_I],
            &[KEY_O, KEY_C],
            &[KEY_O, KEY_T],
            &[KEY_O, KEY_P],
            &[KEY_O],
        ],
        KEY_E => &[
            &[KEY_E, KEY_N, KEY_H],
            &[KEY_E, KEY_H | END_CONSONANT_MASK],
            &[KEY_E, KEY_N, KEY_G],
            &[KEY_E, KEY_G | END_CONSONANT_MASK],
            &[KEY_E, KEY_C, KEY_H],
            &[KEY_E, KEY_K | END_CONSONANT_MASK],
            &[KEY_E, KEY_C],
            &[KEY_E, KEY_T],
            &[KEY_E, KEY_Y],
            &[KEY_E, KEY_U],
            &[KEY_E, KEY_P],
            &[KEY_E, KEY_C],
            &[KEY_E, KEY_N],
            &[KEY_E, KEY_M],
            &[KEY_E],
        ],
        KEY_W => &[
            &[KEY_O, KEY_N],
            &[KEY_U, KEY_O, KEY_N, KEY_G],
            &[KEY_U, KEY_O, KEY_G | END_CONSONANT_MASK],
            &[KEY_U, KEY_O, KEY_N],
            &[KEY_U, KEY_O, KEY_I],
            &[KEY_U, KEY_O, KEY_C],
            &[KEY_O, KEY_I],
            &[KEY_O, KEY_P],
            &[KEY_O, KEY_M],
            &[KEY_O, KEY_A],
            &[KEY_O, KEY_T],
            &[KEY_U, KEY_N, KEY_G],
            &[KEY_U, KEY_G | END_CONSONANT_MASK],
            &[KEY_A, KEY_N, KEY_G],
            &[KEY_A, KEY_G | END_CONSONANT_MASK],
            &[KEY_U, KEY_N],
            &[KEY_U, KEY_M],
            &[KEY_U, KEY_C],
            &[KEY_U, KEY_A],
            &[KEY_U, KEY_I],
            &[KEY_U, KEY_T],
            &[KEY_U],
            &[KEY_A, KEY_P],
            &[KEY_A, KEY_T],
            &[KEY_A, KEY_M],
            &[KEY_A, KEY_N],
            &[KEY_A],
            &[KEY_A, KEY_C],
            &[KEY_A, KEY_C, KEY_H],
            &[KEY_A, KEY_K | END_CONSONANT_MASK],
            &[KEY_O],
            &[KEY_U, KEY_U],
        ],
        _ => &[],
    }
}

/// `_vowelCombine` (`Vietnamese.cpp:99-168`). The first element says whether an
/// end consonant may follow.
///
/// `KEY_* as u32` rather than `u32::from(KEY_*)`: a cast is promotable to
/// `'static`, a `const fn` call is not.
pub(crate) fn vowel_combine(code: u16) -> &'static [&'static [u32]] {
    match code {
        KEY_A => &[
            &[0, KEY_A as u32, KEY_I as u32],
            &[0, KEY_A as u32, KEY_O as u32],
            &[0, KEY_A as u32, KEY_U as u32],
            &[0, KEY_A as u32 | TONE_MASK, KEY_U as u32],
            &[0, KEY_A as u32, KEY_Y as u32],
            &[0, KEY_A as u32 | TONE_MASK, KEY_Y as u32],
        ],
        KEY_E => &[
            &[0, KEY_E as u32, KEY_O as u32],
            &[0, KEY_E as u32 | TONE_MASK, KEY_U as u32],
        ],
        KEY_I => &[
            &[1, KEY_I as u32, KEY_E as u32 | TONE_MASK, KEY_U as u32],
            &[0, KEY_I as u32, KEY_A as u32],
            &[1, KEY_I as u32, KEY_E as u32 | TONE_MASK],
            &[0, KEY_I as u32, KEY_U as u32],
        ],
        KEY_O => &[
            &[0, KEY_O as u32, KEY_A as u32, KEY_I as u32],
            &[0, KEY_O as u32, KEY_A as u32, KEY_O as u32],
            &[0, KEY_O as u32, KEY_A as u32, KEY_Y as u32],
            &[0, KEY_O as u32, KEY_E as u32, KEY_O as u32],
            &[1, KEY_O as u32, KEY_A as u32],
            &[1, KEY_O as u32, KEY_A as u32 | TONEW_MASK],
            &[1, KEY_O as u32, KEY_E as u32],
            &[0, KEY_O as u32, KEY_I as u32],
            &[0, KEY_O as u32 | TONE_MASK, KEY_I as u32],
            &[0, KEY_O as u32 | TONEW_MASK, KEY_I as u32],
            &[1, KEY_O as u32, KEY_O as u32],
            &[1, KEY_O as u32 | TONE_MASK, KEY_O as u32 | TONE_MASK],
        ],
        KEY_U => &[
            &[0, KEY_U as u32, KEY_Y as u32, KEY_U as u32],
            &[1, KEY_U as u32, KEY_Y as u32, KEY_E as u32 | TONE_MASK],
            &[0, KEY_U as u32, KEY_Y as u32, KEY_A as u32],
            &[
                0,
                KEY_U as u32 | TONEW_MASK,
                KEY_O as u32 | TONEW_MASK,
                KEY_U as u32,
            ],
            &[
                0,
                KEY_U as u32 | TONEW_MASK,
                KEY_O as u32 | TONEW_MASK,
                KEY_I as u32,
            ],
            &[0, KEY_U as u32, KEY_O as u32 | TONE_MASK, KEY_I as u32],
            &[0, KEY_U as u32, KEY_A as u32 | TONE_MASK, KEY_Y as u32],
            &[1, KEY_U as u32, KEY_A as u32, KEY_O as u32],
            &[1, KEY_U as u32, KEY_A as u32],
            &[1, KEY_U as u32, KEY_A as u32 | TONEW_MASK],
            &[1, KEY_U as u32, KEY_A as u32 | TONE_MASK],
            &[0, KEY_U as u32 | TONEW_MASK, KEY_A as u32],
            &[1, KEY_U as u32, KEY_E as u32 | TONE_MASK],
            &[0, KEY_U as u32, KEY_I as u32],
            &[0, KEY_U as u32 | TONEW_MASK, KEY_I as u32],
            &[1, KEY_U as u32, KEY_O as u32],
            &[1, KEY_U as u32, KEY_O as u32 | TONE_MASK],
            &[0, KEY_U as u32, KEY_O as u32 | TONEW_MASK],
            &[1, KEY_U as u32 | TONEW_MASK, KEY_O as u32 | TONEW_MASK],
            &[0, KEY_U as u32 | TONEW_MASK, KEY_U as u32],
            &[1, KEY_U as u32, KEY_Y as u32],
        ],
        KEY_Y => &[
            &[0, KEY_Y as u32, KEY_E as u32 | TONE_MASK, KEY_U as u32],
            &[1, KEY_Y as u32, KEY_E as u32 | TONE_MASK],
        ],
        _ => &[],
    }
}

/// `_consonantD` (`Vietnamese.cpp:170-242`).
pub(crate) fn consonant_d() -> &'static [&'static [u16]] {
    &[
        &[KEY_D, KEY_E, KEY_N, KEY_H],
        &[KEY_D, KEY_E, KEY_H | END_CONSONANT_MASK],
        &[KEY_D, KEY_E, KEY_N, KEY_G],
        &[KEY_D, KEY_E, KEY_G | END_CONSONANT_MASK],
        &[KEY_D, KEY_E, KEY_C, KEY_H],
        &[KEY_D, KEY_E, KEY_K | END_CONSONANT_MASK],
        &[KEY_D, KEY_E, KEY_N],
        &[KEY_D, KEY_E, KEY_C],
        &[KEY_D, KEY_E, KEY_M],
        &[KEY_D, KEY_E],
        &[KEY_D, KEY_E, KEY_T],
        &[KEY_D, KEY_E, KEY_U],
        &[KEY_D, KEY_E, KEY_O],
        &[KEY_D, KEY_E, KEY_P],
        &[KEY_D, KEY_U, KEY_N, KEY_G],
        &[KEY_D, KEY_U, KEY_G | END_CONSONANT_MASK],
        &[KEY_D, KEY_U, KEY_N],
        &[KEY_D, KEY_U, KEY_M],
        &[KEY_D, KEY_U, KEY_C],
        &[KEY_D, KEY_U, KEY_O],
        &[KEY_D, KEY_U, KEY_A],
        &[KEY_D, KEY_U, KEY_O, KEY_I],
        &[KEY_D, KEY_U, KEY_O, KEY_C],
        &[KEY_D, KEY_U, KEY_O, KEY_N],
        &[KEY_D, KEY_U, KEY_O, KEY_N, KEY_G],
        &[KEY_D, KEY_U, KEY_O, KEY_G | END_CONSONANT_MASK],
        &[KEY_D, KEY_U],
        &[KEY_D, KEY_U, KEY_P],
        &[KEY_D, KEY_U, KEY_T],
        &[KEY_D, KEY_U, KEY_I],
        &[KEY_D, KEY_I, KEY_C, KEY_H],
        &[KEY_D, KEY_I, KEY_K | END_CONSONANT_MASK],
        &[KEY_D, KEY_I, KEY_C],
        &[KEY_D, KEY_I, KEY_N, KEY_H],
        &[KEY_D, KEY_I, KEY_H | END_CONSONANT_MASK],
        &[KEY_D, KEY_I, KEY_N],
        &[KEY_D, KEY_I],
        &[KEY_D, KEY_I, KEY_A],
        &[KEY_D, KEY_I, KEY_E],
        &[KEY_D, KEY_I, KEY_E, KEY_C],
        &[KEY_D, KEY_I, KEY_E, KEY_U],
        &[KEY_D, KEY_I, KEY_E, KEY_N],
        &[KEY_D, KEY_I, KEY_E, KEY_M],
        &[KEY_D, KEY_I, KEY_E, KEY_P],
        &[KEY_D, KEY_I, KEY_T],
        &[KEY_D, KEY_O],
        &[KEY_D, KEY_O, KEY_A],
        &[KEY_D, KEY_O, KEY_A, KEY_N],
        &[KEY_D, KEY_O, KEY_A, KEY_N, KEY_G],
        &[KEY_D, KEY_O, KEY_A, KEY_G | END_CONSONANT_MASK],
        &[KEY_D, KEY_O, KEY_A, KEY_N, KEY_H],
        &[KEY_D, KEY_O, KEY_A, KEY_H | END_CONSONANT_MASK],
        &[KEY_D, KEY_O, KEY_A, KEY_M],
        &[KEY_D, KEY_O, KEY_E],
        &[KEY_D, KEY_O, KEY_I],
        &[KEY_D, KEY_O, KEY_P],
        &[KEY_D, KEY_O, KEY_C],
        &[KEY_D, KEY_O, KEY_N],
        &[KEY_D, KEY_O, KEY_N, KEY_G],
        &[KEY_D, KEY_O, KEY_G | END_CONSONANT_MASK],
        &[KEY_D, KEY_O, KEY_M],
        &[KEY_D, KEY_O, KEY_T],
        &[KEY_D, KEY_A],
        &[KEY_D, KEY_A, KEY_T],
        &[KEY_D, KEY_A, KEY_Y],
        &[KEY_D, KEY_A, KEY_U],
        &[KEY_D, KEY_A, KEY_I],
        &[KEY_D, KEY_A, KEY_O],
        &[KEY_D, KEY_A, KEY_P],
        &[KEY_D, KEY_A, KEY_C],
        &[KEY_D, KEY_A, KEY_C, KEY_H],
        &[KEY_D, KEY_A, KEY_K | END_CONSONANT_MASK],
        &[KEY_D, KEY_A, KEY_N],
        &[KEY_D, KEY_A, KEY_N, KEY_H],
        &[KEY_D, KEY_A, KEY_H | END_CONSONANT_MASK],
        &[KEY_D, KEY_A, KEY_N, KEY_G],
        &[KEY_D, KEY_A, KEY_G | END_CONSONANT_MASK],
        &[KEY_D, KEY_A, KEY_M],
        &[KEY_D],
    ]
}

/// `_vowelForMark` (`Vietnamese.cpp:244-332`) in `std::map` order — A, E, Y, O,
/// U, I by keycode (`Engine.cpp:1105`, iteration order is observable).
pub(crate) fn vowel_for_mark() -> &'static [(u16, &'static [&'static [u16]])] {
    static ROWS: &[(u16, &[&[u16]])] = &[
        (
            KEY_A,
            &[
                &[KEY_A, KEY_N, KEY_G],
                &[KEY_A, KEY_G | END_CONSONANT_MASK],
                &[KEY_A, KEY_N],
                &[KEY_A, KEY_N, KEY_H],
                &[KEY_A, KEY_H | END_CONSONANT_MASK],
                &[KEY_A, KEY_M],
                &[KEY_A, KEY_U],
                &[KEY_A, KEY_Y],
                &[KEY_A, KEY_T],
                &[KEY_A, KEY_P],
                &[KEY_A],
                &[KEY_A, KEY_C],
                &[KEY_A, KEY_I],
                &[KEY_A, KEY_O],
                &[KEY_A, KEY_C, KEY_H],
                &[KEY_A, KEY_K | END_CONSONANT_MASK],
            ],
        ),
        (
            KEY_E,
            &[
                &[KEY_E, KEY_N, KEY_H],
                &[KEY_E, KEY_H | END_CONSONANT_MASK],
                &[KEY_E, KEY_N, KEY_G],
                &[KEY_E, KEY_G | END_CONSONANT_MASK],
                &[KEY_E, KEY_C, KEY_H],
                &[KEY_E, KEY_K | END_CONSONANT_MASK],
                &[KEY_E, KEY_C],
                &[KEY_E, KEY_T],
                &[KEY_E, KEY_Y],
                &[KEY_E, KEY_U],
                &[KEY_E, KEY_P],
                &[KEY_E, KEY_C],
                &[KEY_E, KEY_N],
                &[KEY_E, KEY_M],
                &[KEY_E],
            ],
        ),
        (KEY_Y, &[&[KEY_Y]]),
        (
            KEY_O,
            &[
                &[KEY_O, KEY_O, KEY_N, KEY_G],
                &[KEY_O, KEY_O, KEY_G | END_CONSONANT_MASK],
                &[KEY_O, KEY_N, KEY_G],
                &[KEY_O, KEY_G | END_CONSONANT_MASK],
                &[KEY_O, KEY_O, KEY_N],
                &[KEY_O, KEY_O, KEY_C],
                &[KEY_O, KEY_O],
                &[KEY_O, KEY_N],
                &[KEY_O, KEY_M],
                &[KEY_O, KEY_I],
                &[KEY_O, KEY_C],
                &[KEY_O, KEY_T],
                &[KEY_O, KEY_P],
                &[KEY_O],
            ],
        ),
        (
            KEY_U,
            &[
                &[KEY_U, KEY_N, KEY_G],
                &[KEY_U, KEY_G | END_CONSONANT_MASK],
                &[KEY_U, KEY_I],
                &[KEY_U, KEY_O],
                &[KEY_U, KEY_Y],
                &[KEY_U, KEY_Y, KEY_N],
                &[KEY_U, KEY_Y, KEY_T],
                &[KEY_U, KEY_Y, KEY_P],
                &[KEY_U, KEY_Y, KEY_C, KEY_H],
                &[KEY_U, KEY_Y, KEY_K | END_CONSONANT_MASK],
                &[KEY_U, KEY_Y, KEY_N, KEY_H],
                &[KEY_U, KEY_Y, KEY_H | END_CONSONANT_MASK],
                &[KEY_U, KEY_T],
                &[KEY_U, KEY_U],
                &[KEY_U, KEY_A],
                &[KEY_U, KEY_I],
                &[KEY_U, KEY_C],
                &[KEY_U, KEY_N],
                &[KEY_U, KEY_M],
                &[KEY_U, KEY_P],
                &[KEY_U],
            ],
        ),
        (
            KEY_I,
            &[
                &[KEY_I, KEY_N, KEY_H],
                &[KEY_I, KEY_H | END_CONSONANT_MASK],
                &[KEY_I, KEY_C, KEY_H],
                &[KEY_I, KEY_K | END_CONSONANT_MASK],
                &[KEY_I, KEY_N],
                &[KEY_I, KEY_T],
                &[KEY_I, KEY_U],
                &[KEY_I, KEY_U, KEY_P],
                &[KEY_I, KEY_N],
                &[KEY_I, KEY_M],
                &[KEY_I, KEY_P],
                &[KEY_I, KEY_A],
                &[KEY_I, KEY_C],
                &[KEY_I],
            ],
        ),
    ];
    ROWS
}

/// `_consonantTable` (`Vietnamese.cpp:334-368`).
pub(crate) fn consonant_table() -> &'static [&'static [u16]] {
    &[
        &[KEY_N, KEY_G, KEY_H],
        &[KEY_P, KEY_H],
        &[KEY_T, KEY_H],
        &[KEY_T, KEY_R],
        &[KEY_G, KEY_I],
        &[KEY_C, KEY_H],
        &[KEY_N, KEY_H],
        &[KEY_N, KEY_G],
        &[KEY_K, KEY_H],
        &[KEY_G, KEY_H],
        &[KEY_G],
        &[KEY_C],
        &[KEY_Q],
        &[KEY_K],
        &[KEY_T],
        &[KEY_R],
        &[KEY_H],
        &[KEY_B],
        &[KEY_M],
        &[KEY_V],
        &[KEY_N],
        &[KEY_L],
        &[KEY_X],
        &[KEY_P],
        &[KEY_S],
        &[KEY_D],
        &[KEY_F | CONSONANT_ALLOW_MASK],
        &[KEY_W | CONSONANT_ALLOW_MASK],
        &[KEY_Z | CONSONANT_ALLOW_MASK],
        &[KEY_J | CONSONANT_ALLOW_MASK],
        &[KEY_F | END_CONSONANT_MASK],
        &[KEY_W | END_CONSONANT_MASK],
        &[KEY_J | END_CONSONANT_MASK],
    ]
}

/// `_endConsonantTable` (`Vietnamese.cpp:370-374`).
pub(crate) fn end_consonant_table() -> &'static [&'static [u16]] {
    &[
        &[KEY_T],
        &[KEY_P],
        &[KEY_C],
        &[KEY_N],
        &[KEY_M],
        &[KEY_G | END_CONSONANT_MASK],
        &[KEY_K | END_CONSONANT_MASK],
        &[KEY_H | END_CONSONANT_MASK],
        &[KEY_C, KEY_H],
        &[KEY_N, KEY_H],
        &[KEY_N, KEY_G],
    ]
}

/// `_standaloneWbad` (`Vietnamese.cpp:376-378`).
pub(crate) fn standalone_w_bad() -> &'static [u16] {
    &[KEY_W, KEY_E, KEY_Y, KEY_F, KEY_J, KEY_K, KEY_Z]
}

/// `_doubleWAllowed` (`Vietnamese.cpp:380-390`).
pub(crate) fn double_w_allowed() -> &'static [[u16; 2]] {
    &[
        [KEY_T, KEY_R],
        [KEY_T, KEY_H],
        [KEY_C, KEY_H],
        [KEY_N, KEY_H],
        [KEY_N, KEY_G],
        [KEY_K, KEY_H],
        [KEY_G, KEY_I],
        [KEY_P, KEY_H],
        [KEY_G, KEY_H],
    ]
}

/// `_quickStartConsonant` (`Vietnamese.cpp:392-396`).
pub(crate) fn quick_start_consonant(code: u16) -> Option<[u16; 2]> {
    match code {
        KEY_F => Some([KEY_P, KEY_H]),
        KEY_J => Some([KEY_G, KEY_I]),
        KEY_W => Some([KEY_Q, KEY_U]),
        _ => None,
    }
}

/// `_quickEndConsonant` (`Vietnamese.cpp:398-402`).
pub(crate) fn quick_end_consonant(code: u16) -> Option<[u16; 2]> {
    match code {
        KEY_G => Some([KEY_N, KEY_G]),
        KEY_H => Some([KEY_N, KEY_H]),
        KEY_K => Some([KEY_C, KEY_H]),
        _ => None,
    }
}

/// `_quickTelex` (`Vietnamese.cpp:497-506`).
pub(crate) fn quick_telex(code: u16) -> Option<[u16; 2]> {
    match code {
        KEY_C => Some([KEY_C, KEY_H]),
        KEY_G => Some([KEY_G, KEY_I]),
        KEY_K => Some([KEY_K, KEY_H]),
        KEY_N => Some([KEY_N, KEY_G]),
        KEY_Q => Some([KEY_Q, KEY_U]),
        KEY_P => Some([KEY_P, KEY_H]),
        KEY_T => Some([KEY_T, KEY_H]),
        KEY_U => Some([KEY_U, KEY_U]),
        _ => None,
    }
}

/// `_codeTable[0..5]` (`Vietnamese.cpp:411-491`), one row list per [`Charset`],
/// in upstream order. The key packs the letter with the tone-modifier bit,
/// exactly as upstream; the column index is `markElem` from `getCharacterCode`
/// (`Engine.cpp:501-556`). The caps/normal pairs interleave, which is why the
/// columns are literal data rather than a formula.
///
/// `charset::tests::the_five_tables_match_the_upstream_source` diffs these
/// cells against the oracle's own text, so one mistyped digit fails the build.
#[rustfmt::skip]
const UNICODE_ROWS: &[(u32, &[u16])] = &[
    (KEY_A as u32, &[0x00C2, 0x00E2, 0x0102, 0x0103, 0x00C1, 0x00E1, 0x00C0, 0x00E0, 0x1EA2, 0x1EA3, 0x00C3, 0x00E3, 0x1EA0, 0x1EA1]),
    (KEY_O as u32, &[0x00D4, 0x00F4, 0x01A0, 0x01A1, 0x00D3, 0x00F3, 0x00D2, 0x00F2, 0x1ECE, 0x1ECF, 0x00D5, 0x00F5, 0x1ECC, 0x1ECD]),
    (KEY_U as u32, &[0x0000, 0x0000, 0x01AF, 0x01B0, 0x00DA, 0x00FA, 0x00D9, 0x00F9, 0x1EE6, 0x1EE7, 0x0168, 0x0169, 0x1EE4, 0x1EE5]),
    (KEY_E as u32, &[0x00CA, 0x00EA, 0x0000, 0x0000, 0x00C9, 0x00E9, 0x00C8, 0x00E8, 0x1EBA, 0x1EBB, 0x1EBC, 0x1EBD, 0x1EB8, 0x1EB9]),
    (KEY_D as u32, &[0x0110, 0x0111]),
    (KEY_A as u32 | TONE_MASK, &[0x1EA4, 0x1EA5, 0x1EA6, 0x1EA7, 0x1EA8, 0x1EA9, 0x1EAA, 0x1EAB, 0x1EAC, 0x1EAD]),
    (KEY_A as u32 | TONEW_MASK, &[0x1EAE, 0x1EAF, 0x1EB0, 0x1EB1, 0x1EB2, 0x1EB3, 0x1EB4, 0x1EB5, 0x1EB6, 0x1EB7]),
    (KEY_O as u32 | TONE_MASK, &[0x1ED0, 0x1ED1, 0x1ED2, 0x1ED3, 0x1ED4, 0x1ED5, 0x1ED6, 0x1ED7, 0x1ED8, 0x1ED9]),
    (KEY_O as u32 | TONEW_MASK, &[0x1EDA, 0x1EDB, 0x1EDC, 0x1EDD, 0x1EDE, 0x1EDF, 0x1EE0, 0x1EE1, 0x1EE2, 0x1EE3]),
    (KEY_U as u32 | TONEW_MASK, &[0x1EE8, 0x1EE9, 0x1EEA, 0x1EEB, 0x1EEC, 0x1EED, 0x1EEE, 0x1EEF, 0x1EF0, 0x1EF1]),
    (KEY_E as u32 | TONE_MASK, &[0x1EBE, 0x1EBF, 0x1EC0, 0x1EC1, 0x1EC2, 0x1EC3, 0x1EC4, 0x1EC5, 0x1EC6, 0x1EC7]),
    (KEY_I as u32, &[0x00CD, 0x00ED, 0x00CC, 0x00EC, 0x1EC8, 0x1EC9, 0x0128, 0x0129, 0x1ECA, 0x1ECB]),
    (KEY_Y as u32, &[0x00DD, 0x00FD, 0x1EF2, 0x1EF3, 0x1EF6, 0x1EF7, 0x1EF8, 0x1EF9, 0x1EF4, 0x1EF5]),
];

#[rustfmt::skip]
const TCVN3_ROWS: &[(u32, &[u16])] = &[
    (KEY_A as u32, &[0x00A2, 0x00A9, 0x00A1, 0x00A8, 0x00B8, 0x00B8, 0x00B5, 0x00B5, 0x00B6, 0x00B6, 0x00B7, 0x00B7, 0x00B9, 0x00B9]),
    (KEY_O as u32, &[0x00A4, 0x00AB, 0x00A5, 0x00AC, 0x00E3, 0x00E3, 0x00DF, 0x00DF, 0x00E1, 0x00E1, 0x00E2, 0x00E2, 0x00E4, 0x00E4]),
    (KEY_U as u32, &[0x0000, 0x0000, 0x00A6, 0x00AD, 0x00F3, 0x00F3, 0x00EF, 0x00EF, 0x00F1, 0x00F1, 0x00F2, 0x00F2, 0x00F4, 0x00F4]),
    (KEY_E as u32, &[0x00A3, 0x00AA, 0x0000, 0x0000, 0x00D0, 0x00D0, 0x00CC, 0x00CC, 0x00CE, 0x00CE, 0x00CF, 0x00CF, 0x00D1, 0x00D1]),
    (KEY_D as u32, &[0x00A7, 0x00AE]),
    (KEY_A as u32 | TONE_MASK, &[0x00CA, 0x00CA, 0x00C7, 0x00C7, 0x00C8, 0x00C8, 0x00C9, 0x00C9, 0x00CB, 0x00CB]),
    (KEY_A as u32 | TONEW_MASK, &[0x00BE, 0x00BE, 0x00BB, 0x00BB, 0x00BC, 0x00BC, 0x00BD, 0x00BD, 0x00C6, 0x00C6]),
    (KEY_O as u32 | TONE_MASK, &[0x00E8, 0x00E8, 0x00E5, 0x00E5, 0x00E6, 0x00E6, 0x00E7, 0x00E7, 0x00E9, 0x00E9]),
    (KEY_O as u32 | TONEW_MASK, &[0x00ED, 0x00ED, 0x00EA, 0x00EA, 0x00EB, 0x00EB, 0x00EC, 0x00EC, 0x00EE, 0x00EE]),
    (KEY_U as u32 | TONEW_MASK, &[0x00F8, 0x00F8, 0x00F5, 0x00F5, 0x00F6, 0x00F6, 0x00F7, 0x00F7, 0x00F9, 0x00F9]),
    (KEY_E as u32 | TONE_MASK, &[0x00D5, 0x00D5, 0x00D2, 0x00D2, 0x00D3, 0x00D3, 0x00D4, 0x00D4, 0x00D6, 0x00D6]),
    (KEY_I as u32, &[0x00DD, 0x00DD, 0x00D7, 0x00D7, 0x00D8, 0x00D8, 0x00DC, 0x00DC, 0x00DE, 0x00DE]),
    (KEY_Y as u32, &[0x00FD, 0x00FD, 0x00FA, 0x00FA, 0x00FB, 0x00FB, 0x00FC, 0x00FC, 0x00FE, 0x00FE]),
];

#[rustfmt::skip]
const VNI_ROWS: &[(u32, &[u16])] = &[
    (KEY_A as u32, &[0xC241, 0xE261, 0xCA41, 0xEA61, 0xD941, 0xF961, 0xD841, 0xF861, 0xDB41, 0xFB61, 0xD541, 0xF561, 0xCF41, 0xEF61]),
    (KEY_O as u32, &[0xC24F, 0xE26F, 0x00D4, 0x00F4, 0xD94F, 0xF96F, 0xD84F, 0xF86F, 0xDB4F, 0xFB6F, 0xD54F, 0xF56F, 0xCF4F, 0xEF6F]),
    (KEY_U as u32, &[0x0000, 0x0000, 0x00D6, 0x00F6, 0xD955, 0xF975, 0xD855, 0xF875, 0xDB55, 0xFB75, 0xD555, 0xF575, 0xCF55, 0xEF75]),
    (KEY_E as u32, &[0xC245, 0xE265, 0x0000, 0x0000, 0xD945, 0xF965, 0xD845, 0xF865, 0xDB45, 0xFB65, 0xD545, 0xF565, 0xCF45, 0xEF65]),
    (KEY_D as u32, &[0x00D1, 0x00F1]),
    (KEY_A as u32 | TONE_MASK, &[0xC141, 0xE161, 0xC041, 0xE061, 0xC541, 0xE561, 0xC341, 0xE361, 0xC441, 0xE461]),
    (KEY_A as u32 | TONEW_MASK, &[0xC941, 0xE961, 0xC841, 0xE861, 0xDA41, 0xFA61, 0xDC41, 0xFC61, 0xCB41, 0xEB61]),
    (KEY_O as u32 | TONE_MASK, &[0xC14F, 0xE16F, 0xC04F, 0xE06F, 0xC54F, 0xE56F, 0xC34F, 0xE36F, 0xC44F, 0xE46F]),
    (KEY_O as u32 | TONEW_MASK, &[0xD9D4, 0xF9F4, 0xD8D4, 0xF8F4, 0xDBD4, 0xFBF4, 0xD5D4, 0xF5F4, 0xCFD4, 0xEFF4]),
    (KEY_U as u32 | TONEW_MASK, &[0xD9D6, 0xF9F6, 0xD8D6, 0xF8F6, 0xDBD6, 0xFBF6, 0xD5D6, 0xF5F6, 0xCFD6, 0xEFF6]),
    (KEY_E as u32 | TONE_MASK, &[0xC145, 0xE165, 0xC045, 0xE065, 0xC545, 0xE565, 0xC345, 0xE365, 0xC445, 0xE465]),
    (KEY_I as u32, &[0x00CD, 0x00ED, 0x00CC, 0x00EC, 0x00C6, 0x00E6, 0x00D3, 0x00F3, 0x00D2, 0x00F2]),
    (KEY_Y as u32, &[0xD959, 0xF979, 0xD859, 0xF879, 0xDB59, 0xFB79, 0xD559, 0xF579, 0x00CE, 0x00EE]),
];

#[rustfmt::skip]
const COMPOUND_ROWS: &[(u32, &[u16])] = &[
    (KEY_A as u32, &[0x00C2, 0x00E2, 0x0102, 0x0103, 0x2041, 0x2061, 0x4041, 0x4061, 0x6041, 0x6061, 0x8041, 0x8061, 0xA041, 0xA061]),
    (KEY_O as u32, &[0x00D4, 0x00F4, 0x01A0, 0x01A1, 0x204F, 0x206F, 0x404F, 0x406F, 0x604F, 0x606F, 0x804F, 0x806F, 0xA04F, 0xA06F]),
    (KEY_U as u32, &[0x0000, 0x0000, 0x01AF, 0x01B0, 0x2055, 0x2075, 0x4055, 0x4075, 0x6055, 0x6075, 0x8055, 0x8075, 0xA055, 0xA075]),
    (KEY_E as u32, &[0x00CA, 0x00EA, 0x0000, 0x0000, 0x2045, 0x2065, 0x4045, 0x4065, 0x6045, 0x6065, 0x8045, 0x8065, 0xA045, 0xA065]),
    (KEY_D as u32, &[0x0110, 0x0111]),
    (KEY_A as u32 | TONE_MASK, &[0x20C2, 0x20E2, 0x40C2, 0x40E2, 0x60C2, 0x60E2, 0x80C2, 0x80E2, 0xA0C2, 0xA0E2]),
    (KEY_A as u32 | TONEW_MASK, &[0x2102, 0x2103, 0x4102, 0x4103, 0x6102, 0x6103, 0x8102, 0x8103, 0xA102, 0xA103]),
    (KEY_O as u32 | TONE_MASK, &[0x20D4, 0x20F4, 0x40D4, 0x40F4, 0x60D4, 0x60F4, 0x80D4, 0x80F4, 0xA0D4, 0xA0F4]),
    (KEY_O as u32 | TONEW_MASK, &[0x21A0, 0x21A1, 0x41A0, 0x41A1, 0x61A0, 0x61A1, 0x81A0, 0x81A1, 0xA1A0, 0xA1A1]),
    (KEY_U as u32 | TONEW_MASK, &[0x21AF, 0x21B0, 0x41AF, 0x41B0, 0x61AF, 0x61B0, 0x81AF, 0x81B0, 0xA1AF, 0xA1B0]),
    (KEY_E as u32 | TONE_MASK, &[0x20CA, 0x20EA, 0x40CA, 0x40EA, 0x60CA, 0x60EA, 0x80CA, 0x80EA, 0xA0CA, 0xA0EA]),
    (KEY_I as u32, &[0x2049, 0x2069, 0x4049, 0x4069, 0x6049, 0x6069, 0x8049, 0x8069, 0xA049, 0xA069]),
    (KEY_Y as u32, &[0x2059, 0x2079, 0x4059, 0x4079, 0x6059, 0x6079, 0x8059, 0x8079, 0xA059, 0xA079]),
];

#[rustfmt::skip]
const CP1258_ROWS: &[(u32, &[u16])] = &[
    (KEY_A as u32, &[0x00C2, 0x00E2, 0x00C3, 0x00E3, 0xEC41, 0xEC61, 0xCC41, 0xCC61, 0xD241, 0xD261, 0xDE41, 0xDE61, 0xF241, 0xF261]),
    (KEY_O as u32, &[0x00D4, 0x00F4, 0x00D5, 0x00F5, 0xEC4F, 0xEC6F, 0xCC4F, 0xCC6F, 0xD24F, 0xD26F, 0xDE4F, 0xDE6F, 0xF24F, 0xF26F]),
    (KEY_U as u32, &[0x0000, 0x0000, 0x00DD, 0x00FD, 0xEC55, 0xEC75, 0xCC55, 0xCC75, 0xD255, 0xD275, 0xDE55, 0xDE75, 0xF255, 0xF275]),
    (KEY_E as u32, &[0x00CA, 0x00EA, 0x0000, 0x0000, 0xEC45, 0xEC65, 0xCC45, 0xCC65, 0xD245, 0xD265, 0xDE45, 0xDE65, 0xF245, 0xF265]),
    (KEY_D as u32, &[0x00D0, 0x00F0]),
    (KEY_A as u32 | TONE_MASK, &[0xECC2, 0xECE2, 0xCCC2, 0xCCE2, 0xD2C2, 0xD2E2, 0xDEC2, 0xDEE2, 0xF2C2, 0xF2E2]),
    (KEY_A as u32 | TONEW_MASK, &[0xECC3, 0xECE3, 0xCCC3, 0xCCE3, 0xD2C3, 0xD2E3, 0xDEC3, 0xDEE3, 0xF2C3, 0xF2E3]),
    (KEY_O as u32 | TONE_MASK, &[0xECD4, 0xECF4, 0xCCD4, 0xCCF4, 0xD2D4, 0xD2F4, 0xDED4, 0xDEF4, 0xF2D4, 0xF2F4]),
    (KEY_O as u32 | TONEW_MASK, &[0xECD5, 0xECF5, 0xCCD5, 0xCCF5, 0xD2D5, 0xD2F5, 0xDED5, 0xDEF5, 0xF2D5, 0xF2F5]),
    (KEY_U as u32 | TONEW_MASK, &[0xECDD, 0xECFD, 0xCCDD, 0xCCFD, 0xD2DD, 0xD2FD, 0xDEDD, 0xDEFD, 0xF2DD, 0xF2FD]),
    (KEY_E as u32 | TONE_MASK, &[0xECCA, 0xECEA, 0xCCCA, 0xCCEA, 0xD2CA, 0xD2EA, 0xDECA, 0xDEEA, 0xF2CA, 0xF2EA]),
    (KEY_I as u32, &[0xEC49, 0xEC69, 0xCC49, 0xCC69, 0xD249, 0xD269, 0xDE49, 0xDE69, 0xF249, 0xF269]),
    (KEY_Y as u32, &[0xEC59, 0xEC79, 0xCC59, 0xCC79, 0xD259, 0xD279, 0xDE59, 0xDE79, 0xF259, 0xF279]),
];

/// The row list for `charset`.
///
/// A variant match, not `as usize`: `Charset`'s declaration order matches
/// `vCodeTable` today, and a later reordering must not silently point a charset
/// at another table (the same rule ADR-0011 gives for `IS_DOUBLE_CODE`).
fn code_table(charset: Charset) -> &'static [(u32, &'static [u16])] {
    match charset {
        Charset::Unicode => UNICODE_ROWS,
        Charset::Tcvn3 => TCVN3_ROWS,
        Charset::VniWindows => VNI_ROWS,
        Charset::UnicodeCompound => COMPOUND_ROWS,
        Charset::Cp1258 => CP1258_ROWS,
    }
}

/// One cell of `_codeTable[vCodeTable]`: `None` when the row is absent or has
/// no such column.
///
/// Upstream checks that the row exists and then indexes the vector without a
/// bounds check (`Engine.cpp:542-545`), which is undefined behaviour for a
/// column it never wrote. Every combination the word model can produce is in
/// range; the port returns `None` where C would read past the end, and
/// `getCharacterCode` treats both as "not in the table".
pub(crate) fn code_value(charset: Charset, key: u32, elem: usize) -> Option<u16> {
    code_table(charset)
        .iter()
        .find(|(row_key, _)| *row_key == key)
        .and_then(|(_, row)| row.get(elem))
        .copied()
}

/// `_unicodeCompoundMark` (`Vietnamese.cpp:494`): sắc, huyền, hỏi, ngã, nặng.
pub(crate) const UNICODE_COMPOUND_MARKS: [u16; 5] = [0x0301, 0x0300, 0x0309, 0x0303, 0x0323];

/// `keyCodeToCharacter` (the inverse of `_characterMap`, `Vietnamese.cpp:508-576`).
///
/// Exact-match semantics: any bit besides `CAPS_MASK` makes the lookup miss, as
/// upstream's `map` lookup does — which is what makes it safe to feed raw
/// `charData` entries to it.
pub(crate) fn key_character(code: u32) -> Option<char> {
    if code & !(CAPS_MASK | CHAR_MASK) != 0 {
        return None;
    }
    let caps = code & CAPS_MASK != 0;
    let base = (code & CHAR_MASK) as u16;
    let letter = match base {
        KEY_A => 'a',
        KEY_B => 'b',
        KEY_C => 'c',
        KEY_D => 'd',
        KEY_E => 'e',
        KEY_F => 'f',
        KEY_G => 'g',
        KEY_H => 'h',
        KEY_I => 'i',
        KEY_J => 'j',
        KEY_K => 'k',
        KEY_L => 'l',
        KEY_M => 'm',
        KEY_N => 'n',
        KEY_O => 'o',
        KEY_P => 'p',
        KEY_Q => 'q',
        KEY_R => 'r',
        KEY_S => 's',
        KEY_T => 't',
        KEY_U => 'u',
        KEY_V => 'v',
        KEY_W => 'w',
        KEY_X => 'x',
        KEY_Y => 'y',
        KEY_Z => 'z',
        KEY_SPACE => return if caps { None } else { Some(' ') },
        KEY_BACKQUOTE => {
            if caps {
                '~'
            } else {
                '`'
            }
        }
        KEY_MINUS => {
            if caps {
                '_'
            } else {
                '-'
            }
        }
        KEY_EQUALS => {
            if caps {
                '+'
            } else {
                '='
            }
        }
        KEY_LEFT_BRACKET => {
            if caps {
                '{'
            } else {
                '['
            }
        }
        KEY_RIGHT_BRACKET => {
            if caps {
                '}'
            } else {
                ']'
            }
        }
        KEY_BACK_SLASH => {
            if caps {
                '|'
            } else {
                '\\'
            }
        }
        KEY_SEMICOLON => {
            if caps {
                ':'
            } else {
                ';'
            }
        }
        KEY_QUOTE => {
            if caps {
                '"'
            } else {
                '\''
            }
        }
        KEY_COMMA => {
            if caps {
                '<'
            } else {
                ','
            }
        }
        KEY_DOT => {
            if caps {
                '>'
            } else {
                '.'
            }
        }
        KEY_SLASH => {
            if caps {
                '?'
            } else {
                '/'
            }
        }
        KEY_1 => {
            if caps {
                '!'
            } else {
                '1'
            }
        }
        KEY_2 => {
            if caps {
                '@'
            } else {
                '2'
            }
        }
        KEY_3 => {
            if caps {
                '#'
            } else {
                '3'
            }
        }
        KEY_4 => {
            if caps {
                '$'
            } else {
                '4'
            }
        }
        KEY_5 => {
            if caps {
                '%'
            } else {
                '5'
            }
        }
        KEY_6 => {
            if caps {
                '^'
            } else {
                '6'
            }
        }
        KEY_7 => {
            if caps {
                '&'
            } else {
                '7'
            }
        }
        KEY_8 => {
            if caps {
                '*'
            } else {
                '8'
            }
        }
        KEY_9 => {
            if caps {
                '('
            } else {
                '9'
            }
        }
        KEY_0 => {
            if caps {
                ')'
            } else {
                '0'
            }
        }
        _ => return None,
    };
    if caps && letter.is_ascii_alphabetic() {
        Some(letter.to_ascii_uppercase())
    } else {
        Some(letter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::word::key_code;

    #[test]
    fn keycode_constants_match_the_word_model() {
        for (key, code) in [
            (Key::A, KEY_A),
            (Key::B, KEY_B),
            (Key::C, KEY_C),
            (Key::D, KEY_D),
            (Key::E, KEY_E),
            (Key::F, KEY_F),
            (Key::G, KEY_G),
            (Key::H, KEY_H),
            (Key::I, KEY_I),
            (Key::J, KEY_J),
            (Key::K, KEY_K),
            (Key::L, KEY_L),
            (Key::M, KEY_M),
            (Key::N, KEY_N),
            (Key::O, KEY_O),
            (Key::P, KEY_P),
            (Key::Q, KEY_Q),
            (Key::R, KEY_R),
            (Key::S, KEY_S),
            (Key::T, KEY_T),
            (Key::U, KEY_U),
            (Key::V, KEY_V),
            (Key::W, KEY_W),
            (Key::X, KEY_X),
            (Key::Y, KEY_Y),
            (Key::Z, KEY_Z),
        ] {
            assert_eq!(key_code(key), Some(code), "{key:?}");
            assert_eq!(platform_code(key), Some(code), "{key:?}");
        }
        for digit in 0..=9u8 {
            assert_eq!(
                key_code(Key::Digit(digit)),
                platform_code(Key::Digit(digit)),
                "digit {digit}"
            );
        }
    }

    #[test]
    fn keycode_to_character_is_the_documented_inverse_map() {
        assert_eq!(key_character(u32::from(KEY_A)), Some('a'));
        assert_eq!(key_character(u32::from(KEY_A) | CAPS_MASK), Some('A'));
        assert_eq!(key_character(u32::from(KEY_1)), Some('1'));
        assert_eq!(key_character(u32::from(KEY_1) | CAPS_MASK), Some('!'));
        assert_eq!(key_character(u32::from(KEY_SPACE)), Some(' '));
        assert_eq!(key_character(u32::from(KEY_SPACE) | CAPS_MASK), None);
        assert_eq!(key_character(u32::from(KEY_DOT)), Some('.'));
        // A mark bit is not part of the map key, exactly like upstream.
        assert_eq!(key_character(u32::from(KEY_A) | TONE_MASK), None);
    }

    #[test]
    fn unicode_table_answers_the_two_documented_examples() {
        let unicode = |key: u16, elem| code_value(Charset::Unicode, u32::from(key), elem);
        // `ư` and `ơ` are the plain U/O rows' TONEW column (`caps + 2`), which
        // is where `getCharacterCode` looks for a bare horn.
        assert_eq!(unicode(KEY_U, 3), Some(0x01B0));
        assert_eq!(unicode(KEY_O, 3), Some(0x01A1));
        // A horn with a tone uses the `KEY_U|TONEW_MASK` row instead: ứ is its
        // second column.
        assert_eq!(
            code_value(Charset::Unicode, u32::from(KEY_U) | TONEW_MASK, 1),
            Some(0x1EE9)
        );
        // `ấ` is A with a circumflex and sắc.
        assert_eq!(
            code_value(Charset::Unicode, u32::from(KEY_A) | TONE_MASK, 1),
            Some(0x1EA5)
        );
        // Rows are as long as upstream wrote them.
        assert_eq!(unicode(KEY_U, 13), Some(0x1EE5));
        assert_eq!(unicode(KEY_U, 14), None);
        assert_eq!(unicode(KEY_D, 2), None);
    }

    #[test]
    fn every_charset_reads_its_own_table() {
        // Column 5 of the A row (sắc, lower case) is a different value per
        // table, so a charset wired to the wrong row list cannot pass this.
        // `code_value` takes the row key, which is `KEY_A` — the mark columns
        // are addressed by element, not by a mark bit.
        let row = u32::from(KEY_A);
        let elem = 5;
        assert_eq!(code_value(Charset::Unicode, row, elem), Some(0x00E1));
        assert_eq!(code_value(Charset::Tcvn3, row, elem), Some(0x00B8));
        assert_eq!(code_value(Charset::VniWindows, row, elem), Some(0xF961));
        assert_eq!(
            code_value(Charset::UnicodeCompound, row, elem),
            Some(0x2061)
        );
        assert_eq!(code_value(Charset::Cp1258, row, elem), Some(0xEC61));
        // `đ` — the D row has no mark columns at all.
        assert_eq!(
            code_value(Charset::Tcvn3, u32::from(KEY_D), 1),
            Some(0x00AE)
        );
        assert_eq!(
            code_value(Charset::Cp1258, u32::from(KEY_D), 1),
            Some(0x00F0)
        );
    }
}
