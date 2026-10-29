//! User-facing settings.
//!
//! Mirrors upstream `extern int v*` globals (see `Engine.h`) but as an owned,
//! serializable struct instead of process-global mutable state. Defaults must
//! match upstream defaults; the parity corpus is generated with these defaults.
//!
//! Persistence and migration from the legacy macOS plist / Windows registry
//! belong to `openkey-platform`, not here (task C-014).

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
    pub check_grammar: bool,
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
    /// Emit per-key instead of one batch edit (compatibility fallback).
    pub send_key_step_by_step: bool,
}

impl Default for Options {
    fn default() -> Self {
        // TODO(C-014): verify each default against upstream macOS/Windows apps.
        Self {
            enabled: true,
            input_method: InputMethod::Telex,
            charset: Charset::Unicode,
            modern_orthography: false,
            quick_telex: false,
            check_spelling: true,
            check_grammar: false,
            restore_if_invalid: true,
            fix_browser_recommend: true,
            use_macro: true,
            use_macro_in_english: false,
            auto_caps_macro: true,
            smart_switch_key: true,
            upper_case_first_char: false,
            temp_off_spelling: true,
            allow_consonant_zfwj: false,
            quick_start_consonant: false,
            quick_end_consonant: false,
            remember_code_per_app: false,
            other_language: false,
            temp_off_hotkey: false,
            send_key_step_by_step: false,
        }
    }
}
