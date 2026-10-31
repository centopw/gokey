//! Event classification: a raw `CGEventTap` callback becomes a [`NativeEvent`].
//!
//! This module is pure and target-independent — it takes the three values the
//! callback can read (event type, keycode, flags, UTF-16 payload) and returns
//! what the adapter should do with them. Keeping it out of [`crate::tap`] is
//! deliberate: the classification is where a mask mistake becomes a typing bug,
//! so it is exercised on all three CI runners rather than only on macOS.
//!
//! ## The mask
//!
//! ADR-0004 §1.1 fixes the mask at upstream's seven types **minus key-up**:
//! key-down, flags-changed and the four left/right mouse down/drag types.
//! Key-up is masked upstream and read nowhere (the only engine dispatch is
//! key-down, `OpenKey.mm:713`), so leaving it out changes no behaviour and saves
//! a callback per keystroke.
//!
//! Flags-changed is not incidental: modifier keys never generate a key-down, so
//! the language toggle and the ⌃/⌘ temporary switches only exist in that branch.
//! The four mouse types start a new word (`OpenKey.mm:689` → `RequestNewSession`).
//! A key-down-only tap passes a smoke test and fails the first hotkey.

use openkey_core::{Key, KeyEvent, Mods, Phase};

use crate::keymap::{Modifier, key_from_keycode, modifier_from_keycode, mods_from_flags};

/// `CGEventType` values the adapter distinguishes
/// (`CoreGraphics/CGEventTypes.h`).
pub mod event_type {
    pub const LEFT_MOUSE_DOWN: u32 = 1;
    pub const RIGHT_MOUSE_DOWN: u32 = 3;
    pub const LEFT_MOUSE_DRAGGED: u32 = 6;
    pub const RIGHT_MOUSE_DRAGGED: u32 = 7;
    pub const KEY_DOWN: u32 = 10;
    /// Masked nowhere in this adapter; present so the exclusion is testable.
    pub const KEY_UP: u32 = 11;
    pub const FLAGS_CHANGED: u32 = 12;

    /// Delivered *as events* when the system disables the tap, not as an error
    /// from `CGEventTapCreate`. Ignoring them produces an IME that types plain
    /// ASCII with no visible cause.
    pub const TAP_DISABLED_BY_TIMEOUT: u32 = 0xffff_fffe;
    pub const TAP_DISABLED_BY_USER_INPUT: u32 = 0xffff_ffff;
}

use event_type::*;

/// The event types in the mask. `MASK` is derived from this list, so the list
/// and the bitmask cannot disagree.
pub const MASKED_TYPES: [u32; 6] = [
    LEFT_MOUSE_DOWN,
    RIGHT_MOUSE_DOWN,
    LEFT_MOUSE_DRAGGED,
    RIGHT_MOUSE_DRAGGED,
    KEY_DOWN,
    FLAGS_CHANGED,
];

/// `CGEventMask` for `CGEventTapCreate`. `0x14CA` — pinned by a test so a
/// dropped bit is a red build and not a missing toggle in a later manual pass.
pub const MASK: u64 = {
    let mut mask = 0u64;
    let mut i = 0;
    while i < MASKED_TYPES.len() {
        mask |= 1u64 << MASKED_TYPES[i];
        i += 1;
    }
    mask
};

/// `kCGEventSourceUserData` (field 42 on the event).
pub const FIELD_USER_DATA: u32 = 42;
/// `kCGKeyboardEventKeycode` (field 9 on the event).
pub const FIELD_KEYCODE: u32 = 9;

/// Marker stamped on every event this adapter emits. Any event carrying it is
/// ours and must pass through untouched: it is the only thing that stops the tap
/// from reprocessing its own output, and the corruption it prevents is silent.
///
/// The value is arbitrary; it must match whatever `tap::emit` stamps. It is not
/// `kCGEventSourceUserData`'s documented domain (the system reserves some
/// values), so it is deliberately not a small integer.
pub const ECHO_MAGIC: i64 = 0x474b_3031; // "GK01"

pub fn is_echo(user_data: i64) -> bool {
    user_data == ECHO_MAGIC
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseKind {
    LeftDown,
    RightDown,
    LeftDrag,
    RightDrag,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisableReason {
    Timeout,
    UserInput,
}

/// What the callback saw, after the mask.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeEvent {
    Key {
        key: Key,
        mods: Mods,
        phase: Phase,
        /// Text the active layout produces, when the event carries any.
        text: Option<char>,
    },
    /// A modifier transition. `keycode` stays native because upstream compares
    /// raw keycodes for its hotkeys (`OpenKey.mm:634-661`); `modifier` is the
    /// decoded form for everything that does not need the left/right pair.
    FlagsChanged {
        keycode: u16,
        modifier: Option<Modifier>,
        mods: Mods,
    },
    /// A click or drag that begins text input elsewhere; upstream starts a new
    /// word here. The reset itself is engine input, not adapter state
    /// (ADR-0004 §1.1) and is not performed by this crate yet.
    Mouse {
        kind: MouseKind,
        mods: Mods,
    },
    TapDisabled {
        reason: DisableReason,
    },
    /// Masked at the tap, or not one of the seven: nothing to do.
    Other,
}

impl NativeEvent {
    /// The engine view of a keystroke, or `None` for events that are not
    /// keystrokes. Flags-changed and mouse events are *not* keystrokes even
    /// though they reach the engine upstream through a different door.
    pub fn to_key_event(self) -> Option<KeyEvent> {
        match self {
            Self::Key {
                key,
                mods,
                phase,
                text,
            } => Some(KeyEvent {
                key,
                mods,
                phase,
                text,
            }),
            _ => None,
        }
    }
}

/// Classify one callback. `utf16` is the event's Unicode payload when the caller
/// could read it (key events only); the buffer is fixed-size and NUL-padded,
/// which [`text_from_utf16`] handles.
pub fn classify(event_type: u32, keycode: u16, flags: u64, utf16: &[u16]) -> NativeEvent {
    let mods = mods_from_flags(flags);
    match event_type {
        KEY_DOWN | KEY_UP => NativeEvent::Key {
            key: key_from_keycode(keycode),
            mods,
            phase: if event_type == KEY_DOWN {
                Phase::Down
            } else {
                Phase::Up
            },
            text: text_from_utf16(utf16),
        },
        FLAGS_CHANGED => NativeEvent::FlagsChanged {
            keycode,
            modifier: modifier_from_keycode(keycode),
            mods,
        },
        LEFT_MOUSE_DOWN => NativeEvent::Mouse {
            kind: MouseKind::LeftDown,
            mods,
        },
        RIGHT_MOUSE_DOWN => NativeEvent::Mouse {
            kind: MouseKind::RightDown,
            mods,
        },
        LEFT_MOUSE_DRAGGED => NativeEvent::Mouse {
            kind: MouseKind::LeftDrag,
            mods,
        },
        RIGHT_MOUSE_DRAGGED => NativeEvent::Mouse {
            kind: MouseKind::RightDrag,
            mods,
        },
        TAP_DISABLED_BY_TIMEOUT => NativeEvent::TapDisabled {
            reason: DisableReason::Timeout,
        },
        TAP_DISABLED_BY_USER_INPUT => NativeEvent::TapDisabled {
            reason: DisableReason::UserInput,
        },
        _ => NativeEvent::Other,
    }
}

/// Decode the first scalar in a NUL-padded UTF-16 buffer.
///
/// `CGEventKeyboardGetUnicodeString` writes UTF-16 code units, so `đ` and the
/// tone-marked vowels arrive as one unit and only characters outside the BMP
/// arrive as a surrogate pair. The engine takes one `char` per keystroke; a
/// multi-character payload (a dead-key composition) is truncated to its first
/// scalar, which is the limitation to revisit if a layout needs more.
pub fn text_from_utf16(units: &[u16]) -> Option<char> {
    char::decode_utf16(units.iter().copied().take_while(|unit| *unit != 0)).find_map(Result::ok)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::{FLAG_OPTION, FLAG_SHIFT, VK_S, VK_Z};

    /// The seven-types-minus-key-up contract of ADR-0004 §1.1, as a number.
    #[test]
    fn mask_is_exactly_the_six_contracted_types() {
        assert_eq!(MASK, 0x14ca, "mask changed; see ADR-0004 §1.1");
        for masked in MASKED_TYPES {
            assert_ne!(MASK & (1u64 << masked), 0, "type {masked} is missing");
        }
        assert_eq!(MASK.count_ones(), 6);
    }

    #[test]
    fn key_up_is_not_masked() {
        assert_eq!(MASK & (1u64 << KEY_UP), 0);
    }

    #[test]
    fn a_key_down_carries_key_mods_phase_and_text() {
        let event = classify(KEY_DOWN, VK_S, FLAG_SHIFT, &[0x00e1]);
        assert_eq!(
            event,
            NativeEvent::Key {
                key: Key::S,
                mods: Mods {
                    shift: true,
                    ..Mods::default()
                },
                phase: Phase::Down,
                text: Some('á'),
            }
        );
    }

    #[test]
    fn a_key_up_is_classified_when_it_arrives_anyway() {
        // The mask keeps it out, but the classifier does not rely on that: a
        // listen-only tap or a future mask change must still produce the right
        // phase. `Engine::apply` is the layer that decides what an up means.
        let event = classify(KEY_UP, VK_S, 0, &[]);
        assert_eq!(
            event.to_key_event().map(|event| event.phase),
            Some(Phase::Up)
        );
    }

    #[test]
    fn flags_changed_keeps_the_native_keycode_and_decodes_the_modifier() {
        let event = classify(FLAGS_CHANGED, VK_Z, FLAG_OPTION, &[]);
        // Option is 0x3a; Z is 0x06. The modifier event carries the *modifier's*
        // keycode, which is why the toggle in MAC-004 reads `keycode` and
        // `mods`, not `key`.
        let option = crate::keymap::VK_OPTION;
        let event_from_option = classify(FLAGS_CHANGED, option, FLAG_OPTION, &[]);
        assert_eq!(
            event_from_option,
            NativeEvent::FlagsChanged {
                keycode: option,
                modifier: Some(Modifier::Option),
                mods: Mods {
                    alt: true,
                    ..Mods::default()
                },
            }
        );
        // A flags event for a non-modifier code is still reported, undecoded.
        assert_eq!(
            event,
            NativeEvent::FlagsChanged {
                keycode: VK_Z,
                modifier: None,
                mods: Mods {
                    alt: true,
                    ..Mods::default()
                },
            }
        );
        assert!(event.to_key_event().is_none());
    }

    #[test]
    fn the_four_mouse_types_are_distinguished() {
        for (event_type, kind) in [
            (LEFT_MOUSE_DOWN, MouseKind::LeftDown),
            (RIGHT_MOUSE_DOWN, MouseKind::RightDown),
            (LEFT_MOUSE_DRAGGED, MouseKind::LeftDrag),
            (RIGHT_MOUSE_DRAGGED, MouseKind::RightDrag),
        ] {
            assert_eq!(
                classify(event_type, 0, 0, &[]),
                NativeEvent::Mouse {
                    kind,
                    mods: Mods::default()
                }
            );
        }
        // Other mouse types are deliberately not masked (ADR-0004 §1.1).
        for event_type in [2u32, 4, 5, 25, 26, 27] {
            assert_eq!(classify(event_type, 0, 0, &[]), NativeEvent::Other);
        }
    }

    #[test]
    fn tap_disabled_arrives_as_two_distinct_events() {
        assert_eq!(
            classify(TAP_DISABLED_BY_TIMEOUT, 0, 0, &[]),
            NativeEvent::TapDisabled {
                reason: DisableReason::Timeout
            }
        );
        assert_eq!(
            classify(TAP_DISABLED_BY_USER_INPUT, 0, 0, &[]),
            NativeEvent::TapDisabled {
                reason: DisableReason::UserInput
            }
        );
    }

    #[test]
    fn the_echo_guard_matches_only_our_marker() {
        assert!(is_echo(ECHO_MAGIC));
        assert!(!is_echo(0));
        assert!(!is_echo(ECHO_MAGIC + 1));
        assert!(!is_echo(-1));
    }

    #[test]
    fn vietnamese_scalars_decode_from_one_utf16_unit() {
        // The letters K-008 round-tripped through the emit ABI, plus the tone
        // marks the engine produces.
        for (unit, expected) in [
            (0x00e1_u16, 'á'),
            (0x01a1, 'ơ'),
            (0x0111, 'đ'),
            (0x1eef, 'ữ'),
            (0x1ec7, 'ệ'),
        ] {
            assert_eq!(text_from_utf16(&[unit]), Some(expected), "U+{unit:04X}");
        }
    }

    #[test]
    fn utf16_decoding_handles_padding_surrogates_and_empty_payloads() {
        assert_eq!(text_from_utf16(&[0x00e1, 0, 0, 0]), Some('á'));
        assert_eq!(text_from_utf16(&[0xd83d, 0xde00]), Some('😀'));
        assert_eq!(text_from_utf16(&[]), None);
        assert_eq!(text_from_utf16(&[0, 0]), None);
        // A lone surrogate has no scalar; it must be `None`, not a replacement
        // character invented here.
        assert_eq!(text_from_utf16(&[0xd83d]), None);
    }
}
