//! The one place a captured event reaches the engine.
//!
//! Two guarantees live here, and both are tested rather than commented:
//!
//! 1. **A panic passes the key through.** `Engine::apply` runs under
//!    `catch_unwind`. A panic disables the engine for the rest of the process —
//!    it is not retried once per keystroke — and every event afterwards is
//!    delivered unchanged. A keyboard that eats input is worse than no keyboard,
//!    so the answer to "the engine crashed" is "type normally", loudly.
//! 2. **Nothing is swallowed before emission exists.** MAC-001 captures; MAC-002
//!    emits. Until then every [`Reaction`] reports `swallows() == false`, so a
//!    captured event is always delivered. MAC-002 flips the edit arm and keeps
//!    the test that pins the rest.
//!
//! This requires unwinding. The workspace does not set `panic = "abort"`
//! (`Cargo.toml`'s release profile sets LTO, codegen units and strip only); if
//! that ever changes, this module's guarantee becomes impossible and the change
//! should be rejected rather than merged with the guarantee removed.

use std::panic::{self, AssertUnwindSafe};

use openkey_core::{Action, Engine, KeyEvent, Outcome};

use crate::capture::NativeEvent;

/// The engine as this adapter needs it: one call, no lifecycle. `Engine` is the
/// production implementation; tests substitute ones that panic or record.
pub trait KeystrokeEngine {
    fn apply(&mut self, event: &KeyEvent) -> Outcome;
}

impl KeystrokeEngine for Engine {
    fn apply(&mut self, event: &KeyEvent) -> Outcome {
        Engine::apply(self, event)
    }
}

/// What the callback should do with the event it is holding.
#[derive(Clone, Debug)]
pub enum Reaction {
    /// Deliver the original event unchanged.
    Pass,
    /// The engine answered with work, but this build cannot emit it yet.
    /// The event is still delivered; the edit is dropped with a one-time
    /// diagnostic so the failure is visible instead of silent.
    EditNotEmitted(Outcome),
    /// The engine panicked. The engine is now disabled for this process and the
    /// event is delivered unchanged.
    EngineFault,
}

impl Reaction {
    /// Whether the tap should return `NULL` (swallow) for this event.
    ///
    /// Always `false` in MAC-001: capture is implemented, emission is MAC-002.
    /// The method exists — rather than the callback hard-coding "return the
    /// event" — so that one place decides, and the test below pins it.
    pub fn swallows(&self) -> bool {
        false
    }

    /// Short token for the probe's output, stable across builds.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::EditNotEmitted(_) => "edit-not-emitted",
            Self::EngineFault => "engine-fault",
        }
    }
}

/// Owns the engine and the fault state for one process.
pub struct EngineHost<E: KeystrokeEngine> {
    engine: E,
    faulted: bool,
    warned_unwired: bool,
}

impl<E: KeystrokeEngine> EngineHost<E> {
    pub fn new(engine: E) -> Self {
        Self {
            engine,
            faulted: false,
            warned_unwired: false,
        }
    }

    /// Whether a panic has disabled the engine. Once true it stays true: the
    /// input path never calls a crashed engine again.
    pub fn faulted(&self) -> bool {
        self.faulted
    }

    pub fn engine(&self) -> &E {
        &self.engine
    }

    pub fn engine_mut(&mut self) -> &mut E {
        &mut self.engine
    }

    /// Feed one captured event.
    pub fn handle(&mut self, event: NativeEvent) -> Reaction {
        let Some(key_event) = event.to_key_event() else {
            // Flags-changed, mouse and disabled notices are not keystrokes.
            // Their engine-side effects are MAC-004's hotkeys and C-002's mouse
            // session entry point (ADR-0004 §1.1); neither is adapter state, so
            // this layer does nothing with them and the event passes.
            return Reaction::Pass;
        };

        if self.faulted {
            return Reaction::EngineFault;
        }

        match panic::catch_unwind(AssertUnwindSafe(|| self.engine.apply(&key_event))) {
            Ok(outcome) => {
                if matches!(outcome.action, Action::Pass) && outcome.edit.is_none() {
                    Reaction::Pass
                } else {
                    if !self.warned_unwired {
                        self.warned_unwired = true;
                        eprintln!(
                            "openkey: the engine produced an edit but emission is not implemented \
                             in this build (MAC-002); the key was delivered unchanged and the \
                             edit discarded"
                        );
                    }
                    Reaction::EditNotEmitted(outcome)
                }
            }
            Err(_) => {
                self.faulted = true;
                eprintln!(
                    "openkey: the engine panicked while handling a keystroke. Vietnamese input \
                     is disabled for the rest of this process; every key now passes through \
                     unchanged. This is a bug — please report it."
                );
                Reaction::EngineFault
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use openkey_core::{Edit, Key, Mods, Phase};

    use super::*;
    use crate::capture::{DisableReason, MouseKind};

    fn key_down() -> NativeEvent {
        NativeEvent::Key {
            key: Key::E,
            mods: Mods::default(),
            phase: Phase::Down,
            text: Some('e'),
        }
    }

    fn edit() -> Outcome {
        Outcome {
            action: Action::Process,
            edit: Some(Edit {
                backspaces: 1,
                text: "ệ".to_owned(),
                beep: false,
            }),
            passthrough: None,
        }
    }

    /// Stand-in for a broken engine: counts calls, then panics.
    struct Panicky {
        calls: Arc<AtomicUsize>,
    }

    impl KeystrokeEngine for Panicky {
        fn apply(&mut self, _event: &KeyEvent) -> Outcome {
            self.calls.fetch_add(1, Ordering::Relaxed);
            panic!("engine bug");
        }
    }

    /// Stand-in that records calls and answers with a fixed outcome.
    struct Canned {
        calls: Arc<AtomicUsize>,
        outcome: Option<Outcome>,
    }

    impl KeystrokeEngine for Canned {
        fn apply(&mut self, _event: &KeyEvent) -> Outcome {
            self.calls.fetch_add(1, Ordering::Relaxed);
            self.outcome.clone().unwrap_or(Outcome {
                action: Action::Pass,
                edit: None,
                passthrough: None,
            })
        }
    }

    #[test]
    fn a_panic_passes_the_key_and_is_not_retried() {
        let calls = Arc::new(AtomicUsize::new(0));
        let mut host = EngineHost::new(Panicky {
            calls: Arc::clone(&calls),
        });

        let first = host.handle(key_down());
        assert!(matches!(first, Reaction::EngineFault), "{first:?}");
        assert!(host.faulted());

        let second = host.handle(key_down());
        assert!(matches!(second, Reaction::EngineFault));
        assert_eq!(
            calls.load(Ordering::Relaxed),
            1,
            "a crashed engine must not be called once per keystroke"
        );
    }

    #[test]
    fn an_engine_edit_is_not_emitted_and_the_key_still_passes() {
        let calls = Arc::new(AtomicUsize::new(0));
        let mut host = EngineHost::new(Canned {
            calls: Arc::clone(&calls),
            outcome: Some(edit()),
        });
        let reaction = host.handle(key_down());
        assert!(
            matches!(reaction, Reaction::EditNotEmitted(_)),
            "{reaction:?}"
        );
        assert!(!reaction.swallows());
        assert!(!host.faulted(), "an edit is not a fault");
        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn an_engine_pass_is_a_pass() {
        let calls = Arc::new(AtomicUsize::new(0));
        let mut host = EngineHost::new(Canned {
            calls: Arc::clone(&calls),
            outcome: None,
        });
        let reaction = host.handle(key_down());
        assert!(matches!(reaction, Reaction::Pass), "{reaction:?}");
        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn nothing_is_swallowed_before_emission_exists() {
        // MAC-002 changes the edit arm to swallow; the other two stay as they
        // are. This test is the line the change must not cross.
        for reaction in [
            Reaction::Pass,
            Reaction::EngineFault,
            Reaction::EditNotEmitted(edit()),
        ] {
            assert!(!reaction.swallows(), "{reaction:?}");
        }
    }

    #[test]
    fn non_keystroke_events_do_not_reach_the_engine() {
        let calls = Arc::new(AtomicUsize::new(0));
        let mut host = EngineHost::new(Canned {
            calls: Arc::clone(&calls),
            outcome: Some(edit()),
        });
        for event in [
            NativeEvent::FlagsChanged {
                keycode: crate::keymap::VK_OPTION,
                modifier: Some(crate::keymap::Modifier::Option),
                mods: Mods::default(),
            },
            NativeEvent::Mouse {
                kind: MouseKind::LeftDown,
                mods: Mods::default(),
            },
            NativeEvent::TapDisabled {
                reason: DisableReason::Timeout,
            },
            NativeEvent::Other,
        ] {
            assert!(matches!(host.handle(event), Reaction::Pass), "{event:?}");
        }
        assert_eq!(
            calls.load(Ordering::Relaxed),
            0,
            "flags, mouse and disabled notices are not keystrokes"
        );
    }

    #[test]
    fn the_production_engine_implements_the_trait() {
        // Compile-time only: `Engine` must stay the thing the tap installs.
        fn assert_engine<T: KeystrokeEngine>() {}
        assert_engine::<Engine>();
    }
}
