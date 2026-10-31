//! User-facing settings.
//!
//! Mirrors upstream `extern int v*` globals (see `Engine.h`) but as an owned,
//! serializable struct instead of process-global mutable state.
//!
//! For 1.0 the default profile is the upstream macOS first-run profile written
//! by `loadDefaultConfig` (`AppDelegate.m:288-315` at the pin), not the C
//! initialisers. macOS and Windows disagree on some of these values; the
//! four-column matrix is recorded in `docs/m0/PORTING-MAP.md` §3.2 and the
//! Windows column is deferred to M4.
//!
//! Persistence and migration from the legacy macOS plist / Windows registry
//! live in `openkey-platform`, not here. Fields obey the ADR-0002 D7 membership
//! rule: a field exists only when core code reads it, so host state such as
//! `send_key_step_by_step` is `openkey_platform::HostSettings`.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputMethod {
    #[default]
    Telex,
    Vni,
    SimpleTelex1,
    SimpleTelex2,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Charset {
    #[default]
    Unicode,
    Tcvn3,
    VniWindows,
    UnicodeCompound,
    Cp1258,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(not(debug_assertions), serde(deny_unknown_fields))]
#[serde(default)]
pub struct Options {
    /// Vietnamese input on/off (upstream `vLanguage`).
    pub enabled: bool,
    pub input_method: InputMethod,
    pub charset: Charset,
    /// `oà, uý` instead of `òa, úy`.
    pub modern_orthography: bool,
    /// `cc=ch, gg=gi, kk=kh, nn=ng, qq=qu, pp=ph, tt=th`.
    pub quick_telex: bool,
    pub check_spelling: bool,
    /// Keep tone marks where the user typed them (upstream `vFreeMark`).
    ///
    /// Persistence is refused: upstream never reads the key back on either
    /// platform (`OpenKey.mm:91`, `OpenKey.cpp:83`), so
    /// `openkey_platform::Settings::from_upstream` always restores `false`
    /// (ADR-0002 D6).
    pub free_mark: bool,
    pub restore_if_invalid: bool,
    pub fix_browser_recommend: bool,
    pub use_macro: bool,
    pub use_macro_in_english: bool,
    pub auto_caps_macro: bool,
    /// Remember input mode per application.
    pub smart_switch_key: bool,
    pub upper_case_first_char: bool,
    /// Hold Ctrl to suspend spell checking temporarily.
    pub temp_off_spelling: bool,
    /// Allow `z f w j` as initial consonants.
    pub allow_consonant_zfwj: bool,
    /// `f->ph, j->gi, w->qu`.
    pub quick_start_consonant: bool,
    /// `g->ng, h->nh, k->ch`.
    pub quick_end_consonant: bool,
    /// Remember charset per application.
    pub remember_code_per_app: bool,
    /// Turn Vietnamese off when typing another language.
    pub other_language: bool,
    /// Hotkey to suspend the whole engine.
    pub temp_off_hotkey: bool,
}

impl Default for Options {
    /// The macOS first-run profile written by `loadDefaultConfig`
    /// (`AppDelegate.m:288-315`), adopted unmodified (C-013).
    fn default() -> Self {
        Self {
            enabled: true,
            input_method: InputMethod::Telex,
            charset: Charset::Unicode,
            modern_orthography: false,
            quick_telex: false,
            check_spelling: true,
            free_mark: false,
            restore_if_invalid: false,
            fix_browser_recommend: true,
            use_macro: true,
            use_macro_in_english: false,
            auto_caps_macro: false,
            smart_switch_key: true,
            upper_case_first_char: false,
            temp_off_spelling: false,
            allow_consonant_zfwj: false,
            quick_start_consonant: false,
            quick_end_consonant: false,
            remember_code_per_app: true,
            other_language: true,
            temp_off_hotkey: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_the_macos_first_run_profile() {
        let options = Options::default();
        // The five fields that disagreed with loadDefaultConfig before C-013.
        assert!(!options.restore_if_invalid);
        assert!(!options.auto_caps_macro);
        assert!(!options.temp_off_spelling);
        assert!(options.remember_code_per_app);
        assert!(options.other_language);
        // R-API-4: upstream `vFreeMark` is 0 on both platforms.
        assert!(!options.free_mark);
        // The rest of `loadDefaultConfig` (AppDelegate.m:288-315).
        assert!(options.enabled);
        assert_eq!(options.input_method, InputMethod::Telex);
        assert_eq!(options.charset, Charset::Unicode);
        assert!(!options.modern_orthography);
        assert!(!options.quick_telex);
        assert!(options.check_spelling);
        assert!(options.fix_browser_recommend);
        assert!(options.use_macro);
        assert!(!options.use_macro_in_english);
        assert!(options.smart_switch_key);
        assert!(!options.upper_case_first_char);
        assert!(!options.allow_consonant_zfwj);
        assert!(!options.quick_start_consonant);
        assert!(!options.quick_end_consonant);
        assert!(!options.temp_off_hotkey);
    }
}
