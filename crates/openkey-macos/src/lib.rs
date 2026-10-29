//! macOS front-end.
//!
//! Adapter design is pending ADR-0004 (timeboxed spike M-001): CGEventTap plus
//! an Accessibility permission prompt, versus a real InputMethodKit input
//! source. The event-tap route reproduces upstream behaviour (it is how the
//! original avoids the marked-text underline bug); InputMethodKit integrates
//! better with the system input-source switcher but forces marked text.
//!
//! This crate compiles to an empty library on non-macOS targets so the
//! workspace builds on every CI runner.

#![cfg(target_os = "macos")]

pub use openkey_core as core;
pub use openkey_platform as platform;
