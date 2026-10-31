//! Replay one corpus case through [`openkey_core::Engine`] (K-005).
//!
//! Telex is the only input method the engine implements (C-002); VNI and Simple
//! Telex 1/2 still end in `engine-pending`, not in a verdict. That is reported
//! as its own status rather than as a disagreement: "the port has not been
//! written yet" and "the port disagrees with the oracle" are different facts and
//! must not collapse into the same exit code.
//!
//! A panic aborts the rest of the case. After a panic the engine's buffer cannot
//! be trusted, so continuing would report a document assembled from a broken
//! state.
//!
//! The document model mirrors the reference harness (`scripts/parity/harness.cpp`,
//! the part of the macOS hook that lives outside the engine). It is the only
//! place in this crate that encodes a typing rule, and every rule carries the
//! upstream reference the harness cites. It is unit-tested over synthetic
//! outcomes, and the real diff against the oracle is the parity run itself.

use std::panic::{self, AssertUnwindSafe};
use std::sync::{Arc, Mutex, OnceLock};

use openkey_core::{Action, Edit, Encoded, Engine, Key, KeyEvent, Mods, Outcome, Phase};

use crate::corpus::Case;
use crate::flags;

/// Why a case produced no document.
#[derive(Debug)]
pub enum ReplayError {
    /// The corpus row asks for something the replay cannot model.
    Invalid(String),
    /// The engine has no implementation for this key yet (`todo!()`).
    Pending(String),
    /// The engine panicked in a way that is not a declared `todo!()`.
    Panic(String),
}

/// Replay a case and return its final document.
pub fn run(case: &Case) -> Result<String, ReplayError> {
    let options = flags::options(case).map_err(ReplayError::Invalid)?;
    let mut engine = Engine::new(options);
    let mut document = String::new();

    for typed in case.input.chars() {
        let event = key_event(typed).ok_or_else(|| {
            ReplayError::Invalid(format!(
                "U+{:04X} is not a key the reference harness can press",
                typed as u32
            ))
        })?;
        let outcome = catch_apply(&mut engine, &event)?;
        apply_outcome(&mut document, typed, &outcome)?;
    }

    Ok(document)
}

/// Feed one event, converting a panic into a reportable error.
///
/// `AssertUnwindSafe` is honest here: the engine is dropped on the error path
/// (the caller stops replaying the case), so no broken state escapes.
fn catch_apply(engine: &mut Engine, event: &KeyEvent) -> Result<Outcome, ReplayError> {
    let slot = panic_slot();
    {
        let mut guard = slot.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        *guard = None;
    }
    match panic::catch_unwind(AssertUnwindSafe(|| engine.apply(event))) {
        Ok(outcome) => Ok(outcome),
        Err(_) => {
            let detail = slot
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .take()
                .unwrap_or_else(|| "panicked without a message".to_string());
            // `PanicInfo` renders as `panicked at <location>:\n<message>`, so the
            // `todo!()` marker is on the message line, not the first line.
            if detail.contains("not yet implemented") {
                let message = detail
                    .lines()
                    .find(|line| line.contains("not yet implemented"))
                    .unwrap_or("not yet implemented")
                    .to_string();
                Err(ReplayError::Pending(message))
            } else {
                Err(ReplayError::Panic(detail.trim().to_string()))
            }
        }
    }
}

/// Install a panic hook that records the message instead of printing it.
///
/// A replay that stops at `todo!()` for every case would otherwise print 178
/// stack traces (CI sets `RUST_BACKTRACE=1`) and bury the report. The hook is
/// process-global, so it is installed once, on the first replay.
fn panic_slot() -> Arc<Mutex<Option<String>>> {
    static SLOT: OnceLock<Arc<Mutex<Option<String>>>> = OnceLock::new();
    SLOT.get_or_init(|| {
        let slot = Arc::new(Mutex::new(None));
        let hook_slot = Arc::clone(&slot);
        panic::set_hook(Box::new(move |info| {
            let mut guard = hook_slot
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if guard.is_none() {
                *guard = Some(info.to_string());
            }
        }));
        slot
    })
    .clone()
}

/// Map one typed character onto the physical key an adapter would deliver.
///
/// Corpus inputs are characters, not keycodes (`tests/corpus/README.md`), so the
/// mapping is the inverse of what a macOS adapter does. Only keys the reference
/// harness can press are mapped; anything else is refused rather than guessed.
pub fn key_event(typed: char) -> Option<KeyEvent> {
    let (key, shift) = match typed {
        '\u{8}' => (Key::Backspace, false),
        '\n' => (Key::Enter, false),
        '\t' => (Key::Tab, false),
        ' ' => (Key::Space, false),
        'a'..='z' | 'A'..='Z' => (letter(typed), typed.is_ascii_uppercase()),
        '0'..='9' => (Key::Digit(typed as u8 - b'0'), false),
        '\'' => (Key::Quote, false),
        '[' => (Key::LeftBracket, false),
        ']' => (Key::RightBracket, false),
        ';' => (Key::Semicolon, false),
        ',' => (Key::Comma, false),
        '.' => (Key::Dot, false),
        '/' => (Key::Slash, false),
        '\\' => (Key::BackSlash, false),
        '-' => (Key::Minus, false),
        '=' => (Key::Equals, false),
        '`' => (Key::Backquote, false),
        _ => return None,
    };
    Some(KeyEvent {
        key,
        mods: Mods {
            shift,
            ..Mods::default()
        },
        phase: Phase::Down,
        text: Some(typed),
    })
}

fn letter(typed: char) -> Key {
    match typed.to_ascii_lowercase() {
        'a' => Key::A,
        'b' => Key::B,
        'c' => Key::C,
        'd' => Key::D,
        'e' => Key::E,
        'f' => Key::F,
        'g' => Key::G,
        'h' => Key::H,
        'i' => Key::I,
        'j' => Key::J,
        'k' => Key::K,
        'l' => Key::L,
        'm' => Key::M,
        'n' => Key::N,
        'o' => Key::O,
        'p' => Key::P,
        'q' => Key::Q,
        'r' => Key::R,
        's' => Key::S,
        't' => Key::T,
        'u' => Key::U,
        'v' => Key::V,
        'w' => Key::W,
        'x' => Key::X,
        'y' => Key::Y,
        'z' => Key::Z,
        _ => unreachable!("letter() is called only for ASCII letters"),
    }
}

/// Apply one engine decision to the document (harness `press`).
///
/// Rules, with the upstream references the harness cites:
/// - `Pass` / `BreakWord`: the system types the key (`OpenKey.mm:713-737`).
/// - `Process`: `backspaceCount` backspaces, then the emitted text
///   (`:764-769`, `:423-480`), then any re-injected key.
/// - `Restore` / `RestoreAndBreak`: as `Process`, then the pressed key is
///   re-sent as a character (`:483-491`), then any re-injected key.
/// - `Macro`: the expansion replaces the typed sequence; nothing else is typed.
///
/// A byte-charset edit is refused rather than decoded: the document model here
/// is a Unicode string, the corpus has no charset option to select one, and
/// guessing a byte decoder would compare against the wrong text.
pub fn apply_outcome(
    document: &mut String,
    typed: char,
    outcome: &Outcome,
) -> Result<(), ReplayError> {
    match outcome.action {
        Action::Pass | Action::BreakWord => {
            if let Some(edit) = &outcome.edit {
                apply_edit(document, edit)?;
            }
            type_char(document, typed);
        }
        Action::Process => {
            if let Some(edit) = &outcome.edit {
                apply_edit(document, edit)?;
            }
            if let Some(event) = &outcome.passthrough {
                type_event(document, event);
            }
        }
        Action::Restore | Action::RestoreAndBreak => {
            if let Some(edit) = &outcome.edit {
                apply_edit(document, edit)?;
            }
            type_char(document, typed);
            if let Some(event) = &outcome.passthrough {
                type_event(document, event);
            }
        }
        Action::Macro => {
            if let Some(edit) = &outcome.edit {
                apply_edit(document, edit)?;
            }
        }
    }
    Ok(())
}

fn apply_edit(document: &mut String, edit: &Edit) -> Result<(), ReplayError> {
    // Refuse a byte-charset edit before touching the document. Backspaces and
    // text are one decision; applying the deletions and then failing would
    // leave the document in a state the engine never produced.
    let text = match &edit.text {
        // Every value in the five tables is BMP, so the loss-free decode here
        // and the harness's surrogate-aware `appendUnits` agree.
        Encoded::Utf16(units) => String::from_utf16_lossy(units),
        Encoded::Bytes(_) => {
            return Err(ReplayError::Invalid(
                "the engine emitted a byte-charset edit; the replay document is a Unicode \
                 string and no corpus case can select a charset (C-005 adds no charset option)"
                    .to_string(),
            ));
        }
    };
    let count = edit.backspaces as usize;
    // Upstream applies the backspaces only when `0 < count < MAX_BUFF`
    // (`OpenKey.mm:764-769`). It is the app's condition, not a safety check of
    // ours, so it is copied rather than tightened.
    if count > 0 && count < Engine::MAX_BUFFER {
        let len = document.chars().count();
        let keep = len.saturating_sub(count);
        *document = document.chars().take(keep).collect();
    }
    document.push_str(&text);
    Ok(())
}

fn type_char(document: &mut String, typed: char) {
    if typed == '\u{8}' {
        document.pop();
    } else {
        document.push(typed);
    }
}

fn type_event(document: &mut String, event: &KeyEvent) {
    if event.key == Key::Backspace {
        document.pop();
        return;
    }
    if let Some(text) = event.text {
        document.push(text);
        return;
    }
    if let Some(text) = key_character(event.key, event.mods.shift) {
        document.push(text);
    }
}

/// The character a key produces with no layout mapping, mirroring
/// `keyCodeToCharacter` in the reference harness.
fn key_character(key: Key, shift: bool) -> Option<char> {
    match key {
        Key::A => Some(if shift { 'A' } else { 'a' }),
        Key::B => Some(if shift { 'B' } else { 'b' }),
        Key::C => Some(if shift { 'C' } else { 'c' }),
        Key::D => Some(if shift { 'D' } else { 'd' }),
        Key::E => Some(if shift { 'E' } else { 'e' }),
        Key::F => Some(if shift { 'F' } else { 'f' }),
        Key::G => Some(if shift { 'G' } else { 'g' }),
        Key::H => Some(if shift { 'H' } else { 'h' }),
        Key::I => Some(if shift { 'I' } else { 'i' }),
        Key::J => Some(if shift { 'J' } else { 'j' }),
        Key::K => Some(if shift { 'K' } else { 'k' }),
        Key::L => Some(if shift { 'L' } else { 'l' }),
        Key::M => Some(if shift { 'M' } else { 'm' }),
        Key::N => Some(if shift { 'N' } else { 'n' }),
        Key::O => Some(if shift { 'O' } else { 'o' }),
        Key::P => Some(if shift { 'P' } else { 'p' }),
        Key::Q => Some(if shift { 'Q' } else { 'q' }),
        Key::R => Some(if shift { 'R' } else { 'r' }),
        Key::S => Some(if shift { 'S' } else { 's' }),
        Key::T => Some(if shift { 'T' } else { 't' }),
        Key::U => Some(if shift { 'U' } else { 'u' }),
        Key::V => Some(if shift { 'V' } else { 'v' }),
        Key::W => Some(if shift { 'W' } else { 'w' }),
        Key::X => Some(if shift { 'X' } else { 'x' }),
        Key::Y => Some(if shift { 'Y' } else { 'y' }),
        Key::Z => Some(if shift { 'Z' } else { 'z' }),
        Key::Digit(digit) => char::from_digit(digit as u32, 10),
        Key::Space => Some(' '),
        Key::Enter => Some('\n'),
        Key::Tab => Some('\t'),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openkey_core::Ext;

    fn outcome(action: Action, edit: Option<Edit>, passthrough: Option<KeyEvent>) -> Outcome {
        Outcome {
            action,
            edit,
            passthrough,
            ext: Ext::Normal,
        }
    }

    /// `apply_outcome`, asserting the case stays in the Unicode charset the
    /// corpus selects (a byte edit is the only error it returns).
    fn apply(document: &mut String, typed: char, outcome: &Outcome) {
        apply_outcome(document, typed, outcome).expect("the corpus never selects a byte charset");
    }

    fn unicode(text: &str) -> Encoded {
        Encoded::Utf16(text.encode_utf16().collect())
    }

    #[test]
    fn pass_types_the_key_and_backspace_deletes() {
        let mut doc = String::from("vi");
        apply(&mut doc, 'e', &outcome(Action::Pass, None, None));
        assert_eq!(doc, "vie");
        apply(&mut doc, '\u{8}', &outcome(Action::Pass, None, None));
        assert_eq!(doc, "vi");
    }

    #[test]
    fn process_applies_backspaces_then_text() {
        let mut doc = String::from("vieejt");
        let edit = Edit {
            backspaces: 4,
            text: unicode("ệt"),
            beep: false,
        };
        apply(&mut doc, 't', &outcome(Action::Process, Some(edit), None));
        assert_eq!(doc, "việt");
    }

    #[test]
    fn backspaces_past_the_buffer_are_ignored_like_upstream() {
        let mut doc = String::from("vi");
        let edit = Edit {
            backspaces: Engine::MAX_BUFFER as u8,
            text: Encoded::default(),
            beep: false,
        };
        apply(&mut doc, 'x', &outcome(Action::Process, Some(edit), None));
        assert_eq!(doc, "vi");
    }

    #[test]
    fn an_edit_longer_than_the_document_clears_it() {
        let mut doc = String::from("vi");
        let edit = Edit {
            backspaces: 8,
            text: unicode("a"),
            beep: false,
        };
        apply(&mut doc, 'a', &outcome(Action::Process, Some(edit), None));
        assert_eq!(doc, "a");
    }

    #[test]
    fn restore_retypes_the_pressed_key_after_the_edit() {
        let mut doc = String::from("as");
        let edit = Edit {
            backspaces: 2,
            text: unicode("á"),
            beep: false,
        };
        apply(&mut doc, 's', &outcome(Action::Restore, Some(edit), None));
        assert_eq!(doc, "ás");
    }

    #[test]
    fn passthrough_is_typed_after_the_edit() {
        let mut doc = String::from("a");
        let edit = Edit {
            backspaces: 1,
            text: unicode("à"),
            beep: false,
        };
        let space = key_event(' ').unwrap();
        apply(
            &mut doc,
            'a',
            &outcome(Action::Process, Some(edit), Some(space)),
        );
        assert_eq!(doc, "à ");
    }

    #[test]
    fn a_byte_charset_edit_is_refused_rather_than_decoded() {
        let mut doc = String::from("vi");
        let edit = Edit {
            backspaces: 2,
            text: Encoded::Bytes(vec![0x76, 0x69]),
            beep: false,
        };
        let error = apply_outcome(&mut doc, 'x', &outcome(Action::Process, Some(edit), None))
            .expect_err("a byte edit has no Unicode document meaning");
        assert!(matches!(error, ReplayError::Invalid(_)), "{error:?}");
        // Backspaces and text are one decision, so the refusal leaves the
        // document exactly as it was.
        assert_eq!(doc, "vi");
    }

    #[test]
    fn maps_letters_digits_and_shift() {
        assert_eq!(key_event('a').unwrap().key, Key::A);
        let upper = key_event('A').unwrap();
        assert_eq!(upper.key, Key::A);
        assert!(upper.mods.shift);
        assert!(!key_event('a').unwrap().mods.shift);
        assert_eq!(key_event('7').unwrap().key, Key::Digit(7));
        assert_eq!(key_event(' ').unwrap().key, Key::Space);
        assert_eq!(key_event('\u{8}').unwrap().key, Key::Backspace);
        assert!(key_event('ệ').is_none());
    }

    #[test]
    fn a_passthrough_key_without_text_falls_back_to_its_keycode() {
        let mut doc = String::new();
        let enter = KeyEvent {
            key: Key::Enter,
            mods: Mods::default(),
            phase: Phase::Down,
            text: None,
        };
        apply(&mut doc, 'x', &outcome(Action::Process, None, Some(enter)));
        assert_eq!(doc, "\n");
    }
}
