//! macOS virtual keycodes → [`Key`], and event flags → [`Mods`].
//!
//! Upstream compiles a different numeric table per platform (`platforms/mac.h`,
//! `platforms/win32.h`). The port keeps the macOS table here so `openkey-core`
//! never sees a native keycode; the `core-purity` gate is what makes this a rule
//! rather than a preference.
//!
//! The codes are from `Carbon/HIToolbox/Events.h` (`kVK_ANSI_A` = `0x00`, ...).
//! Two properties matter more than the individual rows:
//!
//! - **Every mapped code is pinned by a test.** A silently retyped row is a
//!   typing bug that only appears on one letter, which is the hardest kind to
//!   report.
//! - **The fallback is [`Key::Other`], never a guess.** An unmapped key still
//!   reaches the application (nothing in this adapter swallows on its own); the
//!   engine sees `Other`, which upstream also uses for keys outside its map.

use openkey_core::{Key, Mods};

// ── Virtual keycodes (Carbon/HIToolbox/Events.h) ───────────────────────────

pub const VK_A: u16 = 0x00;
pub const VK_S: u16 = 0x01;
pub const VK_D: u16 = 0x02;
pub const VK_F: u16 = 0x03;
pub const VK_H: u16 = 0x04;
pub const VK_G: u16 = 0x05;
pub const VK_Z: u16 = 0x06;
pub const VK_X: u16 = 0x07;
pub const VK_C: u16 = 0x08;
pub const VK_V: u16 = 0x09;
pub const VK_B: u16 = 0x0b;
pub const VK_Q: u16 = 0x0c;
pub const VK_W: u16 = 0x0d;
pub const VK_E: u16 = 0x0e;
pub const VK_R: u16 = 0x0f;
pub const VK_Y: u16 = 0x10;
pub const VK_T: u16 = 0x11;
pub const VK_1: u16 = 0x12;
pub const VK_2: u16 = 0x13;
pub const VK_3: u16 = 0x14;
pub const VK_4: u16 = 0x15;
pub const VK_6: u16 = 0x16;
pub const VK_5: u16 = 0x17;
pub const VK_EQUAL: u16 = 0x18;
pub const VK_9: u16 = 0x19;
pub const VK_7: u16 = 0x1a;
pub const VK_MINUS: u16 = 0x1b;
pub const VK_8: u16 = 0x1c;
pub const VK_0: u16 = 0x1d;
pub const VK_RIGHT_BRACKET: u16 = 0x1e;
pub const VK_O: u16 = 0x1f;
pub const VK_U: u16 = 0x20;
pub const VK_LEFT_BRACKET: u16 = 0x21;
pub const VK_I: u16 = 0x22;
pub const VK_P: u16 = 0x23;
pub const VK_RETURN: u16 = 0x24;
pub const VK_L: u16 = 0x25;
pub const VK_J: u16 = 0x26;
pub const VK_QUOTE: u16 = 0x27;
pub const VK_K: u16 = 0x28;
pub const VK_SEMICOLON: u16 = 0x29;
pub const VK_BACKSLASH: u16 = 0x2a;
pub const VK_COMMA: u16 = 0x2b;
pub const VK_SLASH: u16 = 0x2c;
pub const VK_N: u16 = 0x2d;
pub const VK_M: u16 = 0x2e;
pub const VK_PERIOD: u16 = 0x2f;
pub const VK_TAB: u16 = 0x30;
pub const VK_SPACE: u16 = 0x31;
pub const VK_GRAVE: u16 = 0x32;
pub const VK_DELETE: u16 = 0x33; // backspace
pub const VK_ESCAPE: u16 = 0x35;
pub const VK_FORWARD_DELETE: u16 = 0x75;
pub const VK_LEFT: u16 = 0x7b;
pub const VK_RIGHT: u16 = 0x7c;
pub const VK_DOWN: u16 = 0x7d;
pub const VK_UP: u16 = 0x7e;

// Modifiers. They never produce a key-down; they arrive as
// `kCGEventFlagsChanged`, which is why the mask carries that type at all.
pub const VK_RIGHT_COMMAND: u16 = 0x36;
pub const VK_COMMAND: u16 = 0x37;
pub const VK_SHIFT: u16 = 0x38;
pub const VK_CAPS_LOCK: u16 = 0x39;
pub const VK_OPTION: u16 = 0x3a;
pub const VK_CONTROL: u16 = 0x3b;
pub const VK_RIGHT_SHIFT: u16 = 0x3c;
pub const VK_RIGHT_OPTION: u16 = 0x3d;
pub const VK_RIGHT_CONTROL: u16 = 0x3e;
pub const VK_FN: u16 = 0x3f;

/// The full `(keycode, Key)` table. Tests iterate this, so a row that is added
/// without a test is impossible and a row that is retyped fails its own case.
///
/// Two deliberate omissions and one deliberate addition:
///
/// - **Keypad Enter (0x4c) is absent.** Upstream has distinct codes —
///   `KEY_ENTER 76` and `KEY_RETURN 36` (`platforms/mac.h:15-16`) — and though
///   both are break codes (`Engine.cpp:22`), only Return is in the macro-break
///   list (`Engine.cpp:30-31`). Mapping 76 to [`Key::Enter`] here would erase
///   that difference before the engine can model it, so the keycode survives as
///   [`Key::Other`] and C-002 decides. The key still passes through untouched.
/// - **No media/function keys.** They are not engine input; `Other` carries the
///   code without pretending to know what it is.
/// - **Forward delete (0x75) maps to [`Key::Delete`].** `mac.h` has no entry for
///   0x75 at all (its `KEY_DELETE` is 51, backspace), so this is an intentional
///   extra rather than a ported row: the key exists on every Mac keyboard and
///   the engine must see it as a break key, not as an opaque `Other`.
pub(crate) const TABLE: &[(u16, Key)] = &[
    (VK_A, Key::A),
    (VK_B, Key::B),
    (VK_C, Key::C),
    (VK_D, Key::D),
    (VK_E, Key::E),
    (VK_F, Key::F),
    (VK_G, Key::G),
    (VK_H, Key::H),
    (VK_I, Key::I),
    (VK_J, Key::J),
    (VK_K, Key::K),
    (VK_L, Key::L),
    (VK_M, Key::M),
    (VK_N, Key::N),
    (VK_O, Key::O),
    (VK_P, Key::P),
    (VK_Q, Key::Q),
    (VK_R, Key::R),
    (VK_S, Key::S),
    (VK_T, Key::T),
    (VK_U, Key::U),
    (VK_V, Key::V),
    (VK_W, Key::W),
    (VK_X, Key::X),
    (VK_Y, Key::Y),
    (VK_Z, Key::Z),
    (VK_0, Key::Digit(0)),
    (VK_1, Key::Digit(1)),
    (VK_2, Key::Digit(2)),
    (VK_3, Key::Digit(3)),
    (VK_4, Key::Digit(4)),
    (VK_5, Key::Digit(5)),
    (VK_6, Key::Digit(6)),
    (VK_7, Key::Digit(7)),
    (VK_8, Key::Digit(8)),
    (VK_9, Key::Digit(9)),
    (VK_SPACE, Key::Space),
    (VK_RETURN, Key::Enter),
    (VK_TAB, Key::Tab),
    (VK_ESCAPE, Key::Escape),
    (VK_DELETE, Key::Backspace),
    (VK_FORWARD_DELETE, Key::Delete),
    (VK_LEFT, Key::ArrowLeft),
    (VK_RIGHT, Key::ArrowRight),
    (VK_UP, Key::ArrowUp),
    (VK_DOWN, Key::ArrowDown),
    (VK_LEFT_BRACKET, Key::LeftBracket),
    (VK_RIGHT_BRACKET, Key::RightBracket),
    (VK_SEMICOLON, Key::Semicolon),
    (VK_QUOTE, Key::Quote),
    (VK_COMMA, Key::Comma),
    (VK_PERIOD, Key::Dot),
    (VK_SLASH, Key::Slash),
    (VK_BACKSLASH, Key::BackSlash),
    (VK_MINUS, Key::Minus),
    (VK_EQUAL, Key::Equals),
    (VK_GRAVE, Key::Backquote),
];

/// A modifier key, identified from the keycode carried by `kCGEventFlagsChanged`.
///
/// The left/right distinction is kept because upstream compares raw keycodes for
/// its hotkeys (`OpenKey.mm:634-661`); collapsing the pair here would take that
/// away from MAC-004.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Modifier {
    Command,
    RightCommand,
    Shift,
    RightShift,
    Option,
    RightOption,
    Control,
    RightControl,
    CapsLock,
    Fn,
}

impl Modifier {
    /// The [`Mods`] field this key controls.
    pub fn field(self) -> fn(&mut Mods) {
        match self {
            Self::Command | Self::RightCommand => |m| m.cmd = true,
            Self::Shift | Self::RightShift => |m| m.shift = true,
            Self::Option | Self::RightOption => |m| m.alt = true,
            Self::Control | Self::RightControl => |m| m.ctrl = true,
            Self::CapsLock => |m| m.caps_lock = true,
            // Fn does not map onto an engine flag; it is reported so a hotkey
            // bound to it can be recognised, and ignored by the engine.
            Self::Fn => |_| {},
        }
    }
}

/// Normalise a native keycode. The fallback is deliberately `Other`, not a
/// nearby letter: a wrong letter is a silent typing bug, `Other` is a
/// break/passthrough key the engine already understands.
pub fn key_from_keycode(keycode: u16) -> Key {
    TABLE
        .iter()
        .find_map(|(code, key)| (*code == keycode).then_some(*key))
        .unwrap_or(Key::Other(keycode))
}

/// The modifier a keycode denotes, if it is a modifier key.
pub fn modifier_from_keycode(keycode: u16) -> Option<Modifier> {
    Some(match keycode {
        VK_COMMAND => Modifier::Command,
        VK_RIGHT_COMMAND => Modifier::RightCommand,
        VK_SHIFT => Modifier::Shift,
        VK_RIGHT_SHIFT => Modifier::RightShift,
        VK_OPTION => Modifier::Option,
        VK_RIGHT_OPTION => Modifier::RightOption,
        VK_CONTROL => Modifier::Control,
        VK_RIGHT_CONTROL => Modifier::RightControl,
        VK_CAPS_LOCK => Modifier::CapsLock,
        VK_FN => Modifier::Fn,
        _ => return None,
    })
}

// ── Event flags (`CGEventFlags`, CoreGraphics/CGEventTypes.h) ──────────────

pub const FLAG_SHIFT: u64 = 0x0002_0000;
pub const FLAG_CONTROL: u64 = 0x0004_0000;
pub const FLAG_OPTION: u64 = 0x0008_0000;
pub const FLAG_COMMAND: u64 = 0x0010_0000;
pub const FLAG_CAPS_LOCK: u64 = 0x0001_0000;

/// Flags word → the engine's [`Mods`]. `caps_lock` comes from the flag, not from
/// the buffer, so a held Caps Lock applies to the next word without an extra key
/// event.
pub fn mods_from_flags(flags: u64) -> Mods {
    Mods {
        shift: flags & FLAG_SHIFT != 0,
        ctrl: flags & FLAG_CONTROL != 0,
        alt: flags & FLAG_OPTION != 0,
        cmd: flags & FLAG_COMMAND != 0,
        caps_lock: flags & FLAG_CAPS_LOCK != 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rows that carry Vietnamese input: one case each, so a retyped row
    /// names the letter it broke. The `TABLE` iteration below covers the rest,
    /// but only this list pins the *values*.
    #[test]
    fn every_letter_is_mapped_to_itself() {
        let expected = [
            (VK_A, Key::A),
            (VK_B, Key::B),
            (VK_C, Key::C),
            (VK_D, Key::D),
            (VK_E, Key::E),
            (VK_F, Key::F),
            (VK_G, Key::G),
            (VK_H, Key::H),
            (VK_I, Key::I),
            (VK_J, Key::J),
            (VK_K, Key::K),
            (VK_L, Key::L),
            (VK_M, Key::M),
            (VK_N, Key::N),
            (VK_O, Key::O),
            (VK_P, Key::P),
            (VK_Q, Key::Q),
            (VK_R, Key::R),
            (VK_S, Key::S),
            (VK_T, Key::T),
            (VK_U, Key::U),
            (VK_V, Key::V),
            (VK_W, Key::W),
            (VK_X, Key::X),
            (VK_Y, Key::Y),
            (VK_Z, Key::Z),
        ];
        assert_eq!(expected.len(), 26);
        for (code, key) in expected {
            assert_eq!(key_from_keycode(code), key, "keycode {code:#04x}");
        }
    }

    #[test]
    fn digits_are_mapped_by_their_printed_digit_not_by_row_order() {
        // The macOS table is deliberately not consecutive: 0x16 is 6 and 0x17 is
        // 5. Ordering the rows by keycode would give the wrong digit.
        let expected = [
            (VK_0, 0),
            (VK_1, 1),
            (VK_2, 2),
            (VK_3, 3),
            (VK_4, 4),
            (VK_5, 5),
            (VK_6, 6),
            (VK_7, 7),
            (VK_8, 8),
            (VK_9, 9),
        ];
        for (code, digit) in expected {
            assert_eq!(
                key_from_keycode(code),
                Key::Digit(digit),
                "keycode {code:#04x}"
            );
        }
        assert_eq!(key_from_keycode(VK_5), Key::Digit(5));
        assert_eq!(key_from_keycode(VK_6), Key::Digit(6));
    }

    #[test]
    fn navigation_and_editing_keys_are_distinct() {
        assert_eq!(key_from_keycode(VK_SPACE), Key::Space);
        assert_eq!(key_from_keycode(VK_RETURN), Key::Enter);
        assert_eq!(key_from_keycode(VK_TAB), Key::Tab);
        assert_eq!(key_from_keycode(VK_ESCAPE), Key::Escape);
        assert_eq!(key_from_keycode(VK_DELETE), Key::Backspace);
        assert_eq!(key_from_keycode(VK_FORWARD_DELETE), Key::Delete);
        assert_eq!(key_from_keycode(VK_LEFT), Key::ArrowLeft);
        assert_eq!(key_from_keycode(VK_RIGHT), Key::ArrowRight);
        assert_eq!(key_from_keycode(VK_UP), Key::ArrowUp);
        assert_eq!(key_from_keycode(VK_DOWN), Key::ArrowDown);
    }

    #[test]
    fn punctuation_matches_the_us_layout_keys() {
        let expected = [
            (VK_LEFT_BRACKET, Key::LeftBracket),
            (VK_RIGHT_BRACKET, Key::RightBracket),
            (VK_SEMICOLON, Key::Semicolon),
            (VK_QUOTE, Key::Quote),
            (VK_COMMA, Key::Comma),
            (VK_PERIOD, Key::Dot),
            (VK_SLASH, Key::Slash),
            (VK_BACKSLASH, Key::BackSlash),
            (VK_MINUS, Key::Minus),
            (VK_EQUAL, Key::Equals),
            (VK_GRAVE, Key::Backquote),
        ];
        for (code, key) in expected {
            assert_eq!(key_from_keycode(code), key, "keycode {code:#04x}");
        }
    }

    #[test]
    fn the_table_has_no_duplicate_keycode() {
        let mut seen: Vec<u16> = TABLE.iter().map(|(code, _)| *code).collect();
        let before = seen.len();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), before, "a keycode appears twice in TABLE");
    }

    #[test]
    fn keypad_enter_stays_other_until_the_engine_can_tell_it_apart() {
        // 0x4c is upstream's `KEY_ENTER` (mac.h:15); Return is 0x24/36. Both
        // break a word but only Return breaks a macro, so the distinction has
        // to survive normalisation — `Key::Other(0x4c)` keeps the native code
        // for C-002 instead of guessing `Key::Enter`.
        assert_eq!(key_from_keycode(0x4c), Key::Other(0x4c));
        assert_ne!(key_from_keycode(0x4c), Key::Enter);
    }

    #[test]
    fn unknown_and_media_keys_fall_back_to_other() {
        // F1 and the volume keys are not engine input; they must still be
        // representable, and they must not be guessed as a nearby letter.
        for code in [0x7a_u16, 0x48, 0x49, 0x4a, 0x4b, 0x5f, 0x67] {
            assert_eq!(key_from_keycode(code), Key::Other(code));
        }
        // The same fallback applies to a code with no `Events.h` entry at all.
        assert_eq!(key_from_keycode(0xffff), Key::Other(0xffff));
    }

    #[test]
    fn every_modifier_keycode_maps_to_one_modifier() {
        let expected = [
            (VK_COMMAND, Modifier::Command),
            (VK_RIGHT_COMMAND, Modifier::RightCommand),
            (VK_SHIFT, Modifier::Shift),
            (VK_RIGHT_SHIFT, Modifier::RightShift),
            (VK_OPTION, Modifier::Option),
            (VK_RIGHT_OPTION, Modifier::RightOption),
            (VK_CONTROL, Modifier::Control),
            (VK_RIGHT_CONTROL, Modifier::RightControl),
            (VK_CAPS_LOCK, Modifier::CapsLock),
            (VK_FN, Modifier::Fn),
        ];
        for (code, modifier) in expected {
            assert_eq!(
                modifier_from_keycode(code),
                Some(modifier),
                "keycode {code:#04x}"
            );
        }
        assert_eq!(modifier_from_keycode(VK_A), None);
        assert_eq!(modifier_from_keycode(VK_SPACE), None);
    }

    #[test]
    fn modifier_field_sets_exactly_one_flag() {
        for modifier in [
            Modifier::Command,
            Modifier::RightCommand,
            Modifier::Shift,
            Modifier::RightShift,
            Modifier::Option,
            Modifier::RightOption,
            Modifier::Control,
            Modifier::RightControl,
            Modifier::CapsLock,
        ] {
            let mut mods = Mods::default();
            (modifier.field())(&mut mods);
            let set = [mods.shift, mods.ctrl, mods.alt, mods.cmd, mods.caps_lock]
                .iter()
                .filter(|set| **set)
                .count();
            assert_eq!(set, 1, "{modifier:?} must set one flag");
        }

        let mut mods = Mods::default();
        (Modifier::Fn.field())(&mut mods);
        assert_eq!(mods, Mods::default(), "Fn has no engine flag");
    }

    #[test]
    fn flags_map_to_mods() {
        let mods = mods_from_flags(FLAG_SHIFT | FLAG_OPTION | FLAG_COMMAND);
        assert!(mods.shift && mods.alt && mods.cmd);
        assert!(!mods.ctrl && !mods.caps_lock);

        let mods = mods_from_flags(FLAG_CAPS_LOCK | FLAG_CONTROL);
        assert!(mods.caps_lock && mods.ctrl);
        assert!(!mods.shift && !mods.alt && !mods.cmd);

        assert_eq!(mods_from_flags(0), Mods::default());
    }
}
