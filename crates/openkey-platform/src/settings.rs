//! Settings persistence: GoKey's versioned JSON document and the one-way,
//! read-only migration from upstream OpenKey's stored profile.
//!
//! Two schemas meet here.
//!
//! **Upstream** stores one flat integer key/value map (`NSUserDefaults` on
//! macOS, the registry on Windows). macOS reads a key that was never written as
//! `0` (`integerForKey:`, `OpenKey.mm:23`), so an absent key and a stored `0`
//! are the same *effective* value there — while the GoKey first-run default for
//! the same field can be `1` (for example `vRememberCode`). [`Settings::from_upstream`]
//! therefore encodes the macOS read semantics per key instead of relying on
//! `#[serde(default)]`, which would silently substitute the first-run profile
//! for an upgrade (PORTING-MAP §3.2).
//!
//! **GoKey** stores one JSON document, [`Settings`], carrying an explicit
//! [`SCHEMA_VERSION`]. A version GoKey does not know is refused rather than
//! partially read.
//!
//! Migration is one-way: the loader borrows the upstream map and never writes
//! back to it (ARCHITECTURE §8). The Windows profile differs in four columns
//! recorded in PORTING-MAP §3.2; 1.0 implements the macOS column only and the
//! Windows column lands with its platform milestone (M4).

use std::collections::BTreeMap;
use std::fmt;

use openkey_core::{Charset, InputMethod, Options};
use serde::{Deserialize, Serialize};

/// Version of GoKey's own settings document. Bump together with a migration
/// from the previous version; a file carrying any other value is refused.
pub const SCHEMA_VERSION: u32 = 1;

/// Settings the engine never reads.
///
/// ADR-0002 D7: a field belongs here — not in `openkey_core::Options` — until
/// core code reads it. `send_key_step_by_step` is read only by the platform
/// glue (`OpenKey.mm:558,771`), so core cases can never observe it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(not(debug_assertions), serde(deny_unknown_fields))]
#[serde(default)]
pub struct HostSettings {
    /// Emit text one key at a time instead of one batch edit (upstream
    /// `vSendKeyStepByStep`). macOS first run and reset both leave this `0`.
    pub send_key_step_by_step: bool,
}

/// One GoKey settings file.
///
/// `schema_version` is required: an older GoKey cannot safely guess what a
/// newer document means, so [`Settings::from_json`] refuses unknown versions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(not(debug_assertions), serde(deny_unknown_fields))]
pub struct Settings {
    pub schema_version: u32,
    #[serde(default)]
    pub options: Options,
    #[serde(default)]
    pub host: HostSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            options: Options::default(),
            host: HostSettings::default(),
        }
    }
}

impl Settings {
    /// Serialize to the on-disk JSON form.
    pub fn to_json_pretty(&self) -> Result<String, SettingsError> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    /// Parse a GoKey settings document.
    ///
    /// A missing or non-integer `schema_version` and any version other than
    /// [`SCHEMA_VERSION`] are errors; the document is never partially applied.
    pub fn from_json(json: &str) -> Result<Self, SettingsError> {
        let value: serde_json::Value = serde_json::from_str(json)?;
        let version = value
            .get("schema_version")
            .and_then(serde_json::Value::as_u64)
            .ok_or(SettingsError::MissingSchemaVersion)?;
        if version != u64::from(SCHEMA_VERSION) {
            return Err(SettingsError::UnsupportedSchemaVersion {
                found: version,
                supported: SCHEMA_VERSION,
            });
        }
        Ok(serde_json::from_value(value)?)
    }

    /// Import an upstream settings store.
    ///
    /// The store is the raw integer map upstream persisted. An absent key is
    /// read as `0`, exactly as macOS `integerForKey:` does — including keys
    /// introduced in 2.0, so a 1.x upgrade keeps upstream's effective values
    /// instead of acquiring GoKey's first-run defaults (DL-013; PORTING-MAP
    /// §3.2). A store with no keys at all is therefore *not* the first-run
    /// profile: callers with no upstream configuration use [`Settings::default`].
    ///
    /// `free_mark` is refused: upstream never reads `FreeMark` back, so a
    /// stored `1` is reported in [`Migration::refused`] and the field stays
    /// `false` (ADR-0002 D6). Nothing is written back to `store`.
    ///
    /// Host-only upstream keys are deliberately not imported here:
    /// `SwitchKeyStatus` (`AppDelegate.m:397-400`) and `ShowUIOnStartup`
    /// (`AppDelegate.m:168`) are platform state, and `vFixChromiumBrowser` /
    /// `vPerformLayoutCompat` have no `Options` field yet (M4).
    pub fn from_upstream(store: &UpstreamStore) -> Result<Migration, SettingsError> {
        let input_method = match integer(store, "InputType") {
            0 => InputMethod::Telex,
            1 => InputMethod::Vni,
            2 => InputMethod::SimpleTelex1,
            3 => InputMethod::SimpleTelex2,
            value => {
                return Err(SettingsError::InvalidUpstreamValue {
                    key: "InputType",
                    value,
                });
            }
        };
        let charset = match integer(store, "CodeTable") {
            0 => Charset::Unicode,
            1 => Charset::Tcvn3,
            2 => Charset::VniWindows,
            value => {
                return Err(SettingsError::InvalidUpstreamValue {
                    key: "CodeTable",
                    value,
                });
            }
        };

        let mut refused = Vec::new();
        if store.get("FreeMark").is_some_and(|value| *value != 0) {
            refused.push(RefusedSetting::FreeMark);
        }

        let options = Options {
            enabled: flag(store, "InputMethod"),
            input_method,
            charset,
            modern_orthography: flag(store, "ModernOrthography"),
            quick_telex: flag(store, "QuickTelex"),
            check_spelling: flag(store, "Spelling"),
            free_mark: false,
            restore_if_invalid: flag(store, "RestoreIfInvalidWord"),
            fix_browser_recommend: flag(store, "FixRecommendBrowser"),
            use_macro: flag(store, "UseMacro"),
            use_macro_in_english: flag(store, "UseMacroInEnglishMode"),
            auto_caps_macro: flag(store, "vAutoCapsMacro"),
            smart_switch_key: flag(store, "UseSmartSwitchKey"),
            upper_case_first_char: flag(store, "UpperCaseFirstChar"),
            temp_off_spelling: flag(store, "vTempOffSpelling"),
            allow_consonant_zfwj: flag(store, "vAllowConsonantZFWJ"),
            quick_start_consonant: flag(store, "vQuickStartConsonant"),
            quick_end_consonant: flag(store, "vQuickEndConsonant"),
            remember_code_per_app: flag(store, "vRememberCode"),
            other_language: flag(store, "vOtherLanguage"),
            temp_off_hotkey: flag(store, "vTempOffOpenKey"),
        };

        Ok(Migration {
            settings: Settings {
                schema_version: SCHEMA_VERSION,
                options,
                host: HostSettings {
                    send_key_step_by_step: flag(store, "SendKeyStepByStep"),
                },
            },
            refused,
        })
    }
}

/// Raw upstream settings: integer values keyed by the names upstream persisted.
pub type UpstreamStore = BTreeMap<String, i64>;

/// Outcome of [`Settings::from_upstream`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Migration {
    pub settings: Settings,
    /// Settings upstream stored but GoKey refuses to persist.
    pub refused: Vec<RefusedSetting>,
}

/// A stored upstream value GoKey deliberately ignores.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefusedSetting {
    /// Upstream never reads `FreeMark` back on either platform
    /// (`OpenKey.mm:91`, `OpenKey.cpp:83`), so neither does GoKey.
    FreeMark,
}

/// macOS semantics: an absent key reads as `0`.
fn integer(store: &UpstreamStore, key: &str) -> i64 {
    store.get(key).copied().unwrap_or(0)
}

/// macOS semantics: an absent key reads as `0`, and any non-zero value is on.
fn flag(store: &UpstreamStore, key: &str) -> bool {
    integer(store, key) != 0
}

/// Errors from parsing or migrating settings.
#[derive(Debug)]
pub enum SettingsError {
    Json(serde_json::Error),
    MissingSchemaVersion,
    UnsupportedSchemaVersion { found: u64, supported: u32 },
    InvalidUpstreamValue { key: &'static str, value: i64 },
}

impl fmt::Display for SettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(err) => write!(f, "invalid settings JSON: {err}"),
            Self::MissingSchemaVersion => {
                write!(f, "settings document has no integer schema_version")
            }
            Self::UnsupportedSchemaVersion { found, supported } => write!(
                f,
                "unsupported settings schema_version {found} (this build reads {supported})"
            ),
            Self::InvalidUpstreamValue { key, value } => {
                write!(f, "upstream setting {key} has unknown value {value}")
            }
        }
    }
}

impl std::error::Error for SettingsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Json(err) => Some(err),
            _ => None,
        }
    }
}

impl From<serde_json::Error> for SettingsError {
    fn from(err: serde_json::Error) -> Self {
        Self::Json(err)
    }
}
