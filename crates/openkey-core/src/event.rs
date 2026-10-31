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
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    Digit(u8),
    Space,
    Enter,
    Tab,
    Escape,
    Backspace,
    Delete,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
    LeftBracket,
    RightBracket,
    Semicolon,
    Quote,
    Comma,
    Dot,
    Slash,
    BackSlash,
    Minus,
    Equals,
    Backquote,
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

/// The text an [`Edit`] inserts, in the active charset (ADR-0002 D4).
///
/// Four of the five charsets upstream supports are byte-oriented and do not
/// round-trip through UTF-8, so a `String` cannot carry them; the fifth,
/// Unicode Compound, needs a second code unit for the combining mark. The
/// variant is decided by `Options.charset` alone — no adapter holds a charset
/// table — so two adapters cannot disagree about what TCVN3 means.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Encoded {
    /// Unicode and Unicode Compound: UTF-16 code units, ready for the platform
    /// to send, combining marks included.
    Utf16(Vec<u16>),
    /// TCVN3, VNI-Windows and CP1258: the charset's bytes, in order.
    Bytes(Vec<u8>),
}

impl Encoded {
    /// No characters to insert.
    pub fn is_empty(&self) -> bool {
        match self {
            Encoded::Utf16(units) => units.is_empty(),
            Encoded::Bytes(bytes) => bytes.is_empty(),
        }
    }
}

impl Default for Encoded {
    /// An empty `Utf16`: the variant with no payload, so `Edit::default()` means
    /// "delete nothing, insert nothing".
    fn default() -> Self {
        Encoded::Utf16(Vec::new())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Edit {
    /// Characters to delete before inserting `text`.
    pub backspaces: u8,
    /// Text to insert, already encoded in the active charset.
    pub text: Encoded,
    /// Play the system beep (invalid-word feedback).
    pub beep: bool,
}

/// Upstream `extCode` (`DataType.h:69-75`): why a passing key was not consumed.
///
/// Adapters use it for their own bookkeeping (the macOS hook tracks the
/// `_syncKey` length), never to change what is typed. Upstream leaves `0` as a
/// transient value inside the main branch; every path that returns to the
/// adapter ends on 1-4, so `Normal` (3) is the neutral value here.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Ext {
    /// 3 — an ordinary key; also the neutral value.
    #[default]
    Normal,
    /// 1 — word break.
    Break,
    /// 2 — delete key.
    Delete,
    /// 4 — do not send an empty character (`OpenKey.mm:742`).
    SuppressEmpty,
}

#[derive(Clone, Debug)]
pub struct Outcome {
    pub action: Action,
    pub edit: Option<Edit>,
    /// Key to re-inject (e.g. the break character after a restore).
    pub passthrough: Option<KeyEvent>,
    /// Why the key was or was not consumed (ADR-0002 D5).
    pub ext: Ext,
}
