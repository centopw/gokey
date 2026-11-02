//! C-015: property and no-panic coverage around [`Engine::apply`].
//!
//! The architecture promises that arbitrary key input never panics and that the
//! word buffer stays bounded at 32 letters (`docs/ARCHITECTURE.md` §7). C-002…
//! C-012 pinned concrete behaviours with example tests; this file states the
//! negative property over the same public surface.
//!
//! Everything here uses only `openkey-core`'s public API, because this file is
//! an external test crate. It adds no production line and no dependency: the
//! generator is a std-only `SplitMix64`, seeded from a fixed list plus the 2000
//! parity sequences (`tests/corpus/*.tsv` = 1989, `scripts/parity/smoke.tsv` =
//! 11). A failing case re-reports its seed, profile, step index and complete
//! event sequence so it can be replayed without a shrinker.
//!
//! Corpus inputs are keystrokes, not precomposed Vietnamese letters: all 2000
//! input cells are ASCII. The typed-character mapping mirrors the CLI's
//! `replay::key_event` table and is pinned by exact set here (the CLI crate has
//! no `lib.rs`, so the table cannot be imported); anything unmapped is an
//! error, never a guessed key.
//!
//! The corpus-derived cases are sequence *material* for this property, not a
//! second parity judge: expected output stays `scripts/parity.sh`'s job, and
//! the corpus files are read, never written.

use std::cell::Cell;
use std::collections::BTreeSet;
use std::fs;
use std::panic::{self, AssertUnwindSafe};
use std::path::PathBuf;

use openkey_core::{
    Action, Charset, Engine, Ext, InputMethod, Key, KeyEvent, Macro, Mods, Options, Phase,
};

/// Fixed seeds. Eight gives every profile a distinct stream without making the
/// suite slow; the generator is a pure function of `(seed, profile)`.
const SEEDS: [u64; 8] = [
    0x0000_0000_0000_0001,
    0x0123_4567_89AB_CDEF,
    0x0C15_0C15_0C15_0C15,
    0x5EED_5EED_5EED_5EED,
    0xA11C_E5EE_D123_4567,
    0xBADC_0FFE_E0DD_F00D,
    0xC015_C015_C015_C015,
    0xF00D_FACE_CAFE_BEEF,
];

/// The seeded generator. `splitmix64` is three multiply/xor steps, so the
/// suite stays dependency-free and deterministic.
struct SplitMix64(u64);

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `0..bound`; `bound` must be non-zero.
    fn below(&mut self, bound: usize) -> usize {
        (self.next_u64() % bound as u64) as usize
    }

    fn chance(&mut self, numerator: u64, denominator: u64) -> bool {
        self.next_u64() % denominator < numerator
    }
}

/// One step of a generated or corpus-derived case.
#[derive(Clone, Debug)]
enum Step {
    Key(KeyEvent),
    Break,
    Reconfigure(Options),
}

// ---------------------------------------------------------------------------
// Pools: every shape the public enums can express.
// ---------------------------------------------------------------------------

/// All 26 letters, all ten digits, two out-of-range digits, the six named
/// control keys, four arrows, eleven punctuation keys and three `Other` codes.
///
/// The count is asserted in `pools_cover_the_public_event_and_option_surface`:
/// adding a `Key` variant must extend this pool and that number, so a new
/// variant cannot silently escape the sweep.
fn key_pool() -> Vec<Key> {
    let mut keys = vec![
        Key::A,
        Key::B,
        Key::C,
        Key::D,
        Key::E,
        Key::F,
        Key::G,
        Key::H,
        Key::I,
        Key::J,
        Key::K,
        Key::L,
        Key::M,
        Key::N,
        Key::O,
        Key::P,
        Key::Q,
        Key::R,
        Key::S,
        Key::T,
        Key::U,
        Key::V,
        Key::W,
        Key::X,
        Key::Y,
        Key::Z,
    ];
    keys.extend((0..=9).map(Key::Digit));
    keys.push(Key::Digit(10));
    keys.push(Key::Digit(255));
    keys.extend([
        Key::Space,
        Key::Enter,
        Key::Tab,
        Key::Escape,
        Key::Backspace,
        Key::Delete,
    ]);
    keys.extend([
        Key::ArrowLeft,
        Key::ArrowRight,
        Key::ArrowUp,
        Key::ArrowDown,
    ]);
    keys.extend([
        Key::LeftBracket,
        Key::RightBracket,
        Key::Semicolon,
        Key::Quote,
        Key::Comma,
        Key::Dot,
        Key::Slash,
        Key::BackSlash,
        Key::Minus,
        Key::Equals,
        Key::Backquote,
    ]);
    keys.extend([Key::Other(0), Key::Other(0xFFFF), Key::Other(1234)]);
    keys
}

/// The full cross product of the five modifier booleans.
fn mods_pool() -> Vec<Mods> {
    (0u8..32)
        .map(|bits| Mods {
            shift: bits & 1 != 0,
            ctrl: bits & 2 != 0,
            alt: bits & 4 != 0,
            cmd: bits & 8 != 0,
            caps_lock: bits & 16 != 0,
        })
        .collect()
}

/// `text` content an adapter might attach: absent, ASCII, Vietnamese letters,
/// a CJK character, an astral scalar, NUL and DEL.
fn text_pool() -> Vec<Option<char>> {
    vec![
        None,
        Some('a'),
        Some('A'),
        Some('đ'),
        Some('ệ'),
        Some('中'),
        Some('😀'),
        Some('\0'),
        Some('\u{7F}'),
    ]
}

/// Set every boolean field explicitly. Keeping this list in one place makes
/// "all-true" and "all-false" profiles actually exhaustive: a new field has to
/// be added here, and `cargo` will show the omission at the first profile.
fn set_all_booleans(options: &mut Options, value: bool) {
    options.enabled = value;
    options.modern_orthography = value;
    options.quick_telex = value;
    options.check_spelling = value;
    options.free_mark = value;
    options.restore_if_invalid = value;
    options.fix_browser_recommend = value;
    options.use_macro = value;
    options.use_macro_in_english = value;
    options.auto_caps_macro = value;
    options.smart_switch_key = value;
    options.upper_case_first_char = value;
    options.temp_off_spelling = value;
    options.allow_consonant_zfwj = value;
    options.quick_start_consonant = value;
    options.quick_end_consonant = value;
    options.remember_code_per_app = value;
    options.other_language = value;
    options.temp_off_hotkey = value;
}

/// 20 method×charset pairs, all-true, the English-branch profile, and all-false.
fn option_profiles() -> Vec<Options> {
    let mut profiles = Vec::new();
    for input_method in [
        InputMethod::Telex,
        InputMethod::Vni,
        InputMethod::SimpleTelex1,
        InputMethod::SimpleTelex2,
    ] {
        for charset in [
            Charset::Unicode,
            Charset::Tcvn3,
            Charset::VniWindows,
            Charset::UnicodeCompound,
            Charset::Cp1258,
        ] {
            profiles.push(Options {
                input_method,
                charset,
                ..Options::default()
            });
        }
    }
    let mut all_true = Options::default();
    set_all_booleans(&mut all_true, true);
    profiles.push(all_true);
    // The English-branch profile: `enabled = false` is the only way into
    // `engine.rs:317-322`, and that branch needs both macro flags. It sits at
    // an odd index so `case_macros` supplies a table; `use_macro` stays at the
    // default `true` (`options.rs:98`).
    profiles.push(Options {
        enabled: false,
        use_macro_in_english: true,
        ..Options::default()
    });
    let mut all_false = Options::default();
    set_all_booleans(&mut all_false, false);
    profiles.push(all_false);
    profiles
}

// ---------------------------------------------------------------------------
// Generation and execution.
// ---------------------------------------------------------------------------

fn letter_event(key: Key, shift: bool) -> KeyEvent {
    KeyEvent {
        key,
        mods: Mods {
            shift,
            ..Mods::default()
        },
        phase: Phase::Down,
        text: None,
    }
}

/// A state prefix that fills the word with a real Vietnamese sequence before
/// the event under test is applied.
fn prefix_events() -> Vec<KeyEvent> {
    [
        Key::V,
        Key::I,
        Key::E,
        Key::E,
        Key::J,
        Key::T,
        Key::Space,
        Key::D,
        Key::D,
    ]
    .into_iter()
    .map(|key| letter_event(key, false))
    .collect()
}

fn case_macros(profile: usize) -> Vec<Macro> {
    if profile % 2 == 0 {
        Vec::new()
    } else {
        vec![
            Macro::new("ko", "không"),
            Macro::new("vn", "Việt Nam"),
            Macro::new("btw", "by the way"),
        ]
    }
}

/// A case is a pure function of `(seed, profile)`: the profile chooses the
/// initial options, the seed chooses every step.
fn generate_case(seed: u64, profile: usize) -> (Options, Vec<Step>) {
    let profiles = option_profiles();
    let initial = profiles[profile].clone();
    let mut rng = SplitMix64::new(seed ^ (profile as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
    let len = 8 + rng.below(89);
    let keys = key_pool();
    let mods = mods_pool();
    let texts = text_pool();
    let breaks: BTreeSet<usize> = [len / 4, len / 2, len * 3 / 4].into_iter().collect();
    let switches: BTreeSet<usize> = [len / 3, len * 2 / 3].into_iter().collect();
    let mut steps = Vec::with_capacity(len);
    for index in 0..len {
        if breaks.contains(&index) {
            steps.push(Step::Break);
        } else if switches.contains(&index) {
            let pick = (profile + index + (seed as usize & 0xFF)) % profiles.len();
            steps.push(Step::Reconfigure(profiles[pick].clone()));
        } else if rng.chance(1, 12) {
            steps.push(Step::Break);
        } else {
            let key = keys[rng.below(keys.len())];
            let mods = mods[rng.below(mods.len())];
            let text = texts[rng.below(texts.len())];
            let phase = if rng.chance(1, 5) {
                Phase::Up
            } else {
                Phase::Down
            };
            steps.push(Step::Key(KeyEvent {
                key,
                mods,
                phase,
                text,
            }));
        }
    }
    (initial, steps)
}

/// Run one case and return a transcript of `Outcome` + `Word` per step.
///
/// The transcript exists so a second run can be compared byte for byte; it also
/// proves that `Phase::Up` short-circuits without touching the word.
fn run_steps(
    initial: Options,
    macros: &[Macro],
    steps: &[Step],
    last_step: &Cell<usize>,
) -> Vec<String> {
    let mut engine = if macros.is_empty() {
        Engine::new(initial)
    } else {
        Engine::with_macros(initial, macros)
    };
    let mut transcript = Vec::with_capacity(steps.len());
    for (index, step) in steps.iter().enumerate() {
        last_step.set(index);
        match step {
            Step::Key(event) => {
                let before = engine.word().clone();
                let outcome = engine.apply(event);
                assert!(
                    engine.word().len() <= Engine::MAX_BUFFER,
                    "buffer exceeded at step {index} after {event:?}"
                );
                if event.phase == Phase::Up {
                    assert_eq!(
                        outcome.action,
                        Action::Pass,
                        "Up must pass through: {event:?}"
                    );
                    assert!(outcome.edit.is_none(), "Up must not edit: {event:?}");
                    assert!(
                        outcome.passthrough.is_none(),
                        "Up must not re-inject: {event:?}"
                    );
                    assert_eq!(outcome.ext, Ext::Normal, "Up must report Normal: {event:?}");
                    assert_eq!(&before, engine.word(), "Up changed the word: {event:?}");
                }
                transcript.push(format!(
                    "{index}:{event:?}->{outcome:?}/{:?}",
                    engine.word()
                ));
            }
            Step::Break => {
                engine.session_break();
                assert!(
                    engine.word().len() <= Engine::MAX_BUFFER,
                    "buffer exceeded at break step {index}"
                );
                transcript.push(format!("{index}:break->{:?}", engine.word()));
            }
            Step::Reconfigure(options) => {
                engine.set_options(options.clone());
                assert!(
                    engine.word().len() <= Engine::MAX_BUFFER,
                    "buffer exceeded at option step {index}"
                );
                transcript.push(format!("{index}:options->{:?}", engine.word()));
            }
        }
    }
    transcript
}

/// The replay report required by D1/D3: label, seed, profile, step, sequence.
fn failure_report(label: &str, seed: u64, profile: usize, step: usize, steps: &[Step]) -> String {
    let sequence: Vec<String> = steps.iter().map(|step| format!("{step:?}")).collect();
    format!(
        "C-015 case {label} seed=0x{seed:016X} profile={profile} step={step} sequence=[{}]",
        sequence.join("; ")
    )
}

/// Run a case under `catch_unwind`; a panic is re-raised with the replay data
/// instead of disappearing into a bare unwind.
fn guarded<F>(label: &str, seed: u64, profile: usize, steps: &[Step], run: F) -> Vec<String>
where
    F: FnOnce(&Cell<usize>) -> Vec<String>,
{
    let last_step = Cell::new(0usize);
    match panic::catch_unwind(AssertUnwindSafe(|| run(&last_step))) {
        Ok(transcript) => transcript,
        Err(_) => panic!(
            "{}",
            failure_report(label, seed, profile, last_step.get(), steps)
        ),
    }
}

// ---------------------------------------------------------------------------
// Corpus-derived sequences (run-only; never judged for expected output).
// ---------------------------------------------------------------------------

struct CorpusCase {
    id: String,
    method: String,
    options: String,
    input: String,
    macros: Vec<Macro>,
    source: String,
}

fn corpus_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn corpus_files() -> Vec<PathBuf> {
    let root = corpus_root();
    let mut files: Vec<PathBuf> = fs::read_dir(root.join("tests/corpus"))
        .expect("tests/corpus must exist")
        .map(|entry| entry.expect("readable corpus entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "tsv"))
        .collect();
    files.sort();
    files.push(root.join("scripts/parity/smoke.tsv"));
    files
}

fn decode(raw: &str, where_: &str) -> String {
    if raw == "-" {
        return String::new();
    }
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('b') => out.push('\u{8}'),
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('s') => out.push(' '),
            Some('\\') => out.push('\\'),
            other => panic!("{where_}: bad escape {other:?}"),
        }
    }
    out
}

fn parse_macros(raw: &str) -> Vec<Macro> {
    if raw == "-" {
        return Vec::new();
    }
    raw.split(';')
        .map(|pair| {
            let (name, content) = pair
                .split_once('=')
                .unwrap_or_else(|| panic!("macro pair {pair:?} has no `=`"));
            assert!(!name.is_empty(), "macro pair {pair:?} has an empty name");
            Macro::new(name, content)
        })
        .collect()
}

fn parse_corpus() -> Vec<CorpusCase> {
    let mut cases = Vec::new();
    for path in corpus_files() {
        // Ids are unique per script file: `scripts/parity.sh` runs each file as
        // its own replay, so `smoke.tsv` may reuse a corpus id.
        let mut seen = BTreeSet::new();
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
        for (line_no, raw) in text.lines().enumerate() {
            let line = raw.strip_suffix('\r').unwrap_or(raw);
            if line.trim_start().is_empty() || line.trim_start().starts_with('#') {
                continue;
            }
            let where_ = format!("{}:{}", path.display(), line_no + 1);
            let fields: Vec<&str> = line.split('\t').collect();
            assert!(
                (4..=6).contains(&fields.len()),
                "{where_}: expected 4-6 fields, got {}",
                fields.len()
            );
            let id = fields[0].trim();
            assert!(!id.is_empty(), "{where_}: empty case id");
            assert!(seen.insert(id.to_string()), "{where_}: duplicate id {id:?}");
            let macros = fields
                .get(5)
                .map_or_else(Vec::new, |raw| parse_macros(raw.trim()));
            cases.push(CorpusCase {
                id: id.to_string(),
                method: fields[1].trim().to_string(),
                options: fields[2].trim().to_string(),
                input: decode(fields[3], &where_),
                macros,
                source: where_,
            });
        }
    }
    cases
}

/// Mirror `crates/openkey-cli/src/flags.rs`; an unknown flag is an error, not a
/// default-configuration case.
fn corpus_options(case: &CorpusCase) -> Options {
    let input_method = match case.method.as_str() {
        "telex" => InputMethod::Telex,
        "vni" => InputMethod::Vni,
        "simple_telex_1" => InputMethod::SimpleTelex1,
        "simple_telex_2" => InputMethod::SimpleTelex2,
        other => panic!("{}: unknown method {other:?}", case.source),
    };
    let mut options = Options {
        input_method,
        ..Options::default()
    };
    for flag in case
        .options
        .split(',')
        .map(str::trim)
        .filter(|flag| !flag.is_empty() && *flag != "-")
    {
        match flag {
            "english" => options.enabled = false,
            "vietnamese" => options.enabled = true,
            "spelling" => options.check_spelling = true,
            "no-spelling" => options.check_spelling = false,
            "modern-orthography" => options.modern_orthography = true,
            "quick-telex" => options.quick_telex = true,
            "restore-if-wrong-spelling" => options.restore_if_invalid = true,
            "allow-zfwj" => options.allow_consonant_zfwj = true,
            "quick-start-consonant" => options.quick_start_consonant = true,
            "quick-end-consonant" => options.quick_end_consonant = true,
            "smart-switch-key" => options.smart_switch_key = true,
            "no-smart-switch-key" => options.smart_switch_key = false,
            "macro" => options.use_macro = true,
            "no-macro" => options.use_macro = false,
            "macro-in-english" => options.use_macro_in_english = true,
            "auto-caps-macro" => options.auto_caps_macro = true,
            "upper-case-first" => options.upper_case_first_char = true,
            "temp-off-spelling" => options.temp_off_spelling = true,
            "free-mark" => options.free_mark = true,
            other => panic!("{}: unknown option flag {other:?}", case.source),
        }
    }
    options
}

/// The letter key an ASCII letter lands on, looked up by its lowercase form.
fn letter_key(base: char) -> Key {
    match base {
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
        other => unreachable!("not an ASCII letter: {other:?}"),
    }
}

/// Mirror `crates/openkey-cli/src/replay.rs:138-181` (`key_event`) exactly.
///
/// `crates/openkey-cli` has no `lib.rs`, so the table cannot be imported; the
/// pools test pins this copy by exact set instead. The CLI refuses `\r` and
/// every character outside its table, so this returns `None` for them and
/// `corpus_steps` fails rather than guessing a key.
fn cli_key_event(typed: char) -> Option<KeyEvent> {
    let (key, shift) = match typed {
        '\u{8}' => (Key::Backspace, false),
        '\n' => (Key::Enter, false),
        '\t' => (Key::Tab, false),
        ' ' => (Key::Space, false),
        'a'..='z' | 'A'..='Z' => (
            letter_key(typed.to_ascii_lowercase()),
            typed.is_ascii_uppercase(),
        ),
        '0'..='9' => (Key::Digit(typed as u8 - b'0'), false),
        // `_characterMap` maps shifted digits to the same keycodes with
        // `CAPS_MASK` (`Vietnamese.cpp:535-545`); the shift flag reproduces
        // that mask through `capsStatus`.
        '!' => (Key::Digit(1), true),
        '@' => (Key::Digit(2), true),
        '#' => (Key::Digit(3), true),
        '$' => (Key::Digit(4), true),
        '%' => (Key::Digit(5), true),
        '^' => (Key::Digit(6), true),
        '&' => (Key::Digit(7), true),
        '*' => (Key::Digit(8), true),
        '(' => (Key::Digit(9), true),
        ')' => (Key::Digit(0), true),
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

/// Record one pinned entry of the CLI table.
fn assert_one_mapping(mapped: &mut BTreeSet<char>, typed: char, key: Key, shift: bool) {
    let event = cli_key_event(typed).unwrap_or_else(|| panic!("{typed:?} must map"));
    assert_eq!(event.key, key, "{typed:?}");
    assert_eq!(
        event.mods,
        Mods {
            shift,
            ..Mods::default()
        },
        "{typed:?}"
    );
    assert_eq!(event.phase, Phase::Down, "{typed:?}");
    assert_eq!(event.text, Some(typed), "{typed:?}");
    assert!(mapped.insert(typed), "{typed:?} listed twice");
}

/// The drift alarm for the copied CLI table: 87 characters with their exact
/// key and shift, `text` always present, and the refusal set pinned.
///
/// Written from `replay.rs:139-171` independently of `cli_key_event`, so a
/// change to either copy fails here instead of silently changing what a corpus
/// row means.
fn assert_cli_mapping_is_pinned() {
    let mut mapped = BTreeSet::new();
    assert_one_mapping(&mut mapped, '\u{8}', Key::Backspace, false);
    assert_one_mapping(&mut mapped, '\n', Key::Enter, false);
    assert_one_mapping(&mut mapped, '\t', Key::Tab, false);
    assert_one_mapping(&mut mapped, ' ', Key::Space, false);
    for (typed, key) in [
        ('a', Key::A),
        ('b', Key::B),
        ('c', Key::C),
        ('d', Key::D),
        ('e', Key::E),
        ('f', Key::F),
        ('g', Key::G),
        ('h', Key::H),
        ('i', Key::I),
        ('j', Key::J),
        ('k', Key::K),
        ('l', Key::L),
        ('m', Key::M),
        ('n', Key::N),
        ('o', Key::O),
        ('p', Key::P),
        ('q', Key::Q),
        ('r', Key::R),
        ('s', Key::S),
        ('t', Key::T),
        ('u', Key::U),
        ('v', Key::V),
        ('w', Key::W),
        ('x', Key::X),
        ('y', Key::Y),
        ('z', Key::Z),
    ] {
        assert_one_mapping(&mut mapped, typed, key, false);
        assert_one_mapping(&mut mapped, typed.to_ascii_uppercase(), key, true);
    }
    for digit in 0..=9u8 {
        assert_one_mapping(
            &mut mapped,
            char::from(b'0' + digit),
            Key::Digit(digit),
            false,
        );
    }
    for (typed, digit) in [
        ('!', 1u8),
        ('@', 2),
        ('#', 3),
        ('$', 4),
        ('%', 5),
        ('^', 6),
        ('&', 7),
        ('*', 8),
        ('(', 9),
        (')', 0),
    ] {
        assert_one_mapping(&mut mapped, typed, Key::Digit(digit), true);
    }
    for (typed, key) in [
        ('\'', Key::Quote),
        ('[', Key::LeftBracket),
        (']', Key::RightBracket),
        (';', Key::Semicolon),
        (',', Key::Comma),
        ('.', Key::Dot),
        ('/', Key::Slash),
        ('\\', Key::BackSlash),
        ('-', Key::Minus),
        ('=', Key::Equals),
        ('`', Key::Backquote),
    ] {
        assert_one_mapping(&mut mapped, typed, key, false);
    }
    assert_eq!(mapped.len(), 87, "the CLI table maps exactly 87 characters");

    // Domain closure: every printable ASCII character maps exactly when the
    // table lists it, and nothing else maps.
    for code in 0x20u32..=0x7E {
        let typed = char::from_u32(code).expect("printable ASCII");
        assert_eq!(
            cli_key_event(typed).is_some(),
            mapped.contains(&typed),
            "domain drift at {typed:?}"
        );
    }
    for code in 0u32..0x20 {
        if matches!(code, 8..=10) {
            continue;
        }
        let typed = char::from_u32(code).expect("control");
        assert!(
            cli_key_event(typed).is_none(),
            "control {code:#04X} must be refused"
        );
    }
    for typed in ['\r', '~', '\u{7F}', 'ệ', 'Đ', '中', '😀'] {
        assert!(cli_key_event(typed).is_none(), "{typed:?} must be refused");
    }
}

fn corpus_steps(case: &CorpusCase) -> Vec<Step> {
    case.input
        .chars()
        .map(|typed| {
            let event = cli_key_event(typed).unwrap_or_else(|| {
                panic!(
                    "{}: unmapped corpus character {typed:?} (U+{:04X}); the CLI refuses it",
                    case.source, typed as u32
                )
            });
            Step::Key(event)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Tests.
// ---------------------------------------------------------------------------

#[test]
fn pools_cover_the_public_event_and_option_surface() {
    let keys = key_pool();
    assert_eq!(keys.len(), 62, "extend the pool and this count together");
    let unique: BTreeSet<String> = keys.iter().map(|key| format!("{key:?}")).collect();
    assert_eq!(unique.len(), 62, "the key pool must not repeat a variant");
    assert_eq!(mods_pool().len(), 32);
    assert_eq!(text_pool().len(), 9);
    let profiles = option_profiles();
    assert_eq!(profiles.len(), 23);
    assert_cli_mapping_is_pinned();
    // Every macro-loading profile must open the gate: `engine.rs` checks
    // `use_macro` at every macro entry point, so a table behind a closed gate
    // would be dead weight. The odd profiles are the ones `case_macros` fills.
    for index in (1..profiles.len()).step_by(2) {
        assert!(
            profiles[index].use_macro,
            "profile {index} loads macros behind a closed gate"
        );
    }
    // The English branch (`engine.rs:317-322`) needs `enabled = false` and both
    // macro flags; profile 21 is odd, so it also carries a table.
    let english = &profiles[21];
    assert!(!english.enabled);
    assert!(english.use_macro && english.use_macro_in_english);
    assert!(!case_macros(21).is_empty());
    // ...and it must expand with that table, not merely run: "ko" + Space is
    // the first entry, and the break key is typed after the expansion.
    let mut english_engine = Engine::with_macros(profiles[21].clone(), &case_macros(21));
    for key in [Key::K, Key::O] {
        english_engine.apply(&letter_event(key, false));
    }
    let outcome = english_engine.apply(&letter_event(Key::Space, false));
    assert_eq!(
        outcome.action,
        Action::Macro,
        "the English branch must expand a macro with both flags on"
    );
    assert!(outcome.edit.is_some());
    assert_eq!(Engine::MAX_BUFFER, 32);
    let methods: BTreeSet<String> = profiles
        .iter()
        .map(|options| format!("{:?}", options.input_method))
        .collect();
    assert_eq!(methods.len(), 4);
    let charsets: BTreeSet<String> = profiles
        .iter()
        .map(|options| format!("{:?}", options.charset))
        .collect();
    assert_eq!(charsets.len(), 5);
    assert_eq!(SEEDS.len() * profiles.len(), 184);
    for seed in SEEDS {
        for profile in 0..profiles.len() {
            let (options, steps) = generate_case(seed, profile);
            let (options_again, steps_again) = generate_case(seed, profile);
            assert_eq!(options, options_again);
            assert_eq!(format!("{steps:?}"), format!("{steps_again:?}"));
            assert!((8..=96).contains(&steps.len()));
        }
    }
}

#[test]
fn every_up_event_passes_through_and_leaves_the_word_untouched() {
    let mut engine = Engine::new(Options::default());
    for event in prefix_events() {
        engine.apply(&event);
    }
    let mut count = 0usize;
    for key in key_pool() {
        for mods in mods_pool() {
            for text in text_pool() {
                let before = engine.word().clone();
                let event = KeyEvent {
                    key,
                    mods,
                    phase: Phase::Up,
                    text,
                };
                let outcome = engine.apply(&event);
                assert_eq!(outcome.action, Action::Pass, "{event:?}");
                assert!(outcome.edit.is_none(), "{event:?}");
                assert!(outcome.passthrough.is_none(), "{event:?}");
                assert_eq!(outcome.ext, Ext::Normal, "{event:?}");
                assert_eq!(&before, engine.word(), "{event:?}");
                count += 1;
            }
        }
    }
    assert_eq!(count, 17_856);
}

#[test]
fn one_event_of_every_shape_never_panics_and_stays_bounded() {
    let profiles = [
        Options::default(),
        Options {
            input_method: InputMethod::Vni,
            modern_orthography: true,
            ..Options::default()
        },
    ];
    let prefix = prefix_events();
    let prefixes: [&[KeyEvent]; 2] = [&[], &prefix];
    let mut count = 0usize;
    for profile in &profiles {
        for prefix in prefixes {
            let mut engine = Engine::new(profile.clone());
            for event in prefix {
                engine.apply(event);
            }
            for key in key_pool() {
                for mods in mods_pool() {
                    for phase in [Phase::Down, Phase::Up] {
                        for text in text_pool() {
                            let before = engine.word().clone();
                            let event = KeyEvent {
                                key,
                                mods,
                                phase,
                                text,
                            };
                            let outcome = engine.apply(&event);
                            assert!(
                                engine.word().len() <= Engine::MAX_BUFFER,
                                "{event:?} left {} letters",
                                engine.word().len()
                            );
                            if phase == Phase::Up {
                                assert_eq!(outcome.action, Action::Pass, "{event:?}");
                                assert!(outcome.edit.is_none(), "{event:?}");
                                assert!(outcome.passthrough.is_none(), "{event:?}");
                                assert_eq!(outcome.ext, Ext::Normal, "{event:?}");
                                assert_eq!(&before, engine.word(), "{event:?}");
                            }
                            count += 1;
                        }
                    }
                }
            }
        }
    }
    assert_eq!(count, 142_848);
}

#[test]
fn seeded_sequences_never_panic_and_stay_bounded() {
    let profiles = option_profiles();
    for seed in SEEDS {
        for profile in 0..profiles.len() {
            let (initial, steps) = generate_case(seed, profile);
            let macros = case_macros(profile);
            let label = format!("seed-{seed:016X}-profile-{profile}");
            let first = guarded(&label, seed, profile, &steps, |last| {
                run_steps(initial.clone(), &macros, &steps, last)
            });
            let second = guarded(&label, seed, profile, &steps, |last| {
                run_steps(initial.clone(), &macros, &steps, last)
            });
            assert_eq!(
                first, second,
                "replay diverged for seed=0x{seed:016X} profile={profile}"
            );
            assert_eq!(first.len(), steps.len());
        }
    }
}

#[test]
fn every_parity_sequence_never_panics_and_stays_bounded() {
    let cases = parse_corpus();
    assert_eq!(
        cases.len(),
        2000,
        "1989 corpus rows + 11 smoke rows; a drift must be investigated, not absorbed"
    );
    for case in &cases {
        let options = corpus_options(case);
        let steps = corpus_steps(case);
        let label = format!("{} ({})", case.id, case.source);
        let first = guarded(&label, 0, 0, &steps, |last| {
            run_steps(options.clone(), &case.macros, &steps, last)
        });
        let second = guarded(&label, 0, 0, &steps, |last| {
            run_steps(options.clone(), &case.macros, &steps, last)
        });
        assert_eq!(first, second, "replay diverged for {}", case.id);
        assert_eq!(first.len(), steps.len(), "{}", case.id);
    }
}

#[test]
fn crossing_the_buffer_capacity_keeps_32_letters_and_a_break_clears_them() {
    assert_eq!(Engine::MAX_BUFFER, 32);
    let mut engine = Engine::new(Options::default());
    for index in 0..100 {
        engine.apply(&letter_event(Key::B, index % 2 == 1));
        assert!(
            engine.word().len() <= Engine::MAX_BUFFER,
            "100 consonants overflowed at step {index}"
        );
        if index >= 31 {
            assert_eq!(
                engine.word().len(),
                32,
                "capacity must be reached at step {index}"
            );
        }
    }

    // Overflow restore (`Word::pop`, `word.rs:370-383`): 40 pushes fill 32 and
    // park 8 in `overflow`. Deletes 0-7 pop the overflow back into the word and
    // leave `len` at 32; from index 8 on each delete removes one letter, so the
    // word is empty at index 39.
    let mut engine = Engine::new(Options::default());
    for _ in 0..40 {
        engine.apply(&letter_event(Key::C, false));
    }
    assert_eq!(engine.word().len(), 32);
    for index in 0..40 {
        engine.apply(&letter_event(Key::Backspace, false));
        let len = engine.word().len();
        let expected = if index < 8 { 32 } else { 39 - index };
        assert_eq!(len, expected, "delete {index} left {len} letters");
        assert!(len <= Engine::MAX_BUFFER);
    }
    assert_eq!(engine.word().len(), 0, "40 deletes must empty 40 pushes");

    // A session break clears the word; the next run fills it again exactly once.
    let mut engine = Engine::new(Options::default());
    for _ in 0..40 {
        engine.apply(&letter_event(Key::C, false));
    }
    engine.session_break();
    assert_eq!(engine.word().len(), 0);
    for _ in 0..33 {
        engine.apply(&letter_event(Key::C, false));
    }
    assert_eq!(engine.word().len(), 32);
}

#[test]
fn the_failure_report_names_the_seed_step_and_full_sequence() {
    let (_, steps) = generate_case(SEEDS[0], 0);
    let report = failure_report("synthetic", 0xC015_C015_C015_C015, 3, 7, &steps);
    assert!(report.contains("seed=0xC015C015C015C015"), "{report}");
    assert!(report.contains("profile=3"), "{report}");
    assert!(report.contains("step=7"), "{report}");
    assert!(report.contains("sequence=["), "{report}");
    for step in &steps {
        assert!(
            report.contains(&format!("{step:?}")),
            "the whole sequence must replay: {report}"
        );
    }
}
