//! macOS front-end: the capture half of ADR-0004
//! (`docs/adr/0004-macos-adapter.md`).
//!
//! One active `CGEventTap` at the HID location, upstream's event mask minus
//! key-up, the adapter-owned keycode table, the Accessibility trust state and
//! the re-entrancy guard. Emission is MAC-002; the ⌥Z toggle and the menu-bar
//! indicator are MAC-004.
//!
//! # Layout
//!
//! `openkey-core` never sees a macOS keycode, so the translation and decision
//! logic lives here in modules that compile on **every** target — `keymap`,
//! `capture`, `trust`, `engine_host`. Only [`tap`] (FFI and the run loop) is
//! macOS-only. CI therefore exercises the parts where a mistake becomes a typing
//! bug on all three runners, not only on the macOS one.
//!
//! # Two guarantees the crate enforces
//!
//! - **Nothing is swallowed until emission exists.**
//!   [`engine_host::Reaction::swallows`] is `false` for every arm, so a captured
//!   event is always delivered. A keyboard that eats input is worse than no
//!   keyboard; MAC-002 flips the edit arm and keeps the test that pins the rest.
//! - **An engine fault passes the key through.** `Engine::apply` runs under
//!   `catch_unwind`; a panic disables the engine for the rest of the process
//!   instead of being retried once per keystroke, and every key is delivered
//!   unchanged with a loud message on stderr.
//!
//! The second relies on unwinding, so the workspace must not set
//! `panic = "abort"` (it does not).

pub use openkey_core as core;
pub use openkey_platform as platform;

pub mod capture;
pub mod engine_host;
pub mod keymap;
#[cfg(target_os = "macos")]
pub mod tap;
pub mod trust;

/// The event mask the tap installs, for callers that need to print it. Defined
/// in [`capture`] so the list and the bitmask cannot drift.
pub const EVENT_MASK: u64 = capture::MASK;
