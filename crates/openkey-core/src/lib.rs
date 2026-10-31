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
pub mod options;
mod word;

pub use engine::{Engine, Letter, Mark, Word};
pub use event::{Action, Edit, Key, KeyEvent, Mods, Outcome, Phase};
pub use options::{Charset, InputMethod, Options};
