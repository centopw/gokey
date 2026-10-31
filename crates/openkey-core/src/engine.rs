//! Keystroke state machine — the port of upstream `Engine.cpp` (~52 KB).
//!
//! Keep this module pure: no I/O, no globals, no OS calls. Every public
//! function must be deterministic for a given (`Options`, buffer, `KeyEvent`)
//! triple so the parity harness can diff it against the reference C++ build.
//!
//! Upstream encodes tone/mark/caps flags in the high bits of `Uint32 charData`
//! (`CAPS_MASK`, `TONE_MASK`, `MARK1_MASK`, ...). The port keeps a typed word
//! model instead, with a `legacy` compatibility shim only if parity requires it.

use crate::event::{KeyEvent, Outcome};
use crate::options::Options;

pub use crate::word::{Letter, Mark, Word};

/// Per-focus-session typing state, bounded to 32 characters like upstream
/// `MAX_BUFF`.
#[derive(Debug)]
pub struct Engine {
    options: Options,
    word: Word,
}

impl Engine {
    pub const MAX_BUFFER: usize = crate::word::MAX_LETTERS;

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

    /// Current word model, for tests and the parity harness.
    pub fn word(&self) -> &Word {
        &self.word
    }

    /// Start a new word (upstream `startNewSession`).
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
