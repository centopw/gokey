//! K-008 spike — can **one Rust process** own a `CGEventTap`, swallow a
//! keystroke, run it through `openkey-core`'s types, and emit the result?
//!
//! This is a feasibility probe for ADR-0004, not production code. It exists to
//! answer three questions with measurements rather than documentation:
//!
//! 1. Does tap creation require Accessibility trust? (Run it untrusted and see.)
//! 2. Does it survive as a plain non-bundled process? Does the *focused* app
//!    need to be accessible?
//! 3. How many lines of FFI does the whole adapter cost?
//!
//! Run: `cargo run -p openkey-macos --bin k008_spike -- [seconds]`
//! The process prints its trust status, installs an active tap, and (when
//! trusted) self-tests by posting synthetic `a`, `s` key events through the
//! same path a human keystroke takes.
//!
//! No crates: only `std` and raw CoreGraphics/CoreFoundation. That is the point
//! — the answer to (3) is only meaningful if it is not hidden behind a wrapper.

#![cfg(target_os = "macos")]

use std::ffi::c_void;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use openkey_core::{Key, KeyEvent, Mods, Phase};

// ───────────────────────── FFI: the whole boundary ──────────────────────────
// This block plus the `unsafe` call sites below are the complete platform
// surface. Counted for the ADR; nothing else in this file talks to the OS.

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> u8;
    fn CGEventTapCreate(
        tap: u32,
        place: u32,
        options: u32,
        events_of_interest: u64,
        callback: unsafe extern "C" fn(*mut c_void, u32, *mut c_void, *mut c_void) -> *mut c_void,
        user_info: *mut c_void,
    ) -> *mut c_void;
    fn CGEventTapEnable(tap: *mut c_void, enable: u8);
    fn CGEventGetIntegerValueField(event: *mut c_void, field: u32) -> i64;
    fn CGEventSetIntegerValueField(event: *mut c_void, field: u32, value: i64);
    fn CGEventGetFlags(event: *mut c_void) -> u64;
    fn CGEventKeyboardGetUnicodeString(
        event: *mut c_void,
        max: usize,
        actual: *mut usize,
        buf: *mut u16,
    );
    fn CGEventKeyboardSetUnicodeString(event: *mut c_void, len: usize, buf: *const u16);
    fn CGEventCreateKeyboardEvent(source: *mut c_void, keycode: u16, key_down: u8) -> *mut c_void;
    fn CGEventPost(tap: u32, event: *mut c_void);
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFMachPortCreateRunLoopSource(
        alloc: *mut c_void,
        port: *mut c_void,
        order: isize,
    ) -> *mut c_void;
    fn CFRunLoopGetCurrent() -> *mut c_void;
    fn CFRunLoopAddSource(rl: *mut c_void, source: *mut c_void, mode: *const c_void);
    fn CFRunLoopRunInMode(mode: *const c_void, seconds: f64, return_after: u8) -> i32;
    fn CFRelease(cf: *mut c_void);
    static kCFRunLoopDefaultMode: *const c_void;
}

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    /// 0 = granted, 1 = denied, 2 = unknown (never asked).
    /// request type: 1 = post event, 2 = listen event.
    fn IOHIDCheckAccess(request: u32) -> u32;
}

// ───────────────────────────── Platform constants ───────────────────────────

const HID_TAP: u32 = 0; // kCGHIDEventTap
const SESSION_TAP: u32 = 1; // kCGSessionEventTap
const HEAD_INSERT: u32 = 0; // kCGHeadInsertEventTap
const TAP_ACTIVE: u32 = 0; // kCGEventTapOptionDefault — may swallow/modify
const KEYDOWN: u64 = 1 << 10; // kCGEventKeyDown
const FIELD_KEYCODE: u32 = 9; // kCGKeyboardEventKeycode
const FIELD_USER_DATA: u32 = 42; // kCGEventSourceUserData
const MAGIC: i64 = 0x4b30_3038; // "K008" — marks our own emissions

const FLAG_SHIFT: u64 = 0x0002_0000;
const FLAG_CTRL: u64 = 0x0004_0000;
const FLAG_OPTION: u64 = 0x0008_0000;
const FLAG_CMD: u64 = 0x0010_0000;
const FLAG_CAPS: u64 = 0x0001_0000;

// ───────────────────────── Adapter: native keycode → core ───────────────────
// Upstream keeps one numeric table per platform (`platforms/mac.h`). The port
// keeps that table here, in the adapter, so `openkey-core` never sees a macOS
// keycode. This is the table the ADR has to own.

fn key_from_macos(keycode: u16) -> Key {
    match keycode {
        0x00 => Key::A,
        0x01 => Key::S,
        0x02 => Key::D,
        0x03 => Key::F,
        0x04 => Key::H,
        0x05 => Key::G,
        0x06 => Key::Z,
        0x07 => Key::X,
        0x08 => Key::C,
        0x09 => Key::V,
        0x0b => Key::B,
        0x0c => Key::Q,
        0x0d => Key::W,
        0x0e => Key::E,
        0x0f => Key::R,
        0x10 => Key::Y,
        0x11 => Key::T,
        0x12 => Key::Digit(1),
        0x13 => Key::Digit(2),
        0x14 => Key::Digit(3),
        0x15 => Key::Digit(4),
        0x16 => Key::Digit(6),
        0x17 => Key::Digit(5),
        0x18 => Key::Equals,
        0x19 => Key::Digit(9),
        0x1a => Key::Digit(7),
        0x1b => Key::Minus,
        0x1c => Key::Digit(8),
        0x1d => Key::Digit(0),
        0x1e => Key::RightBracket,
        0x1f => Key::O,
        0x20 => Key::U,
        0x21 => Key::LeftBracket,
        0x22 => Key::I,
        0x23 => Key::P,
        0x25 => Key::L,
        0x26 => Key::J,
        0x27 => Key::Quote,
        0x28 => Key::K,
        0x29 => Key::Semicolon,
        0x2a => Key::BackSlash,
        0x2b => Key::Comma,
        0x2c => Key::Slash,
        0x2d => Key::N,
        0x2e => Key::M,
        0x2f => Key::Dot,
        0x32 => Key::Backquote,
        0x24 => Key::Enter,
        0x30 => Key::Tab,
        0x31 => Key::Space,
        0x33 => Key::Backspace,
        0x35 => Key::Escape,
        0x75 => Key::Delete,
        0x7b => Key::ArrowLeft,
        0x7c => Key::ArrowRight,
        0x7d => Key::ArrowDown,
        0x7e => Key::ArrowUp,
        other => Key::Other(other),
    }
}

fn mods_from(flags: u64) -> Mods {
    Mods {
        shift: flags & FLAG_SHIFT != 0,
        ctrl: flags & FLAG_CTRL != 0,
        alt: flags & FLAG_OPTION != 0,
        cmd: flags & FLAG_CMD != 0,
        caps_lock: flags & FLAG_CAPS != 0,
    }
}

// ─────────────────────── Stand-in for `Engine::apply` ───────────────────────
// `openkey_core::Engine::apply` is `todo!()` until C-002, so the spike cannot
// call it. This stand-in exists so the emitted text is *visibly different from
// the input* — proof that the keystroke really went out to the core types and
// came back, rather than being echoed. It implements exactly one Telex rule
// (`s` = dấu sắc) over a bounded buffer, and nothing else.
//
// The swap to the real engine is one call site: same `KeyEvent` in, same
// `Option<String>` out.

#[derive(Default)]
struct SpikeState {
    buffer: Vec<char>,
}

fn engine_stand_in(state: &mut SpikeState, ev: &KeyEvent) -> Option<String> {
    if ev.phase == Phase::Up {
        return None;
    }
    if let Some(t) = ev.text {
        match t {
            's' if !state.buffer.is_empty() => {
                let acute = |c: char| match c {
                    'a' => 'á',
                    'e' => 'é',
                    'i' => 'í',
                    'o' => 'ó',
                    'u' => 'ú',
                    'y' => 'ý',
                    _ => c,
                };
                let last = state.buffer.len() - 1;
                state.buffer[last] = acute(state.buffer[last]);
                Some(state.buffer[last].to_string())
            }
            ' ' => {
                state.buffer.clear();
                None
            }
            c if c.is_alphanumeric() => {
                state.buffer.push(c);
                Some(c.to_string())
            }
            _ => None,
        }
    } else {
        None
    }
}

// ─────────────────────────────── The tap ────────────────────────────────────

static SWALLOWED: AtomicU64 = AtomicU64::new(0);
static EMITTED: AtomicU64 = AtomicU64::new(0);
static STATE: std::sync::Mutex<SpikeState> =
    std::sync::Mutex::new(SpikeState { buffer: Vec::new() });

unsafe extern "C" fn tap_callback(
    _proxy: *mut c_void,
    _kind: u32,
    event: *mut c_void,
    _user: *mut c_void,
) -> *mut c_void {
    unsafe {
        // Loop guard: our own synthetic emission must pass through untouched,
        // otherwise the tap would eat the character it just produced.
        if CGEventGetIntegerValueField(event, FIELD_USER_DATA) == MAGIC {
            return event;
        }

        let keycode = CGEventGetIntegerValueField(event, FIELD_KEYCODE) as u16;
        let flags = CGEventGetFlags(event);

        let mut buf = [0u16; 8];
        let mut actual = 0usize;
        CGEventKeyboardGetUnicodeString(event, buf.len(), &mut actual, buf.as_mut_ptr());
        let text = String::from_utf16_lossy(&buf[..actual])
            .chars()
            .next()
            .filter(|c| !c.is_control());

        let ev = KeyEvent {
            key: key_from_macos(keycode),
            mods: mods_from(flags),
            phase: Phase::Down,
            text,
        };

        let out = {
            let mut st = STATE.lock().unwrap();
            engine_stand_in(&mut st, &ev)
        };

        match out {
            Some(s) => {
                emit(&s);
                SWALLOWED.fetch_add(1, Ordering::Relaxed);
                EMITTED.fetch_add(1, Ordering::Relaxed);
                std::ptr::null_mut() // swallow the real key
            }
            None => event, // let the OS deliver it
        }
    }
}

unsafe fn emit(text: &str) {
    unsafe {
        let units: Vec<u16> = text.encode_utf16().collect();
        for down in [1u8, 0u8] {
            // keycode 0 is unused because the payload is the unicode string:
            // this is layout-independent, which is what a Vietnamese IME needs.
            let ev = CGEventCreateKeyboardEvent(std::ptr::null_mut(), 0, down);
            if ev.is_null() {
                return;
            }
            CGEventSetIntegerValueField(ev, FIELD_USER_DATA, MAGIC);
            CGEventKeyboardSetUnicodeString(ev, units.len(), units.as_ptr());
            CGEventPost(SESSION_TAP, ev);
            CFRelease(ev);
        }
    }
}

// ──────────────────────────────── Driver ────────────────────────────────────

/// Proves the emit-side ABI without any tap or permission: build an event,
/// stamp the loop guard, write a multi-byte Vietnamese string into it, read it
/// back through the same getter the tap uses. If this round-trips, the FFI
/// declarations and the UTF-16 handling are correct, so a later tap failure is
/// purely a trust problem.
fn emit_probe() {
    let cases = ["á", "ơ", "đ", "ữ"];
    let mut ok = 0;
    for case in cases {
        unsafe {
            let ev = CGEventCreateKeyboardEvent(std::ptr::null_mut(), 0, 1);
            assert!(!ev.is_null(), "CGEventCreateKeyboardEvent returned NULL");
            CGEventSetIntegerValueField(ev, FIELD_USER_DATA, MAGIC);
            let units: Vec<u16> = case.encode_utf16().collect();
            CGEventKeyboardSetUnicodeString(ev, units.len(), units.as_ptr());

            let mut back = [0u16; 8];
            let mut n = 0usize;
            CGEventKeyboardGetUnicodeString(ev, back.len(), &mut n, back.as_mut_ptr());
            let recovered = String::from_utf16_lossy(&back[..n]);
            let guard = CGEventGetIntegerValueField(ev, FIELD_USER_DATA);
            CFRelease(ev);

            let pass = recovered == case && guard == MAGIC;
            if pass {
                ok += 1;
            }
            println!(
                "emit-probe {case:?} -> {recovered:?} units={n} guard={:#x} {}",
                guard,
                if pass { "ok" } else { "FAIL" }
            );
        }
    }
    println!(
        "RESULT={}",
        if ok == cases.len() {
            "emit-abi-ok"
        } else {
            "emit-abi-fail"
        }
    );
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let listen_only = args.iter().any(|a| a == "--listen-only");
    let location = if args.iter().any(|a| a == "--session") {
        SESSION_TAP
    } else {
        HID_TAP
    };

    if args.iter().any(|a| a == "--emit-probe") {
        emit_probe();
        return;
    }

    let seconds: f64 = args.iter().find_map(|a| a.parse().ok()).unwrap_or(6.0);

    let trusted = unsafe { AXIsProcessTrusted() } != 0;
    let post_access = unsafe { IOHIDCheckAccess(1) };
    let listen_access = unsafe { IOHIDCheckAccess(2) };
    let name = |v: u32| match v {
        0 => "granted",
        1 => "denied",
        2 => "unknown",
        _ => "?",
    };
    println!("k008 spike — pid {}", std::process::id());
    println!("AXIsProcessTrusted() = {trusted}  (Accessibility)");
    println!(
        "IOHIDCheckAccess(post)   = {post_access} {}",
        name(post_access)
    );
    println!(
        "IOHIDCheckAccess(listen) = {listen_access} {}",
        name(listen_access)
    );
    println!(
        "tap option = {}",
        if listen_only { "listen-only" } else { "active" }
    );
    println!("tap location = {location}");

    let options = if listen_only { 1 } else { TAP_ACTIVE };
    let tap = unsafe {
        CGEventTapCreate(
            location,
            HEAD_INSERT,
            options,
            KEYDOWN,
            tap_callback,
            std::ptr::null_mut(),
        )
    };

    if tap.is_null() {
        println!("CGEventTapCreate -> NULL");
        println!("RESULT=no-tap");
        println!("diagnosis: the symbol resolved and the call returned cleanly, so NULL is a");
        println!("           refusal, not an ABI mistake. Check the two TCC axes printed above:");
        println!("           a HID tap needs Input Monitoring (listen), an active tap needs");
        println!("           Accessibility to swallow/modify. Neither is granted here.");
        std::process::exit(2);
    }
    println!("CGEventTapCreate -> {tap:p}");

    unsafe {
        let source = CFMachPortCreateRunLoopSource(std::ptr::null_mut(), tap, 0);
        println!("CFMachPortCreateRunLoopSource -> {source:p}");
        CFRunLoopAddSource(CFRunLoopGetCurrent(), source, kCFRunLoopDefaultMode);
        CGEventTapEnable(tap, 1);
        CFRelease(source);
    }

    // Self-test: push real key events through the same path a human keystroke
    // takes. Synthetic events are indistinguishable from hardware at the HID
    // tap, which is exactly what makes this a fair test.
    let probe = std::thread::spawn(|| {
        std::thread::sleep(Duration::from_millis(400));
        for keycode in [0x00u16, 0x01u16, 0x31u16] {
            // 0x00 = a, 0x01 = s (should become á), 0x31 = space
            for down in [1u8, 0u8] {
                unsafe {
                    let ev = CGEventCreateKeyboardEvent(std::ptr::null_mut(), keycode, down);
                    CGEventPost(HID_TAP, ev);
                    CFRelease(ev);
                }
            }
            std::thread::sleep(Duration::from_millis(120));
        }
    });

    let deadline = Instant::now() + Duration::from_secs_f64(seconds);
    while Instant::now() < deadline {
        unsafe {
            CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.25, 0);
        }
    }
    let _ = probe.join();

    let swallowed = SWALLOWED.load(Ordering::Relaxed);
    let emitted = EMITTED.load(Ordering::Relaxed);
    println!("swallowed={swallowed} emitted={emitted}");
    println!(
        "RESULT={}",
        if swallowed >= 3 {
            "swallow-and-reemit-ok"
        } else {
            "tap-installed-no-traffic"
        }
    );
}
