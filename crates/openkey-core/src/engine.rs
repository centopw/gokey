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
//! `hBPC`'s byte wrap).
//!
//! Macros (C-009) are ported through [`crate::macros`]: `findMacro` is called
//! at the same four loci upstream calls it, `hMacroKey` is [`Engine`]'s
//! `macro_key`, and `hMacroData` is its `macro_data`.
//!
//! Four deviations are guarded instead of copied, because the C versions read
//! out of bounds or depend on undefined behaviour:
//!
//! - `upperCaseFirstCharacter`'s `hMacroKey[0] |= CAPS_MASK`
//!   (`Engine.cpp:1050-1051`) indexes an empty vector; the write happens only
//!   when an entry exists.
//! - The main branch copies `TypingWord[_index - hBPC ..]` into `hMacroKey`
//!   (`Engine.cpp:1526-1530`); for quick-telex `hNCC` reaches one past the
//!   word, where C reads an unwritten slot. The port reads the word model's
//!   zero past the end.
//! - `CHR(_index - 1)` in `checkForStandaloneChar` at `_index == 0`
//!   (`Engine.cpp:996`) reads `TypingWord[-1]`; here the branch is skipped.
//! - `CHR(_index)` in `checkSpelling`'s first-vowel probe can read
//!   `TypingWord[32]` in a full buffer; here it reads the last slot as 0.
//! - A forward delete (`Key::Delete`) has no macOS constant at the pin and is
//!   not in `_breakCode`; it is modelled as a word break instead of guessed.
//!
//! Telex (C-002), VNI (C-003), Simple Telex 1/2 (C-004) and macros (C-009) are
//! ported. A declared `todo!()` would be reported by the replay as
//! `engine-pending`, and any other panic as `engine-panic`, so "not written
//! yet" and "wrong" never share an exit code; no method reaches that path
//! today.

use crate::charset;
use crate::event::{Action, Edit, Encoded, Ext, KeyEvent, Outcome, Phase};
use crate::macros::{Macro, MacroTable};
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
    /// `_hasHandledMacro`: a macro consumed this word, so the space and
    /// restore branches must not touch it again (`Engine.cpp:114, 1388-1395`).
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
    /// `macroMap`: the user's macros, keyed by their converted name
    /// (`Macro.cpp:216-232`). Empty unless [`Engine::with_macros`] supplied one.
    macro_table: MacroTable,
    /// `hMacroKey`: the keystrokes since the last word boundary, in the
    /// engine's own bits (`Engine.cpp:66`).
    macro_key: Vec<u32>,
    /// `hMacroData`: the matched macro's content, ready for `charData`
    /// (`Engine.cpp:67`).
    macro_data: Vec<u32>,
}

impl Engine {
    pub const MAX_BUFFER: usize = crate::word::MAX_LETTERS;

    /// An engine with no macros, which is the app's state before the user
    /// table is loaded.
    pub fn new(options: Options) -> Self {
        Self::with_macros(options, &[])
    }

    /// An engine over the user's macro table (`Macro.cpp:216-232`).
    ///
    /// The table is built once and rebuilt only when the charset changes
    /// (`onTableCodeChange`, `Macro.cpp:242-246`): the names keep the codes they
    /// were converted with, the contents do not.
    pub fn with_macros(options: Options, macros: &[Macro]) -> Self {
        Self {
            check_spelling: options.check_spelling,
            use_spell_checking_before: options.check_spelling,
            macro_table: MacroTable::from_macros(options.charset, macros),
            macro_key: Vec::new(),
            macro_data: Vec::new(),
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

    /// The loaded macro table, for tests and the parity harness.
    pub fn macro_table(&self) -> &MacroTable {
        &self.macro_table
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
        self.macro_table.rebuild(options.charset);
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
        if !self.options.enabled {
            // The English branch returns before the mouse handling
            // (`OpenKey.mm:669-684`), so no session actually starts over. Only
            // the per-event hook state is reset so a stale edit cannot leak.
            self.hook.code = DO_NOTHING;
            self.hook.backspaces = 0;
            self.hook.new_chars = 0;
            return self.outcome();
        }
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
        let mut outcome = self.outcome();
        if outcome.action == Action::Macro {
            // `handleMacro` ends with `SendKeyCode(_keycode | shift)`
            // (`OpenKey.mm:574`): the break key that terminated the macro is
            // typed after the expansion.
            outcome.passthrough = Some(*event);
        }
        outcome
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

        if !self.options.enabled {
            // English mode never reaches `vKeyHandleEvent`: the hook calls
            // `vEnglishMode` and returns the event (`OpenKey.mm:669-684`). The
            // macro tracker only runs when both macro flags are on; with either
            // off the event is returned untouched and even the tracker is left
            // alone.
            self.hook.code = DO_NOTHING;
            self.hook.backspaces = 0;
            self.hook.new_chars = 0;
            if source == Source::Keyboard
                && self.options.use_macro
                && self.options.use_macro_in_english
            {
                self.english_branch(source, data, other_control);
            }
            return;
        }

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

    /// `vEnglishMode` (`Engine.cpp:1284-1312`).
    ///
    /// English mode has no word model, only the macro tracker. Unlike the
    /// Vietnamese space branch it never sets `has_handled_macro`, and its break
    /// test is `is_break_code && !is_char_key_code` — a comma is a break but is
    /// also a character key, so it is kept in the key. Mouse events never get
    /// here (`OpenKey.mm:669-684` only calls this for key down).
    fn english_branch(&mut self, source: Source, data: u16, other_control: bool) {
        self.hook.code = DO_NOTHING;
        if source == Source::Mouse || (other_control && !self.is_caps) {
            self.macro_key.clear();
            self.will_temp_off_engine = false;
        } else if data == KEY_SPACE {
            if !self.has_handled_macro {
                if let Some(content) = self.lookup_macro() {
                    self.macro_data = content;
                    self.hook.code = REPLACE_MACRO;
                    self.hook.backspaces = self.macro_key.len() as u8;
                }
            }
            self.macro_key.clear();
            self.will_temp_off_engine = false;
        } else if data == KEY_DELETE {
            if self.macro_key.pop().is_none() {
                self.will_temp_off_engine = false;
            }
        } else if is_break_code(data) && !is_char_key_code(data) {
            self.macro_key.clear();
            self.will_temp_off_engine = false;
        } else if !self.will_temp_off_engine {
            self.macro_key
                .push(u32::from(data) | if self.is_caps { CAPS_MASK } else { 0 });
        }
    }

    /// `findMacro(hMacroKey, hMacroData)` (`Macro.cpp:155-197`).
    ///
    /// The key vector is passed by reference upstream and is left mutated — it
    /// is converted in place, and the auto-caps path lowercases what it probes —
    /// so the caller's tracker must keep whatever comes back, hit or miss.
    fn lookup_macro(&mut self) -> Option<Vec<u32>> {
        let auto_caps = self.options.auto_caps_macro;
        crate::macros::find_macro(&mut self.macro_key, &self.macro_table, auto_caps)
    }

    /// The first branch of `vKeyHandleEvent` (`Engine.cpp:1321-1383`).
    fn break_branch(&mut self, source: Source, data: u16, word_break: bool) {
        self.hook.code = DO_NOTHING;
        self.hook.backspaces = 0;
        self.hook.new_chars = 0;
        self.hook.ext = 1;

        // The macro check comes first and is independent of `tempDisableKey`:
        // a break key that matches a macro is consumed before quick consonants
        // or restore can claim it (`Engine.cpp:1328-1334`).
        let mut macro_hit = false;
        if self.options.use_macro && is_macro_break_code(data) && !self.has_handled_macro {
            if let Some(content) = self.lookup_macro() {
                self.macro_data = content;
                self.hook.code = REPLACE_MACRO;
                self.hook.backspaces = self.macro_key.len() as u8;
                self.has_handled_macro = true;
                macro_hit = true;
            }
        }

        if macro_hit {
            // Nothing else claims the key.
        } else if (self.options.quick_start_consonant || self.options.quick_end_consonant)
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

        // The break key joins the macro tracker only when it is a character
        // key; anything else ends the tracked run (`Engine.cpp:1367-1373`). The
        // push happens even for a key that arrived with a control flag, exactly
        // like upstream.
        if self.options.use_macro {
            if is_char_key {
                self.macro_key
                    .push(u32::from(data) | if self.is_caps { CAPS_MASK } else { 0 });
            } else {
                self.macro_key.clear();
            }
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
        // A space-terminated macro is checked first, before quick consonants
        // and restore, and the space still counts towards `spaceCount`
        // (`Engine.cpp:1388-1394`).
        let mut macro_hit = false;
        if self.options.use_macro && !self.has_handled_macro {
            if let Some(content) = self.lookup_macro() {
                self.macro_data = content;
                self.hook.code = REPLACE_MACRO;
                self.hook.backspaces = self.macro_key.len() as u8;
                self.space_count += 1;
                self.has_handled_macro = true;
                macro_hit = true;
            }
        }

        if macro_hit {
            // Nothing else claims the space.
        } else if (self.options.quick_start_consonant || self.options.quick_end_consonant)
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

        // Unconditional, even when the macro just fired: the space ends the
        // tracked run (`Engine.cpp:1404-1406`).
        if self.options.use_macro {
            self.macro_key.clear();
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
        if self.options.use_macro {
            // `hMacroKey.size() > 0` guard (`Engine.cpp:1451-1453`); `pop` on an
            // empty vector is the same no-op.
            self.macro_key.pop();
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
        if !is_special_key(self.options.input_method, data) || self.temp_disable_key {
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
        if !self.options.free_mark && !is_key_d(self.options.input_method, data) {
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

        // "insert or replace key for macro feature" (`Engine.cpp:1517-1531`).
        //
        // A plain key is recorded as typed. A transformation replaced the last
        // `backspaces` entries with the `new_chars` entries now at
        // `index - backspaces`, so the tracker is rebuilt from the word instead
        // of guessed. Upstream reads `TypingWord` past `_index` when the
        // grammar moved a mark (`new_chars == backspaces + 1`); that slot is
        // stale buffer state, so the window is clamped to the word and the
        // deviation is recorded in the module docs.
        if self.options.use_macro {
            match self.hook.code {
                DO_NOTHING => self
                    .macro_key
                    .push(u32::from(data) | if self.is_caps { CAPS_MASK } else { 0 }),
                WILL_PROCESS | RESTORE => {
                    let backspaces = usize::from(self.hook.backspaces);
                    for _ in 0..backspaces {
                        self.macro_key.pop();
                    }
                    let index = self.word.len();
                    let start = index.saturating_sub(backspaces);
                    let end = (start + usize::from(self.hook.new_chars)).min(index);
                    for i in start..end {
                        let entry = self.word.bits_at(i);
                        self.macro_key.push(entry);
                    }
                }
                _ => {}
            }
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
            // The macro's content is what was typed, so that is the history
            // entry; it is chunked at `MAX_BUFF` and always stored at least
            // once, even when the content is empty (`Engine.cpp:400-414`).
            let mut chunk: Vec<u32> = Vec::new();
            for (i, &entry) in self.macro_data.iter().enumerate() {
                if i != 0 && i % MAX_LETTERS == 0 {
                    self.typing_states.push(std::mem::take(&mut chunk));
                }
                chunk.push(entry);
            }
            self.typing_states.push(chunk);
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
                    if !is_key_d(self.options.input_method, code) {
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
            // Upstream writes `hMacroKey[0]` unguarded (`Engine.cpp:1050-1051`),
            // which is out of bounds on an empty tracker; only the effect on an
            // existing entry is observable, so the write is guarded.
            if self.options.use_macro {
                if let Some(first) = self.macro_key.first_mut() {
                    *first |= CAPS_MASK;
                }
            }
        }
    }

    /// `handleMainKey` (`Engine.cpp:1055-1193`).
    fn handle_main_key(&mut self, data: u16) {
        let method = self.options.input_method;
        if is_key_z(method, data) {
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

        if is_key_d(method, data) {
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

        if let Some(mark) = mark_mask(method, data) {
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

        // VNI finds the last O/A/E for `keyForAEO` (`Engine.cpp:1145-1152`);
        // when none exists the previous `vowelEndIndex` is reused, like
        // upstream's global.
        if method == InputMethod::Vni {
            for i in (0..self.word.len()).rev() {
                if matches!(self.chr(i), KEY_O | KEY_A | KEY_E) {
                    self.vowel_end = i as u8;
                    break;
                }
            }
        }
        // `keyForAEO` (`Engine.cpp:1154`): the pressed key in Telex; `7`/`8`
        // stand for W in VNI, and `6` doubles whichever vowel `VEI` points at.
        let key_for_aeo = match method {
            InputMethod::Vni => match data {
                KEY_7 | KEY_8 => KEY_W,
                KEY_6 => self.chr(self.vowel_end as usize),
                _ => data,
            },
            InputMethod::Telex | InputMethod::SimpleTelex1 | InputMethod::SimpleTelex2 => data,
        };
        self.is_changed = false;
        for row in vowel(key_for_aeo) {
            if self.word.len() < row.len() {
                continue;
            }
            if self.check_correct_vowel(row, data) {
                self.is_changed = true;
                if is_key_double(method, data) {
                    self.insert_aoe(key_for_aeo);
                } else if is_key_w(method, data) {
                    // `7`/`8` are refused when the vowel they would modify is
                    // not one they can touch (`Engine.cpp:1170-1178`); the
                    // digit then passes through because `isChanged` is
                    // already set and nothing is inserted.
                    if method == InputMethod::Vni {
                        for j in (0..self.word.len()).rev() {
                            if matches!(self.chr(j), KEY_O | KEY_U | KEY_A | KEY_E) {
                                self.vowel_end = j as u8;
                                break;
                            }
                        }
                        let vei = self.vowel_end as usize;
                        if (data == KEY_7
                            && self.chr(vei) == KEY_A
                            && (vei == 0 || self.chr(vei - 1) != KEY_U))
                            || (data == KEY_8 && matches!(self.chr(vei), KEY_O | KEY_U))
                        {
                            break;
                        }
                    }
                    self.insert_w(key_for_aeo);
                }
                break;
            }
        }
        if !self.is_changed {
            // `Engine.cpp:1187`: an unconsumed `w` becomes `ư` for every
            // method except Simple Telex 1, which types the letter literally.
            if data == KEY_W && method != InputMethod::SimpleTelex1 {
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
        let restrict_ct = is_restricted_end_mark(self.options.input_method, mark_key);
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
            // `handleMacro` deletes the typed key, then sends the macro content
            // (`OpenKey.mm:544-575`). `macroData` is in reading order, so its
            // own renderer walks it forwards (`SendNewCharString(true)`).
            REPLACE_MACRO => (
                Action::Macro,
                Some(Edit {
                    backspaces: self.hook.backspaces,
                    text: charset::render_macro(&self.macro_data, self.options.charset),
                    beep: false,
                }),
            ),
            // `vBreakWord` is never assigned upstream (`DataType.h:60`).
            _ => (Action::BreakWord, None),
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

    fn vni_engine() -> Engine {
        Engine::new(Options {
            input_method: InputMethod::Vni,
            ..Options::default()
        })
    }

    fn simple_telex_1_engine() -> Engine {
        Engine::new(Options {
            input_method: InputMethod::SimpleTelex1,
            ..Options::default()
        })
    }

    fn simple_telex_2_engine() -> Engine {
        Engine::new(Options {
            input_method: InputMethod::SimpleTelex2,
            ..Options::default()
        })
    }

    /// A Telex engine over the default profile with the given flags switched on.
    fn telex_with(apply: impl FnOnce(&mut Options)) -> Engine {
        let mut options = Options::default();
        apply(&mut options);
        Engine::new(options)
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
            '.' => (Key::Dot, false),
            ',' => (Key::Comma, false),
            '-' => (Key::Minus, false),
            '=' => (Key::Equals, false),
            '[' => (Key::LeftBracket, false),
            ']' => (Key::RightBracket, false),
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
        let is_macro = outcome.action == Action::Macro;
        if let Some(edit) = &outcome.edit {
            let backspaces = usize::from(edit.backspaces);
            // Backspaces count characters, not bytes, exactly as in
            // `openkey-cli`'s replay (`replay.rs::apply_edit`). `handleMacro`
            // has no `MAX_BUFF` guard (`OpenKey.mm:552-556`), so a macro edit
            // deletes exactly what it says.
            if backspaces > 0 && (is_macro || backspaces < Engine::MAX_BUFFER) {
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
            Action::Process => {}
            Action::Macro => {
                // `handleMacro` retypes the break key after the expansion
                // (`OpenKey.mm:574`), which is the outcome's passthrough.
                let event = outcome
                    .passthrough
                    .expect("a macro outcome carries the break key");
                document.push(event.text.unwrap_or(ch));
            }
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
    fn vni_digit_keys_render_the_documented_words() {
        assert_eq!(type_all(&mut vni_engine(), "a6"), "â");
        assert_eq!(type_all(&mut vni_engine(), "a8"), "ă");
        assert_eq!(type_all(&mut vni_engine(), "e6"), "ê");
        assert_eq!(type_all(&mut vni_engine(), "o7"), "ơ");
        assert_eq!(type_all(&mut vni_engine(), "u7"), "ư");
        assert_eq!(type_all(&mut vni_engine(), "d9"), "đ");
        assert_eq!(type_all(&mut vni_engine(), "a1"), "á");
        assert_eq!(type_all(&mut vni_engine(), "a61"), "ấ");
        assert_eq!(type_all(&mut vni_engine(), "vie65t"), "việt");
        assert_eq!(type_all(&mut vni_engine(), "d9uo7c5"), "được");
        assert_eq!(type_all(&mut vni_engine(), "thu7o7ng"), "thương");
    }

    #[test]
    fn vni_digits_type_literally_when_the_reference_does_not_consume_them() {
        // `7` on a lone `a`, `8` on `o`, a repeated tone digit and `0` with no
        // mark all fall through to the literal digit (`Engine.cpp:1154-1186`).
        assert_eq!(type_all(&mut vni_engine(), "a7"), "a7");
        assert_eq!(type_all(&mut vni_engine(), "o8"), "o8");
        assert_eq!(type_all(&mut vni_engine(), "a11"), "a1");
        assert_eq!(type_all(&mut vni_engine(), "a0"), "a0");
    }

    #[test]
    fn vni_zero_removes_the_tone_and_six_carries_upper_case() {
        assert_eq!(type_all(&mut vni_engine(), "a10"), "a");
        assert_eq!(type_all(&mut vni_engine(), "vie6t50"), "viêt");
        assert_eq!(type_all(&mut vni_engine(), "A61"), "Ấ");
    }

    #[test]
    fn vni_grave_hook_and_tilde_do_not_sit_on_a_c_or_t_ending() {
        // `2`/`3`/`4` share Telex F/R/X's restriction (`Engine.cpp:488-493`):
        // on a "c"/"t" ending they stay literal, while acute and dot-below are
        // accepted. Expected documents from the pinned reference harness.
        assert_eq!(type_all(&mut vni_engine(), "ac2"), "ac2");
        assert_eq!(type_all(&mut vni_engine(), "at3"), "at3");
        assert_eq!(type_all(&mut vni_engine(), "ac1"), "ác");
        assert_eq!(type_all(&mut vni_engine(), "at5"), "ạt");
    }

    #[test]
    fn simple_telex_1_doubles_letters_and_places_the_tone() {
        assert_eq!(type_all(&mut simple_telex_1_engine(), "aa"), "â");
        assert_eq!(type_all(&mut simple_telex_1_engine(), "ee"), "ê");
        assert_eq!(type_all(&mut simple_telex_1_engine(), "oo"), "ô");
        assert_eq!(type_all(&mut simple_telex_1_engine(), "dd"), "đ");
        assert_eq!(type_all(&mut simple_telex_1_engine(), "vieejt"), "việt");
        assert_eq!(type_all(&mut simple_telex_1_engine(), "tieengs"), "tiếng");
        // Simple Telex 1 places the tone inside `uo` differently from Telex:
        // `nguoif` is `nguòi`, not `người` (the corpus pins the same row).
        assert_eq!(type_all(&mut simple_telex_1_engine(), "nguoif"), "nguòi");
    }

    #[test]
    fn simple_telex_2_reads_w_as_the_horn() {
        assert_eq!(type_all(&mut simple_telex_2_engine(), "w"), "ư");
        assert_eq!(type_all(&mut simple_telex_2_engine(), "wa"), "ưa");
        assert_eq!(type_all(&mut simple_telex_2_engine(), "wow"), "ươ");
        assert_eq!(type_all(&mut simple_telex_2_engine(), "wows"), "ướ");
        assert_eq!(type_all(&mut simple_telex_2_engine(), "wowcj"), "ược");
        assert_eq!(type_all(&mut simple_telex_2_engine(), "twow"), "tươ");
        assert_eq!(type_all(&mut simple_telex_2_engine(), "ww"), "w");
    }

    #[test]
    fn an_unconsumed_w_is_literal_only_in_simple_telex_1() {
        // `Engine.cpp:1187`: the standalone-`w` transformation is skipped for
        // Simple Telex 1, so the letter is typed; Telex and Simple Telex 2
        // produce `ư`.
        assert_eq!(type_all(&mut simple_telex_1_engine(), "w"), "w");
        assert_eq!(type_all(&mut engine(), "w"), "ư");
        assert_eq!(type_all(&mut simple_telex_2_engine(), "w"), "ư");
    }

    #[test]
    fn brackets_commit_the_word_instead_of_transforming_in_simple_telex() {
        // The brackets are not special keys in Simple Telex 1/2
        // (`DataType.h:141-145`), so the press commits the word and types the
        // bracket (`Engine.cpp:1541`). In Telex the same press is a
        // transformation. Expected documents measured from the pinned harness.
        assert_eq!(type_all(&mut simple_telex_1_engine(), "as["), "á[");
        assert_eq!(type_all(&mut simple_telex_1_engine(), "as]"), "á]");
        assert_eq!(type_all(&mut simple_telex_2_engine(), "wow["), "ươ[");
        assert_eq!(type_all(&mut simple_telex_2_engine(), "wow]"), "ươ]");
        assert_eq!(type_all(&mut engine(), "as["), "aớ");
        assert_eq!(type_all(&mut engine(), "as]"), "aứ");
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
    fn the_spelling_check_blocks_a_mark_on_an_invalid_cluster() {
        // `zz` and `xq` cannot start a syllable, so `checkSpelling` leaves
        // `tempDisableKey` set and the `s` is inserted raw instead of toning the
        // word (`Engine.cpp:176-288`, `Engine.cpp:1486`).
        assert_eq!(type_all(&mut engine(), "zzas"), "zzas");
        assert_eq!(type_all(&mut engine(), "xq"), "xq");

        // With the check off nothing judges the cluster, so the tone lands.
        let mut off = Engine::new(Options {
            check_spelling: false,
            ..Options::default()
        });
        assert_eq!(type_all(&mut off, "zzas"), "zzá");
    }

    #[test]
    fn allow_zfwj_changes_whether_z_and_j_can_start_a_syllable() {
        // Off (the default profile): `z`/`j` are not onset consonants, so the
        // mark is refused and the key is inserted raw.
        assert_eq!(type_all(&mut engine(), "zas"), "zas");
        assert_eq!(type_all(&mut engine(), "jis"), "jis");

        let mut z = Engine::new(Options {
            allow_consonant_zfwj: true,
            ..Options::default()
        });
        assert_eq!(type_all(&mut z, "zas"), "zá");

        let mut j = Engine::new(Options {
            allow_consonant_zfwj: true,
            ..Options::default()
        });
        assert_eq!(type_all(&mut j, "jis"), "jí");
    }

    #[test]
    fn modern_orthography_widens_the_mark_position() {
        // `oà, uý` instead of `òa, úy`; the traditional spellings are the macOS
        // first-run default (corpus `opt-modern-*`, `opt-trad-*`).
        for (input, widened, traditional) in [
            ("hoaf", "hoà", "hòa"),
            ("thuys", "thuý", "thúy"),
            ("khoer", "khoẻ", "khỏe"),
        ] {
            assert_eq!(type_all(&mut engine(), input), traditional, "{input}");
            let mut modern = telex_with(|options| options.modern_orthography = true);
            assert_eq!(type_all(&mut modern, input), widened, "{input}");
        }
    }

    #[test]
    fn quick_telex_expands_the_seven_documented_pairs() {
        // `cc=ch, gg=gi, kk=kh, nn=ng, qq=qu, pp=ph, tt=th` (corpus
        // `opt-quick-*`): the second key of the pair performs the expansion.
        for (input, expanded) in [
            ("cc", "ch"),
            ("gg", "gi"),
            ("kk", "kh"),
            ("nn", "ng"),
            ("qq", "qu"),
            ("pp", "ph"),
            ("tt", "th"),
        ] {
            assert_eq!(type_all(&mut engine(), input), input, "{input}");
            let mut quick = telex_with(|options| options.quick_telex = true);
            assert_eq!(type_all(&mut quick, input), expanded, "{input}");
        }
    }

    #[test]
    fn zfwj_keeps_the_letter_literal_when_the_flag_is_on() {
        // `opt-zfwj-z` / `opt-zfwj-f`: the flag widens the onset set for marks
        // (`allow_zfwj_changes_whether_z_and_j_can_start_a_syllable` pins that
        // side); it does not rewrite the letter itself.
        let mut z = telex_with(|options| options.allow_consonant_zfwj = true);
        assert_eq!(type_all(&mut z, "z"), "z");
        let mut f = telex_with(|options| options.allow_consonant_zfwj = true);
        assert_eq!(type_all(&mut f, "f"), "f");
    }

    #[test]
    fn quick_start_consonant_expands_on_the_break_key() {
        // The lookup runs from the break branches and returns early while
        // `_index <= 1` (`Engine.cpp:1238`, `:1333`), so `f` alone never becomes
        // `ph`; the break key stays in the document (corpus `opt-start-*`).
        let mut bare = telex_with(|options| options.quick_start_consonant = true);
        assert_eq!(type_all(&mut bare, "fa"), "fa");
        let mut f = telex_with(|options| options.quick_start_consonant = true);
        assert_eq!(type_all(&mut f, "fa "), "pha ");
        let mut j = telex_with(|options| options.quick_start_consonant = true);
        assert_eq!(type_all(&mut j, "ja "), "gia ");
        let mut caps = telex_with(|options| options.quick_start_consonant = true);
        assert_eq!(type_all(&mut caps, "Fan "), "Phan ");

        // Recorded upstream quirk: `w` is rewritten to `ư` on insert
        // (`Engine.cpp:1186`) before the break-time lookup reads the first
        // letter, so the documented `w->qu` shortcut cannot match here.
        // Bug-for-bug parity (DL-026); the corpus pins `ưa `.
        let mut w = telex_with(|options| options.quick_start_consonant = true);
        assert_eq!(type_all(&mut w, "wa "), "ưa ");
    }

    #[test]
    fn quick_end_consonant_expands_on_the_break_key() {
        for (input, expanded) in [("ag ", "ang "), ("ah ", "anh "), ("ak ", "ach ")] {
            let mut quick = telex_with(|options| options.quick_end_consonant = true);
            assert_eq!(type_all(&mut quick, input), expanded, "{input}");
        }
        assert_eq!(type_all(&mut engine(), "ag "), "ag ");
    }

    #[test]
    fn upper_case_first_capitalises_after_a_stop_and_a_newline() {
        // `Engine.cpp:1376-1381` sets `_upperCaseStatus`, `:1532` applies it at
        // `_index == 1` (corpus `opt-upper-*`).
        let mut dot = telex_with(|options| options.upper_case_first_char = true);
        assert_eq!(type_all(&mut dot, ". vieejt"), ". Việt");
        let mut newline = telex_with(|options| options.upper_case_first_char = true);
        assert_eq!(type_all(&mut newline, "\nvieejt"), "\nViệt");
        assert_eq!(type_all(&mut engine(), ". vieejt"), ". việt");
    }

    #[test]
    fn no_spelling_keeps_the_mark_on_a_cluster_the_check_would_refuse() {
        // `opt-no-spelling` pins `vieejt`, where both settings agree; the flag's
        // visible effect is on an invalid cluster (`Engine.cpp:1486`).
        //
        // The two inputs need separate engines (a second word typed onto the
        // first is not what these cases mean), so the option set is named once
        // and applied to each instead of repeating the builder call.
        let no_spelling = |options: &mut Options| options.check_spelling = false;
        assert_eq!(type_all(&mut telex_with(no_spelling), "vieejt"), "việt");
        assert_eq!(type_all(&mut telex_with(no_spelling), "zzas"), "zzá");
        assert_eq!(type_all(&mut engine(), "zzas"), "zzas");
    }

    #[test]
    fn option_combinations_keep_each_flag_effective() {
        // `options-combos.tsv`: crossings the single-flag rows cannot show. The
        // corpus rows pin the same documents through the reference engine; this
        // test pins that each flag still does its own job when another is set.
        let mut start_quick = telex_with(|options| {
            options.quick_start_consonant = true;
            options.quick_telex = true;
        });
        assert_eq!(type_all(&mut start_quick, "fa "), "pha ");

        let mut end_modern = telex_with(|options| {
            options.quick_end_consonant = true;
            options.modern_orthography = true;
        });
        assert_eq!(type_all(&mut end_modern, "ag "), "ang ");

        let mut zfwj_quick = telex_with(|options| {
            options.allow_consonant_zfwj = true;
            options.quick_telex = true;
        });
        assert_eq!(type_all(&mut zfwj_quick, "zz"), "zz");

        // `upper-case-first` does not reach the two letters quick Telex expands
        // to: capitalisation applies at the first inserted character only.
        let mut upper_quick = telex_with(|options| {
            options.upper_case_first_char = true;
            options.quick_telex = true;
        });
        assert_eq!(type_all(&mut upper_quick, ". cc"), ". ch");

        let mut upper_modern = telex_with(|options| {
            options.upper_case_first_char = true;
            options.modern_orthography = true;
        });
        assert_eq!(type_all(&mut upper_modern, ". hoaf"), ". Hoà");

        let mut upper_nospell = telex_with(|options| {
            options.upper_case_first_char = true;
            options.check_spelling = false;
        });
        assert_eq!(type_all(&mut upper_nospell, ". hoaf"), ". Hòa");

        let mut modern_nospell = telex_with(|options| {
            options.modern_orthography = true;
            options.check_spelling = false;
        });
        assert_eq!(type_all(&mut modern_nospell, "thuys"), "thuý");
    }

    #[test]
    fn a_break_key_restores_an_invalid_word_to_its_raw_keys() {
        // The break branch restores with `vRestoreAndStartNewSession`
        // (`Engine.cpp:1335-1341`); the raw keys replace the marked word and the
        // break key itself passes through.
        let mut engine = Engine::new(Options {
            restore_if_invalid: true,
            ..Options::default()
        });
        let mut document = String::new();
        for ch in "asd".chars() {
            press(&mut engine, &mut document, ch);
        }
        assert_eq!(document, "ád");

        let outcome = press(&mut engine, &mut document, '.');
        assert_eq!(outcome.action, Action::RestoreAndBreak);
        assert_eq!(document, "asd.");
        // The engine leaves the restored raw keys in the word; the platform layer
        // is what starts the new session for this hook code (`OpenKey.mm:492-507`).
        assert_eq!(engine.word().len(), 3);
    }

    #[test]
    fn space_restores_an_invalid_word_before_the_break() {
        // The space branch restores with `vRestore` (`Engine.cpp:1384-1397`), so
        // the raw word is emitted and the space follows it.
        let mut engine = Engine::new(Options {
            restore_if_invalid: true,
            ..Options::default()
        });
        let mut document = String::new();
        for ch in "asd".chars() {
            press(&mut engine, &mut document, ch);
        }
        assert_eq!(document, "ád");

        let outcome = press(&mut engine, &mut document, ' ');
        assert_eq!(outcome.action, Action::Restore);
        assert_eq!(document, "asd ");
    }

    #[test]
    fn the_restore_path_is_off_under_the_default_options() {
        // `restore_if_invalid` defaults false (the macOS first-run profile), so
        // the marked word survives and the space passes through untouched.
        let mut engine = engine();
        assert!(!engine.options().restore_if_invalid);
        let mut document = String::new();
        for ch in "asd".chars() {
            press(&mut engine, &mut document, ch);
        }
        assert_eq!(document, "ád");

        let outcome = press(&mut engine, &mut document, ' ');
        assert_eq!(outcome.action, Action::Pass);
        assert_eq!(document, "ád ");
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

    /// A Telex engine over the given macros, with the given flags switched on.
    fn macro_engine(apply: impl FnOnce(&mut Options), macros: &[Macro]) -> Engine {
        let mut options = Options::default();
        apply(&mut options);
        Engine::with_macros(options, macros)
    }

    fn ko_macros() -> Vec<Macro> {
        vec![Macro::new("ko", "không")]
    }

    #[test]
    fn a_space_terminated_macro_replaces_the_typed_key() {
        // `findMacro` in the space branch sets `vReplaceMaro` with the key
        // length as the backspace count (`Engine.cpp:1388-1394`) and
        // `handleMacro` retypes the space (`OpenKey.mm:544-575`).
        let mut engine = macro_engine(|_| {}, &ko_macros());
        assert_eq!(type_all(&mut engine, "ko "), "không ");
    }

    #[test]
    fn a_macro_can_be_terminated_by_any_of_its_break_keys() {
        // Each case gets its own engine, like the parity harness: the comma
        // that ends a macro is itself a character key and stays in the tracker.
        for (input, expected) in [("ko,", "không,"), ("ko\n", "không\n"), ("ko.", "không.")] {
            let mut engine = macro_engine(|_| {}, &ko_macros());
            assert_eq!(type_all(&mut engine, input), expected, "{input:?}");
        }
    }

    #[test]
    fn minus_and_equals_are_macro_break_keys_too() {
        // `_macroBreakCode` is return, comma, dot, slash, semicolon, quote,
        // backslash, minus, equals (`Engine.cpp:30-32`); all nine also sit in
        // `_breakCode`, so each ends the word and is retyped after the content.
        for (input, expected) in [("ko-", "không-"), ("ko=", "không=")] {
            let mut engine = macro_engine(|_| {}, &ko_macros());
            assert_eq!(type_all(&mut engine, input), expected, "{input:?}");
        }
    }

    #[test]
    fn a_macro_outcome_deletes_the_key_and_retypes_the_break_key() {
        let mut engine = macro_engine(|_| {}, &ko_macros());
        engine.apply(&key('k'));
        engine.apply(&key('o'));
        let outcome = engine.apply(&key(' '));
        assert_eq!(outcome.action, Action::Macro);
        let edit = outcome.edit.expect("a macro carries its edit");
        assert_eq!(edit.backspaces, 2);
        assert_eq!(
            edit.text,
            Encoded::Utf16("không".encode_utf16().collect::<Vec<u16>>())
        );
        assert!(!edit.beep);
        assert_eq!(outcome.passthrough.expect("the break key").key, Key::Space);
    }

    #[test]
    fn a_key_outside_the_table_keeps_the_literal_text() {
        let mut engine = macro_engine(|_| {}, &ko_macros());
        assert_eq!(type_all(&mut engine, "ka "), "ka ");
        // An empty table is the negative control the corpus row pins.
        assert_eq!(type_all(&mut Engine::new(Options::default()), "ko "), "ko ");
    }

    #[test]
    fn use_macro_off_never_matches() {
        let mut engine = macro_engine(|o| o.use_macro = false, &ko_macros());
        assert_eq!(type_all(&mut engine, "ko "), "ko ");
    }

    #[test]
    fn a_backspace_shortens_the_tracked_key() {
        // The delete branch pops one entry from `hMacroKey` in its final else
        // (`Engine.cpp:1451-1453`), so the macro matches after the correction.
        // `b` is not a Telex transformation key, unlike `x`.
        let mut engine = macro_engine(|_| {}, &ko_macros());
        assert_eq!(type_all(&mut engine, "kob\u{8} "), "không ");
    }

    #[test]
    fn auto_caps_uppercases_the_first_letter_of_the_content() {
        let mut engine = macro_engine(|o| o.auto_caps_macro = true, &ko_macros());
        assert_eq!(type_all(&mut engine, "Ko "), "Không ");
        assert_eq!(type_all(&mut engine, "ko "), "không ");
    }

    #[test]
    fn auto_caps_off_leaves_a_capitalised_key_literal() {
        let mut engine = macro_engine(|_| {}, &ko_macros());
        assert_eq!(type_all(&mut engine, "Ko "), "Ko ");
    }

    #[test]
    fn auto_caps_only_probes_when_the_second_key_changes() {
        // `findMacro` only lowercases the rest and re-cases the content when
        // `modifyCaseUnicode(key[1])` changed something (`Macro.cpp:168-196`);
        // a capitalised second key leaves the first one untouched and the
        // lookup misses.
        let mut engine = macro_engine(|o| o.auto_caps_macro = true, &ko_macros());
        assert_eq!(type_all(&mut engine, "kO "), "kO ");
    }

    #[test]
    fn upper_case_first_char_writes_the_caps_bit_into_the_macro_key() {
        // With auto-caps on but no caps write, the tracked key would be `ko`
        // and the content would stay lower case; `upperCaseFirstCharacter`
        // puts the bit on `hMacroKey[0]` (`Engine.cpp:1050-1051`), so the match
        // goes through the auto-caps path and the content is capitalised.
        let mut engine = macro_engine(
            |o| {
                o.auto_caps_macro = true;
                o.upper_case_first_char = true;
            },
            &ko_macros(),
        );
        assert_eq!(type_all(&mut engine, "\nko "), "\nKhông ");
    }

    #[test]
    fn english_mode_matches_only_when_macro_in_english_is_on() {
        let mut on = macro_engine(
            |o| {
                o.enabled = false;
                o.use_macro_in_english = true;
            },
            &ko_macros(),
        );
        assert_eq!(type_all(&mut on, "ko "), "không ");
        assert_eq!(type_all(&mut on, "ka "), "ka ");

        let mut off = macro_engine(|o| o.enabled = false, &ko_macros());
        assert_eq!(type_all(&mut off, "ko "), "ko ");
    }

    #[test]
    fn english_mode_does_not_run_the_vietnamese_transformations() {
        let mut engine = macro_engine(
            |o| {
                o.enabled = false;
                o.use_macro_in_english = true;
            },
            &ko_macros(),
        );
        assert_eq!(type_all(&mut engine, "as"), "as");
        assert_eq!(type_all(&mut engine, "vieejt"), "vieejt");
    }

    #[test]
    fn macro_content_may_hold_spaces_and_capitals() {
        let table = [Macro::new("vn", "Việt Nam")];
        let mut engine = Engine::with_macros(Options::default(), &table);
        assert_eq!(type_all(&mut engine, "vn "), "Việt Nam ");
    }

    #[test]
    fn an_empty_macro_content_still_deletes_the_typed_key() {
        let table = [Macro::new("x", "")];
        let mut engine = Engine::with_macros(Options::default(), &table);
        assert_eq!(type_all(&mut engine, "x "), " ");
    }

    #[test]
    fn the_macro_content_is_encoded_in_the_active_charset() {
        let table = [Macro::new("vn", "Việt")];
        let mut engine = Engine::with_macros(
            Options {
                charset: Charset::VniWindows,
                ..Options::default()
            },
            &table,
        );
        engine.apply(&key('v'));
        engine.apply(&key('n'));
        let edit = engine.apply(&key(' ')).edit.expect("the macro fires");
        assert_eq!(edit.backspaces, 2);
        // `ệ` is the VNI-Windows `E|TONE_MASK` nặng cell 0xE465, which the
        // charset sends as `e` then the 0xE4 glyph byte.
        assert_eq!(
            edit.text,
            Encoded::Bytes(vec![0x56, 0x69, 0x65, 0xE4, 0x74])
        );
    }

    #[test]
    fn changing_the_charset_rebuilds_the_content_but_not_the_key() {
        // `onTableCodeChange` re-converts only the content (`Macro.cpp:242-246`).
        let table = [Macro::new("vn", "Việt")];
        let mut engine = Engine::with_macros(Options::default(), &table);
        engine.set_options(Options {
            charset: Charset::Tcvn3,
            ..Options::default()
        });
        engine.apply(&key('v'));
        engine.apply(&key('n'));
        let edit = engine.apply(&key(' ')).edit.expect("the macro fires");
        // `ệ` in TCVN3 is the `E|TONE_MASK` nặng cell 0xD6 (one byte); the name
        // `vn` is still the same key because only the content is re-converted.
        assert_eq!(edit.text, Encoded::Bytes(vec![0x56, 0x69, 0xD6, 0x74]));
    }
}
