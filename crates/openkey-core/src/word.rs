//! Typed word model — ADR-0002 D3 (task C-001).
//!
//! Replaces upstream `Uint32 TypingWord[32]` and its high-bit flag encoding
//! (`DataType.h:88-122`) with named fields. `Word` is the only type here that
//! indexes `letters`; `core-purity` fails any other file that does.

use crate::event::Key;

/// `MAX_BUFF` (`DataType.h:19`).
pub const MAX_LETTERS: usize = 32;

/// Tone marks in upstream mask order (`DataType.h:96-108`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Mark {
    #[default]
    None,
    /// `MARK1_MASK` — sắc (á).
    Sac,
    /// `MARK2_MASK` — huyền (à).
    Huyen,
    /// `MARK3_MASK` — hỏi (ả).
    Hoi,
    /// `MARK4_MASK` — ngã (ã).
    Nga,
    /// `MARK5_MASK` — nặng (ạ).
    Nang,
}

impl Mark {
    /// The `MARK*_MASK` bits this mark sets; zero for [`Mark::None`].
    pub const fn to_bits(self) -> u32 {
        match self {
            Self::None => 0,
            Self::Sac => 0x0008_0000,
            Self::Huyen => 0x0010_0000,
            Self::Hoi => 0x0020_0000,
            Self::Nga => 0x0040_0000,
            Self::Nang => 0x0080_0000,
        }
    }

    /// Exactly one `MARK*_MASK` bit, or zero; anything else is not a mark.
    pub const fn from_bits(bits: u32) -> Option<Self> {
        match bits {
            0 => Some(Self::None),
            0x0008_0000 => Some(Self::Sac),
            0x0010_0000 => Some(Self::Huyen),
            0x0020_0000 => Some(Self::Hoi),
            0x0040_0000 => Some(Self::Nga),
            0x0080_0000 => Some(Self::Nang),
            _ => None,
        }
    }
}

/// One typed character and the mark state pending on it (`TypingWord` entry).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Letter {
    /// Base identity — `CHAR_MASK`.
    pub key: Key,
    /// `CAPS_MASK`.
    pub caps: bool,
    /// `TONE_MASK` — circumflex (â ê ô).
    pub circ: bool,
    /// `TONEW_MASK` — horn/breve (ă ơ ư).
    pub w_mod: bool,
    /// `MARK_MASK`.
    pub mark: Mark,
    /// `STANDALONE_MASK` — produced by a standalone `W`.
    pub standalone: bool,
    /// `CHAR_CODE_MASK` — holds a character code, not a key code.
    pub resolved: bool,
}

impl Default for Letter {
    /// `Key` has no natural default. `Other(0)` fills unused storage slots only;
    /// `Word::get` never reads past `len`.
    fn default() -> Self {
        Self {
            key: Key::Other(0),
            caps: false,
            circ: false,
            w_mod: false,
            mark: Mark::None,
            standalone: false,
            resolved: false,
        }
    }
}

impl Letter {
    const CHAR_MASK: u32 = 0x0000_FFFF;
    const CAPS_MASK: u32 = 0x0001_0000;
    const TONE_MASK: u32 = 0x0002_0000;
    const TONEW_MASK: u32 = 0x0004_0000;
    const MARK_MASK: u32 = 0x00F8_0000;
    const STANDALONE_MASK: u32 = 0x0100_0000;
    const CHAR_CODE_MASK: u32 = 0x0200_0000;
    const KNOWN_MASK: u32 = Self::CHAR_MASK
        | Self::CAPS_MASK
        | Self::TONE_MASK
        | Self::TONEW_MASK
        | Self::MARK_MASK
        | Self::STANDALONE_MASK
        | Self::CHAR_CODE_MASK;

    /// The C bit layout (`DataType.h:88-122`).
    ///
    /// `None` for keys the word model does not admit. Bit equality with the C
    /// build is not an oracle: low 16 bits hold an OS keycode there, and
    /// `platforms/*.h` differ per OS (ADR-0002 D3).
    pub fn to_bits(self) -> Option<u32> {
        Some(
            u32::from(key_code(self.key)?)
                | (u32::from(self.caps) << 16)
                | (u32::from(self.circ) << 17)
                | (u32::from(self.w_mod) << 18)
                | self.mark.to_bits()
                | (u32::from(self.standalone) << 24)
                | (u32::from(self.resolved) << 25),
        )
    }

    /// Inverse of [`Letter::to_bits`]; `None` on an unknown key code, an
    /// unknown mark combination, or bits outside the layout.
    pub fn from_bits(bits: u32) -> Option<Self> {
        if bits & !Self::KNOWN_MASK != 0 {
            return None;
        }
        Some(Self {
            key: key_from_code(u16::try_from(bits & Self::CHAR_MASK).ok()?)?,
            caps: bits & Self::CAPS_MASK != 0,
            circ: bits & Self::TONE_MASK != 0,
            w_mod: bits & Self::TONEW_MASK != 0,
            mark: Mark::from_bits(bits & Self::MARK_MASK)?,
            standalone: bits & Self::STANDALONE_MASK != 0,
            resolved: bits & Self::CHAR_CODE_MASK != 0,
        })
    }
}

/// macOS virtual keycode for a word key (`platforms/mac.h`; DL-026 normative).
///
/// The exhaustive match is deliberate: a new `Key` variant must state whether it
/// is a word key rather than fall through a wildcard.
fn key_code(key: Key) -> Option<u16> {
    match key {
        Key::A => Some(0),
        Key::B => Some(11),
        Key::C => Some(8),
        Key::D => Some(2),
        Key::E => Some(14),
        Key::F => Some(3),
        Key::G => Some(5),
        Key::H => Some(4),
        Key::I => Some(34),
        Key::J => Some(38),
        Key::K => Some(40),
        Key::L => Some(37),
        Key::M => Some(46),
        Key::N => Some(45),
        Key::O => Some(31),
        Key::P => Some(35),
        Key::Q => Some(12),
        Key::R => Some(15),
        Key::S => Some(1),
        Key::T => Some(17),
        Key::U => Some(32),
        Key::V => Some(9),
        Key::W => Some(13),
        Key::X => Some(7),
        Key::Y => Some(16),
        Key::Z => Some(6),
        Key::Digit(0) => Some(29),
        Key::Digit(1) => Some(18),
        Key::Digit(2) => Some(19),
        Key::Digit(3) => Some(20),
        Key::Digit(4) => Some(21),
        Key::Digit(5) => Some(23),
        Key::Digit(6) => Some(22),
        Key::Digit(7) => Some(26),
        Key::Digit(8) => Some(28),
        Key::Digit(9) => Some(25),
        Key::Digit(_)
        | Key::Space
        | Key::Enter
        | Key::Tab
        | Key::Escape
        | Key::Backspace
        | Key::Delete
        | Key::ArrowLeft
        | Key::ArrowRight
        | Key::ArrowUp
        | Key::ArrowDown
        | Key::LeftBracket
        | Key::RightBracket
        | Key::Semicolon
        | Key::Quote
        | Key::Comma
        | Key::Dot
        | Key::Slash
        | Key::BackSlash
        | Key::Minus
        | Key::Equals
        | Key::Backquote
        | Key::Other(_) => None,
    }
}

fn key_from_code(code: u16) -> Option<Key> {
    match code {
        0 => Some(Key::A),
        11 => Some(Key::B),
        8 => Some(Key::C),
        2 => Some(Key::D),
        14 => Some(Key::E),
        3 => Some(Key::F),
        5 => Some(Key::G),
        4 => Some(Key::H),
        34 => Some(Key::I),
        38 => Some(Key::J),
        40 => Some(Key::K),
        37 => Some(Key::L),
        46 => Some(Key::M),
        45 => Some(Key::N),
        31 => Some(Key::O),
        35 => Some(Key::P),
        12 => Some(Key::Q),
        15 => Some(Key::R),
        1 => Some(Key::S),
        17 => Some(Key::T),
        32 => Some(Key::U),
        9 => Some(Key::V),
        13 => Some(Key::W),
        7 => Some(Key::X),
        16 => Some(Key::Y),
        6 => Some(Key::Z),
        29 => Some(Key::Digit(0)),
        18 => Some(Key::Digit(1)),
        19 => Some(Key::Digit(2)),
        20 => Some(Key::Digit(3)),
        21 => Some(Key::Digit(4)),
        23 => Some(Key::Digit(5)),
        22 => Some(Key::Digit(6)),
        26 => Some(Key::Digit(7)),
        28 => Some(Key::Digit(8)),
        25 => Some(Key::Digit(9)),
        _ => None,
    }
}

/// Bounded word buffer: `TypingWord[MAX_BUFF]` plus `_longWordHelper`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Word {
    letters: [Letter; MAX_LETTERS],
    len: u8,
    overflow: Vec<Letter>,
}

impl Default for Word {
    fn default() -> Self {
        Self {
            letters: [Letter::default(); MAX_LETTERS],
            len: 0,
            overflow: Vec::new(),
        }
    }
}

impl Word {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        usize::from(self.len)
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Letters currently in the word, oldest first.
    pub fn letters(&self) -> &[Letter] {
        &self.letters[..self.len()]
    }

    pub fn get(&self, index: usize) -> Option<Letter> {
        if index < self.len() {
            Some(self.letters[index])
        } else {
            None
        }
    }

    /// `setKeyData`: writes a slot without changing `len`; out-of-range writes
    /// are ignored, because the buffer is fixed-size upstream.
    pub fn set(&mut self, index: usize, letter: Letter) {
        if index < MAX_LETTERS {
            self.letters[index] = letter;
        }
    }

    /// `insertKey` (`Engine.cpp:350-356`): at capacity the front letter moves to
    /// `overflow` and the buffer shifts left.
    pub fn push(&mut self, letter: Letter) {
        if self.len() >= MAX_LETTERS {
            self.overflow.push(self.letters[0]);
            self.letters.copy_within(1..MAX_LETTERS, 0);
            self.letters[MAX_LETTERS - 1] = letter;
        } else {
            self.letters[usize::from(self.len)] = letter;
            self.len += 1;
        }
    }

    /// Delete (`Engine.cpp:1436-1447`): when `overflow` is non-empty the last
    /// dropped letter shifts back in at the front and `len` stays put.
    pub fn pop(&mut self) {
        if self.len == 0 {
            return;
        }
        match self.overflow.pop() {
            Some(letter) => {
                self.letters.copy_within(0..MAX_LETTERS - 1, 1);
                self.letters[0] = letter;
            }
            None => self.len -= 1,
        }
    }

    pub fn clear(&mut self) {
        self.len = 0;
        self.overflow.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn letter(n: u16) -> Letter {
        Letter {
            key: Key::Other(n),
            ..Letter::default()
        }
    }

    fn is_word_key(key: Key) -> bool {
        matches!(
            key,
            Key::A
                | Key::B
                | Key::C
                | Key::D
                | Key::E
                | Key::F
                | Key::G
                | Key::H
                | Key::I
                | Key::J
                | Key::K
                | Key::L
                | Key::M
                | Key::N
                | Key::O
                | Key::P
                | Key::Q
                | Key::R
                | Key::S
                | Key::T
                | Key::U
                | Key::V
                | Key::W
                | Key::X
                | Key::Y
                | Key::Z
        ) || matches!(key, Key::Digit(0..=9))
    }

    fn word_keys() -> Vec<Key> {
        let mut keys = vec![
            Key::A,
            Key::B,
            Key::C,
            Key::D,
            Key::E,
            Key::F,
            Key::G,
            Key::H,
            Key::I,
            Key::J,
            Key::K,
            Key::L,
            Key::M,
            Key::N,
            Key::O,
            Key::P,
            Key::Q,
            Key::R,
            Key::S,
            Key::T,
            Key::U,
            Key::V,
            Key::W,
            Key::X,
            Key::Y,
            Key::Z,
        ];
        keys.extend((0..=9).map(Key::Digit));
        keys
    }

    fn non_word_keys() -> Vec<Key> {
        let mut keys = vec![
            Key::Space,
            Key::Enter,
            Key::Tab,
            Key::Escape,
            Key::Backspace,
            Key::Delete,
            Key::ArrowLeft,
            Key::ArrowRight,
            Key::ArrowUp,
            Key::ArrowDown,
            Key::LeftBracket,
            Key::RightBracket,
            Key::Semicolon,
            Key::Quote,
            Key::Comma,
            Key::Dot,
            Key::Slash,
            Key::BackSlash,
            Key::Minus,
            Key::Equals,
            Key::Backquote,
            Key::Digit(10),
            Key::Digit(u8::MAX),
        ];
        keys.extend((0..=u16::MAX).map(Key::Other));
        keys
    }

    #[test]
    fn every_upstream_mark_and_modifier_combination_is_representable() {
        let marks = [
            (Mark::None, 0),
            (Mark::Sac, 0x0008_0000),
            (Mark::Huyen, 0x0010_0000),
            (Mark::Hoi, 0x0020_0000),
            (Mark::Nga, 0x0040_0000),
            (Mark::Nang, 0x0080_0000),
        ];
        let mut seen = 0;
        for (mark, mask) in marks {
            for caps in [false, true] {
                for circ in [false, true] {
                    for w_mod in [false, true] {
                        for standalone in [false, true] {
                            for resolved in [false, true] {
                                let letter = Letter {
                                    key: Key::A,
                                    caps,
                                    circ,
                                    w_mod,
                                    mark,
                                    standalone,
                                    resolved,
                                };
                                let bits = letter.to_bits().expect("A is a word key");
                                assert_eq!(bits & Letter::MARK_MASK, mask);
                                assert_eq!(Letter::from_bits(bits), Some(letter));
                                seen += 1;
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(seen, 6 * 2 * 2 * 2 * 2 * 2);
    }

    #[test]
    fn circumflex_and_horn_are_independent_bits() {
        let letter = Letter {
            key: Key::O,
            circ: true,
            w_mod: true,
            ..Letter::default()
        };
        let bits = letter.to_bits().unwrap();
        assert_eq!(bits & Letter::TONE_MASK, Letter::TONE_MASK);
        assert_eq!(bits & Letter::TONEW_MASK, Letter::TONEW_MASK);
        assert_eq!(Letter::from_bits(bits), Some(letter));
    }

    #[test]
    fn letter_bits_use_the_data_type_h_layout() {
        let letter = Letter {
            key: Key::S,
            caps: true,
            circ: true,
            w_mod: true,
            mark: Mark::Nang,
            standalone: true,
            resolved: true,
        };
        let bits = letter.to_bits().unwrap();
        assert_eq!(bits & Letter::CHAR_MASK, 1, "KEY_S is 1 on macOS");
        assert_eq!(bits & Letter::CAPS_MASK, Letter::CAPS_MASK);
        assert_eq!(bits & Letter::TONE_MASK, Letter::TONE_MASK);
        assert_eq!(bits & Letter::TONEW_MASK, Letter::TONEW_MASK);
        assert_eq!(bits & Letter::MARK_MASK, 0x0080_0000);
        assert_eq!(bits & Letter::STANDALONE_MASK, Letter::STANDALONE_MASK);
        assert_eq!(bits & Letter::CHAR_CODE_MASK, Letter::CHAR_CODE_MASK);
        assert_eq!(bits, 0x0387_0001);
    }

    #[test]
    fn to_bits_and_from_bits_round_trip_over_the_whole_key_domain() {
        let mut admitted = 0;
        for key in word_keys().into_iter().chain(non_word_keys()) {
            let letter = Letter {
                key,
                ..Letter::default()
            };
            match letter.to_bits() {
                Some(bits) => {
                    admitted += 1;
                    assert!(is_word_key(key), "{key:?} must not be admitted");
                    assert_eq!(Letter::from_bits(bits), Some(letter), "{key:?}");
                }
                None => assert!(
                    !is_word_key(key),
                    "{key:?} is a word key and must round-trip"
                ),
            }
        }
        assert_eq!(admitted, 36, "26 letters and 10 digits");

        let refused = non_word_keys()
            .into_iter()
            .filter(|key| {
                Letter {
                    key: *key,
                    ..Letter::default()
                }
                .to_bits()
                .is_none()
            })
            .count();
        assert_eq!(refused, 23 + 65_536, "named non-word keys plus every Other");
    }

    #[test]
    fn marks_round_trip_through_their_masks() {
        for mark in [
            Mark::None,
            Mark::Sac,
            Mark::Huyen,
            Mark::Hoi,
            Mark::Nga,
            Mark::Nang,
        ] {
            assert_eq!(Mark::from_bits(mark.to_bits()), Some(mark));
        }
        assert_eq!(Mark::from_bits(0x0018_0000), None, "two marks at once");
        assert_eq!(Mark::from_bits(0x0004_0000), None, "not a mark bit");
    }

    #[test]
    fn from_bits_rejects_bits_outside_the_layout() {
        assert_eq!(
            Letter::from_bits(0),
            Some(Letter {
                key: Key::A,
                ..Letter::default()
            })
        );
        assert_eq!(Letter::from_bits(1 << 26), None, "bit 26 is not modelled");
        assert_eq!(Letter::from_bits(u32::MAX), None);
        assert_eq!(
            Letter::from_bits(10),
            None,
            "10 is not a macOS word keycode"
        );
        assert_eq!(Letter::from_bits(0x0018_0000), None, "two marks at once");
    }

    #[test]
    fn push_below_capacity_appends() {
        let mut word = Word::new();
        assert!(word.is_empty());
        word.push(letter(1));
        word.push(letter(2));
        assert_eq!(word.len(), 2);
        assert_eq!(word.get(0), Some(letter(1)));
        assert_eq!(word.get(1), Some(letter(2)));
        assert_eq!(word.get(2), None);
        assert_eq!(word.letters(), &[letter(1), letter(2)][..]);
    }

    #[test]
    fn push_at_capacity_drops_the_front_into_overflow() {
        let mut word = Word::new();
        for n in 0..MAX_LETTERS as u16 {
            word.push(letter(n));
        }
        word.push(letter(32));

        assert_eq!(word.len(), MAX_LETTERS);
        assert_eq!(word.overflow, vec![letter(0)]);
        assert_eq!(word.get(0), Some(letter(1)));
        assert_eq!(word.get(MAX_LETTERS - 1), Some(letter(32)));
    }

    #[test]
    fn pop_with_overflow_right_shifts_the_last_dropped_letter_back_in() {
        let mut word = Word::new();
        for n in 0..(MAX_LETTERS as u16 + 2) {
            word.push(letter(n));
        }
        assert_eq!(word.len(), MAX_LETTERS);
        assert_eq!(word.overflow, vec![letter(0), letter(1)]);
        assert_eq!(word.get(0), Some(letter(2)));

        word.pop();
        assert_eq!(word.len(), MAX_LETTERS, "a refill keeps the buffer full");
        assert_eq!(word.overflow, vec![letter(0)]);
        assert_eq!(word.get(0), Some(letter(1)));
        assert_eq!(word.get(MAX_LETTERS - 1), Some(letter(32)));

        word.pop();
        assert_eq!(word.len(), MAX_LETTERS);
        assert!(word.overflow.is_empty());
        assert_eq!(word.get(0), Some(letter(0)));
        assert_eq!(word.get(MAX_LETTERS - 1), Some(letter(31)));

        word.pop();
        assert_eq!(word.len(), MAX_LETTERS - 1);
        assert_eq!(word.get(MAX_LETTERS - 1), None);
    }

    #[test]
    fn pop_on_an_empty_word_is_a_no_op() {
        let mut word = Word::new();
        word.pop();
        assert_eq!(word.len(), 0);
    }

    #[test]
    fn set_writes_without_growing_len_and_ignores_out_of_range() {
        let mut word = Word::new();
        word.push(letter(1));
        word.set(0, letter(9));
        assert_eq!(word.len(), 1);
        assert_eq!(word.get(0), Some(letter(9)));

        word.set(MAX_LETTERS, letter(2));
        word.set(usize::MAX, letter(3));
        assert_eq!(word.len(), 1);
    }

    #[test]
    fn clear_resets_the_word_and_its_overflow() {
        let mut word = Word::new();
        for n in 0..(MAX_LETTERS as u16 + 1) {
            word.push(letter(n));
        }
        assert!(!word.overflow.is_empty());
        word.clear();
        assert_eq!(word.len(), 0);
        assert!(word.overflow.is_empty());
        assert_eq!(word.get(0), None);
        assert!(word.letters().is_empty());
    }
}
