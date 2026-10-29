//! Platform-neutral input/output types shared by the engine and every adapter.
//!
//! Adapters translate native keycodes (macOS virtual keycodes, Windows VK codes,
//! IBus keysyms) into [`Key`] before calling the engine. Nothing in this crate
//! performs I/O.

/// Physical key identity, normalized across platforms.
///
/// Upstream compiles a different numeric table per platform
/// (`platforms/mac.h`, `platforms/win32.h`); that mapping belongs in the
/// adapter, not in the engine.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    A, B, C, D, E, F, G, H, I, J, K, L, M,
    N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
    Digit(u8),
    Space, Enter, Tab, Escape, Backspace, Delete,
    ArrowLeft, ArrowRight, ArrowUp, ArrowDown,
    LeftBracket, RightBracket,
    Semicolon, Quote, Comma, Dot, Slash, BackSlash,
    Minus, Equals, Backquote,
    /// Anything the engine should treat as a plain break/passthrough key.
    Other(u16),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    /// Command on macOS, Windows key on Windows.
    pub cmd: bool,
    pub caps_lock: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Down,
    Up,
}

#[derive(Clone, Copy, Debug)]
pub struct KeyEvent {
    pub key: Key,
    pub mods: Mods,
    pub phase: Phase,
    /// Character produced by the active keyboard layout, when the platform can
    /// supply it. Used for non-Latin layouts and dead keys.
    pub text: Option<char>,
}

/// What the engine decided to do with the event.
///
/// Mirrors upstream `HoolCodeState` in `DataType.h`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Let the platform deliver the key untouched.
    Pass,
    /// Engine consumed the key; apply [`Edit`] instead.
    Process,
    /// Word boundary reached (space, punctuation, ...).
    BreakWord,
    /// Restore the pre-transformation characters.
    Restore,
    /// Macro expansion replaced the typed sequence.
    Macro,
    /// Restore the key *and* start a new word (invalid word + break char).
    RestoreAndBreak,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Edit {
    /// Characters to delete before inserting `text`.
    pub backspaces: u8,
    /// Text to insert, already encoded in the active charset.
    pub text: String,
    /// Play the system beep (invalid-word feedback).
    pub beep: bool,
}

#[derive(Clone, Debug)]
pub struct Outcome {
    pub action: Action,
    pub edit: Option<Edit>,
    /// Key to re-inject (e.g. the break character after a restore).
    pub passthrough: Option<KeyEvent>,
}
