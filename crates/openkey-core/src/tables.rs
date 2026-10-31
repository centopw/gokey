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

/// `IS_SPECIALKEY` for Telex (`DataType.h:133-137`).
pub(crate) fn is_special_key_telex(code: u16) -> bool {
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

/// `IS_KEY_DOUBLE` for Telex (`Engine.cpp:45-46`).
pub(crate) fn is_key_double_telex(code: u16) -> bool {
    matches!(code, KEY_A | KEY_O | KEY_E)
}

/// `IS_MARK_KEY` for Telex (`Engine.cpp:53-54`).
pub(crate) fn is_mark_key_telex(code: u16) -> bool {
    matches!(code, KEY_S | KEY_F | KEY_R | KEY_J | KEY_X)
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

/// `_codeTable[0]` (Unicode), `Vietnamese.cpp:411-430`. The key packs the
/// consonant with the tone-modifier bit, exactly as upstream.
pub(crate) fn unicode_code(key: u32, elem: usize) -> Option<u16> {
    let row: &[u16] = match key {
        k if k == u32::from(KEY_A) => &[
            0x00C2, 0x00E2, 0x0102, 0x0103, 0x00C1, 0x00E1, 0x00C0, 0x00E0, 0x1EA2, 0x1EA3, 0x00C3,
            0x00E3, 0x1EA0, 0x1EA1,
        ],
        k if k == u32::from(KEY_O) => &[
            0x00D4, 0x00F4, 0x01A0, 0x01A1, 0x00D3, 0x00F3, 0x00D2, 0x00F2, 0x1ECE, 0x1ECF, 0x00D5,
            0x00F5, 0x1ECC, 0x1ECD,
        ],
        k if k == u32::from(KEY_U) => &[
            0x0000, 0x0000, 0x01AF, 0x01B0, 0x00DA, 0x00FA, 0x00D9, 0x00F9, 0x1EE6, 0x1EE7, 0x0168,
            0x0169, 0x1EE4, 0x1EE5,
        ],
        k if k == u32::from(KEY_E) => &[
            0x00CA, 0x00EA, 0x0000, 0x0000, 0x00C9, 0x00E9, 0x00C8, 0x00E8, 0x1EBA, 0x1EBB, 0x1EBC,
            0x1EBD, 0x1EB8, 0x1EB9,
        ],
        k if k == u32::from(KEY_D) => &[0x0110, 0x0111],
        k if k == u32::from(KEY_A) | TONE_MASK => &[
            0x1EA4, 0x1EA5, 0x1EA6, 0x1EA7, 0x1EA8, 0x1EA9, 0x1EAA, 0x1EAB, 0x1EAC, 0x1EAD,
        ],
        k if k == u32::from(KEY_A) | TONEW_MASK => &[
            0x1EAE, 0x1EAF, 0x1EB0, 0x1EB1, 0x1EB2, 0x1EB3, 0x1EB4, 0x1EB5, 0x1EB6, 0x1EB7,
        ],
        k if k == u32::from(KEY_O) | TONE_MASK => &[
            0x1ED0, 0x1ED1, 0x1ED2, 0x1ED3, 0x1ED4, 0x1ED5, 0x1ED6, 0x1ED7, 0x1ED8, 0x1ED9,
        ],
        k if k == u32::from(KEY_O) | TONEW_MASK => &[
            0x1EDA, 0x1EDB, 0x1EDC, 0x1EDD, 0x1EDE, 0x1EDF, 0x1EE0, 0x1EE1, 0x1EE2, 0x1EE3,
        ],
        k if k == u32::from(KEY_U) | TONEW_MASK => &[
            0x1EE8, 0x1EE9, 0x1EEA, 0x1EEB, 0x1EEC, 0x1EED, 0x1EEE, 0x1EEF, 0x1EF0, 0x1EF1,
        ],
        k if k == u32::from(KEY_E) | TONE_MASK => &[
            0x1EBE, 0x1EBF, 0x1EC0, 0x1EC1, 0x1EC2, 0x1EC3, 0x1EC4, 0x1EC5, 0x1EC6, 0x1EC7,
        ],
        k if k == u32::from(KEY_I) => &[
            0x00CD, 0x00ED, 0x00CC, 0x00EC, 0x1EC8, 0x1EC9, 0x0128, 0x0129, 0x1ECA, 0x1ECB,
        ],
        k if k == u32::from(KEY_Y) => &[
            0x00DD, 0x00FD, 0x1EF2, 0x1EF3, 0x1EF6, 0x1EF7, 0x1EF8, 0x1EF9, 0x1EF4, 0x1EF5,
        ],
        _ => return None,
    };
    row.get(elem).copied()
}

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
        // `ư` and `ơ` are the plain U/O rows' TONEW column (`caps + 2`), which
        // is where `getCharacterCode` looks for a bare horn.
        assert_eq!(unicode_code(u32::from(KEY_U), 3), Some(0x01B0));
        assert_eq!(unicode_code(u32::from(KEY_O), 3), Some(0x01A1));
        // A horn with a tone uses the `KEY_U|TONEW_MASK` row instead: ứ is its
        // second column.
        assert_eq!(unicode_code(u32::from(KEY_U) | TONEW_MASK, 1), Some(0x1EE9));
        // `ấ` is A with a circumflex and sắc.
        assert_eq!(unicode_code(u32::from(KEY_A) | TONE_MASK, 1), Some(0x1EA5));
        // Rows are as long as upstream wrote them.
        assert_eq!(unicode_code(u32::from(KEY_U), 13), Some(0x1EE5));
        assert_eq!(unicode_code(u32::from(KEY_U), 14), None);
        assert_eq!(unicode_code(u32::from(KEY_D), 2), None);
    }
}
