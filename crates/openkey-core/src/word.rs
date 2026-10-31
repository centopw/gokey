//! Typed word model — the replacement for upstream's `Uint32 TypingWord[MAX_BUFF]`.
//!
//! Upstream packs one key into one `Uint32` and reads it back with macros
//! (`CHR(i)`, `MARK_MASK`, ...; `DataType.h:92-122`). A `Vec<char>` cannot hold
//! that pending mark/tone state, which is why the scaffold could not support the
//! port (`docs/adr/0002-crate-graph-core-api-adapter-contract.md`, D3).
//!
//! [`Letter::to_bits`]/[`Letter::from_bits`] reproduce the C layout so a port
//! decision can be checked against the C source line it came from. They are a
//! reasoning aid, **not an oracle**: in C the low 16 bits hold an OS keycode
//! (`KEY_A` is `0` on macOS and `0x41` on Windows), and this crate must not know
//! platform keycodes at all. Parity compares rendered text, never bits.

use crate::event::Key;

/// Upstream `MAX_BUFF` (`DataType.h:19`): the word cap, and also the output cap.
pub const MAX_LETTERS: usize = 32;

// Upstream masks, `DataType.h:92-122`, kept verbatim so the port is reviewable
// against the C file. The engine reads none of these directly.
const CAPS_MASK: u32 = 0x0001_0000;
/// `^` on `a e o` (`TONE_MASK`).
const TONE_MASK: u32 = 0x0002_0000;
/// `w` on `a o u` (`TONEW_MASK`).
const TONEW_MASK: u32 = 0x0004_0000;
const MARK1_MASK: u32 = 0x0008_0000;
const MARK2_MASK: u32 = 0x0010_0000;
const MARK3_MASK: u32 = 0x0020_0000;
const MARK4_MASK: u32 = 0x0040_0000;
const MARK5_MASK: u32 = 0x0080_0000;
const MARK_MASK: u32 = 0x00F8_0000;
const CHAR_MASK: u32 = 0x0000_FFFF;
const STANDALONE_MASK: u32 = 0x0100_0000;
const CHAR_CODE_MASK: u32 = 0x0200_0000;

/// Everything upstream can store in a word slot. Bits outside this set (e.g.
/// `PURE_CHARACTER_MASK`, `0x8000_0000`) belong to output characters or table
/// entries, never to a [`Letter`], so [`Letter::from_bits`] rejects them.
const KNOWN_MASK: u32 =
    CHAR_MASK | CAPS_MASK | TONE_MASK | TONEW_MASK | MARK_MASK | STANDALONE_MASK | CHAR_CODE_MASK;

/// The five Vietnamese tone marks, `DataType.h:96-108`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Mark {
    #[default]
    None,
    /// Dấu sắc — `á`.
    Sac,
    /// Dấu huyền — `à`.
    Huyen,
    /// Dấu hỏi — `ả`.
    Hoi,
    /// Dấu ngã — `ã`.
    Nga,
    /// Dấu nặng — `ạ`.
    Nang,
}

impl Mark {
    /// The `MARK*_MASK` bit for this mark, in the C layout.
    fn mask(self) -> u32 {
        match self {
            Self::None => 0,
            Self::Sac => MARK1_MASK,
            Self::Huyen => MARK2_MASK,
            Self::Hoi => MARK3_MASK,
            Self::Nga => MARK4_MASK,
            Self::Nang => MARK5_MASK,
        }
    }

    /// Inverse of [`Mark::mask`] over the `MARK_MASK` field. `None` for a value
    /// that sets no mark bit or more than one.
    fn from_mask(mask: u32) -> Option<Self> {
        let mark = match mask {
            0 => Self::None,
            MARK1_MASK => Self::Sac,
            MARK2_MASK => Self::Huyen,
            MARK3_MASK => Self::Hoi,
            MARK4_MASK => Self::Nga,
            MARK5_MASK => Self::Nang,
            _ => return None,
        };
        Some(mark)
    }
}

/// One letter of the word being typed.
///
/// `circ` and `w_mod` are two booleans rather than one enum because upstream
/// sets them independently and gives `TONE` precedence when rendering
/// (`insertAOE` clears `TONEW`, `insertW` clears `TONE`; `getCharacterCode`
/// checks `TONE` first). Two flags keep that ordering explicit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Letter {
    /// Base identity (upstream `CHAR_MASK`), semantic rather than an OS keycode.
    pub key: Key,
    /// Caps applied when the key was typed (upstream `CAPS_MASK`).
    pub caps: bool,
    /// `^` — `â ê ô` (upstream `TONE_MASK`).
    pub circ: bool,
    /// `w` — `ă ơ ư` (upstream `TONEW_MASK`).
    pub w_mod: bool,
    /// One of the five tone marks (upstream `MARK_MASK`).
    pub mark: Mark,
    /// Built by a standalone key press, e.g. `w` typed on its own
    /// (upstream `STANDALONE_MASK`).
    pub standalone: bool,
    /// Already resolved to a charset code (upstream `CHAR_CODE_MASK`).
    pub resolved: bool,
}

impl Default for Letter {
    /// A blank slot. `Word` only exposes slots below its length, so a defaulted
    /// letter is never observed through the public API.
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
    /// Pack into upstream's `Uint32` layout. For reasoning and debugging only;
    /// see the module docs on why bit equality is not a parity oracle.
    pub fn to_bits(self) -> u32 {
        u32::from(key_code(self.key))
            | if self.caps { CAPS_MASK } else { 0 }
            | if self.circ { TONE_MASK } else { 0 }
            | if self.w_mod { TONEW_MASK } else { 0 }
            | self.mark.mask()
            | if self.standalone { STANDALONE_MASK } else { 0 }
            | if self.resolved { CHAR_CODE_MASK } else { 0 }
    }

    /// Unpack from upstream's `Uint32` layout.
    ///
    /// Returns `None` when a bit outside [`KNOWN_MASK`] is set, or when the mark
    /// field is not one of the six legal values. Every value produced by
    /// [`Letter::to_bits`] round-trips through here.
    pub fn from_bits(bits: u32) -> Option<Self> {
        if bits & !KNOWN_MASK != 0 {
            return None;
        }
        let mark = Mark::from_mask(bits & MARK_MASK)?;
        Some(Self {
            key: key_from_code((bits & CHAR_MASK) as u16),
            caps: bits & CAPS_MASK != 0,
            circ: bits & TONE_MASK != 0,
            w_mod: bits & TONEW_MASK != 0,
            mark,
            standalone: bits & STANDALONE_MASK != 0,
            resolved: bits & CHAR_CODE_MASK != 0,
        })
    }
}

/// Rust-internal key code for the low 16 bits of [`Letter::to_bits`].
///
/// Deliberately **not** an OS keycode: `platforms/*.h` are per-OS and must never
/// reach this crate. Printable keys use their ASCII code so the packed value is
/// readable in a debugger; keys without one use `0xE0xx`. The mapping is stable
/// but arbitrary — it exists to make the C layout inspectable, not to be
/// compared with `KEY_*` constants.
fn key_code(key: Key) -> u16 {
    match key {
        Key::A => 0x41,
        Key::B => 0x42,
        Key::C => 0x43,
        Key::D => 0x44,
        Key::E => 0x45,
        Key::F => 0x46,
        Key::G => 0x47,
        Key::H => 0x48,
        Key::I => 0x49,
        Key::J => 0x4A,
        Key::K => 0x4B,
        Key::L => 0x4C,
        Key::M => 0x4D,
        Key::N => 0x4E,
        Key::O => 0x4F,
        Key::P => 0x50,
        Key::Q => 0x51,
        Key::R => 0x52,
        Key::S => 0x53,
        Key::T => 0x54,
        Key::U => 0x55,
        Key::V => 0x56,
        Key::W => 0x57,
        Key::X => 0x58,
        Key::Y => 0x59,
        Key::Z => 0x5A,
        // `Digit` is defined as 0-9 by `event.rs`; anything else is out of the
        // domain and aliases codes above the digit row.
        Key::Digit(d) => {
            debug_assert!(d <= 9, "Key::Digit is 0-9");
            0x30 + d as u16
        }
        Key::Space => 0x20,
        Key::Enter => 0x0D,
        Key::Tab => 0x09,
        Key::Escape => 0x1B,
        Key::Backspace => 0x08,
        Key::Delete => 0x7F,
        Key::ArrowLeft => 0xE000,
        Key::ArrowRight => 0xE001,
        Key::ArrowUp => 0xE002,
        Key::ArrowDown => 0xE003,
        Key::LeftBracket => 0x5B,
        Key::RightBracket => 0x5D,
        Key::Semicolon => 0x3B,
        Key::Quote => 0x27,
        Key::Comma => 0x2C,
        Key::Dot => 0x2E,
        Key::Slash => 0x2F,
        Key::BackSlash => 0x5C,
        Key::Minus => 0x2D,
        Key::Equals => 0x3D,
        Key::Backquote => 0x60,
        // Passed through unchanged: a code with no named variant keeps its
        // value, which is what makes the 16-bit space total.
        Key::Other(code) => code,
    }
}

/// Inverse of [`key_code`]. Total: any code without a named variant becomes
/// `Key::Other(code)`, so `key_code(key_from_code(c)) == c` for all `u16`.
fn key_from_code(code: u16) -> Key {
    match code {
        0x41 => Key::A,
        0x42 => Key::B,
        0x43 => Key::C,
        0x44 => Key::D,
        0x45 => Key::E,
        0x46 => Key::F,
        0x47 => Key::G,
        0x48 => Key::H,
        0x49 => Key::I,
        0x4A => Key::J,
        0x4B => Key::K,
        0x4C => Key::L,
        0x4D => Key::M,
        0x4E => Key::N,
        0x4F => Key::O,
        0x50 => Key::P,
        0x51 => Key::Q,
        0x52 => Key::R,
        0x53 => Key::S,
        0x54 => Key::T,
        0x55 => Key::U,
        0x56 => Key::V,
        0x57 => Key::W,
        0x58 => Key::X,
        0x59 => Key::Y,
        0x5A => Key::Z,
        0x30..=0x39 => Key::Digit((code - 0x30) as u8),
        0x20 => Key::Space,
        0x0D => Key::Enter,
        0x09 => Key::Tab,
        0x1B => Key::Escape,
        0x08 => Key::Backspace,
        0x7F => Key::Delete,
        0xE000 => Key::ArrowLeft,
        0xE001 => Key::ArrowRight,
        0xE002 => Key::ArrowUp,
        0xE003 => Key::ArrowDown,
        0x5B => Key::LeftBracket,
        0x5D => Key::RightBracket,
        0x3B => Key::Semicolon,
        0x27 => Key::Quote,
        0x2C => Key::Comma,
        0x2E => Key::Dot,
        0x2F => Key::Slash,
        0x5C => Key::BackSlash,
        0x2D => Key::Minus,
        0x3D => Key::Equals,
        0x60 => Key::Backquote,
        other => Key::Other(other),
    }
}

/// Which call site is scanning, i.e. upstream's `forGrammar` argument to
/// `findAndCalculateVowel` (`Engine.cpp:560`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scan {
    /// `forGrammar == false` — mark placement: stop at `gi`/`qu` so the `i`/`u`
    /// is not treated as part of the vowel run.
    Marks,
    /// `forGrammar == true` — grammar repositioning: count the whole run.
    Grammar,
}

/// Upstream's `VSI`, `VEI` and `vowelCount` after `findAndCalculateVowel`.
///
/// `VWSM` (the letter a mark lands on) is deliberately absent: it is decided by
/// `handleOldMark`/`handleModernMark`, which consult the end-consonant table.
/// Tables land in C-002 under the "no table ported by eye" rule
/// (`docs/m0/PORTING-MAP.md` §4.7 criterion 6), so the slot is not invented here.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VowelSpan {
    pub start: u8,
    pub end: u8,
    pub count: u8,
}

/// The word currently being typed: upstream `TypingWord[MAX_BUFF]`, `_index`
/// and `_longWordHelper` (`Engine.cpp:80-85`).
///
/// Only this type indexes `letters`. Everything else reads through [`Word::get`],
/// [`Word::span`] and [`Word::tail_matches`]; `scripts/ci/check-core-purity.py`
/// enforces that.
#[derive(Clone, Debug)]
pub struct Word {
    letters: [Letter; MAX_LETTERS],
    len: u8,
    /// Letters dropped off the front when the word passed [`MAX_LETTERS`].
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

    /// Overwrite slot `i`. Upstream `setKeyData`; the caller owns the index.
    ///
    /// # Panics
    ///
    /// If `i >= MAX_LETTERS`. Upstream would write out of bounds.
    pub fn set(&mut self, i: usize, letter: Letter) {
        assert!(
            i < MAX_LETTERS,
            "letter index {i} out of range: a word holds at most {MAX_LETTERS}"
        );
        self.letters[i] = letter;
    }

    /// Append a letter, dropping the front one into the overflow queue when the
    /// word is full (upstream `insertKey`, `Engine.cpp:350-356`).
    ///
    /// The overflow queue is unbounded within a word, exactly like
    /// `_longWordHelper`; it only clears on [`Word::clear`].
    pub fn push(&mut self, letter: Letter) {
        if self.len as usize >= MAX_LETTERS {
            self.overflow.push(self.letters[0]);
            self.letters.copy_within(1..MAX_LETTERS, 0);
            self.letters[MAX_LETTERS - 1] = letter;
        } else {
            self.letters[self.len as usize] = letter;
            self.len += 1;
        }
    }

    /// Remove the last letter, pulling the most recently dropped front letter
    /// back in when the word overflowed (upstream backspace, `Engine.cpp:1439-1447`).
    ///
    /// Returns the letter that left the visible word, or `None` on an empty word.
    pub fn pop(&mut self) -> Option<Letter> {
        if self.len == 0 {
            return None;
        }
        self.len -= 1;
        let removed = self.letters[self.len as usize];
        if let Some(restored) = self.overflow.pop() {
            self.letters.copy_within(0..MAX_LETTERS - 1, 1);
            self.letters[0] = restored;
            self.len += 1;
        }
        Some(removed)
    }

    /// Letter at `i`, or `None` past the end. The only read path into `letters`.
    pub fn get(&self, i: usize) -> Option<Letter> {
        if i < self.len as usize {
            Some(self.letters[i])
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.len as usize
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Start a new word (upstream `startNewSession`, `Engine.cpp:457`): the
    /// letters and the overflow queue both go.
    pub fn clear(&mut self) {
        self.len = 0;
        self.overflow.clear();
    }

    /// The vowel run at the end of the word — upstream `findAndCalculateVowel`
    /// (`Engine.cpp:560-585`), including the 2019-08-26 "don't count the `u` in
    /// `qu`" correction.
    pub fn span(&self, scan: Scan) -> VowelSpan {
        let mut span = VowelSpan::default();
        let mut i = self.len as isize - 1;
        while i >= 0 {
            let index = i as usize;
            let key = self.letters[index].key;
            if is_consonant(key) {
                if span.count > 0 {
                    break;
                }
            } else {
                if span.count == 0 {
                    span.end = index as u8;
                }
                if scan == Scan::Marks && index > 0 {
                    let previous = self.letters[index - 1].key;
                    if (key == Key::I && previous == Key::G)
                        || (key == Key::U && previous == Key::Q)
                    {
                        break;
                    }
                }
                span.start = index as u8;
                span.count += 1;
            }
            i -= 1;
        }
        // "don't count 'u' at 'q u' as a vowel" (Engine.cpp:583-585).
        if span.start > 0 {
            let start = span.start as usize;
            if self.letters[start].key == Key::U && self.letters[start - 1].key == Key::Q {
                span.start += 1;
                span.count -= 1;
            }
        }
        span
    }

    /// Does the tail ending at `from` equal `row` — upstream `checkCorrectVowel`
    /// (`Engine.cpp:468-499`)?
    ///
    /// `from` is **one past the end** of the tail, i.e. upstream's `_index`; it
    /// is not a start offset. Upstream's callers pass `k = _index` only because
    /// `k` is an out-parameter — the first line of the function overwrites it
    /// with `_index - 1` (`:474`), so the incoming value is never read.
    ///
    /// `mark_key` is the key applying the mark. `PORTING-MAP.md` §4.2 sketches
    /// this as `tail_matches(row, from)`, but the `C`/`T` rule below reads the
    /// mark key (`:485`), and §4.7's third criterion requires that rule, so the
    /// parameter cannot be omitted without dropping it. Rows are `&[Key]` for
    /// the same reason `RowFlags` (`PORTING-MAP.md` R8) is not here yet: rows
    /// carry the per-ending flags only inside the tables, which are C-002.
    ///
    /// Reproduces upstream's four checks, in its order:
    ///
    /// 1. a word whose last two letters are `q u` never matches (`:470-473`);
    /// 2. `row` is matched backwards from `from - 1`, letter by letter;
    /// 3. an ending whose second letter is `c`/`t`, or whose third is `t`,
    ///    rejects the `f`/`x`/`r` marks (`:485-491`);
    /// 4. a doubled letter at the join (`aa`, `nn`) never matches (`:496-499`).
    ///
    /// Returns `false` rather than reading before the start when `row` is longer
    /// than `from`, or when `from` is past the end of the word. Upstream relies
    /// on its callers' `_index >= charset[i].size()` guard (`:1109`, `:1155`);
    /// a slice API has no caller to rely on.
    pub fn tail_matches(&self, row: &[Key], from: usize, mark_key: Key) -> bool {
        if row.is_empty() || from > self.len() {
            return false;
        }
        // 1. "ignore 'qu' case" (Engine.cpp:470-473).
        if from >= 2 && self.letters[from - 1].key == Key::U && self.letters[from - 2].key == Key::Q
        {
            return false;
        }
        if row.len() > from {
            return false;
        }
        // 2. Walk the row backwards over the word (:476-482).
        let mut k = from as isize - 1;
        for &key in row.iter().rev() {
            if self.letters[k as usize].key != key {
                return false;
            }
            k -= 1;
            if k < 0 {
                break;
            }
        }
        // 3. "limit mark for end consonant: C, T" (:485-491).
        if row.len() > 1 && matches!(mark_key, Key::F | Key::X | Key::R) {
            if row[1] == Key::C || row[1] == Key::T {
                return false;
            }
            if row.len() > 2 && row[2] == Key::T {
                return false;
            }
        }
        // 4. A doubled letter at the join, e.g. `aac` (:496-499).
        if k >= 0 {
            let before = k as usize;
            if self.letters[before].key == self.letters[before + 1].key {
                return false;
            }
        }
        true
    }
}

/// Upstream `IS_CONSONANT` (`DataType.h:130`): everything that is not one of the
/// six vowels, including punctuation and digits.
fn is_consonant(key: Key) -> bool {
    !matches!(key, Key::A | Key::E | Key::U | Key::Y | Key::I | Key::O)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn letter(key: Key) -> Letter {
        Letter {
            key,
            ..Letter::default()
        }
    }

    /// Every named variant, so a new one cannot be silently left out of
    /// `key_code` without this list (and the length assertion) changing.
    fn all_named_keys() -> Vec<Key> {
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
        ];
        keys.extend((0..=9).map(Key::Digit));
        keys
    }

    #[test]
    fn code_to_key_to_code_is_total_over_the_16_bit_space() {
        // The C layout has exactly one 16-bit code space; every value in it has
        // a key and comes back unchanged. This is what keeps `Other(n)` usable.
        for code in 0..=u16::MAX {
            assert_eq!(
                key_code(key_from_code(code)),
                code,
                "code {code:#06x} did not survive key_from_code/key_code"
            );
        }
    }

    #[test]
    fn every_named_key_round_trips() {
        for key in all_named_keys() {
            assert_eq!(
                key_from_code(key_code(key)),
                key,
                "{key:?} did not round-trip through its key code"
            );
        }
    }

    #[test]
    fn named_codes_are_distinct() {
        let mut codes: Vec<u16> = all_named_keys().into_iter().map(key_code).collect();
        codes.sort_unstable();
        let count = codes.len();
        codes.dedup();
        assert_eq!(codes.len(), count, "two named keys share a key code");
    }

    #[test]
    fn an_unnamed_code_stays_other() {
        // 0x0100 is inside CHAR_MASK but has no named variant, so it survives as
        // `Other` rather than collapsing to a neighbour.
        assert_eq!(key_from_code(0x0100), Key::Other(0x0100));
        assert_eq!(key_from_code(0x4041), Key::Other(0x4041));
    }

    #[test]
    fn a_named_code_spelled_as_other_aliases_the_named_key() {
        // One 16-bit code space means `Other(0x41)` *is* `Key::A`, exactly as a
        // named variant is an alias for a code in C. Documented, not accidental.
        assert_eq!(key_from_code(0x41), Key::A);
        assert_eq!(letter(Key::Other(0x41)).to_bits(), letter(Key::A).to_bits());
    }

    #[test]
    fn letter_bits_match_the_c_layout() {
        // DataType.h:92-122, one line of the table per assertion.
        let a = letter(Key::A);
        assert_eq!(a.to_bits(), 0x41);
        assert_eq!(Letter { caps: true, ..a }.to_bits(), 0x41 | CAPS_MASK);
        assert_eq!(Letter { circ: true, ..a }.to_bits(), 0x41 | TONE_MASK);
        assert_eq!(Letter { w_mod: true, ..a }.to_bits(), 0x41 | TONEW_MASK);
        assert_eq!(
            Letter {
                standalone: true,
                ..a
            }
            .to_bits(),
            0x41 | STANDALONE_MASK
        );
        assert_eq!(
            Letter {
                resolved: true,
                ..a
            }
            .to_bits(),
            0x41 | CHAR_CODE_MASK
        );
        assert_eq!(
            Letter {
                mark: Mark::Nang,
                ..a
            }
            .to_bits(),
            0x41 | MARK5_MASK
        );
    }

    #[test]
    fn every_mark_is_representable_and_round_trips() {
        // The table test C-001 asks for: enumerate DataType.h:96-122 in full —
        // 5 tone marks x `^` x `w` x caps x standalone x resolved.
        let marks = [
            (Mark::None, 0u32),
            (Mark::Sac, MARK1_MASK),
            (Mark::Huyen, MARK2_MASK),
            (Mark::Hoi, MARK3_MASK),
            (Mark::Nga, MARK4_MASK),
            (Mark::Nang, MARK5_MASK),
        ];
        let mut seen = 0usize;
        for (mark, mask) in marks {
            assert_eq!(mark.mask(), mask, "{mark:?} has the wrong MARK_MASK bit");
            for circ in [false, true] {
                for w_mod in [false, true] {
                    for caps in [false, true] {
                        for standalone in [false, true] {
                            for resolved in [false, true] {
                                let original = Letter {
                                    key: Key::A,
                                    caps,
                                    circ,
                                    w_mod,
                                    mark,
                                    standalone,
                                    resolved,
                                };
                                let bits = original.to_bits();
                                assert_eq!(bits & MARK_MASK, mask);
                                assert_eq!(
                                    Letter::from_bits(bits),
                                    Some(original),
                                    "letter lost information: {original:?}"
                                );
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
    fn every_mark_round_trips_on_every_key() {
        let marks = [
            Mark::None,
            Mark::Sac,
            Mark::Huyen,
            Mark::Hoi,
            Mark::Nga,
            Mark::Nang,
        ];
        for key in all_named_keys() {
            for mark in marks {
                let original = Letter {
                    mark,
                    ..letter(key)
                };
                assert_eq!(Letter::from_bits(original.to_bits()), Some(original));
            }
        }
    }

    #[test]
    fn from_bits_rejects_what_is_not_a_letter() {
        // Two tone marks at once is not a legal MARK_MASK value.
        assert_eq!(Letter::from_bits(MARK1_MASK | MARK2_MASK), None);
        // PURE_CHARACTER_MASK belongs to output characters, not word slots.
        assert_eq!(Letter::from_bits(0x8000_0000), None);
        assert_eq!(Letter::from_bits(0x0400_0000), None);
    }

    fn a_letter(n: u16) -> Letter {
        letter(Key::Other(n))
    }

    #[test]
    fn push_at_capacity_drops_the_front_letter_into_overflow() {
        // Engine.cpp:350-356.
        let mut word = Word::new();
        for n in 0..MAX_LETTERS as u16 {
            word.push(a_letter(n));
        }
        word.push(a_letter(MAX_LETTERS as u16));

        assert_eq!(word.len(), MAX_LETTERS);
        assert_eq!(word.get(0), Some(a_letter(1)));
        assert_eq!(
            word.get(MAX_LETTERS - 1),
            Some(a_letter(MAX_LETTERS as u16))
        );
        assert_eq!(word.overflow, vec![a_letter(0)]);
    }

    #[test]
    fn pop_with_overflow_shifts_the_dropped_letter_back_in() {
        // Engine.cpp:1439-1447.
        let mut word = Word::new();
        for n in 0..=MAX_LETTERS as u16 {
            word.push(a_letter(n));
        }
        assert_eq!(word.overflow.len(), 1);

        let removed = word.pop();
        assert_eq!(removed, Some(a_letter(MAX_LETTERS as u16)));
        assert_eq!(word.len(), MAX_LETTERS);
        assert_eq!(word.get(0), Some(a_letter(0)));
        assert_eq!(word.get(1), Some(a_letter(1)));
        assert_eq!(
            word.get(MAX_LETTERS - 1),
            Some(a_letter(MAX_LETTERS as u16 - 1))
        );
        assert!(word.overflow.is_empty());

        // Without overflow the word simply shrinks.
        let removed = word.pop();
        assert_eq!(removed, Some(a_letter(MAX_LETTERS as u16 - 1)));
        assert_eq!(word.len(), MAX_LETTERS - 1);
    }

    #[test]
    fn overflow_round_trips_through_a_full_drain() {
        let total = MAX_LETTERS + 8;
        let mut word = Word::new();
        for n in 0..total as u16 {
            word.push(a_letter(n));
        }
        // The visible word is the last MAX_LETTERS letters, newest last.
        for i in 0..MAX_LETTERS {
            assert_eq!(
                word.get(i),
                Some(a_letter((total - MAX_LETTERS + i) as u16))
            );
        }

        for n in (0..total as u16).rev() {
            assert_eq!(word.pop(), Some(a_letter(n)), "drain order broke at {n}");
        }
        assert_eq!(word.len(), 0);
        assert!(word.overflow.is_empty());
        assert_eq!(word.pop(), None, "popping an empty word must not panic");
    }

    #[test]
    fn clear_drops_letters_and_overflow() {
        let mut word = Word::new();
        for n in 0..(MAX_LETTERS + 4) as u16 {
            word.push(a_letter(n));
        }
        word.clear();
        assert_eq!(word.len(), 0);
        assert!(word.is_empty());
        assert!(word.overflow.is_empty());
        assert_eq!(word.get(0), None);
    }

    #[test]
    fn get_is_bounds_checked() {
        let mut word = Word::new();
        word.push(a_letter(7));
        assert_eq!(word.get(0), Some(a_letter(7)));
        assert_eq!(word.get(1), None);
        assert_eq!(word.get(usize::MAX), None);
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn set_past_the_end_panics_instead_of_writing_out_of_bounds() {
        let mut word = Word::new();
        word.set(MAX_LETTERS, a_letter(1));
    }

    fn word_of(keys: &[Key]) -> Word {
        let mut word = Word::new();
        for &key in keys {
            word.push(letter(key));
        }
        word
    }

    fn span_of(keys: &[Key], scan: Scan) -> VowelSpan {
        word_of(keys).span(scan)
    }

    #[test]
    fn span_counts_a_trailing_vowel_run() {
        // Engine.cpp:560-585, the plain case: consonants after the run stop it.
        assert_eq!(
            span_of(&[Key::V, Key::I, Key::E, Key::E, Key::T], Scan::Marks),
            VowelSpan {
                start: 1,
                end: 3,
                count: 3
            }
        );
        assert_eq!(
            span_of(&[Key::U, Key::O, Key::I], Scan::Marks),
            VowelSpan {
                start: 0,
                end: 2,
                count: 3
            }
        );
        assert_eq!(span_of(&[], Scan::Marks), VowelSpan::default());
        assert_eq!(
            span_of(&[Key::T, Key::H, Key::U], Scan::Marks),
            VowelSpan {
                start: 2,
                end: 2,
                count: 1
            }
        );
    }

    #[test]
    fn span_stops_at_gi_and_qu_when_placing_marks() {
        // `gi`: the `i` is not part of the run for mark placement...
        assert_eq!(
            span_of(&[Key::G, Key::I, Key::A], Scan::Marks),
            VowelSpan {
                start: 2,
                end: 2,
                count: 1
            }
        );
        // ...but grammar scanning counts it.
        assert_eq!(
            span_of(&[Key::G, Key::I, Key::A], Scan::Grammar),
            VowelSpan {
                start: 1,
                end: 2,
                count: 2
            }
        );
        // `qui`: same, and the `u` never enters the mark span.
        assert_eq!(
            span_of(&[Key::Q, Key::U, Key::I], Scan::Marks),
            VowelSpan {
                start: 2,
                end: 2,
                count: 1
            }
        );
        assert_eq!(
            span_of(&[Key::Q, Key::U, Key::I], Scan::Grammar),
            VowelSpan {
                start: 2,
                end: 2,
                count: 1
            }
        );
    }

    #[test]
    fn span_does_not_count_the_u_after_q() {
        // The 2019-08-26 correction, applied after the scan in both modes.
        assert_eq!(
            span_of(&[Key::Q, Key::U], Scan::Grammar),
            VowelSpan {
                start: 2,
                end: 1,
                count: 0
            }
        );
        assert_eq!(
            span_of(&[Key::Q, Key::U], Scan::Marks),
            VowelSpan {
                start: 0,
                end: 1,
                count: 0
            }
        );
        // A `u` that follows a `q` deeper in the word is corrected the same way.
        assert_eq!(
            span_of(&[Key::T, Key::Q, Key::U, Key::A, Key::N], Scan::Grammar),
            VowelSpan {
                start: 3,
                end: 3,
                count: 1
            }
        );
    }

    #[test]
    fn tail_matches_the_ending_letter_by_letter() {
        // Engine.cpp:476-482: the row is walked backwards from `from - 1`.
        let ac = word_of(&[Key::A, Key::C]);
        assert!(ac.tail_matches(&[Key::A, Key::C], 2, Key::S));
        // A longer word matches on its tail only.
        let hac = word_of(&[Key::H, Key::A, Key::C]);
        assert!(hac.tail_matches(&[Key::A, Key::C], 3, Key::S));
        // One wrong letter anywhere fails.
        assert!(!ac.tail_matches(&[Key::A, Key::T], 2, Key::S));
        assert!(!ac.tail_matches(&[Key::O, Key::C], 2, Key::S));
        // A one-letter row matches the last letter.
        assert!(ac.tail_matches(&[Key::C], 2, Key::S));
        // An empty row, a row longer than the tail, and a `from` past the end
        // are refused rather than read backwards out of bounds.
        assert!(!ac.tail_matches(&[], 2, Key::S));
        assert!(!ac.tail_matches(&[Key::T, Key::A, Key::C], 2, Key::S));
        assert!(!ac.tail_matches(&[Key::C], 3, Key::S));
    }

    #[test]
    fn tail_matches_refuses_the_qu_ending() {
        // Engine.cpp:470-473: "ignore 'qu' case", checked before the row is
        // even looked at, so a row that would otherwise match still fails.
        let qu = word_of(&[Key::Q, Key::U]);
        assert!(!qu.tail_matches(&[Key::U], 2, Key::S));
        // `qu` earlier in the word does not affect the real ending.
        let quan = word_of(&[Key::Q, Key::U, Key::A, Key::N]);
        assert!(quan.tail_matches(&[Key::A, Key::N], 4, Key::S));
    }

    #[test]
    fn tail_matches_restricts_f_x_r_before_c_and_t() {
        // Engine.cpp:485-491: huyền/ngã/hỏi do not sit on a c/t stop final;
        // sắc and nặng do. The same ending answers differently per mark key,
        // which is why the key is a parameter and not table state.
        let ac = word_of(&[Key::A, Key::C]);
        let at = word_of(&[Key::A, Key::T]);
        for mark in [Key::F, Key::X, Key::R] {
            assert!(!ac.tail_matches(&[Key::A, Key::C], 2, mark), "{mark:?}");
            assert!(!at.tail_matches(&[Key::A, Key::T], 2, mark), "{mark:?}");
        }
        for mark in [Key::S, Key::J] {
            assert!(ac.tail_matches(&[Key::A, Key::C], 2, mark), "{mark:?}");
            assert!(at.tail_matches(&[Key::A, Key::T], 2, mark), "{mark:?}");
        }
        // The third-letter rule (`row[2] == T`) fires when the second letter is
        // neither C nor T. Rows of this shape come from C-002's tables; the
        // rule itself is what C-001 pins.
        let ant = word_of(&[Key::A, Key::N, Key::T]);
        assert!(!ant.tail_matches(&[Key::A, Key::N, Key::T], 3, Key::F));
        assert!(ant.tail_matches(&[Key::A, Key::N, Key::T], 3, Key::S));
        // A one-letter row is not restricted: upstream guards the rule with
        // `charset[i].size() > 1`, so `c` alone takes any mark. A quirk,
        // preserved because parity preserves quirks (PORTING-MAP §4.8).
        let c = word_of(&[Key::C]);
        assert!(c.tail_matches(&[Key::C], 1, Key::F));
    }

    #[test]
    fn tail_matches_refuses_a_doubled_letter_at_the_join() {
        // Engine.cpp:496-499: the letter before the match may not repeat the
        // match's first letter, so "aa"+"c" is not an ending and "ba"+"c" is.
        let aac = word_of(&[Key::A, Key::A, Key::C]);
        assert!(!aac.tail_matches(&[Key::A, Key::C], 3, Key::S));
        let bac = word_of(&[Key::B, Key::A, Key::C]);
        assert!(bac.tail_matches(&[Key::A, Key::C], 3, Key::S));
        // The guard reads only the letter before the match, so a doubled
        // letter inside the row is fine.
        let baa = word_of(&[Key::B, Key::A, Key::A]);
        assert!(baa.tail_matches(&[Key::A, Key::A], 3, Key::S));
    }
}
