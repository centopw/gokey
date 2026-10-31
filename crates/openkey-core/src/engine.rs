//! Keystroke state machine — the port of upstream `Engine.cpp` (~52 KB).
//!
//! Keep this module pure: no I/O, no globals, no OS calls. Every public
//! function must be deterministic for a given (`Options`, buffer, `KeyEvent`)
//! triple so the parity harness can diff it against the reference C++ build.
//!
//! Upstream encodes tone/mark/caps flags in the high bits of `Uint32 charData`
//! (`CAPS_MASK`, `TONE_MASK`, `MARK1_MASK`, ...). The port keeps a typed word
//! model instead ([`Word`]) and writes the same bits only at the encoding edge
//! (`charset`), which is where upstream's `GET` runs too.
//!
//! # Fidelity
//!
//! The body of this file is a translation, not a redesign: names, order of
//! checks, and quirks follow `Engine.cpp` at the pin (89c2fd3b, macOS normative
//! — DL-026). Each function carries the line range it ports. Upstream bugs that
//! are observable in output are reproduced rather than fixed (`insertAOE`'s
//! `IS_KEY_D` clear, `handleModernMark`'s self-comparing rule 3.1 clauses,
//! `hBPC`'s byte wrap). Two rules are deliberately *not* reproduced because
//! they cannot be modelled without the macro table:
//!
//! - `findMacro` is constant false here, so `vReplaceMaro` is unreachable and
//!   `hMacroKey`/`hMacroData` bookkeeping is omitted. Macro expansion is a
//!   separate task.
//! - `upperCaseFirstCharacter`'s `hMacroKey[0] |= CAPS_MASK`
//!   (`Engine.cpp:1050-1051`) indexes an empty vector upstream; with no macro
//!   state there is nothing to mirror.
//!
//! Three more deviations are guarded instead of copied, because the C versions
//! read out of bounds or depend on undefined behaviour:
//!
//! - `CHR(_index - 1)` in `checkForStandaloneChar` at `_index == 0`
//!   (`Engine.cpp:996`) reads `TypingWord[-1]`; here the branch is skipped.
//! - `CHR(_index)` in `checkSpelling`'s first-vowel probe can read
//!   `TypingWord[32]` in a full buffer; here it reads the last slot as 0.
//! - A forward delete (`Key::Delete`) has no macOS constant at the pin and is
//!   not in `_breakCode`; it is modelled as a word break instead of guessed.
//!
//! Only the Telex branch is ported (C-002). VNI (C-003) and Simple Telex 1/2
//! (C-004) panic with `not yet implemented`, which the parity replay reports as
//! `engine-pending` rather than as a disagreement.

use crate::charset;
use crate::event::{Action, Edit, Encoded, Ext, KeyEvent, Outcome, Phase};
use crate::options::{InputMethod, Options};
use crate::tables::*;
use crate::word::{MAX_LETTERS, Scan};

pub use crate::word::{Letter, Mark, Word};

// `HoolCodeState` (`DataType.h:57-66`). `vBreakWord` (2) is never assigned
// upstream, so it has no constant here; `outcome` names it at the one place
// that would have to handle it.
const DO_NOTHING: u8 = 0;
const WILL_PROCESS: u8 = 1;
const RESTORE: u8 = 3;
const REPLACE_MACRO: u8 = 4;
const RESTORE_AND_START_NEW_SESSION: u8 = 5;

/// `vKeyEvent` (`DataType.h:24-27`). The engine only distinguishes them for
/// word breaks (`isWordBreak`, `Engine.cpp:145-154`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
    Keyboard,
    Mouse,
}

/// `vKeyHookState` (`DataType.h:78-94`), minus the macro fields.
///
/// Persisted across events like the upstream global: `charData` is read past
/// `newCharCount` in the bracket branch (`Engine.cpp:1541`), so zeroing it per
/// key would not be faithful.
#[derive(Clone, Debug, Default)]
struct Hook {
    code: u8,
    backspaces: u8,
    new_chars: u8,
    ext: u8,
    char_data: [u32; MAX_LETTERS],
}

/// Per-focus-session typing state, bounded to 32 characters like upstream
/// `MAX_BUFF`.
#[derive(Debug)]
pub struct Engine {
    options: Options,
    hook: Hook,
    /// `TypingWord` + `_index`.
    word: Word,
    /// `KeyStates` + `_stateIndex`: the pre-transformation keys, for
    /// `checkRestoreIfWrongSpelling` (`Engine.cpp:92-93, 1204-1221`).
    key_states: Word,
    /// `_typingStates`: word history for delete (`Engine.cpp:86, 435-455`).
    typing_states: Vec<Vec<u32>>,
    /// `_specialChar`: standalone punctuation typed in the current word.
    special_char: Vec<u32>,
    /// `_spaceCount` (`Engine.cpp:113`).
    space_count: usize,
    /// `_upperCaseStatus` (`Engine.cpp:115`).
    upper_case_status: u8,
    /// `vCheckSpelling`, which `temp_off_spell_checking` may flip.
    check_spelling: bool,
    /// `_useSpellCheckingBefore` (`Engine.cpp:118`).
    use_spell_checking_before: bool,
    temp_disable_key: bool,
    /// `_hasHandledMacro`; kept for fidelity even though no macro can be found.
    has_handled_macro: bool,
    has_handle_quick_consonant: bool,
    will_temp_off_engine: bool,
    /// `_isCaps` for the current event (`Engine.cpp:111, 1319-1320`).
    is_caps: bool,
    /// `isChanged`; `insertW` clears it and `handleMainKey` reads it after the
    /// call, so it cannot be a local there (`Engine.cpp:100, 920-925, 1186`).
    is_changed: bool,
    /// `findAndCalculateVowel` outputs (`Engine.cpp:101-104`).
    vowel_start: u8,
    vowel_end: u8,
    vowel_count: u8,
    vowel_will_set_mark: u8,
}

impl Engine {
    pub const MAX_BUFFER: usize = crate::word::MAX_LETTERS;

    pub fn new(options: Options) -> Self {
        Self {
            check_spelling: options.check_spelling,
            use_spell_checking_before: options.check_spelling,
            options,
            hook: Hook::default(),
            word: Word::new(),
            key_states: Word::new(),
            typing_states: Vec::new(),
            special_char: Vec::new(),
            space_count: 0,
            upper_case_status: 0,
            temp_disable_key: false,
            has_handled_macro: false,
            has_handle_quick_consonant: false,
            will_temp_off_engine: false,
            is_caps: false,
            is_changed: false,
            vowel_start: 0,
            vowel_end: 0,
            vowel_count: 0,
            vowel_will_set_mark: 0,
        }
    }

    pub fn options(&self) -> &Options {
        &self.options
    }

    /// Replace the settings, as the app does when the user edits them
    /// (`vSetCheckSpelling`, `Engine.cpp:1229-1231`: the "before" snapshot
    /// follows the loaded value).
    pub fn set_options(&mut self, options: Options) {
        self.check_spelling = options.check_spelling;
        self.use_spell_checking_before = options.check_spelling;
        self.options = options;
    }

    /// Current word model, for tests and the parity harness.
    pub fn word(&self) -> &Word {
        &self.word
    }

    /// Start a new word (upstream `startNewSession`, `Engine.cpp:457-466`).
    ///
    /// This is not the adapter's session entry. Upstream ends a session through
    /// the mouse/break branch, which also clears the word history, saves or
    /// drops the standalone-punctuation queue, and honours
    /// `restoreIfWrongSpelling`. Use [`Engine::session_break`] for the adapter
    /// entry; ADR-0011 records why the two are not interchangeable.
    pub fn reset(&mut self) {
        self.start_new_session();
    }

    /// Upstream's session entry: `vKeyHandleEvent(Mouse, MouseDown, 0)`
    /// (`OpenKey.mm:147-153`).
    ///
    /// The macOS adapter calls this for mouse down and mouse drag before it
    /// returns the event, and discards the [`Outcome`]: the document has
    /// already been typed, and a mouse event produces none. It must *not* clear
    /// its own sync key here — that happens only for the double-byte charsets
    /// (`IS_DOUBLE_CODE`, `DataType.h:149`) and is adapter state.
    pub fn session_break(&mut self) -> Outcome {
        self.is_caps = false;
        self.dispatch(Source::Mouse, 0, 0, false);
        self.outcome()
    }

    /// `vTempOffSpellChecking` (`Engine.cpp:1223-1227`). The caller gates it on
    /// the Ctrl flag and `Options::temp_off_spelling`.
    pub fn temp_off_spell_checking(&mut self) {
        if self.use_spell_checking_before {
            self.check_spelling = !self.check_spelling;
        }
    }

    /// `vTempOffEngine` (`Engine.cpp:1233-1235`).
    pub fn temp_off_engine(&mut self, off: bool) {
        self.will_temp_off_engine = off;
    }

    /// Feed one key event.
    ///
    /// Upstream receives key-down only; the macOS hook returns before the
    /// engine for `keyUp` (`OpenKey.mm:711-714`), so an [`Phase::Up`] event is
    /// passed through untouched here too.
    pub fn apply(&mut self, event: &KeyEvent) -> Outcome {
        if self.options.input_method != InputMethod::Telex {
            todo!(
                "input methods VNI (C-003) and Simple Telex 1/2 (C-004) — \
                 only Telex (C-002) is ported"
            );
        }
        if event.phase != Phase::Down {
            return Outcome {
                action: Action::Pass,
                edit: None,
                passthrough: None,
                ext: Ext::Normal,
            };
        }
        let Some(data) = platform_code(event.key) else {
            return self.forward_delete();
        };
        // `capsStatus = shift ? 1 : (capsLock ? 2 : 0)` (`OpenKey.mm:715-719`).
        let caps_status = if event.mods.shift {
            1
        } else if event.mods.caps_lock {
            2
        } else {
            0
        };
        // `OTHER_CONTROL_KEY` is command, control or option (`OpenKey.mm:16`).
        let other_control = event.mods.ctrl || event.mods.alt || event.mods.cmd;
        self.dispatch(Source::Keyboard, data, caps_status, other_control);
        self.outcome()
    }

    /// A forward delete, which upstream has no path for. The event is treated
    /// as a word break: nothing is consumed, and the session starts over.
    fn forward_delete(&mut self) -> Outcome {
        self.hook.code = DO_NOTHING;
        self.hook.backspaces = 0;
        self.hook.new_chars = 0;
        self.hook.ext = 1;
        self.special_char.clear();
        self.typing_states.clear();
        self.start_new_session();
        self.check_spelling = self.use_spell_checking_before;
        self.will_temp_off_engine = false;
        self.outcome()
    }

    /// `vKeyHandleEvent` (`Engine.cpp:1314-1558`) for Vietnamese mode.
    fn dispatch(&mut self, source: Source, data: u16, caps_status: u8, other_control: bool) {
        self.is_caps = caps_status == 1 || caps_status == 2;
        let word_break = source == Source::Mouse || is_break_code(data);

        if (is_number_key(data) && caps_status == 1)
            || other_control
            || word_break
            || (self.word.is_empty() && is_number_key(data))
        {
            self.break_branch(source, data, word_break);
        } else if data == KEY_SPACE {
            self.space_branch();
        } else if data == KEY_DELETE {
            self.delete_branch();
        } else {
            self.main_branch(data);
        }
    }

    /// The first branch of `vKeyHandleEvent` (`Engine.cpp:1321-1383`).
    fn break_branch(&mut self, source: Source, data: u16, word_break: bool) {
        self.hook.code = DO_NOTHING;
        self.hook.backspaces = 0;
        self.hook.new_chars = 0;
        self.hook.ext = 1;

        if (self.options.quick_start_consonant || self.options.quick_end_consonant)
            && !self.temp_disable_key
            && is_macro_break_code(data)
        {
            // The return value is ignored upstream; the flag is what matters.
            self.check_quick_consonant();
        } else if self.options.restore_if_invalid && word_break {
            if !self.temp_disable_key && self.check_spelling {
                self.check_spelling(true);
            }
            if self.temp_disable_key
                && !self.check_restore_if_wrong_spelling(RESTORE_AND_START_NEW_SESSION)
            {
                self.hook.code = DO_NOTHING;
            }
        }

        let is_char_key = source == Source::Keyboard && is_char_key_code(data);
        if !is_char_key {
            self.special_char.clear();
            self.typing_states.clear();
        } else {
            if self.space_count > 0 {
                self.save_word_repeat(KEY_SPACE, self.space_count);
                self.space_count = 0;
            } else {
                self.save_word();
            }
            self.special_char
                .push(u32::from(data) | if self.is_caps { CAPS_MASK } else { 0 });
            self.hook.ext = 3;
        }

        if self.hook.code == DO_NOTHING {
            self.start_new_session();
            self.check_spelling = self.use_spell_checking_before;
            self.will_temp_off_engine = false;
        } else if self.hook.code == REPLACE_MACRO || self.has_handle_quick_consonant {
            // The edit is already in `charData`.
            self.word.set_len(0);
        }

        if self.options.upper_case_first_char {
            self.upper_case_status = if data == KEY_DOT {
                1
            } else if data == KEY_ENTER || data == KEY_RETURN {
                2
            } else {
                0
            };
        }
    }

    /// The space branch (`Engine.cpp:1384-1419`).
    fn space_branch(&mut self) {
        if !self.temp_disable_key && self.check_spelling {
            self.check_spelling(true);
        }
        if (self.options.quick_start_consonant || self.options.quick_end_consonant)
            && !self.temp_disable_key
            && self.check_quick_consonant()
        {
            self.space_count += 1;
        } else if self.options.restore_if_invalid
            && self.temp_disable_key
            && !self.has_handled_macro
        {
            if !self.check_restore_if_wrong_spelling(RESTORE) {
                self.hook.code = DO_NOTHING;
            }
            self.space_count += 1;
        } else {
            self.hook.code = DO_NOTHING;
            self.space_count += 1;
        }

        if self.options.upper_case_first_char && self.upper_case_status == 1 {
            self.upper_case_status = 2;
        }
        if self.space_count == 1 {
            if !self.special_char.is_empty() {
                self.save_special_char();
            } else {
                self.save_word();
            }
        }
        self.check_spelling = self.use_spell_checking_before;
        self.will_temp_off_engine = false;
    }

    /// The delete branch (`Engine.cpp:1420-1465`).
    fn delete_branch(&mut self) {
        self.hook.code = DO_NOTHING;
        self.hook.ext = 2;
        if !self.special_char.is_empty() {
            self.special_char.pop();
            if self.special_char.is_empty() {
                self.restore_last_typing_state();
            }
            return;
        }
        if self.space_count > 0 {
            self.space_count -= 1;
            if self.space_count == 0 {
                self.restore_last_typing_state();
            }
            return;
        }

        if !self.key_states.is_empty() {
            self.key_states.set_len(self.key_states.len() - 1);
        }
        if !self.word.is_empty() {
            // `Word::pop` refills from the long-word helper, as `Engine.cpp:1439-1447`.
            self.word.pop();
            if self.check_spelling {
                self.check_spelling(false);
            }
        }
        self.hook.backspaces = 0;
        self.hook.new_chars = 0;
        self.hook.ext = 2;
        if self.word.is_empty() {
            self.start_new_session();
            self.special_char.clear();
            self.restore_last_typing_state();
        } else {
            self.check_grammar(1);
        }
    }

    /// The main branch — "START AND CHECK KEY" (`Engine.cpp:1466-1552`).
    fn main_branch(&mut self, data: u16) {
        if self.will_temp_off_engine {
            self.hook.code = DO_NOTHING;
            self.hook.ext = 3;
            return;
        }
        if self.space_count > 0 {
            self.hook.backspaces = 0;
            self.hook.new_chars = 0;
            self.hook.ext = 0;
            self.start_new_session();
            self.save_word_repeat(KEY_SPACE, self.space_count);
            self.space_count = 0;
        } else if !self.special_char.is_empty() {
            self.save_special_char();
        }

        self.insert_state(data);

        let previous = match self.word.len() {
            0 => 0,
            len => self.word.code_at(len - 1),
        };
        if !is_special_key_telex(data) || self.temp_disable_key {
            if self.options.quick_telex && is_quick_telex_key(self.word.len(), data, || previous) {
                self.handle_quick_telex(data);
                return;
            }
            self.hook.code = DO_NOTHING;
            self.hook.backspaces = 0;
            self.hook.new_chars = 0;
            self.hook.ext = 3;
            self.insert_key(data, self.is_caps, true);
        } else {
            self.hook.code = DO_NOTHING;
            self.hook.ext = 3;
            self.handle_main_key(data);
        }

        // Grammar projection, gated by the free-mark preference (`Engine.cpp:1504`).
        if !self.options.free_mark && data != KEY_D {
            if self.hook.code == DO_NOTHING {
                self.check_grammar(-1);
            } else {
                self.check_grammar(0);
            }
        }

        if self.hook.code == RESTORE {
            self.insert_key(data, self.is_caps, true);
            self.key_states
                .set_len(self.key_states.len().saturating_sub(1));
        }

        if self.options.upper_case_first_char {
            if self.word.len() == 1 && self.upper_case_status == 2 {
                self.upper_case_first_character();
            }
            self.upper_case_status = 0;
        }

        if is_bracket_key(data)
            && (is_bracket_key(self.hook.char_data[0] as u16)
                || matches!(
                    self.options.input_method,
                    InputMethod::SimpleTelex1 | InputMethod::SimpleTelex2
                ))
        {
            let backspaces = if self.hook.code == WILL_PROCESS {
                usize::from(self.hook.backspaces)
            } else {
                0
            };
            if self.word.len() > backspaces {
                self.word.set_len(self.word.len() - 1);
                self.save_word();
            }
            self.word.set_len(0);
            self.temp_disable_key = false;
            self.key_states.set_len(0);
            self.hook.ext = 3;
            self.special_char
                .push(u32::from(data) | if self.is_caps { CAPS_MASK } else { 0 });
        }
    }

    /// `startNewSession` (`Engine.cpp:457-466`).
    fn start_new_session(&mut self) {
        self.word.set_len(0);
        self.hook.backspaces = 0;
        self.hook.new_chars = 0;
        self.temp_disable_key = false;
        self.key_states.set_len(0);
        self.has_handled_macro = false;
        self.has_handle_quick_consonant = false;
        self.word.clear_overflow();
    }

    /// `saveWord()` (`Engine.cpp:381-416`).
    fn save_word(&mut self) {
        if self.hook.code == REPLACE_MACRO {
            // The macro-history half of `saveWord` needs `hMacroData`.
            return;
        }
        if self.word.is_empty() {
            return;
        }
        if !self.word.overflow().is_empty() {
            let overflow: Vec<u32> = self
                .word
                .overflow()
                .iter()
                .map(|letter| letter.raw_bits())
                .collect();
            for chunk in overflow.chunks(MAX_LETTERS) {
                self.typing_states.push(chunk.to_vec());
            }
            self.word.clear_overflow();
        }
        let current: Vec<u32> = (0..self.word.len()).map(|i| self.word.bits_at(i)).collect();
        self.typing_states.push(current);
    }

    /// `saveWord(keyCode, count)` (`Engine.cpp:418-424`).
    fn save_word_repeat(&mut self, code: u16, count: usize) {
        self.typing_states.push(vec![u32::from(code); count]);
    }

    /// `saveSpecialChar` (`Engine.cpp:426-433`).
    fn save_special_char(&mut self) {
        let chars = std::mem::take(&mut self.special_char);
        self.typing_states.push(chars);
    }

    /// `restoreLastTypingState` (`Engine.cpp:435-455`).
    fn restore_last_typing_state(&mut self) {
        let Some(state) = self.typing_states.pop() else {
            return;
        };
        if state.is_empty() {
            return;
        }
        if state[0] == u32::from(KEY_SPACE) {
            self.space_count = state.len();
            self.word.set_len(0);
        } else if is_char_key_code(state[0] as u16) {
            self.word.set_len(0);
            self.special_char = state;
            self.check_spelling(false);
        } else {
            for (i, &bits) in state.iter().enumerate() {
                self.word.set_bits(i, bits);
            }
            self.word.set_len(state.len());
        }
    }

    /// `checkSpelling` (`Engine.cpp:176-288`).
    ///
    /// The `VSI`/`VEI` writes upstream makes here are not observable: every
    /// caller that needs a vowel span recomputes it (`insertMark`,
    /// `insertW`/`insertAOE`, `removeMark`, `checkGrammar`). They are locals.
    fn check_spelling(&mut self, force_check_vowel: bool) {
        let mut spelling_ok = false;
        let mut spelling_vowel_ok = true;
        let index = self.word.len();
        let mut spelling_end = index;
        if index > 0 && self.chr(index - 1) == KEY_RIGHT_BRACKET {
            spelling_end -= 1;
        }

        if spelling_end > 0 {
            let mut j: isize = 0;
            if is_consonant(self.chr(0)) {
                for row in consonant_table() {
                    let mut flag = spelling_end < row.len();
                    let mut at = 0usize;
                    while at < row.len() {
                        j = at as isize;
                        let without_end = if self.options.quick_start_consonant {
                            row[at] & !END_CONSONANT_MASK
                        } else {
                            row[at]
                        };
                        let without_allow = if self.options.allow_consonant_zfwj {
                            row[at] & !CONSONANT_ALLOW_MASK
                        } else {
                            row[at]
                        };
                        if spelling_end > at
                            && without_end != self.chr(at)
                            && without_allow != self.chr(at)
                        {
                            flag = true;
                            break;
                        }
                        at += 1;
                    }
                    if !flag {
                        // The C `for` leaves `j` at `row.size()`.
                        j = row.len() as isize;
                        break;
                    }
                }
            }
            if j as usize == spelling_end {
                spelling_ok = true;
            }

            let mut k = j;
            // `VSI`/`VEI` are globals upstream; no caller reads them after
            // `checkSpelling` (every span consumer recomputes), so the
            // assignments are omitted here and only `k` is tracked.
            // "que't" (`Engine.cpp:216-219`).
            if self.chr(k as usize) == KEY_U
                && k > 0
                && (k as usize) < spelling_end - 1
                && self.chr((k - 1) as usize) == KEY_Q
            {
                k += 1;
                j = k;
            } else if index >= 2
                && self.chr(0) == KEY_G
                && self.chr(1) == KEY_I
                && is_consonant(self.chr(2))
            {
                // "gìn" (`Engine.cpp:220-222`).
                k = 1;
                j = 1;
            }
            for _ in 0..3 {
                if (k as usize) < spelling_end && !is_consonant(self.chr(k as usize)) {
                    k += 1;
                }
            }

            if k > j {
                spelling_vowel_ok = false;
                if k - j > 1 && force_check_vowel {
                    for row in vowel_combine(self.chr(j as usize)) {
                        let mut flag = false;
                        let mut ii = 1usize;
                        while ii < row.len() {
                            let slot = (j + ii as isize - 1) as usize;
                            if (slot as isize) < spelling_end as isize
                                && row[ii]
                                    != (u32::from(self.chr(slot))
                                        | (self.bits(slot) & TONEW_MASK)
                                        | (self.bits(slot) & TONE_MASK))
                            {
                                flag = true;
                                break;
                            }
                            ii += 1;
                        }
                        let last = (j + ii as isize - 1) as usize;
                        if flag
                            || ((k as usize) < spelling_end && row[0] == 0)
                            || ((last as isize) < spelling_end as isize
                                && !is_consonant(self.chr(last)))
                        {
                            continue;
                        }
                        spelling_vowel_ok = true;
                        break;
                    }
                } else if !is_consonant(self.chr(j as usize)) {
                    spelling_vowel_ok = true;
                }

                for row in end_consonant_table() {
                    let mut flag = false;
                    let mut at = 0usize;
                    while at < row.len() {
                        let code = if self.options.quick_end_consonant {
                            row[at] & !END_CONSONANT_MASK
                        } else {
                            row[at]
                        };
                        if spelling_end > k as usize + at && code != self.chr(k as usize + at) {
                            flag = true;
                            break;
                        }
                        at += 1;
                    }
                    if flag {
                        continue;
                    }
                    if k as usize + at >= spelling_end {
                        spelling_ok = true;
                        break;
                    }
                }

                // `ch`/`t` cannot carry `~`, `` ` ``, `?` (`Engine.cpp:272-279`).
                // The two clauses are mutually exclusive (`CHR(index-1)` cannot be
                // both `h` and `t`), so one `if` states the same result.
                if spelling_ok
                    && ((index >= 3
                        && self.chr(index - 1) == KEY_H
                        && self.chr(index - 2) == KEY_C
                        && !(self.bits(index - 3) & MARK1_MASK != 0
                            || self.bits(index - 3) & MARK5_MASK != 0
                            || self.bits(index - 3) & MARK_MASK == 0))
                        || (index >= 2
                            && self.chr(index - 1) == KEY_T
                            && !(self.bits(index - 2) & MARK1_MASK != 0
                                || self.bits(index - 2) & MARK5_MASK != 0
                                || self.bits(index - 2) & MARK_MASK == 0)))
                {
                    spelling_ok = false;
                }
            } else {
                spelling_ok = true;
            }
        } else {
            spelling_ok = true;
        }

        self.temp_disable_key = !(spelling_ok && spelling_vowel_ok);
    }

    /// `checkGrammar` (`Engine.cpp:290-347`).
    fn check_grammar(&mut self, delta_backspace: i32) {
        let index = self.word.len();
        if index <= 1 || index >= MAX_LETTERS {
            return;
        }
        self.find_vowels(Scan::Grammar);
        if self.vowel_count == 0 {
            return;
        }
        let mut is_checked = false;
        let start = self.vowel_start as usize;

        // "thuơn", "ưoi", "ưom", "ưoc" (`Engine.cpp:302-317`).
        if index >= 3 {
            for i in (0..index).rev() {
                if matches!(self.chr(i), KEY_N | KEY_C | KEY_I | KEY_M | KEY_P | KEY_T)
                    && i >= 2
                    && self.chr(i - 1) == KEY_O
                    && self.chr(i - 2) == KEY_U
                {
                    let first = self.bits(i - 2) & TONEW_MASK != 0;
                    let second = self.bits(i - 1) & TONEW_MASK != 0;
                    if first ^ second {
                        self.word.or_bits(i - 2, TONEW_MASK);
                        self.word.or_bits(i - 1, TONEW_MASK);
                        is_checked = true;
                        break;
                    }
                }
            }
        }

        // Move the mark to the orthographically correct slot (`Engine.cpp:319-331`).
        if index >= 2 {
            for i in start..=self.vowel_end as usize {
                let mark = self.bits(i) & MARK_MASK;
                if mark != 0 {
                    self.word.clear_bits(i, MARK_MASK);
                    self.insert_mark(mark, false);
                    if i != self.vowel_will_set_mark as usize {
                        is_checked = true;
                    }
                    break;
                }
            }
        }

        if is_checked {
            if self.hook.code == DO_NOTHING {
                self.hook.code = WILL_PROCESS;
            }
            self.hook.backspaces = 0;
            for i in (start..index).rev() {
                self.hook.backspaces += 1;
                self.hook.char_data[index - 1 - i] =
                    charset::encode(self.bits(i), self.options.charset);
            }
            self.hook.new_chars = self.hook.backspaces;
            self.hook.backspaces = self.hook.backspaces.wrapping_add(delta_backspace as u8);
            self.hook.ext = 4;
        }
    }

    /// `insertKey` (`Engine.cpp:349-367`).
    fn insert_key(&mut self, code: u16, caps: bool, check: bool) {
        self.word.push(letter(code, caps));
        if self.check_spelling && check {
            self.check_spelling(false);
        }
        // "allow d after consonant" (`Engine.cpp:364-366`).
        if code == KEY_D && self.word.len() >= 2 && is_consonant(self.chr(self.word.len() - 2)) {
            self.temp_disable_key = false;
        }
    }

    /// `insertState` (`Engine.cpp:369-379`).
    fn insert_state(&mut self, code: u16) {
        self.key_states.push_drop_front(letter(code, self.is_caps));
    }

    /// `insertMark` (`Engine.cpp:753-806`).
    fn insert_mark(&mut self, mark: u32, can_modify_flag: bool) {
        self.vowel_count = 0;
        if can_modify_flag {
            self.hook.code = WILL_PROCESS;
        }
        self.hook.backspaces = 0;
        self.hook.new_chars = 0;

        self.find_vowels(Scan::Word);
        self.vowel_will_set_mark = 0;
        let index = self.word.len();

        if self.vowel_count == 1 {
            self.vowel_will_set_mark = self.vowel_end;
            self.hook.backspaces = (index - self.vowel_end as usize) as u8;
        } else {
            if self.options.modern_orthography {
                self.handle_modern_mark();
            } else {
                self.handle_old_mark();
            }
            if self.bits(self.vowel_end as usize) & (TONE_MASK | TONEW_MASK) != 0 {
                self.vowel_will_set_mark = self.vowel_end;
            }
        }

        let start = self.vowel_start as usize;
        let slot = self.vowel_will_set_mark as usize;
        if self.bits(slot) & mark != 0 {
            // The same mark again restores the word (`Engine.cpp:778-789`).
            self.word.clear_bits(slot, MARK_MASK);
            if can_modify_flag {
                self.hook.code = RESTORE;
            }
            for i in start..index {
                self.word.clear_bits(i, MARK_MASK);
                self.hook.char_data[index - 1 - i] =
                    charset::encode(self.bits(i), self.options.charset);
            }
            self.temp_disable_key = true;
        } else {
            self.word.clear_bits(slot, MARK_MASK);
            self.word.or_bits(slot, mark);
            for i in start..index {
                if i != slot {
                    self.word.clear_bits(i, MARK_MASK);
                }
                self.hook.char_data[index - 1 - i] =
                    charset::encode(self.bits(i), self.options.charset);
            }
            self.hook.backspaces = (index - start) as u8;
        }
        self.hook.new_chars = self.hook.backspaces;
    }

    /// `handleOldMark` (`Engine.cpp:727-751`).
    fn handle_old_mark(&mut self) {
        let start = self.vowel_start as usize;
        let end = self.vowel_end as usize;
        let index = self.word.len();

        if self.vowel_count == 0 && self.chr(end) == KEY_I {
            self.vowel_will_set_mark = self.vowel_end;
        } else {
            self.vowel_will_set_mark = self.vowel_start;
        }
        self.hook.backspaces = (index - self.vowel_will_set_mark as usize) as u8;

        if self.vowel_count == 3
            || (end + 1 < index && is_consonant(self.chr(end + 1)) && self.can_has_end_consonant())
        {
            self.vowel_will_set_mark = (start + 1) as u8;
            self.hook.backspaces = (index - start - 1) as u8;
        }

        for i in start..=end {
            if (self.chr(i) == KEY_E && self.bits(i) & TONE_MASK != 0)
                || (self.chr(i) == KEY_O && self.bits(i) & TONEW_MASK != 0)
            {
                self.vowel_will_set_mark = i as u8;
                self.hook.backspaces = (index - i) as u8;
                break;
            }
        }
        self.hook.new_chars = self.hook.backspaces;
    }

    /// `handleModernMark` (`Engine.cpp:629-725`), including its quirks: rule
    /// 3.2's first three clauses compare the same slot twice, and the
    /// `ch`/`nh`/`ng` alternatives in rule 3.1 test one character against two.
    fn handle_modern_mark(&mut self) {
        let start = self.vowel_start as usize;
        let end = self.vowel_end as usize;
        let index = self.word.len();

        self.vowel_will_set_mark = self.vowel_end;
        self.hook.backspaces = (index - end) as u8;

        // Rule 2.
        if self.vowel_count == 3
            && ((self.chr(start) == KEY_O
                && self.chr(start + 1) == KEY_A
                && self.chr(start + 2) == KEY_I)
                || (self.chr(start) == KEY_U
                    && self.chr(start + 1) == KEY_Y
                    && self.chr(start + 2) == KEY_U)
                || (self.chr(start) == KEY_O
                    && self.chr(start + 1) == KEY_E
                    && self.chr(start + 2) == KEY_O)
                || (self.chr(start) == KEY_U
                    && self.chr(start + 1) == KEY_Y
                    && self.chr(start + 2) == KEY_A))
        {
            self.vowel_will_set_mark = (start + 1) as u8;
            self.hook.backspaces = (index - start - 1) as u8;
        } else if (self.chr(start) == KEY_O && self.chr(start + 1) == KEY_I)
            || (self.chr(start) == KEY_A && self.chr(start + 1) == KEY_I)
            || (self.chr(start) == KEY_U && self.chr(start + 1) == KEY_I)
        {
            self.vowel_will_set_mark = start as u8;
            self.hook.backspaces = (index - start) as u8;
        } else if self.chr(end - 1) == KEY_A && self.chr(end) == KEY_Y {
            self.vowel_will_set_mark = (end - 1) as u8;
            self.hook.backspaces = (index - end + 1) as u8;
        } else if self.chr(start) == KEY_U && self.chr(start + 1) == KEY_O {
            self.vowel_will_set_mark = (start + 1) as u8;
            self.hook.backspaces = (index - start - 1) as u8;
        } else if self.chr(start + 1) == KEY_O || self.chr(start + 1) == KEY_U {
            self.vowel_will_set_mark = (end - 1) as u8;
            self.hook.backspaces = (index - end + 1) as u8;
        } else if self.chr(start) == KEY_O || self.chr(start) == KEY_U {
            self.vowel_will_set_mark = end as u8;
            self.hook.backspaces = (index - end) as u8;
        }

        // Rule 3.1.
        if (self.chr(start) == KEY_I && self.bits(start + 1) & (u32::from(KEY_E) | TONE_MASK) != 0)
            || (self.chr(start) == KEY_Y
                && self.bits(start + 1) & (u32::from(KEY_E) | TONE_MASK) != 0)
            || (self.chr(start) == KEY_U && self.bits(start + 1) == (u32::from(KEY_O) | TONE_MASK))
            || (self.bits(start) == (u32::from(KEY_U) | TONEW_MASK)
                && self.bits(start + 1) == (u32::from(KEY_O) | TONEW_MASK))
        {
            if start + 2 < index {
                if matches!(
                    self.chr(start + 2),
                    KEY_P | KEY_T | KEY_M | KEY_N | KEY_O | KEY_U | KEY_I | KEY_C
                ) || (start + 3 < index
                    && self.chr(start + 2) == KEY_C
                    && self.chr(start + 2) == KEY_H)
                    || (start + 3 < index
                        && self.chr(start + 2) == KEY_N
                        && self.chr(start + 2) == KEY_H)
                    || (start + 3 < index
                        && self.chr(start + 2) == KEY_N
                        && self.chr(start + 2) == KEY_G)
                {
                    self.vowel_will_set_mark = (start + 1) as u8;
                    self.hook.backspaces = (index - start - 1) as u8;
                } else {
                    self.vowel_will_set_mark = start as u8;
                    self.hook.backspaces = (index - start) as u8;
                }
            } else {
                self.vowel_will_set_mark = start as u8;
                self.hook.backspaces = (index - start) as u8;
            }
        }
        // Rule 3.2.
        else if (self.chr(start) == KEY_I && self.chr(start) == KEY_A)
            || (self.chr(start) == KEY_Y && self.chr(start) == KEY_A)
            || (self.chr(start) == KEY_U && self.chr(start) == KEY_A)
            || (self.chr(start) == KEY_U && self.bits(start + 1) == (u32::from(KEY_U) | TONEW_MASK))
        {
            self.vowel_will_set_mark = start as u8;
            self.hook.backspaces = (index - start) as u8;
        }

        // Rule 4.
        if self.vowel_count == 2 {
            if (self.chr(start) == KEY_I && self.chr(start + 1) == KEY_A)
                || (self.chr(start) == KEY_I && self.chr(start + 1) == KEY_U)
                || (self.chr(start) == KEY_I && self.chr(start + 1) == KEY_O)
            {
                if start == 0 || self.chr(start - 1) != KEY_G {
                    self.vowel_will_set_mark = start as u8;
                } else {
                    self.vowel_will_set_mark = (start + 1) as u8;
                }
                self.hook.backspaces = (index - self.vowel_will_set_mark as usize) as u8;
            } else if self.chr(start) == KEY_U && self.chr(start + 1) == KEY_A {
                if start == 0 || self.chr(start - 1) != KEY_Q {
                    if end + 1 >= index || !self.can_has_end_consonant() {
                        self.vowel_will_set_mark = start as u8;
                        self.hook.backspaces = (index - start) as u8;
                    }
                } else {
                    self.vowel_will_set_mark = (start + 1) as u8;
                    self.hook.backspaces = (index - start - 1) as u8;
                }
            } else if self.chr(start) == KEY_O && self.chr(start + 1) == KEY_O {
                self.vowel_will_set_mark = end as u8;
                self.hook.backspaces = (index - end) as u8;
            }
        }
    }

    /// `canHasEndConsonant` (`Engine.cpp:612-627`).
    fn can_has_end_consonant(&self) -> bool {
        let start = self.vowel_start as usize;
        let end = self.vowel_end as usize;
        for row in vowel_combine(self.chr(start)) {
            let mut at = start;
            let mut ii = 1usize;
            while ii < row.len() {
                if at > end
                    || (u32::from(self.chr(at))
                        | (self.bits(at) & TONE_MASK)
                        | (self.bits(at) & TONEW_MASK))
                        != row[ii]
                {
                    break;
                }
                at += 1;
                ii += 1;
            }
            if ii >= row.len() {
                return row[0] == 1;
            }
        }
        false
    }

    /// `insertD` (`Engine.cpp:808-831`).
    fn insert_d(&mut self) {
        let index = self.word.len();
        self.hook.code = WILL_PROCESS;
        self.hook.backspaces = 0;
        for i in (0..index).rev() {
            self.hook.backspaces += 1;
            if self.chr(i) == KEY_D {
                if self.bits(i) & TONE_MASK != 0 {
                    self.hook.code = RESTORE;
                    self.word.clear_bits(i, TONE_MASK);
                    self.hook.char_data[index - 1 - i] = self.bits(i);
                    self.temp_disable_key = true;
                } else {
                    self.word.or_bits(i, TONE_MASK);
                    self.hook.char_data[index - 1 - i] =
                        charset::encode(self.bits(i), self.options.charset);
                }
                break;
            } else {
                self.hook.char_data[index - 1 - i] =
                    charset::encode(self.bits(i), self.options.charset);
            }
        }
        self.hook.new_chars = self.hook.backspaces;
    }

    /// `insertAOE` (`Engine.cpp:833-869`), including the unconditional
    /// `TONEW` clear for A/O/E (`IS_KEY_D` is only true for D).
    fn insert_aoe(&mut self, code: u16) {
        self.find_vowels(Scan::Word);
        for i in self.vowel_start as usize..=self.vowel_end as usize {
            self.word.clear_bits(i, TONEW_MASK);
        }
        let index = self.word.len();
        self.hook.code = WILL_PROCESS;
        self.hook.backspaces = 0;
        for i in (0..index).rev() {
            self.hook.backspaces += 1;
            if self.chr(i) == code {
                if self.bits(i) & TONE_MASK != 0 {
                    self.hook.code = RESTORE;
                    self.word.clear_bits(i, TONE_MASK);
                    self.hook.char_data[index - 1 - i] = self.bits(i);
                    if code != KEY_O {
                        self.temp_disable_key = true;
                    }
                } else {
                    self.word.or_bits(i, TONE_MASK);
                    if code != KEY_D {
                        self.word.clear_bits(i, TONEW_MASK);
                    }
                    self.hook.char_data[index - 1 - i] =
                        charset::encode(self.bits(i), self.options.charset);
                }
                break;
            } else {
                self.hook.char_data[index - 1 - i] =
                    charset::encode(self.bits(i), self.options.charset);
            }
        }
        self.hook.new_chars = self.hook.backspaces;
    }

    /// `insertW` (`Engine.cpp:871-984`).
    fn insert_w(&mut self, code: u16) {
        self.find_vowels(Scan::Word);
        for i in self.vowel_start as usize..=self.vowel_end as usize {
            self.word.clear_bits(i, TONE_MASK);
        }
        let index = self.word.len();
        let start = self.vowel_start as usize;

        if self.vowel_count > 1 {
            self.hook.backspaces = (index - start) as u8;
            self.hook.new_chars = self.hook.backspaces;

            let first_w = self.bits(start) & TONEW_MASK != 0;
            if (first_w && self.bits(start + 1) & TONEW_MASK != 0)
                || (first_w && self.chr(start + 1) == KEY_I)
                || (first_w && self.chr(start + 1) == KEY_A)
            {
                self.hook.code = RESTORE;
                for i in start..index {
                    self.word.clear_bits(i, TONEW_MASK);
                    self.hook.char_data[index - 1 - i] =
                        charset::encode(self.bits(i), self.options.charset) & !STANDALONE_MASK;
                }
                self.temp_disable_key = true;
            } else {
                self.hook.code = WILL_PROCESS;
                if self.chr(start) == KEY_U && self.chr(start + 1) == KEY_O {
                    if start >= 2
                        && self.bits(start - 2) == u32::from(KEY_T)
                        && self.bits(start - 1) == u32::from(KEY_H)
                    {
                        self.word.or_bits(start + 1, TONEW_MASK);
                        if start + 2 < index && self.chr(start + 2) == KEY_N {
                            self.word.or_bits(start, TONEW_MASK);
                        }
                    } else if start >= 1 && self.bits(start - 1) == u32::from(KEY_Q) {
                        self.word.or_bits(start + 1, TONEW_MASK);
                    } else {
                        self.word.or_bits(start, TONEW_MASK);
                        self.word.or_bits(start + 1, TONEW_MASK);
                    }
                } else if (self.chr(start) == KEY_U && self.chr(start + 1) == KEY_A)
                    || (self.chr(start) == KEY_U && self.chr(start + 1) == KEY_I)
                    || (self.chr(start) == KEY_U && self.chr(start + 1) == KEY_U)
                    || (self.chr(start) == KEY_O && self.chr(start + 1) == KEY_I)
                {
                    self.word.or_bits(start, TONEW_MASK);
                } else if (self.chr(start) == KEY_I && self.chr(start + 1) == KEY_O)
                    || (self.chr(start) == KEY_O && self.chr(start + 1) == KEY_A)
                {
                    self.word.or_bits(start + 1, TONEW_MASK);
                } else {
                    // `isChanged = false` here makes the caller treat W as a
                    // standalone key (`Engine.cpp:920-925, 1186`).
                    self.temp_disable_key = true;
                    self.is_changed = false;
                    self.hook.code = DO_NOTHING;
                }
                for i in start..index {
                    self.hook.char_data[index - 1 - i] =
                        charset::encode(self.bits(i), self.options.charset);
                }
            }
            return;
        }

        self.hook.code = WILL_PROCESS;
        self.hook.backspaces = 0;
        for i in (start..index).rev() {
            self.hook.backspaces += 1;
            match self.chr(i) {
                KEY_A | KEY_U | KEY_O => {
                    if self.bits(i) & TONEW_MASK != 0 {
                        if self.bits(i) & STANDALONE_MASK != 0 {
                            self.hook.code = WILL_PROCESS;
                            if self.chr(i) == KEY_U {
                                self.word.set_bits(
                                    i,
                                    u32::from(KEY_W)
                                        | if self.bits(i) & CAPS_MASK != 0 {
                                            CAPS_MASK
                                        } else {
                                            0
                                        },
                                );
                            } else if self.chr(i) == KEY_O {
                                self.hook.code = RESTORE;
                                self.word.set_bits(
                                    i,
                                    u32::from(KEY_O)
                                        | if self.bits(i) & CAPS_MASK != 0 {
                                            CAPS_MASK
                                        } else {
                                            0
                                        },
                                );
                            }
                            self.hook.char_data[index - 1 - i] = self.bits(i);
                        } else {
                            self.hook.code = RESTORE;
                            self.word.clear_bits(i, TONEW_MASK);
                            self.hook.char_data[index - 1 - i] = self.bits(i);
                        }
                        self.temp_disable_key = true;
                    } else {
                        self.word.or_bits(i, TONEW_MASK);
                        self.word.clear_bits(i, TONE_MASK);
                        self.hook.char_data[index - 1 - i] =
                            charset::encode(self.bits(i), self.options.charset);
                    }
                }
                _ => {
                    self.hook.char_data[index - 1 - i] =
                        charset::encode(self.bits(i), self.options.charset);
                }
            }
        }
        self.hook.new_chars = self.hook.backspaces;
        // `code` is unused past this point upstream too; argument kept for the
        // signature of the C function.
        let _ = code;
    }

    /// `reverseLastStandaloneChar` (`Engine.cpp:986-993`).
    fn reverse_last_standalone_char(&mut self, code: u16) {
        self.hook.code = WILL_PROCESS;
        self.hook.backspaces = 0;
        self.hook.new_chars = 1;
        self.hook.ext = 4;
        let index = self.word.len();
        self.word.set_bits(
            index - 1,
            u32::from(code)
                | TONEW_MASK
                | STANDALONE_MASK
                | if self.is_caps { CAPS_MASK } else { 0 },
        );
        self.hook.char_data[0] = charset::encode(self.bits(index - 1), self.options.charset);
    }

    /// `checkForStandaloneChar` (`Engine.cpp:995-1040`).
    fn check_for_standalone_char(&mut self, data: u16, key_will_reverse: u16) {
        let index = self.word.len();
        // Upstream reads `TypingWord[-1]` when `_index == 0`; the guard is the
        // deviation documented in the module header.
        if index > 0
            && self.chr(index - 1) == key_will_reverse
            && self.bits(index - 1) & TONEW_MASK != 0
        {
            self.hook.code = WILL_PROCESS;
            self.hook.backspaces = 1;
            self.hook.new_chars = 1;
            self.word.set_bits(
                index - 1,
                u32::from(data) | if self.is_caps { CAPS_MASK } else { 0 },
            );
            self.hook.char_data[0] = charset::encode(self.bits(index - 1), self.options.charset);
            return;
        }

        // Standalone `w` -> `ư`.
        if index > 0 && self.chr(index - 1) == KEY_U && key_will_reverse == KEY_O {
            self.insert_key(key_will_reverse, self.is_caps, true);
            self.reverse_last_standalone_char(key_will_reverse);
            return;
        }

        if index == 0 {
            self.insert_key(data, self.is_caps, false);
            self.reverse_last_standalone_char(key_will_reverse);
            return;
        }
        if index == 1 {
            if standalone_w_bad().contains(&self.chr(0)) {
                self.insert_key(data, self.is_caps, true);
                return;
            }
            self.insert_key(data, self.is_caps, false);
            self.reverse_last_standalone_char(key_will_reverse);
            return;
        }
        if index == 2 {
            if double_w_allowed()
                .iter()
                .any(|pair| self.chr(0) == pair[0] && self.chr(1) == pair[1])
            {
                self.insert_key(data, self.is_caps, false);
                self.reverse_last_standalone_char(key_will_reverse);
                return;
            }
            self.insert_key(data, self.is_caps, true);
            return;
        }
        self.insert_key(data, self.is_caps, true);
    }

    /// `upperCaseFirstCharacter` (`Engine.cpp:1042-1053`). The `hMacroKey`
    /// write is omitted with the macro state.
    fn upper_case_first_character(&mut self) {
        if self.word.bits_at(0) & CAPS_MASK == 0 {
            self.hook.code = WILL_PROCESS;
            self.hook.backspaces = 0;
            self.hook.new_chars = 1;
            self.word.or_bits(0, CAPS_MASK);
            self.hook.char_data[0] = charset::encode(self.bits(0), self.options.charset);
            self.upper_case_status = 0;
        }
    }

    /// `handleMainKey` (`Engine.cpp:1055-1193`).
    fn handle_main_key(&mut self, data: u16) {
        if data == KEY_Z {
            let changed = self.remove_mark();
            if !changed {
                self.insert_key(data, self.is_caps, true);
            }
            return;
        }

        if data == KEY_LEFT_BRACKET {
            self.check_for_standalone_char(data, KEY_O);
            return;
        }
        if data == KEY_RIGHT_BRACKET {
            self.check_for_standalone_char(data, KEY_U);
            return;
        }

        if data == KEY_D {
            let mut changed = false;
            for row in consonant_d() {
                if self.word.len() < row.len() {
                    continue;
                }
                let mut correct = self.check_correct_vowel(row, data);
                if !correct
                    && self.word.len() >= 2
                    && self.chr(self.word.len() - 1) == KEY_D
                    && is_consonant(self.chr(self.word.len() - 2))
                {
                    correct = true;
                }
                if correct {
                    changed = true;
                    self.insert_d();
                    break;
                }
            }
            if !changed {
                self.insert_key(data, self.is_caps, true);
            }
            return;
        }

        if is_mark_key_telex(data) {
            let mark = if data == KEY_S {
                MARK1_MASK
            } else if data == KEY_F {
                MARK2_MASK
            } else if data == KEY_R {
                MARK3_MASK
            } else if data == KEY_X {
                MARK4_MASK
            } else {
                MARK5_MASK
            };
            for (_, rows) in vowel_for_mark() {
                self.is_changed = false;
                let mut correct = false;
                for row in *rows {
                    if self.word.len() < row.len() {
                        continue;
                    }
                    correct = self.check_correct_vowel(row, data);
                    if correct {
                        self.is_changed = true;
                        self.insert_mark(mark, true);
                        break;
                    }
                }
                if correct {
                    break;
                }
            }
            if !self.is_changed {
                self.insert_key(data, self.is_caps, true);
            }
            return;
        }

        // Telex: `keyForAEO` is the pressed key itself (`Engine.cpp:1154`).
        let key_for_aeo = data;
        self.is_changed = false;
        for row in vowel(key_for_aeo) {
            if self.word.len() < row.len() {
                continue;
            }
            if self.check_correct_vowel(row, data) {
                self.is_changed = true;
                if is_key_double_telex(data) {
                    self.insert_aoe(key_for_aeo);
                } else if data == KEY_W {
                    self.insert_w(key_for_aeo);
                }
                break;
            }
        }
        if !self.is_changed {
            if data == KEY_W {
                self.check_for_standalone_char(data, KEY_U);
            } else {
                self.insert_key(data, self.is_caps, true);
            }
        }
    }

    /// `handleQuickTelex` (`Engine.cpp:1195-1202`).
    fn handle_quick_telex(&mut self, data: u16) {
        let pair = quick_telex(data).expect("IS_QUICK_TELEX_KEY guarantees a quick-telex row");
        self.hook.code = WILL_PROCESS;
        self.hook.backspaces = 1;
        self.hook.new_chars = 2;
        self.hook.char_data[1] = u32::from(pair[0]) | if self.is_caps { CAPS_MASK } else { 0 };
        self.hook.char_data[0] = u32::from(pair[1]) | if self.is_caps { CAPS_MASK } else { 0 };
        self.insert_key(pair[1], self.is_caps, false);
    }

    /// `checkRestoreIfWrongSpelling` (`Engine.cpp:1204-1221`).
    fn check_restore_if_wrong_spelling(&mut self, handle_code: u8) -> bool {
        for i in 0..self.word.len() {
            if !is_consonant(self.chr(i))
                && self.word.bits_at(i) & (MARK_MASK | TONE_MASK | TONEW_MASK) != 0
            {
                self.hook.code = handle_code;
                self.hook.backspaces = self.word.len() as u8;
                self.hook.new_chars = self.key_states.len() as u8;
                for i in 0..self.key_states.len() {
                    self.word.set_bits(i, self.key_states.bits_at(i));
                    self.hook.char_data[self.key_states.len() - 1 - i] = self.word.bits_at(i);
                }
                self.word.set_len(self.key_states.len());
                return true;
            }
        }
        false
    }

    /// `checkQuickConsonant` (`Engine.cpp:1237-1281`).
    fn check_quick_consonant(&mut self) -> bool {
        if self.word.len() <= 1 {
            return false;
        }
        let mut handled = false;

        if self.options.quick_start_consonant {
            if let Some(pair) = quick_start_consonant(self.chr(0)) {
                self.hook.code = RESTORE;
                self.hook.backspaces = self.word.len() as u8;
                self.hook.new_chars = (self.word.len() + 1) as u8;
                self.word.insert_quick_start_pair(pair);
                handled = true;
            }
        }
        if self.options.quick_end_consonant
            && self.word.len() >= 2
            && !is_consonant(self.chr(self.word.len() - 2))
        {
            if let Some(pair) = quick_end_consonant(self.chr(self.word.len() - 1)) {
                self.hook.code = RESTORE;
                if handled {
                    self.hook.new_chars += 1;
                } else {
                    self.hook.backspaces = 1;
                    self.hook.new_chars = 2;
                }
                self.word.insert_quick_end_pair(pair);
                handled = true;
            }
        }

        if handled {
            self.has_handle_quick_consonant = true;
            let index = self.word.len();
            for i in 0..index {
                self.hook.char_data[index - 1 - i] =
                    charset::encode(self.bits(i), self.options.charset);
            }
            return true;
        }
        false
    }

    /// `removeMark` (`Engine.cpp:587-610`); returns `isChanged`.
    fn remove_mark(&mut self) -> bool {
        self.find_vowels(Scan::Grammar);
        let mut changed = false;
        let start = self.vowel_start as usize;
        let end = self.vowel_end as usize;
        if !self.word.is_empty() {
            for i in start..=end {
                if self.bits(i) & MARK_MASK != 0 {
                    self.word.clear_bits(i, MARK_MASK);
                    changed = true;
                }
            }
        }
        if changed {
            self.hook.code = WILL_PROCESS;
            self.hook.backspaces = 0;
            let index = self.word.len();
            for i in (start..index).rev() {
                self.hook.backspaces += 1;
                self.hook.char_data[index - 1 - i] =
                    charset::encode(self.bits(i), self.options.charset);
            }
            self.hook.new_chars = self.hook.backspaces;
        } else {
            self.hook.code = DO_NOTHING;
        }
        changed
    }

    /// `checkCorrectVowel` (`Engine.cpp:468-499`), which is `Word::tail_matches`
    /// with the caller's mark-key restriction.
    fn check_correct_vowel(&self, row: &[u16], mark_key: u16) -> bool {
        let restrict_ct = mark_key == KEY_F || mark_key == KEY_X || mark_key == KEY_R;
        self.word.tail_matches(
            row,
            self.word.len(),
            self.options.quick_end_consonant,
            restrict_ct,
        )
    }

    /// `findAndCalculateVowel` (`Engine.cpp:560-585`), storing its outputs.
    fn find_vowels(&mut self, scan: Scan) {
        let span = self.word.span(scan);
        self.vowel_start = span.start;
        self.vowel_end = span.end;
        self.vowel_count = span.count;
    }

    /// `CHR(index)` (`DataType.h:127`).
    fn chr(&self, index: usize) -> u16 {
        self.word.code_at(index)
    }

    /// `TypingWord[index]`.
    fn bits(&self, index: usize) -> u32 {
        self.word.bits_at(index)
    }

    /// Translate the hook state into the crate's [`Outcome`].
    fn outcome(&self) -> Outcome {
        let ext = match self.hook.ext {
            1 => Ext::Break,
            2 => Ext::Delete,
            4 => Ext::SuppressEmpty,
            _ => Ext::Normal,
        };
        let edit = || Edit {
            backspaces: self.hook.backspaces,
            text: self.edit_text(),
            beep: false,
        };
        let (action, edit) = match self.hook.code {
            DO_NOTHING => (Action::Pass, None),
            WILL_PROCESS => (Action::Process, Some(edit())),
            RESTORE => (Action::Restore, Some(edit())),
            RESTORE_AND_START_NEW_SESSION => (Action::RestoreAndBreak, Some(edit())),
            // `vBreakWord` is never assigned upstream (`DataType.h:60`).
            2 => (Action::BreakWord, None),
            // `vReplaceMaro` needs a macro table, which this port does not have.
            _ => (Action::Macro, None),
        };
        Outcome {
            action,
            edit,
            passthrough: None,
            ext,
        }
    }

    /// `charData[0..newCharCount]` rendered in the active charset
    /// (`OpenKey.mm:423-480`, harness `emitUnits`).
    ///
    /// `charData` holds the last character at index 0 (the hook writes
    /// `charData[_index - 1 - i]`), so [`charset::render`] walks it backwards and
    /// returns the units in reading order.
    fn edit_text(&self) -> Encoded {
        let entries = &self.hook.char_data[..usize::from(self.hook.new_chars)];
        charset::render(entries, self.options.charset)
    }
}

/// A bare keycode plus the caps bit, as one `TypingWord` entry.
fn letter(code: u16, caps: bool) -> Letter {
    Letter::from_bits_raw(u32::from(code) | if caps { CAPS_MASK } else { 0 })
        .expect("a bare keycode and the caps bit are inside the word layout")
}

/// `IS_BRACKET_KEY` (`Engine.cpp:55`).
fn is_bracket_key(code: u16) -> bool {
    code == KEY_LEFT_BRACKET || code == KEY_RIGHT_BRACKET
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{Key, Mods};
    use crate::options::Charset;

    fn engine() -> Engine {
        Engine::new(Options::default())
    }

    fn key(ch: char) -> KeyEvent {
        let (key, shift) = match ch {
            'a'..='z' => (
                match ch {
                    'a' => Key::A,
                    'b' => Key::B,
                    'c' => Key::C,
                    'd' => Key::D,
                    'e' => Key::E,
                    'f' => Key::F,
                    'g' => Key::G,
                    'h' => Key::H,
                    'i' => Key::I,
                    'j' => Key::J,
                    'k' => Key::K,
                    'l' => Key::L,
                    'm' => Key::M,
                    'n' => Key::N,
                    'o' => Key::O,
                    'p' => Key::P,
                    'q' => Key::Q,
                    'r' => Key::R,
                    's' => Key::S,
                    't' => Key::T,
                    'u' => Key::U,
                    'v' => Key::V,
                    'w' => Key::W,
                    'x' => Key::X,
                    'y' => Key::Y,
                    _ => Key::Z,
                },
                false,
            ),
            'A'..='Z' => {
                let mut lower = ch.to_ascii_lowercase();
                if lower == 'z' {
                    lower = 'z';
                }
                let mut event = key(lower);
                event.mods.shift = true;
                return event;
            }
            '0'..='9' => (Key::Digit(ch as u8 - b'0'), false),
            ' ' => (Key::Space, false),
            '\t' => (Key::Tab, false),
            '\n' => (Key::Enter, false),
            '\u{8}' => (Key::Backspace, false),
            _ => panic!("test key {ch:?}"),
        };
        KeyEvent {
            key,
            mods: Mods {
                shift,
                ..Mods::default()
            },
            phase: Phase::Down,
            text: Some(ch),
        }
    }

    /// The document model of the reference harness (`press`,
    /// `scripts/parity/harness.cpp`) and `openkey-cli`'s replay. Test-only.
    fn press(engine: &mut Engine, document: &mut String, ch: char) -> Outcome {
        let outcome = engine.apply(&key(ch));
        if let Some(edit) = &outcome.edit {
            let backspaces = usize::from(edit.backspaces);
            // Backspaces count characters, not bytes, exactly as in
            // `openkey-cli`'s replay (`replay.rs::apply_edit`).
            if backspaces > 0 && backspaces < Engine::MAX_BUFFER {
                let keep = document.chars().count().saturating_sub(backspaces);
                *document = document.chars().take(keep).collect();
            }
            match &edit.text {
                Encoded::Utf16(units) => document.push_str(&String::from_utf16_lossy(units)),
                // Every test here types in the default Unicode charset; a byte
                // edit would mean the engine ignored `Options::charset`.
                Encoded::Bytes(_) => panic!("byte-charset edit in a Unicode document model"),
            }
        }
        match outcome.action {
            Action::Pass | Action::BreakWord => {
                if ch == '\u{8}' {
                    document.pop();
                } else {
                    document.push(ch);
                }
            }
            Action::Restore | Action::RestoreAndBreak => document.push(ch),
            Action::Process | Action::Macro => {}
        }
        outcome
    }

    fn type_all(engine: &mut Engine, input: &str) -> String {
        let mut document = String::new();
        for ch in input.chars() {
            press(engine, &mut document, ch);
        }
        document
    }

    #[test]
    fn telex_basics_render_the_documented_words() {
        assert_eq!(type_all(&mut engine(), "vieejt"), "việt");
        assert_eq!(type_all(&mut engine(), "tieengs"), "tiếng");
        assert_eq!(type_all(&mut engine(), "ddi"), "đi");
        assert_eq!(type_all(&mut engine(), "aw"), "ă");
        assert_eq!(type_all(&mut engine(), "ow"), "ơ");
        assert_eq!(type_all(&mut engine(), "uw"), "ư");
        assert_eq!(type_all(&mut engine(), "w"), "ư");
        assert_eq!(type_all(&mut engine(), "dd"), "đ");
    }

    #[test]
    fn a_repeated_tone_key_restores_the_word() {
        assert_eq!(type_all(&mut engine(), "as"), "á");
        assert_eq!(type_all(&mut engine(), "ass"), "as");
        assert_eq!(type_all(&mut engine(), "aas"), "ấ");
        assert_eq!(type_all(&mut engine(), "aasz"), "â");
        assert_eq!(type_all(&mut engine(), "asz"), "a");
    }

    #[test]
    fn every_outcome_carries_its_ext_code() {
        let mut first = engine();
        let mut document = String::new();
        let pass = press(&mut first, &mut document, 'a');
        assert_eq!(pass.action, Action::Pass);
        assert_eq!(pass.ext, Ext::Normal);

        let tab = press(&mut first, &mut document, '\t');
        assert_eq!(tab.action, Action::Pass);
        assert_eq!(tab.ext, Ext::Break);

        let delete = press(&mut first, &mut document, '\u{8}');
        assert_eq!(delete.action, Action::Pass);
        assert_eq!(delete.ext, Ext::Delete);

        // A standalone W sets `hExt = 4` (`Engine.cpp:990`).
        let mut standalone = engine();
        let mut w_document = String::new();
        let w = press(&mut standalone, &mut w_document, 'w');
        assert_eq!(w.action, Action::Process);
        assert_eq!(w.ext, Ext::SuppressEmpty);
        assert_eq!(w_document, "ư");
    }

    #[test]
    fn free_mark_decides_whether_the_mark_migrates() {
        let mut off = engine();
        assert!(!off.options().free_mark);
        assert_eq!(type_all(&mut off, "osang"), "oáng");

        let mut on = Engine::new(Options {
            free_mark: true,
            ..Options::default()
        });
        assert_eq!(type_all(&mut on, "osang"), "óang");
    }

    #[test]
    fn a_break_key_passes_through_and_ends_the_session() {
        let mut engine = engine();
        let mut document = String::new();
        for ch in "vi".chars() {
            press(&mut engine, &mut document, ch);
        }
        let tab = press(&mut engine, &mut document, '\t');
        assert_eq!(tab.action, Action::Pass);
        assert_eq!(tab.ext, Ext::Break);
        assert_eq!(engine.word().len(), 0);

        press(&mut engine, &mut document, 'e');
        assert_eq!(document, "vi\te");
        assert_eq!(engine.word().len(), 1);
    }

    #[test]
    fn session_break_discards_the_word_and_starts_a_new_one() {
        let mut engine = engine();
        let mut document = String::new();
        for ch in "vieejt".chars() {
            press(&mut engine, &mut document, ch);
        }
        assert_eq!(document, "việt");
        assert_eq!(engine.word().len(), 4);

        let outcome = engine.session_break();
        assert_eq!(outcome.action, Action::Pass);
        assert_eq!(outcome.ext, Ext::Break);
        assert_eq!(engine.word().len(), 0);

        press(&mut engine, &mut document, 'a');
        assert_eq!(document, "việta");
    }

    #[test]
    fn upper_case_is_preserved_by_the_encoding_edge() {
        assert_eq!(type_all(&mut engine(), "DDi"), "Đi");
        assert_eq!(type_all(&mut engine(), "AW"), "Ă");
    }

    #[test]
    fn charset_choice_reaches_the_edit_text() {
        // D4: the engine encodes at its edge, so `Options::charset` alone
        // decides the emitted units. `as` produces one edit, `á` — the A row's
        // sắc column in each of the five tables (`Vietnamese.cpp:411-491`) —
        // and the render differs per charset exactly as `SendNewCharString`
        // does (`OpenKey.mm:452-475`).
        let typing = |charset: Charset| {
            let mut engine = Engine::new(Options {
                charset,
                ..Options::default()
            });
            engine.apply(&key('a'));
            engine
                .apply(&key('s'))
                .edit
                .expect("s tones the vowel")
                .text
        };
        assert_eq!(typing(Charset::Unicode), Encoded::Utf16(vec![0x00E1]));
        assert_eq!(typing(Charset::Tcvn3), Encoded::Bytes(vec![0xB8]));
        assert_eq!(
            typing(Charset::VniWindows),
            Encoded::Bytes(vec![0x61, 0xF9])
        );
        assert_eq!(
            typing(Charset::UnicodeCompound),
            Encoded::Utf16(vec![0x0061, 0x0301])
        );
        assert_eq!(typing(Charset::Cp1258), Encoded::Bytes(vec![0x61, 0xEC]));
    }
}
