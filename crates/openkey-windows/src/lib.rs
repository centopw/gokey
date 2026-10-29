//! Windows front-end.
//!
//! v1 mirrors upstream: `WH_KEYBOARD_LL` hook for capture, `SendInput` for
//! backspace + Unicode emission. TSF (Text Services Framework) is a v2 decision
//! (ADR-0005) — it is the correct long-term integration but far more work.
//!
//! This crate compiles to an empty library on non-Windows targets.

#![cfg(target_os = "windows")]

pub use openkey_core as core;
pub use openkey_platform as platform;
