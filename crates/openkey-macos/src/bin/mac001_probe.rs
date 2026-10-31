//! MAC-001 diagnostic: is the tap actually installed, and does every captured
//! event still reach the application?
//!
//! Capture is implemented, emission is not (MAC-002). So the only correct
//! behaviour this build can demonstrate is **capture and pass through**: the tap
//! sees the keystroke, the engine is consulted, and the original event is
//! delivered unchanged. This binary measures exactly that.
//!
//! Run: `cargo run -p openkey-macos --bin mac001_probe -- [seconds] [--session] [--listen-only]
//! [--wait-for-grant=<seconds>]`
//!
//! `--wait-for-grant` is the in-process permission measurement: the probe stays
//! alive while the user grants Accessibility, prints each trust transition from
//! re-reading the system, and installs the tap **without a restart** as soon as
//! the grant is visible. It is how the S-09 "no restart" copy is checked rather
//! than asserted.
//!
//! It prints the permission answers, the trust state and S-09's banner copy,
//! installs the tap, posts three synthetic keystrokes through the HID path a
//! human keystroke takes, and reports:
//!
//! - `key_events` — must be 3, not 6: key-up is deliberately unmasked.
//! - `swallowed` — must be 0 while emission does not exist.
//! - `engine_faults` — with the C-002 stub this equals the number of key-downs,
//!   because `Engine::apply` is `todo!()`. The keys still appear in the focused
//!   app, which is the panic-passthrough rule demonstrated live rather than
//!   asserted in a unit test.
//!
//! The synthetic keystrokes are delivered to whatever app has focus, so run it
//! with a scratch window focused or expect `a`, `s` and a space to be typed.
//! This is a diagnostic, not a test; it needs a human to grant Accessibility.

#[cfg(target_os = "macos")]
fn main() {
    imp::main();
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("mac001_probe: this is a macOS-only CGEventTap diagnostic; nothing was probed.");
    std::process::exit(1);
}

#[cfg(target_os = "macos")]
mod imp {
    use std::ffi::c_void;
    use std::time::{Duration, Instant};

    use openkey_macos::capture;
    use openkey_macos::keymap;
    use openkey_macos::tap::{self, EventTap, HID_TAP, TapConfig};
    use openkey_macos::trust;

    // Diagnostic-only symbols. The library's FFI stops at capture; posting a
    // synthetic keystroke is how this probe proves the path, and emission
    // (MAC-002) is what will add `CGEventPost` to the library.
    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn CGEventCreateKeyboardEvent(
            source: *mut c_void,
            keycode: u16,
            key_down: u8,
        ) -> *mut c_void;
        fn CGEventPost(tap: u32, event: *mut c_void);
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFRelease(cf: *mut c_void);
    }

    pub fn main() {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let session = args.iter().any(|arg| arg == "--session");
        let listen_only = args.iter().any(|arg| arg == "--listen-only");
        let wait_for_grant: Option<Duration> = args
            .iter()
            .find_map(|arg| arg.strip_prefix("--wait-for-grant="))
            .and_then(|value| value.parse().ok())
            .map(Duration::from_secs);
        let seconds: f64 = args
            .iter()
            .filter(|arg| !arg.starts_with("--"))
            .find_map(|arg| arg.parse().ok())
            .unwrap_or(4.0);

        let config = TapConfig {
            location: if session { tap::SESSION_TAP } else { HID_TAP },
            listen_only,
        };

        println!("mac001 probe — capture only; emission is MAC-002");
        println!("pid {}", std::process::id());

        let report = tap::access_report();
        println!(
            "access: accessibility={} hid_post={:?} hid_listen={:?}",
            report.accessibility, report.hid_post, report.hid_listen
        );
        let state = report.trust();
        println!("trust: {}", state.as_str());
        match trust::banner(state) {
            Some(banner) => {
                println!("banner.title: {}", banner.title);
                println!("banner.body: {}", banner.body);
                println!("banner.action: {} -> {}", banner.action, banner.url);
                if let Some((label, url)) = banner.secondary {
                    println!("banner.secondary: {label} -> {url}");
                }
                println!("banner.after: {}", banner.after);
            }
            None => println!("banner: none (granted)"),
        }

        println!(
            "mask: {:#06x} over {:?}",
            capture::MASK,
            capture::MASKED_TYPES
        );
        println!(
            "config: location={} option={}",
            config.location,
            if config.listen_only {
                "listen-only"
            } else {
                "active"
            }
        );

        let tap = match EventTap::install(config) {
            Ok(tap) => tap,
            Err(error) => {
                println!();
                println!("{error}");
                match wait_for_grant {
                    Some(limit) => match install_after_grant(config, limit) {
                        Some(tap) => tap,
                        None => {
                            println!(
                                "RESULT=no-tap trust={}",
                                tap::access_report().trust().as_str()
                            );
                            std::process::exit(2);
                        }
                    },
                    None => {
                        println!("RESULT=no-tap trust={}", state.as_str());
                        std::process::exit(2);
                    }
                }
            }
        };
        println!("tap installed; trust now {}", tap.trust().as_str());

        let poster = std::thread::spawn(|| {
            std::thread::sleep(Duration::from_millis(400));
            for keycode in [keymap::VK_A, keymap::VK_S, keymap::VK_SPACE] {
                for down in [1u8, 0u8] {
                    // SAFETY: two framework calls with no preconditions; the
                    // event is released exactly once, immediately after posting.
                    unsafe {
                        let event = CGEventCreateKeyboardEvent(std::ptr::null_mut(), keycode, down);
                        CGEventPost(HID_TAP, event);
                        CFRelease(event);
                    }
                }
                std::thread::sleep(Duration::from_millis(120));
            }
        });

        tap.run_for(Duration::from_secs_f64(seconds));
        let _ = poster.join();

        let stats = tap.stats();
        // A swallow is an input event that reached the adapter and was not
        // delivered. Disable notices and NULL deliveries are counted separately
        // and are not in `captured`; subtracting them is belt and braces so a
        // future accounting drift cannot turn a re-enable into a false swallow.
        let swallowed = stats
            .captured
            .saturating_sub(stats.passed)
            .saturating_sub(stats.disabled_notices)
            .saturating_sub(stats.null_events);
        println!(
            "captured={} key_events={} flags={} mouse={} passed={} swallowed={} echoes={} \
             disabled={} recoveries={} null_events={} engine_faults={}",
            stats.captured,
            stats.key_events,
            stats.flags_events,
            stats.mouse_events,
            stats.passed,
            swallowed,
            stats.echoes,
            stats.disabled_notices,
            stats.recoveries,
            stats.null_events,
            stats.engine_faults,
        );

        let ok = stats.captured >= 3 && stats.key_events >= 3 && swallowed == 0;
        if !ok {
            println!(
                "expected key_events>=3 and swallowed=0; the tap did not see the posted keys or \
                 swallowed one"
            );
        }
        println!(
            "note: emitted key-ups are unmasked on purpose; {} posted events produced {} captured \
             events",
            6, stats.captured
        );
        println!(
            "RESULT={}",
            if ok {
                "capture-passthrough-ok"
            } else {
                "capture-failed"
            }
        );
        if !ok {
            std::process::exit(1);
        }
    }

    /// Stay alive while the user grants Accessibility, re-reading the system
    /// answer and installing the tap in **this** process when the grant appears.
    /// `None` means the deadline passed with no tap; the caller reports that as
    /// the no-tap result.
    ///
    /// This is the measurement behind S-09's "no restart" line: the probe prints
    /// every trust transition it observes and says whether the tap could then be
    /// created in the same process. If TCC requires a relaunch here, that is the
    /// result to record — and the copy would have to stop promising otherwise.
    fn install_after_grant(config: TapConfig, limit: Duration) -> Option<EventTap> {
        println!(
            "waiting up to {}s for the grant — grant Accessibility to this binary in System \
             Settings, then come back; the probe keeps running",
            limit.as_secs()
        );
        let deadline = Instant::now() + limit;
        let mut last = tap::access_report().trust();
        let mut failure_reported = false;
        while Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(500));
            let now = tap::access_report().trust();
            if now != last {
                println!("re-check: {} -> {}", last.as_str(), now.as_str());
                last = now;
            }
            if now.is_granted() {
                match EventTap::install(config) {
                    Ok(tap) => {
                        println!("tap installed in-process after the grant; no restart");
                        return Some(tap);
                    }
                    Err(error) => {
                        if !failure_reported {
                            println!("install after grant failed: {error}");
                            failure_reported = true;
                        }
                    }
                }
            }
        }
        println!("grant wait expired; trust is still {}", last.as_str());
        None
    }
}
