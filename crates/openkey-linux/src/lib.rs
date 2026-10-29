//! Linux front-end.
//!
//! Upstream has no working Linux build, only a stub. Global key capture is not
//! viable on Wayland, so Linux must be a proper input-method engine:
//! IBus first (GNOME default), Fcitx5 addon second (ADR-0006).
//!
//! This crate compiles to an empty library on non-Linux targets.

#![cfg(target_os = "linux")]

pub use openkey_core as core;
pub use openkey_platform as platform;
