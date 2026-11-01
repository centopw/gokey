//! `openkey-core` — the platform-neutral Vietnamese input engine.
//!
//! This is the Rust port of upstream `Sources/OpenKey/engine` (GPL-3.0).
//!
//! Invariants, enforced by review and CI:
//! - no I/O, no process globals, no `unsafe` in this crate;
//! - every decision is a pure function of (`Options`, session state, `KeyEvent`),
//!   so the parity harness can diff it against the reference C++ build;
//! - no platform keycode constants (upstream `platforms/*.h`) leak in here;
//!   adapters normalize every native keycode into [`event::Key`] first.
//!
//! Upstream state that lives in `extern int v*` globals
//! (`vInputType`, `vCodeTable`, `vCheckSpelling`, ...) is represented by
//! [`options::Options`], owned by the caller.

#![forbid(unsafe_code)]

pub mod charset;
pub mod engine;
pub mod event;
// `macro` is a reserved keyword, so the module is named `macros` while its
// file keeps the upstream name (`Macro.cpp`).
#[path = "macro.rs"]
pub mod macros;
pub mod options;
pub mod smart_switch;
mod tables;
mod word;

pub use engine::{Engine, Letter, Mark, Word};
pub use event::{Action, Edit, Encoded, Ext, Key, KeyEvent, Mods, Outcome, Phase};
pub use macros::{Macro, MacroTable, find_macro};
pub use options::{Charset, InputMethod, Options};
pub use smart_switch::{
    ActiveAppSwitch, SmartSwitchError, SmartSwitchKey, on_active_app_changed,
    on_input_method_changed, on_table_code_change, pack, unpack,
};
