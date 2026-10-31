//! Keystroke state machine — the port of upstream `Engine.cpp` (~52 KB).
//!
//! Keep this module pure: no I/O, no globals, no OS calls. Every public
//! function must be deterministic for a given (`Options`, buffer, `KeyEvent`)
//! triple so the parity harness can diff it against the reference C++ build.
//!
//! Upstream encodes tone/mark/caps flags in the high bits of `Uint32 charData`
//! (`DataType.h:92-122`). The port keeps the typed [`Word`] model instead; see
//! [`crate::word`] for the bit layout that survives only as a reasoning aid.

use crate::event::{KeyEvent, Outcome};
use crate::options::Options;
pub use crate::word::{Letter, Mark, Word};

use crate::word::MAX_LETTERS;

/// Per-focus-session typing state, bounded to 32 characters like upstream
/// `MAX_BUFF` (`DataType.h:19`).
#[derive(Debug)]
pub struct Engine {
    options: Options,
    word: Word,
}

impl Engine {
    /// Upstream `MAX_BUFF`; alias of [`MAX_LETTERS`] for callers that used the
    /// scaffold's name.
    pub const MAX_BUFFER: usize = MAX_LETTERS;

    pub fn new(options: Options) -> Self {
        Self {
            options,
            word: Word::new(),
        }
    }

    pub fn options(&self) -> &Options {
        &self.options
    }

    pub fn set_options(&mut self, options: Options) {
        self.options = options;
    }

    /// Current word, for tests and the parity harness.
    ///
    /// Rendering it needs the charset tables (C-005); until they land, read it
    /// through [`Word::get`]/[`Word::len`].
    pub fn buffer(&self) -> &Word {
        &self.word
    }

    /// Start a new word (upstream `startNewSession`, `Engine.cpp:457`).
    pub fn reset(&mut self) {
        self.word.clear();
    }

    /// Feed one key event.
    ///
    /// TODO(C-002): port the Telex path of `Engine::vKeyHandleEvent`.
    /// TODO(C-003): add VNI. TODO(C-004): add Simple Telex 1/2.
    pub fn apply(&mut self, _event: &KeyEvent) -> Outcome {
        todo!("port Engine.cpp — task C-002 (Telex)")
    }
}
