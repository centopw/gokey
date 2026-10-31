//! `openkey-platform` — contracts shared by the macOS, Windows and Linux
//! front-ends.
//!
//! An adapter's job: capture key events, normalize them to
//! [`openkey_core::KeyEvent`], ask the [`openkey_core::Engine`], then either
//! swallow the event or emit an [`Edit`]. All `unsafe` lives in the adapter
//! crates, never here and never in core.
//!
//! Platform mechanics this crate abstracts over:
//!
//! | Concern | macOS | Windows | Linux |
//! | --- | --- | --- | --- |
//! | Key capture | CGEventTap (Accessibility) or InputMethodKit | WH_KEYBOARD_LL hook (TSF in v2) | IBus engine (Fcitx5 in v2) |
//! | Suppression | tap returns NULL | hook returns 1 | engine consumes event |
//! | Focus / app id | NSWorkspace + AX observer | foreground window -> exe path | IBus focus-in/out |
//! | Text emission | CGEventKeyboardSetUnicodeString | SendInput / VkKeyScanW | IBus commit_text |
//!
//! See `docs/ARCHITECTURE.md` for the full capability matrix.

use openkey_core::{Edit, KeyEvent};

pub mod settings;

pub use settings::{
    HostSettings, Migration, RefusedSetting, SCHEMA_VERSION, Settings, SettingsError, UpstreamStore,
};

/// Stable identifier of the focused application, used for per-app mode and
/// charset memory (upstream `SmartSwitchKey`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum AppId {
    /// Reverse-DNS bundle id, e.g. `com.microsoft.VSCode`.
    MacBundle(String),
    /// Executable name, e.g. `Code.exe`.
    WindowsExe(String),
    /// Desktop app id, e.g. `code.desktop`.
    LinuxDesktop(String),
    Unknown,
}

/// Capabilities an adapter advertises. The settings UI hides or disables
/// options that the current platform cannot honour instead of silently
/// ignoring them.
#[derive(Clone, Copy, Debug, Default)]
pub struct Capabilities {
    pub can_suppress_keys: bool,
    pub can_query_focus: bool,
    /// Can read text before the caret (AX / UIA). Not required by the engine.
    pub can_read_context: bool,
    pub needs_accessibility_permission: bool,
    /// Emit text one key at a time instead of a single batch edit.
    pub supports_per_key_emission: bool,
}

/// Implemented by each platform front-end.
pub trait Host {
    fn app_id(&self) -> AppId;
    fn caps_lock(&self) -> bool;
    fn emit(&mut self, edit: Edit);
    fn beep(&mut self);
    fn capabilities(&self) -> Capabilities;
}

/// Emitted key events enter here; adapters own the platform event loop.
pub trait KeySource {
    /// Block until the next relevant event, or return `None` on shutdown.
    fn next_event(&mut self) -> Option<KeyEvent>;
}
