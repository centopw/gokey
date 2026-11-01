//! Smart switch key: per-application input method state.
//!
//! Port of upstream `Sources/OpenKey/engine/SmartSwitchKey.cpp` (`initSmartSwitchKey`
//! `:19-37`, `getSmartSwitchKeySaveData` `:39-52`, `getAppInputMethodStatus`
//! `:54-67`, `setAppInputMethodStatus` `:69-73`) and of the three hook policies
//! that drive it (`Sources/OpenKey/macOS/ModernKey/OpenKey.mm:207-242`).
//!
//! Upstream keeps the table in a file-static `map<string, Int8>` and the lookup
//! cache in two more file statics (`:15-17`). Here the table, the cache and the
//! policy are owned values, so two sessions can be compared side by side and no
//! host state leaks into the engine.
//!
//! The stored byte is upstream's `vLanguage | (vCodeTable << 1)` — bit 0 is the
//! input language, the rest is the charset index.
//!
//! The map is keyed by bytes, not `String`: upstream's `std::string` is a byte
//! string, the load path copies whatever the preference blob contained, and the
//! ratified error set has no "not UTF-8" case. Lookups, by contrast, always come
//! from a bundle id converted from an `NSString`, so the public API takes
//! `&str` — the same asymmetry upstream has, where a key that is not UTF-8 can
//! be loaded and never matched.
//!
//! Two deliberate differences from upstream, both on paths upstream leaves
//! undefined:
//!
//! - a truncated entry or a stored byte outside `0..=9` returns
//!   [`SmartSwitchError`] instead of reading past the buffer (`:30-35` has no
//!   bound check) or storing a value the two-field model cannot represent;
//! - the table is built into a local map, so a parse error yields no table at
//!   all rather than a half-filled one.
//!
//! What is *not* a difference: decoding the packed byte. Upstream's
//! `vLanguage = _languageTemp` (`OpenKey.mm:211`) cannot be observed — the guard
//! already isolates bit 0, the next line calls `onImputMethodChanged:NO`, which
//! overwrites `vLanguage` with the toggled persisted `0/1`
//! (`AppDelegate.m:427-437`), and because `willNotify` is `NO` the
//! `OnInputMethodChanged` write-back (`:235`) is never reached on this path.
//! `language = value & 1` and `charset = value >> 1` are therefore exactly what
//! the app does for every value in `0..=9`.

use std::collections::BTreeMap;
use std::fmt;

use crate::options::{Charset, Options};

/// The largest value the two-field model can represent: Vietnamese (1) with the
/// last charset index (4), i.e. `1 | (4 << 1)`. Five charsets and two languages
/// bound the domain; anything above it is upstream data the port cannot decode.
pub const MAX_VALUE: u8 = 1 | (CHARSET_MAX << 1);

/// Highest charset index (`Charset::Cp1258`).
const CHARSET_MAX: u8 = 4;

/// The sentinel upstream returns for an application it has not seen before.
/// Modelled as an `i32` because upstream compares and shifts it arithmetically
/// (`(-1) & 1 == 1`, `(-1) >> 1 == -1`), and those two facts decide which
/// branches save the table.
const UNKNOWN: i32 = -1;

/// A load or store that upstream would accept but the port cannot represent, or
/// input that is shorter than its own header claims.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SmartSwitchError {
    /// No byte left for an entry's bundle-id length.
    TruncatedBundleLength { offset: usize },
    /// The bundle-id length reaches past the end of the buffer.
    TruncatedBundle {
        offset: usize,
        declared: usize,
        available: usize,
    },
    /// No byte left for the entry's value.
    TruncatedValue { offset: usize },
    /// A stored byte above [`MAX_VALUE`].
    ValueOutOfRange(u8),
}

impl fmt::Display for SmartSwitchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::TruncatedBundleLength { offset } => {
                write!(
                    f,
                    "smart switch data ends at {offset}: bundle-id length missing"
                )
            }
            Self::TruncatedBundle {
                offset,
                declared,
                available,
            } => write!(
                f,
                "smart switch data ends at {offset}: bundle id declares {declared} byte(s), \
                 {available} available"
            ),
            Self::TruncatedValue { offset } => {
                write!(f, "smart switch data ends at {offset}: value missing")
            }
            Self::ValueOutOfRange(value) => {
                write!(
                    f,
                    "smart switch value {value} is above the model maximum {MAX_VALUE}"
                )
            }
        }
    }
}

impl std::error::Error for SmartSwitchError {}

/// The per-application state upstream keeps in `_smartSwitchKeyData` plus its
/// two-entry lookup cache (`SmartSwitchKey.cpp:15-17`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SmartSwitchKey {
    entries: BTreeMap<Vec<u8>, u8>,
    cache_key: Vec<u8>,
    cache_value: u8,
}

impl Default for SmartSwitchKey {
    fn default() -> Self {
        Self::new()
    }
}

impl SmartSwitchKey {
    /// An empty table with upstream's initial cache (`:16-17`: key `""`, value
    /// `0`).
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
            cache_key: Vec::new(),
            cache_value: 0,
        }
    }

    /// `initSmartSwitchKey` (`:19-37`).
    ///
    /// A buffer shorter than the two-byte count is the empty table (upstream
    /// leaves `count = 0`), trailing bytes after the last entry are ignored, an
    /// empty bundle id is stored under `""`, and a repeated id keeps the last
    /// value. A truncated entry or a value above [`MAX_VALUE`] is an error, and
    /// because the map is built locally no partial table escapes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, SmartSwitchError> {
        let mut table = Self::new();
        if bytes.len() < 2 {
            return Ok(table);
        }
        let count = usize::from(u16::from_le_bytes([bytes[0], bytes[1]]));
        let mut entries = BTreeMap::new();
        let mut cursor = 2usize;
        for _ in 0..count {
            let Some(&declared) = bytes.get(cursor) else {
                return Err(SmartSwitchError::TruncatedBundleLength { offset: cursor });
            };
            cursor += 1;
            let declared = usize::from(declared);
            let Some(end) = cursor
                .checked_add(declared)
                .filter(|end| *end <= bytes.len())
            else {
                return Err(SmartSwitchError::TruncatedBundle {
                    offset: cursor,
                    declared,
                    available: bytes.len().saturating_sub(cursor),
                });
            };
            let bundle = bytes[cursor..end].to_vec();
            cursor = end;
            let Some(&value) = bytes.get(cursor) else {
                return Err(SmartSwitchError::TruncatedValue { offset: cursor });
            };
            cursor += 1;
            if value > MAX_VALUE {
                return Err(SmartSwitchError::ValueOutOfRange(value));
            }
            entries.insert(bundle, value);
        }
        table.entries = entries;
        Ok(table)
    }

    /// `getSmartSwitchKeySaveData` (`:39-52`): little-endian `u16` count, then
    /// length-prefixed bundle ids and values in ascending key order.
    ///
    /// The count is a `u16` cast exactly as upstream writes it; a table with
    /// more than `u16::MAX` entries would write a count that disagrees with the
    /// body, which is upstream's own behaviour and unreachable in practice.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(2 + self.entries.len() * 8);
        let count = self.entries.len() as u16;
        out.extend_from_slice(&count.to_le_bytes());
        for (bundle, value) in &self.entries {
            out.push(bundle.len() as u8);
            out.extend_from_slice(bundle);
            out.push(*value);
        }
        out
    }

    /// Number of stored applications.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the table stores no application.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// What the table stores for `bundle`, without touching the cache.
    #[must_use]
    pub fn stored(&self, bundle: &str) -> Option<u8> {
        self.entries.get(bundle.as_bytes()).copied()
    }

    /// `getAppInputMethodStatus` (`:54-67`).
    ///
    /// `current` is upstream's `vLanguage | (vCodeTable << 1)`. Returns `None`
    /// for an application the table had not seen — upstream's `-1` — after
    /// **seeding that application with `current`**, which is what makes the
    /// caller save. A hit, including the cache, returns the stored byte.
    ///
    /// The cache starts at key `""` / value `0` (`:16-17`), so a first
    /// `get("", _)` returns `Some(0)` and seeds nothing. That is upstream's
    /// initial state, kept deliberately.
    pub fn get(&mut self, bundle: &str, current: u8) -> Option<u8> {
        let bundle = bundle.as_bytes();
        if self.cache_key == bundle {
            return Some(self.cache_value);
        }
        if let Some(&value) = self.entries.get(bundle) {
            self.cache_key = bundle.to_vec();
            self.cache_value = value;
            return Some(value);
        }
        self.cache_key = bundle.to_vec();
        self.cache_value = current;
        self.entries.insert(bundle.to_vec(), current);
        None
    }

    /// `setAppInputMethodStatus` (`:69-73`), rejecting a value the model cannot
    /// hold. The table is unchanged when it returns an error.
    pub fn set(&mut self, bundle: &str, value: u8) -> Result<(), SmartSwitchError> {
        if value > MAX_VALUE {
            return Err(SmartSwitchError::ValueOutOfRange(value));
        }
        self.put(bundle.as_bytes(), value);
        Ok(())
    }

    /// The shared body of `set`, for callers that already hold a valid packed
    /// value.
    fn put(&mut self, bundle: &[u8], value: u8) {
        self.entries.insert(bundle.to_vec(), value);
        self.cache_key = bundle.to_vec();
        self.cache_value = value;
    }
}

/// `vLanguage | (vCodeTable << 1)`.
#[must_use]
pub const fn pack(language: bool, charset: Charset) -> u8 {
    (language as u8) | ((charset as u8) << 1)
}

/// Split a stored byte into `(language, charset)`.
///
/// Equivalent to upstream's use of the packed value for every byte in
/// `0..=9` — see the module docs for why `vLanguage = _languageTemp`
/// (`OpenKey.mm:211`) is not observable.
pub fn unpack(value: u8) -> Result<(bool, Charset), SmartSwitchError> {
    // `value >> 1 <= CHARSET_MAX` is the same bound as `value <= MAX_VALUE`.
    let charset = charset_from_index(value >> 1).ok_or(SmartSwitchError::ValueOutOfRange(value))?;
    Ok((value & 1 == 1, charset))
}

fn charset_from_index(index: u8) -> Option<Charset> {
    Some(match index {
        0 => Charset::Unicode,
        1 => Charset::Tcvn3,
        2 => Charset::VniWindows,
        3 => Charset::UnicodeCompound,
        4 => Charset::Cp1258,
        _ => return None,
    })
}

/// What the host must do after the frontmost application changed
/// (`OpenKey.mm:207-219`).
///
/// `language` and `charset` are set only when that field differs from the
/// current option, so the caller can apply them in order and start a new
/// session once. `save` asks for `saveSmartSwitchKeyData` (`:17-21`): upstream
/// calls it when the application was unknown, i.e. when `get` just seeded it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ActiveAppSwitch {
    pub language: Option<bool>,
    pub charset: Option<Charset>,
    pub new_session: bool,
    pub save: bool,
}

impl ActiveAppSwitch {
    /// Whether the switch changes nothing the caller can observe.
    #[must_use]
    pub fn is_noop(&self) -> bool {
        self.language.is_none() && self.charset.is_none() && !self.new_session && !self.save
    }
}

/// `OnActiveAppChanged` (`OpenKey.mm:207-219`).
///
/// The comparisons are the upstream ones, including the arithmetic shift of the
/// `-1` sentinel: `(-1) & 1 == 1` leaves English alone on a first sight, and
/// `(-1) >> 1 == -1` differs from every charset, so an unknown application
/// always asks for a save.
///
/// Upstream passes `vLanguage | (vCodeTable << 1)` inline as the seed; that is
/// exactly `pack(options.enabled, options.charset)`, so the value is derived
/// here rather than accepted as a parameter that could disagree with `options`.
pub fn on_active_app_changed(
    table: &mut SmartSwitchKey,
    options: &Options,
    bundle: &str,
) -> ActiveAppSwitch {
    let current = pack(options.enabled, options.charset);
    let stored = table.get(bundle, current).map_or(UNKNOWN, i32::from);
    let known = stored != UNKNOWN;
    let mut switch = ActiveAppSwitch::default();

    if (stored & 1) != i32::from(options.enabled) {
        if known {
            switch.language = Some(stored & 1 == 1);
            // `onImputMethodChanged:NO` then `startNewSession` (`:210-213`).
            switch.new_session = true;
        } else {
            switch.save = true;
        }
    }

    if options.remember_code_per_app && (stored >> 1) != i32::from(options.charset as u8) {
        if known {
            switch.charset = charset_from_index((stored as u8) >> 1);
        } else {
            switch.save = true;
        }
    }

    switch
}

/// `OnInputMethodChanged` (`OpenKey.mm:231-237`): the user picked a language, so
/// remember it for the frontmost application when the option is on.
/// Returns whether the caller must save.
///
/// `options` must already carry the **new** selection: upstream reads
/// `vLanguage` after the toggle, which is why `onImputMethodChanged:NO` rewrites
/// it before this hook runs (`AppDelegate.m:427-437`).
pub fn on_input_method_changed(
    table: &mut SmartSwitchKey,
    options: &Options,
    bundle: &str,
) -> bool {
    if !options.smart_switch_key {
        return false;
    }
    table.put(bundle.as_bytes(), pack(options.enabled, options.charset));
    true
}

/// `OnTableCodeChange` (`OpenKey.mm:221-229`): `onTableCodeChange()` has already
/// rebuilt the tables, so remember the new charset for the frontmost
/// application when the option is on. Returns whether the caller must save.
///
/// As above, `options` must already carry the **new** charset; the caller is
/// also responsible for the rebuild, which is why the adapter applies this
/// through `Engine::set_options`.
pub fn on_table_code_change(table: &mut SmartSwitchKey, options: &Options, bundle: &str) -> bool {
    if !options.remember_code_per_app {
        return false;
    }
    table.put(bundle.as_bytes(), pack(options.enabled, options.charset));
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::InputMethod;

    fn options() -> Options {
        Options::default()
    }

    fn english(mut options: Options) -> Options {
        options.enabled = false;
        options
    }

    fn vni_windows(mut options: Options) -> Options {
        options.charset = Charset::VniWindows;
        options
    }

    // --- encoding -----------------------------------------------------------

    #[test]
    fn an_empty_table_round_trips_as_the_two_byte_count() {
        let table = SmartSwitchKey::new();
        assert_eq!(table.to_bytes(), [0, 0]);
        let reloaded = SmartSwitchKey::from_bytes(&table.to_bytes()).expect("well-formed");
        assert!(reloaded.is_empty());
        assert_eq!(reloaded, table);
    }

    #[test]
    fn a_table_round_trips_with_its_values() {
        let mut table = SmartSwitchKey::new();
        table
            .set("com.apple.Safari", pack(true, Charset::Tcvn3))
            .unwrap();
        table
            .set("com.google.Chrome", pack(false, Charset::Cp1258))
            .unwrap();
        let reloaded = SmartSwitchKey::from_bytes(&table.to_bytes()).expect("well-formed");
        assert_eq!(reloaded.len(), 2);
        assert_eq!(
            reloaded.stored("com.apple.Safari"),
            Some(pack(true, Charset::Tcvn3))
        );
        assert_eq!(
            reloaded.stored("com.google.Chrome"),
            Some(pack(false, Charset::Cp1258))
        );
    }

    #[test]
    fn entries_are_saved_in_ascending_key_order() {
        let mut table = SmartSwitchKey::new();
        table.set("b.app", 1).unwrap();
        table.set("a.app", 0).unwrap();
        table.set("c.app", 3).unwrap();
        let bytes = table.to_bytes();
        assert_eq!(bytes[0..2], [3, 0]);
        assert_eq!(&bytes[2..], b"\x05a.app\x00\x05b.app\x01\x05c.app\x03");
    }

    #[test]
    fn a_buffer_shorter_than_the_count_is_the_empty_table() {
        assert!(SmartSwitchKey::from_bytes(&[]).expect("size 0").is_empty());
        assert!(SmartSwitchKey::from_bytes(&[7]).expect("size 1").is_empty());
    }

    #[test]
    fn trailing_bytes_after_the_last_entry_are_ignored() {
        let bytes = b"\x01\x00\x03app\x05leftover";
        let table = SmartSwitchKey::from_bytes(bytes).expect("well-formed prefix");
        assert_eq!(table.stored("app"), Some(5));
        assert_eq!(table.len(), 1);
    }

    #[test]
    fn an_empty_bundle_id_is_stored_under_the_empty_key() {
        let bytes = b"\x01\x00\x00\x07";
        let table = SmartSwitchKey::from_bytes(bytes).expect("well-formed");
        assert_eq!(table.stored(""), Some(7));
        assert_eq!(table.to_bytes(), bytes);
    }

    #[test]
    fn a_non_ascii_bundle_id_is_saved_with_its_byte_length() {
        let bundle = "ứng.dụng";
        let mut table = SmartSwitchKey::new();
        table.set(bundle, 3).unwrap();
        let bytes = table.to_bytes();
        assert_eq!(bytes[0..2], [1, 0]);
        assert_eq!(usize::from(bytes[2]), bundle.len());
        assert!(usize::from(bytes[2]) > bundle.chars().count());
        let reloaded = SmartSwitchKey::from_bytes(&bytes).expect("well-formed");
        assert_eq!(reloaded.stored(bundle), Some(3));
        assert_eq!(reloaded.to_bytes(), bytes);
    }

    #[test]
    fn a_loaded_key_that_is_not_utf8_is_kept_but_cannot_be_looked_up() {
        // Upstream copies the preference blob into its `std::string` map without
        // validating it, and only ever looks a key up from an `NSString`. The
        // port reproduces that asymmetry instead of adding a fifth error.
        let bytes = b"\x01\x00\x02\xff\xfe\x02";
        let table = SmartSwitchKey::from_bytes(bytes).expect("well-formed");
        assert_eq!(table.len(), 1);
        assert_eq!(table.to_bytes(), bytes);
        assert_eq!(table.stored(""), None);
    }

    #[test]
    fn a_duplicate_bundle_id_keeps_the_last_value() {
        let bytes = b"\x02\x00\x03app\x01\x03app\x04";
        let table = SmartSwitchKey::from_bytes(bytes).expect("well-formed");
        assert_eq!(table.stored("app"), Some(4));
        assert_eq!(table.len(), 1);
    }

    // --- malformed input ----------------------------------------------------

    #[test]
    fn a_missing_bundle_length_is_an_error() {
        let bytes = b"\x01\x00";
        assert_eq!(
            SmartSwitchKey::from_bytes(bytes),
            Err(SmartSwitchError::TruncatedBundleLength { offset: 2 })
        );
    }

    #[test]
    fn a_bundle_length_past_the_end_is_an_error() {
        let bytes = b"\x01\x00\x05ab";
        assert_eq!(
            SmartSwitchKey::from_bytes(bytes),
            Err(SmartSwitchError::TruncatedBundle {
                offset: 3,
                declared: 5,
                available: 2
            })
        );
    }

    #[test]
    fn a_missing_value_is_an_error() {
        let bytes = b"\x01\x00\x03app";
        assert_eq!(
            SmartSwitchKey::from_bytes(bytes),
            Err(SmartSwitchError::TruncatedValue { offset: 6 })
        );
    }

    #[test]
    fn an_out_of_range_value_is_an_error() {
        assert_eq!(
            SmartSwitchKey::from_bytes(b"\x01\x00\x03app\x0a"),
            Err(SmartSwitchError::ValueOutOfRange(10))
        );
    }

    #[test]
    fn a_refused_store_leaves_the_table_unchanged() {
        let mut table = SmartSwitchKey::new();
        table.set("app", 3).unwrap();
        assert_eq!(
            table.set("other", 10),
            Err(SmartSwitchError::ValueOutOfRange(10))
        );
        assert_eq!(table.len(), 1);
        assert_eq!(table.stored("other"), None);
    }

    // --- lookup cache -------------------------------------------------------

    #[test]
    fn the_cache_starts_at_the_empty_key_with_zero() {
        let mut table = SmartSwitchKey::new();
        assert_eq!(table.get("", pack(true, Charset::Unicode)), Some(0));
        assert!(table.is_empty(), "the cache hit must not seed the table");
    }

    #[test]
    fn a_first_sight_seeds_the_table_and_reports_unknown() {
        let mut table = SmartSwitchKey::new();
        let current = pack(true, Charset::Tcvn3);
        assert_eq!(table.get("app", current), None);
        assert_eq!(table.stored("app"), Some(current));
    }

    #[test]
    fn a_seeded_application_is_known_on_the_second_read() {
        let mut table = SmartSwitchKey::new();
        let current = pack(true, Charset::Tcvn3);
        assert_eq!(table.get("app", current), None);
        assert_eq!(
            table.get("app", pack(false, Charset::Unicode)),
            Some(current)
        );
    }

    #[test]
    fn a_store_updates_the_cache_and_the_next_read() {
        let mut table = SmartSwitchKey::new();
        table.set("app", pack(false, Charset::VniWindows)).unwrap();
        assert_eq!(
            table.get("app", pack(true, Charset::Unicode)),
            Some(pack(false, Charset::VniWindows))
        );
    }

    #[test]
    fn the_cache_is_per_table() {
        let mut first = SmartSwitchKey::new();
        let mut second = SmartSwitchKey::new();
        first.set("app", 4).unwrap();
        assert_eq!(second.get("app", 1), None);
        assert_eq!(first.get("app", 1), Some(4));
    }

    // --- packed value -------------------------------------------------------

    #[test]
    fn the_packed_value_round_trips_over_its_whole_domain() {
        let charsets = [
            Charset::Unicode,
            Charset::Tcvn3,
            Charset::VniWindows,
            Charset::UnicodeCompound,
            Charset::Cp1258,
        ];
        for language in [false, true] {
            for charset in charsets {
                let packed = pack(language, charset);
                assert!(packed <= MAX_VALUE);
                assert_eq!(unpack(packed), Ok((language, charset)));
            }
        }
        assert_eq!(MAX_VALUE, 9);
    }

    #[test]
    fn unpack_rejects_a_value_above_the_domain() {
        assert_eq!(unpack(10), Err(SmartSwitchError::ValueOutOfRange(10)));
        assert_eq!(unpack(255), Err(SmartSwitchError::ValueOutOfRange(255)));
    }

    // --- policy -------------------------------------------------------------

    #[test]
    fn a_known_application_switches_language_and_starts_a_new_session() {
        let mut table = SmartSwitchKey::new();
        table.set("app", pack(false, Charset::Unicode)).unwrap();
        let switch = on_active_app_changed(&mut table, &options(), "app");
        assert_eq!(switch.language, Some(false));
        assert_eq!(switch.charset, None);
        assert!(switch.new_session);
        assert!(!switch.save);
    }

    #[test]
    fn a_language_and_charset_change_are_one_decision() {
        let mut table = SmartSwitchKey::new();
        table.set("app", pack(false, Charset::Cp1258)).unwrap();
        let switch = on_active_app_changed(&mut table, &options(), "app");
        assert_eq!(switch.language, Some(false));
        assert_eq!(switch.charset, Some(Charset::Cp1258));
        assert!(switch.new_session);
        assert!(!switch.save);
    }

    #[test]
    fn an_unknown_application_asks_for_a_save() {
        let mut table = SmartSwitchKey::new();
        let switch = on_active_app_changed(&mut table, &options(), "app");
        assert_eq!(switch.language, None);
        assert_eq!(switch.charset, None);
        assert!(!switch.new_session);
        assert!(switch.save);
        assert_eq!(
            table.stored("app"),
            Some(pack(true, Charset::Unicode)),
            "the lookup seeds the table with the current value"
        );
    }

    #[test]
    fn a_known_application_with_nothing_to_switch_saves_nothing() {
        let mut table = SmartSwitchKey::new();
        table.set("app", pack(true, Charset::Unicode)).unwrap();
        let switch = on_active_app_changed(&mut table, &options(), "app");
        assert!(switch.is_noop());
    }

    #[test]
    fn remembering_the_code_table_off_keeps_the_charset() {
        let mut table = SmartSwitchKey::new();
        table.set("app", pack(true, Charset::Cp1258)).unwrap();
        let mut opts = options();
        opts.remember_code_per_app = false;
        let switch = on_active_app_changed(&mut table, &opts, "app");
        assert!(switch.is_noop());
    }

    #[test]
    fn smart_switch_off_does_not_record_the_input_method() {
        let mut table = SmartSwitchKey::new();
        let mut opts = english(options());
        opts.smart_switch_key = false;
        assert!(!on_input_method_changed(&mut table, &opts, "app"));
        assert!(table.is_empty());
    }

    #[test]
    fn choosing_a_language_records_it_when_the_option_is_on() {
        let mut table = SmartSwitchKey::new();
        let opts = english(options());
        assert!(on_input_method_changed(&mut table, &opts, "app"));
        assert_eq!(table.stored("app"), Some(pack(false, Charset::Unicode)));
    }

    #[test]
    fn a_table_code_change_is_recorded_only_when_remembering() {
        let mut table = SmartSwitchKey::new();
        let mut opts = vni_windows(options());
        opts.remember_code_per_app = false;
        assert!(!on_table_code_change(&mut table, &opts, "app"));
        assert!(table.is_empty());

        opts.remember_code_per_app = true;
        assert!(on_table_code_change(&mut table, &opts, "app"));
        assert_eq!(table.stored("app"), Some(pack(true, Charset::VniWindows)));
    }

    #[test]
    fn sampling_a_known_application_does_not_disturb_another_cache_entry() {
        let mut table = SmartSwitchKey::new();
        table.set("one", pack(false, Charset::Tcvn3)).unwrap();
        table.set("two", pack(true, Charset::Cp1258)).unwrap();
        assert_eq!(table.get("two", 0), Some(pack(true, Charset::Cp1258)));
        assert_eq!(table.get("one", 0), Some(pack(false, Charset::Tcvn3)));
        assert_eq!(table.len(), 2);
    }

    #[test]
    fn a_vietnamese_first_sight_does_not_report_a_language_switch() {
        // `(-1) & 1 == 1`, so upstream's sentinel compares equal to Vietnamese
        // and only the charset branch fires — which is why the save is still
        // requested.
        let mut table = SmartSwitchKey::new();
        let mut opts = options();
        opts.input_method = InputMethod::Telex;
        let switch = on_active_app_changed(&mut table, &opts, "app");
        assert_eq!(switch.language, None);
        assert!(switch.save);
    }
}
