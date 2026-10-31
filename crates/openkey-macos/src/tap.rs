//! The `CGEventTap`: installation, dispatch, recovery and the emitted-event
//! guard.
//!
//! Everything in this file is macOS-only FFI; the decisions it makes are all in
//! [`crate::capture`], [`crate::keymap`], [`crate::engine_host`] and
//! [`crate::trust`], which compile on every target and are tested on all three
//! CI runners.
//!
//! Design points that are load-bearing rather than stylistic:
//!
//! - **Active tap at the HID location.** `kCGHIDEventTap` is the earliest point,
//!   so a swallow is final for every consumer downstream; the option is
//!   `kCGEventTapOptionDefault` because listen-only cannot swallow
//!   (ADR-0004 §1.2).
//! - **A refused tap is an error, not a warning.** `CGEventTapCreate` returns
//!   `NULL` with no dialog when Accessibility is not granted. Returning `Err`
//!   here is what stops the app from installing and doing nothing.
//! - **The system's answer wins over our prediction.** The tap is attempted even
//!   when `AXIsProcessTrusted()` is false, because the trusted run may show that
//!   Input Monitoring alone is enough; the error is classified from what actually
//!   happened.
//! - **A disabled tap is re-enabled from the callback.** `…ByTimeout` and
//!   `…ByUserInput` arrive as events. Ignoring them leaves an IME that worked a
//!   minute ago and now types plain English with no visible cause.

use std::ffi::c_void;
use std::fmt;
use std::marker::PhantomData;
use std::ptr;
use std::time::{Duration, Instant};

use openkey_core::{Engine, Options};

use crate::capture::{self, NativeEvent};
use crate::engine_host::{EngineHost, Reaction};
use crate::trust::{ACCESSIBILITY_PANE_URL, Access, AccessReport, TrustState};

/// `kCGHIDEventTap`: the point where a synthetic event is indistinguishable from
/// hardware.
pub const HID_TAP: u32 = 0;
/// `kCGSessionEventTap`: upstream's location (`OpenKeyManager.m:53`). Kept for
/// the fallback recorded in ADR-0004 §1.2.
pub const SESSION_TAP: u32 = 1;

/// `kCGHeadInsertEventTap`: our decision is made before any other tap's.
const HEAD_INSERT: u32 = 0;
/// `kCGEventTapOptionDefault` — may swallow and modify.
const OPTION_ACTIVE: u32 = 0;
/// `kCGEventTapOptionListenOnly` — may not swallow. Diagnostics only.
const OPTION_LISTEN_ONLY: u32 = 1;

/// Largest UTF-16 payload read per key event. A keyboard layout produces one
/// scalar for every Vietnamese letter; eight units leaves room for a surrogate
/// pair and a combining sequence without allocating in the callback.
const UNICODE_UNITS: usize = 8;

mod ffi {
    use std::ffi::c_void;

    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        pub fn AXIsProcessTrusted() -> u8;
        pub fn CGEventTapCreate(
            tap: u32,
            place: u32,
            options: u32,
            events_of_interest: u64,
            callback: unsafe extern "C" fn(
                *mut c_void,
                u32,
                *mut c_void,
                *mut c_void,
            ) -> *mut c_void,
            user_info: *mut c_void,
        ) -> *mut c_void;
        pub fn CGEventTapEnable(tap: *mut c_void, enable: u8);
        pub fn CGEventGetIntegerValueField(event: *mut c_void, field: u32) -> i64;
        pub fn CGEventGetFlags(event: *mut c_void) -> u64;
        pub fn CGEventKeyboardGetUnicodeString(
            event: *mut c_void,
            max_units: usize,
            actual_units: *mut usize,
            buf: *mut u16,
        );
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        pub fn CFMachPortCreateRunLoopSource(
            alloc: *mut c_void,
            port: *mut c_void,
            order: isize,
        ) -> *mut c_void;
        pub fn CFRunLoopGetCurrent() -> *mut c_void;
        pub fn CFRunLoopAddSource(rl: *mut c_void, source: *mut c_void, mode: *const c_void);
        pub fn CFRunLoopRemoveSource(rl: *mut c_void, source: *mut c_void, mode: *const c_void);
        pub fn CFRunLoopRunInMode(
            mode: *const c_void,
            seconds: f64,
            return_after_source_handled: u8,
        ) -> i32;
        pub fn CFRelease(cf: *mut c_void);
        pub static kCFRunLoopDefaultMode: *const c_void;
    }

    #[link(name = "IOKit", kind = "framework")]
    unsafe extern "C" {
        /// `kIOHIDRequestTypePostEvent` = 1, `kIOHIDRequestTypeListenEvent` = 2.
        pub fn IOHIDCheckAccess(request: u32) -> u32;
    }
}

/// Read the three permission answers, now. Cheap enough to call from the banner
/// and the menu bar; calling it again is the "re-check without a restart" path.
pub fn access_report() -> AccessReport {
    // SAFETY: three framework functions with no preconditions and no arguments
    // that can be invalid. `AXIsProcessTrusted` returns a C boolean; the IOKit
    // answer is mapped by `Access::from_raw`, which treats unknown values as
    // not-granted.
    unsafe {
        AccessReport {
            accessibility: ffi::AXIsProcessTrusted() != 0,
            hid_post: Access::from_raw(ffi::IOHIDCheckAccess(1)),
            hid_listen: Access::from_raw(ffi::IOHIDCheckAccess(2)),
        }
    }
}

/// Where and how to install the tap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TapConfig {
    /// [`HID_TAP`] or [`SESSION_TAP`].
    pub location: u32,
    /// `true` only for diagnostics: a listen-only tap cannot swallow, so it
    /// cannot implement the typing model.
    pub listen_only: bool,
}

impl Default for TapConfig {
    fn default() -> Self {
        Self {
            location: HID_TAP,
            listen_only: false,
        }
    }
}

/// Why the tap is not running. Every variant is a state the UI must show, not a
/// log line: `S-09` renders it, `S-08` repeats it in the menu bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TapError {
    /// Creation returned `NULL` and Accessibility is not granted. Expected on
    /// first run; also the state after a revocation.
    NotTrusted { report: AccessReport },
    /// Creation returned `NULL` (or the run-loop source did) with Accessibility
    /// granted. Not expected; a real defect or an environment we have not seen.
    CreateFailed {
        report: AccessReport,
        location: u32,
        stage: &'static str,
    },
}

impl fmt::Display for TapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotTrusted { report } => write!(
                f,
                "the event tap was refused: CGEventTapCreate returned NULL and Accessibility is \
                 not granted (accessibility={}, input listening={:?}). Nothing is being \
                 intercepted, so typing is unaffected and uncorrected. Grant access in System \
                 Settings → Privacy & Security → Accessibility ({ACCESSIBILITY_PANE_URL}) and \
                 re-check; no restart is needed.",
                report.accessibility, report.hid_listen,
            ),
            Self::CreateFailed {
                report,
                location,
                stage,
            } => write!(
                f,
                "{stage} returned NULL with Accessibility granted (accessibility={}, input \
                 listening={:?}, posting={:?}) at tap location {location}. This is not a \
                 permission state and is not expected; report it with this line verbatim.",
                report.accessibility, report.hid_listen, report.hid_post,
            ),
        }
    }
}

impl std::error::Error for TapError {}

/// Counters for the probe, the acceptance run and the menu bar.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// Events the tap delivered, after the echo guard.
    pub captured: u64,
    pub key_events: u64,
    pub flags_events: u64,
    pub mouse_events: u64,
    /// Events delivered to the application. Equal to `captured` while emission
    /// does not exist (MAC-001); the difference is the swallow count.
    pub passed: u64,
    /// Our own emitted events seen again and passed through untouched.
    pub echoes: u64,
    pub disabled_notices: u64,
    pub recoveries: u64,
    pub engine_faults: u64,
}

/// A running tap. Dropping it removes the source from the run loop and releases
/// the Mach port, which restores the application's normal key handling.
///
/// The source is attached to the *current* run loop at install time, so an
/// AppKit application needs no loop of its own; [`EventTap::run_for`] exists for
/// the probe and for tests.
pub struct EventTap {
    tap: *mut c_void,
    source: *mut c_void,
    run_loop: *mut c_void,
    adapter: *mut Adapter,
    config: TapConfig,
    /// The tap is bound to the thread's run loop; moving it would be a use-after
    /// -free waiting to happen.
    _not_send: PhantomData<*mut ()>,
}

struct Adapter {
    host: EngineHost<Engine>,
    tap: *mut c_void,
    stats: Stats,
}

impl EventTap {
    /// Install an active tap with the mask from ADR-0004 §1.1.
    ///
    /// Returns `Err` when the system refuses, and `NotTrusted` is classified
    /// from the actual answers rather than from a pre-check — see the module
    /// documentation.
    pub fn install(config: TapConfig) -> Result<Self, TapError> {
        let report = access_report();
        let adapter = Box::into_raw(Box::new(Adapter {
            host: EngineHost::new(Engine::new(Options::default())),
            tap: ptr::null_mut(),
            stats: Stats::default(),
        }));

        let options = if config.listen_only {
            OPTION_LISTEN_ONLY
        } else {
            OPTION_ACTIVE
        };

        // SAFETY: the callback is an `extern "C"` function of the exact
        // signature CoreGraphics expects, `user_info` is a live `Adapter`
        // allocation owned by `self` until `drop`, and the mask is the u64 the
        // contract defines.
        let tap = unsafe {
            ffi::CGEventTapCreate(
                config.location,
                HEAD_INSERT,
                options,
                capture::MASK,
                callback,
                adapter.cast(),
            )
        };

        if tap.is_null() {
            // SAFETY: created above, not used anywhere else.
            drop(unsafe { Box::from_raw(adapter) });
            return Err(if report.accessibility {
                TapError::CreateFailed {
                    report,
                    location: config.location,
                    stage: "CGEventTapCreate",
                }
            } else {
                TapError::NotTrusted { report }
            });
        }

        // SAFETY: `tap` is non-null and owned by this function until `Self` owns it.
        unsafe { (*adapter).tap = tap };

        // SAFETY: a non-null Mach port with no allocator (NULL = default).
        let source = unsafe { ffi::CFMachPortCreateRunLoopSource(ptr::null_mut(), tap, 0) };
        if source.is_null() {
            // SAFETY: both handles were created here and are not shared.
            unsafe {
                ffi::CFRelease(tap);
                drop(Box::from_raw(adapter));
            }
            return Err(TapError::CreateFailed {
                report,
                location: config.location,
                stage: "CFMachPortCreateRunLoopSource",
            });
        }

        // SAFETY: the current thread's run loop exists for the process lifetime;
        // the source and mode are valid CoreFoundation objects.
        let run_loop = unsafe { ffi::CFRunLoopGetCurrent() };
        // SAFETY: as above; `kCFRunLoopDefaultMode` is the documented mode and
        // enabling a non-null tap is a no-op if it is already enabled.
        unsafe {
            ffi::CFRunLoopAddSource(run_loop, source, ffi::kCFRunLoopDefaultMode);
            ffi::CGEventTapEnable(tap, 1);
        }

        Ok(Self {
            tap,
            source,
            run_loop,
            adapter,
            config,
            _not_send: PhantomData,
        })
    }

    pub fn config(&self) -> TapConfig {
        self.config
    }

    /// The trust state as of now. Call it again after the user visits System
    /// Settings: that is the re-check, and it needs no restart.
    pub fn trust(&self) -> TrustState {
        access_report().trust()
    }

    pub fn stats(&self) -> Stats {
        // SAFETY: the adapter lives until `drop`, and this is called between
        // run-loop iterations on the same thread that owns it.
        unsafe { (*self.adapter).stats }
    }

    /// Drive the run loop for a bounded time. Each slice is short enough that a
    /// caller can inspect stats between them.
    pub fn run_for(&self, duration: Duration) {
        let deadline = Instant::now() + duration;
        while Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let slice = remaining.min(Duration::from_millis(50)).as_secs_f64();
            // SAFETY: the mode is the one the source was added under; the loop is
            // the current thread's. A return value of 0 is a timeout.
            unsafe {
                ffi::CFRunLoopRunInMode(ffi::kCFRunLoopDefaultMode, slice, 0);
            }
        }
    }
}

impl Drop for EventTap {
    fn drop(&mut self) {
        // SAFETY: every handle was created by `install` and is released exactly
        // once. The source is removed before it is released, and the adapter is
        // freed only after the source can no longer invoke the callback.
        unsafe {
            ffi::CFRunLoopRemoveSource(self.run_loop, self.source, ffi::kCFRunLoopDefaultMode);
            ffi::CFRelease(self.source);
            ffi::CFRelease(self.tap);
            drop(Box::from_raw(self.adapter));
        }
    }
}

/// Read the event's native fields. Only key events carry a Unicode payload;
/// asking a mouse event for one would read a field it does not have.
unsafe fn read_native(kind: u32, event: *mut c_void) -> NativeEvent {
    use capture::event_type::{KEY_DOWN, KEY_UP};

    // SAFETY: `event` is non-null (checked by the caller) and is a CGEvent of
    // the type in `kind`. The field numbers are the documented CGEventField
    // values, and the buffer is sized and passed with its capacity.
    unsafe {
        let keycode = ffi::CGEventGetIntegerValueField(event, capture::FIELD_KEYCODE) as u16;
        let flags = ffi::CGEventGetFlags(event);
        let mut units = [0u16; UNICODE_UNITS];
        let mut len = 0usize;
        if kind == KEY_DOWN || kind == KEY_UP {
            ffi::CGEventKeyboardGetUnicodeString(event, units.len(), &mut len, units.as_mut_ptr());
        }
        capture::classify(kind, keycode, flags, &units[..len.min(units.len())])
    }
}

unsafe extern "C" fn callback(
    _proxy: *mut c_void,
    kind: u32,
    event: *mut c_void,
    user_info: *mut c_void,
) -> *mut c_void {
    if event.is_null() || user_info.is_null() {
        return event;
    }

    // SAFETY: `user_info` is the live `Box<Adapter>` created in `install`; the
    // callback runs on the run-loop thread that owns it and nothing else mutates
    // it concurrently.
    let adapter = unsafe { &mut *(user_info.cast::<Adapter>()) };

    // SAFETY: `event` is non-null and a CGEvent.
    let user_data = unsafe { ffi::CGEventGetIntegerValueField(event, capture::FIELD_USER_DATA) };
    if capture::is_echo(user_data) {
        // Our own emission coming back. Pass it through and do not let the
        // engine see it: reprocessing our own output is the silent corruption
        // the guard exists to prevent.
        adapter.stats.echoes += 1;
        return event;
    }

    adapter.stats.captured += 1;

    // SAFETY: as above.
    let native = unsafe { read_native(kind, event) };

    match native {
        NativeEvent::Key { .. } => adapter.stats.key_events += 1,
        NativeEvent::FlagsChanged { .. } => adapter.stats.flags_events += 1,
        NativeEvent::Mouse { .. } => adapter.stats.mouse_events += 1,
        NativeEvent::TapDisabled { reason } => {
            adapter.stats.disabled_notices += 1;
            // SAFETY: `tap` is the port that delivered this notice and is still
            // alive; re-enabling is the documented recovery for both reasons.
            unsafe { ffi::CGEventTapEnable(adapter.tap, 1) };
            adapter.stats.recoveries += 1;
            eprintln!(
                "openkey: the event tap was disabled by the system ({reason:?}) and has been \
                 re-enabled. Keystrokes typed while it was off were delivered unchanged."
            );
            return event;
        }
        NativeEvent::Other => {}
    }

    let reaction = adapter.host.handle(native);
    match reaction {
        // MAC-001 has no emitter: both of the first two arms deliver the event.
        Reaction::Pass | Reaction::EditNotEmitted(_) => adapter.stats.passed += 1,
        Reaction::EngineFault => {
            adapter.stats.engine_faults += 1;
            adapter.stats.passed += 1;
        }
    }
    debug_assert!(!reaction.swallows(), "MAC-001 must never swallow");
    event
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_tap_is_active_at_the_hid_location() {
        let config = TapConfig::default();
        assert_eq!(config.location, HID_TAP);
        assert!(
            !config.listen_only,
            "listen-only cannot implement the model"
        );
    }

    #[test]
    fn a_refused_tap_names_the_pane_and_the_fix() {
        let error = TapError::NotTrusted {
            report: AccessReport::default(),
        };
        let message = error.to_string();
        assert!(message.contains("Accessibility"));
        assert!(message.contains("Nothing is being intercepted"));
        assert!(message.contains(ACCESSIBILITY_PANE_URL));
        assert!(message.contains("no restart"));
    }

    #[test]
    fn a_granted_but_failed_tap_is_reported_as_unexpected() {
        let error = TapError::CreateFailed {
            report: AccessReport {
                accessibility: true,
                hid_post: Access::Granted,
                hid_listen: Access::Granted,
            },
            location: HID_TAP,
            stage: "CGEventTapCreate",
        };
        let message = error.to_string();
        assert!(message.contains("not a permission state"));
        assert!(message.contains("CGEventTapCreate"));
        assert!(message.contains("location 0"));
    }

    #[test]
    fn access_report_is_readable_without_a_grant() {
        // Not an assertion about the host's permissions — CI has none — but the
        // call must be safe and must produce one of the three states.
        let report = access_report();
        assert!(matches!(
            report.trust(),
            TrustState::NotDetermined | TrustState::Denied | TrustState::Granted
        ));
    }
}
