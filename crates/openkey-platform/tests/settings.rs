//! C-013: versioned settings and one-way migration from the upstream store.
//!
//! The fixtures model the upstream macOS profile at three points in its life:
//! a 1.x store (2.0 keys absent), a 2.x first run (`loadDefaultConfig`), and a
//! post-reset store where a persisted `vAutoCapsMacro` survives because
//! `loadDefaultConfig` omits it (PORTING-MAP §3.2 findings 1–3). They are
//! reconstructed from the pinned load path, not captured from a live install.

use std::collections::BTreeMap;

use openkey_core::{Charset, InputMethod, Options};
use openkey_platform::settings::{RefusedSetting, SettingsError};
use openkey_platform::{SCHEMA_VERSION, Settings, UpstreamStore};

fn store(json: &str) -> UpstreamStore {
    serde_json::from_str(json).expect("fixture is an integer map")
}

fn migration(json: &str) -> openkey_platform::Migration {
    Settings::from_upstream(&store(json)).expect("fixture migrates")
}

#[test]
fn a_2x_first_run_store_maps_every_engine_setting() {
    let migrated = migration(include_str!("fixtures/upstream-macos-2x-first-run.json"));
    assert!(migrated.refused.is_empty());
    assert_eq!(migrated.settings.schema_version, SCHEMA_VERSION);

    let options = migrated.settings.options;
    assert!(options.enabled);
    assert_eq!(options.input_method, InputMethod::Telex);
    assert_eq!(options.charset, Charset::Unicode);
    assert!(!options.modern_orthography);
    assert!(!options.quick_telex);
    assert!(options.check_spelling);
    assert!(!options.free_mark);
    assert!(!options.restore_if_invalid);
    assert!(options.fix_browser_recommend);
    assert!(options.use_macro);
    assert!(!options.use_macro_in_english);
    // `loadDefaultConfig` never writes `vAutoCapsMacro`; absent reads 0.
    assert!(!options.auto_caps_macro);
    assert!(options.smart_switch_key);
    assert!(!options.upper_case_first_char);
    assert!(!options.temp_off_spelling);
    assert!(!options.allow_consonant_zfwj);
    assert!(!options.quick_start_consonant);
    assert!(!options.quick_end_consonant);
    assert!(options.remember_code_per_app);
    assert!(options.other_language);
    assert!(!options.temp_off_hotkey);
    assert!(!migrated.settings.host.send_key_step_by_step);
}

#[test]
fn a_1x_upgrade_reads_absent_keys_as_macos_does() {
    let migrated = migration(include_str!("fixtures/upstream-macos-1x.json"));

    // The two 2.0 keys are absent from a 1.x store. macOS reads 0, so the
    // upgrade must not silently acquire GoKey's first-run `true` (DL-013).
    assert!(!migrated.settings.options.remember_code_per_app);
    assert!(!migrated.settings.options.other_language);

    // Upstream never reads `FreeMark` back; a stored 1 is refused, not imported.
    assert!(!migrated.settings.options.free_mark);
    assert_eq!(migrated.refused, vec![RefusedSetting::FreeMark]);
}

#[test]
fn a_macos_reset_keeps_a_persisted_auto_caps_macro() {
    // `loadDefaultConfig` omits `vAutoCapsMacro`, so a reset is not a first
    // run: a value persisted before the reset survives (PORTING-MAP §3.2 F1).
    let migrated = migration(include_str!(
        "fixtures/upstream-macos-reset-kept-autocaps.json"
    ));
    assert!(migrated.settings.options.auto_caps_macro);
}

#[test]
fn upstream_enum_values_map_by_upstream_order() {
    let mut raw = BTreeMap::new();
    raw.insert("InputType".to_string(), 3);
    raw.insert("CodeTable".to_string(), 2);
    let options = Settings::from_upstream(&raw).unwrap().settings.options;
    assert_eq!(options.input_method, InputMethod::SimpleTelex2);
    assert_eq!(options.charset, Charset::VniWindows);

    let mut raw = BTreeMap::new();
    raw.insert("InputType".to_string(), 1);
    raw.insert("CodeTable".to_string(), 1);
    let options = Settings::from_upstream(&raw).unwrap().settings.options;
    assert_eq!(options.input_method, InputMethod::Vni);
    assert_eq!(options.charset, Charset::Tcvn3);
}

#[test]
fn unknown_upstream_enum_values_are_refused() {
    let mut raw = BTreeMap::new();
    raw.insert("InputType".to_string(), 9);
    match Settings::from_upstream(&raw).unwrap_err() {
        SettingsError::InvalidUpstreamValue { key, value } => {
            assert_eq!(key, "InputType");
            assert_eq!(value, 9);
        }
        other => panic!("expected InvalidUpstreamValue, got {other}"),
    }

    let mut raw = BTreeMap::new();
    raw.insert("CodeTable".to_string(), 3);
    match Settings::from_upstream(&raw).unwrap_err() {
        SettingsError::InvalidUpstreamValue { key, value } => {
            assert_eq!(key, "CodeTable");
            assert_eq!(value, 3);
        }
        other => panic!("expected InvalidUpstreamValue, got {other}"),
    }
}

#[test]
fn migration_never_writes_the_upstream_store() {
    let json = include_str!("fixtures/upstream-macos-1x.json");
    let before: serde_json::Value = serde_json::from_str(json).unwrap();
    let raw = store(json);

    let _ = Settings::from_upstream(&raw).unwrap();

    // The API borrows the store, and re-serializing it is byte-stable in
    // value terms: no key was added, removed or changed.
    assert_eq!(before, serde_json::to_value(&raw).unwrap());
}

#[test]
fn settings_round_trip_with_a_schema_version() {
    let settings = Settings::default();
    let json = settings.to_json_pretty().unwrap();
    assert!(json.contains("\"schema_version\": 1"), "{json}");
    assert_eq!(Settings::from_json(&json).unwrap(), settings);
    assert_eq!(settings.options, Options::default());
}

#[test]
fn a_missing_or_future_schema_version_is_refused() {
    match Settings::from_json("{\"options\": {}}").unwrap_err() {
        SettingsError::MissingSchemaVersion => {}
        other => panic!("expected MissingSchemaVersion, got {other}"),
    }

    match Settings::from_json("{\"schema_version\": 2}").unwrap_err() {
        SettingsError::UnsupportedSchemaVersion { found, supported } => {
            assert_eq!((found, supported), (2, SCHEMA_VERSION));
        }
        other => panic!("expected UnsupportedSchemaVersion, got {other}"),
    }
}

#[cfg(debug_assertions)]
#[test]
fn unknown_fields_are_tolerated_in_dev() {
    let parsed = Settings::from_json("{\"schema_version\": 1, \"future\": true}").unwrap();
    assert_eq!(parsed, Settings::default());

    let parsed =
        Settings::from_json("{\"schema_version\": 1, \"options\": {\"future\": true}}").unwrap();
    assert_eq!(parsed.options, Options::default());
}

#[cfg(not(debug_assertions))]
#[test]
fn unknown_fields_are_rejected_in_release() {
    assert!(Settings::from_json("{\"schema_version\": 1, \"future\": true}").is_err());
    assert!(
        Settings::from_json("{\"schema_version\": 1, \"options\": {\"future\": true}}").is_err()
    );
}
