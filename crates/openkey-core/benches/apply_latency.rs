//! Per-keystroke latency harness (C-016, V-PERF).
//!
//! Measures [`Engine::apply`] over a deterministic 100,000-keystroke stream
//! derived from the parity corpus and prints one JSON object
//! (`schema: openkey-bench/1`) with p50/p99/p999, throughput, the commit and
//! the runner. `scripts/ci/bench-report.py` holds the gate
//! (p99 <= 0.500 ms) and the 1.10x sentinel; this binary only measures.
//!
//! Why a custom harness instead of criterion: criterion's stock output carries
//! no quantiles (`estimates.json` has mean/median/slope/std-dev/MAD/R2 and the
//! bencher format prints ns/iter), and V-PERF's gate is a p99. The sampler is
//! `std::time::Instant`, which is portable, so the target compiles on
//! ubuntu/macos/windows and `cargo test --all-targets` stays green there.
//!
//! Workload: every row of `tests/corpus/*.tsv` (sorted by name) followed by
//! `scripts/parity/smoke.tsv`, decoded with the documented input escapes
//! (`\b \n \t \s \\`), each character mapped through the CLI's physical-key
//! table (`replay.rs:138-181`), concatenated and cycled to exactly 100,000
//! key-down events. Rows alternate the default Telex profile with VNI + modern
//! orthography; the engine is rebuilt per row, outside the timed region. The
//! stream's FNV-1a-64 is recorded so a corpus change is visible in every
//! report instead of silently moving the baseline.
//!
//! # Exit status
//!
//! `0` measured and emitted, or invoked without a measurement request (cargo
//! test's bench invocation); `2` harness error (unreadable corpus, unmapped
//! character, drifted mapping, zero events, bad argument). The latency gate is
//! **not** in the exit code: `cargo test --workspace --all-targets` runs bench
//! targets too, and a machine-noise exit 1 there would turn three-OS CI red for
//! a number V-PERF measures on one runner. The workflow decides from the JSON.
//!
//! ```text
//! cargo bench -p openkey-core --bench apply_latency -- --out bench.json
//! ```

#![forbid(unsafe_code)]

use std::env;
use std::fs;
use std::hint::black_box;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use openkey_core::{Engine, InputMethod, Key, KeyEvent, Mods, Options, Phase};

const SCHEMA: &str = "openkey-bench/1";
/// V-PERF's pass condition (plan §5.2; G1).
const GATE_MS: f64 = 0.500;
const DEFAULT_EVENTS: usize = 100_000;
const DEFAULT_REPS: usize = 5;
/// Discarded passes before the measured ones; one pass settles caches and
/// branch predictors on a deterministic stream.
const DEFAULT_WARMUP: usize = 1;

/// One corpus row: its decoded typed characters and source location.
struct Row {
    input: String,
    source: String,
}

/// One keystroke plus the row that produced it, so the engine can be rebuilt
/// with that row's profile outside the timed region.
struct Step {
    event: KeyEvent,
    row: usize,
}

struct Stream {
    steps: Vec<Step>,
    cases: usize,
    fnv1a: u64,
}

struct Args {
    events: usize,
    reps: usize,
    warmup: usize,
    out: Option<PathBuf>,
}

struct Report {
    p50_ms: f64,
    p99_ms: f64,
    p999_ms: f64,
    max_ms: f64,
    keystrokes_per_s: f64,
    p99_per_rep: Vec<f64>,
    p99_pass: bool,
}

fn main() {
    // Cargo runs this target from two places. `cargo bench` appends `--bench`;
    // `cargo test --all-targets` runs the same binary as a libtest-style target
    // with **no arguments at all** (measured at rev r1), so a profile-based
    // guard would still measure 100k x 6 in debug inside three CI jobs. The
    // guard keys on the invocation: only `--bench` (cargo) or `--measure`
    // (explicit, by hand) measure; anything else exits 0 before reading the
    // corpus.
    if !measurement_requested() {
        eprintln!("openkey-bench: not a bench invocation (no --bench/--measure); nothing measured");
        return;
    }
    if let Err(error) = run() {
        eprintln!("bench: {error}");
        std::process::exit(2);
    }
}

fn measurement_requested() -> bool {
    env::args()
        .skip(1)
        .any(|arg| matches!(arg.as_str(), "--bench" | "--measure"))
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    assert_mapping_is_pinned()?;
    let corpus = read_corpus()?;
    let stream = build_stream(&corpus, args.events)?;
    let report = measure(&stream, args.warmup, args.reps)?;
    let json = render_json(&stream, &args, &report);
    if let Some(path) = &args.out {
        // A relative `--out` resolves against the process cwd (the package
        // root under cargo); the CI workflow always passes an absolute path.
        fs::write(path, format!("{json}\n"))
            .map_err(|err| format!("write {}: {err}", path.display()))?;
    }
    println!("{json}");
    Ok(())
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        events: DEFAULT_EVENTS,
        reps: DEFAULT_REPS,
        warmup: DEFAULT_WARMUP,
        out: None,
    };
    let mut argv = env::args().skip(1);
    while let Some(arg) = argv.next() {
        match arg.as_str() {
            // `cargo bench` passes `--bench` to the target; it is not ours.
            "--bench" => {}
            // The explicit by-hand switch; `cargo test` never passes it.
            "--measure" => {}
            "--events" => args.events = number(argv.next(), "--events")?,
            "--reps" => args.reps = number(argv.next(), "--reps")?,
            "--warmup" => args.warmup = number(argv.next(), "--warmup")?,
            "--out" => {
                let value = argv.next().ok_or("--out needs a path")?;
                args.out = Some(PathBuf::from(value));
            }
            other => return Err(format!("unknown argument {other:?}")),
        }
    }
    if args.events == 0 {
        return Err("--events must be at least 1".to_string());
    }
    if args.reps == 0 {
        return Err("--reps must be at least 1".to_string());
    }
    Ok(args)
}

fn number(value: Option<String>, flag: &str) -> Result<usize, String> {
    let raw = value.ok_or_else(|| format!("{flag} needs a number"))?;
    raw.parse::<usize>()
        .map_err(|err| format!("{flag} {raw:?}: {err}"))
}

/// Cargo runs a `harness = false` bench with cwd = the **package root**
/// (`crates/openkey-core`), where `tests/corpus` does not exist, so the corpus
/// is resolved from `CARGO_MANIFEST_DIR/../..` and never from the process cwd.
fn corpus_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn corpus_files() -> Result<Vec<PathBuf>, String> {
    let root = corpus_root();
    let dir = root.join("tests/corpus");
    if !dir.is_dir() {
        return Err(format!(
            "corpus directory {} does not exist (resolved from CARGO_MANIFEST_DIR/../..)",
            dir.display()
        ));
    }
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .map_err(|err| format!("read {}: {err}", dir.display()))?
        .map(|entry| entry.expect("readable corpus entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "tsv"))
        .collect();
    files.sort();
    files.push(root.join("scripts/parity/smoke.tsv"));
    Ok(files)
}

/// Decode one `input` cell: `-` is empty, and the only escapes are the four
/// the corpus README documents. Anything else is an error, never a guess.
fn decode(raw: &str, where_: &str) -> Result<String, String> {
    if raw == "-" {
        return Ok(String::new());
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
            other => return Err(format!("{where_}: bad escape {other:?}")),
        }
    }
    Ok(out)
}

fn read_corpus() -> Result<Vec<Row>, String> {
    let mut rows = Vec::new();
    for path in corpus_files()? {
        let text =
            fs::read_to_string(&path).map_err(|err| format!("read {}: {err}", path.display()))?;
        for (line_no, raw) in text.lines().enumerate() {
            let line = raw.strip_suffix('\r').unwrap_or(raw);
            if line.trim_start().is_empty() || line.trim_start().starts_with('#') {
                continue;
            }
            let where_ = format!("{}:{}", path.display(), line_no + 1);
            let fields: Vec<&str> = line.split('\t').collect();
            if !(4..=6).contains(&fields.len()) {
                return Err(format!(
                    "{where_}: expected 4-6 fields, got {}",
                    fields.len()
                ));
            }
            rows.push(Row {
                input: decode(fields[3], &where_)?,
                source: where_,
            });
        }
    }
    if rows.is_empty() {
        return Err("the corpus has no rows".to_string());
    }
    Ok(rows)
}

/// The physical-key mapping, mirroring `crates/openkey-cli/src/replay.rs:138-181`.
///
/// The CLI crate is binary-only (no `lib.rs`), so the table cannot be imported;
/// [`assert_mapping_is_pinned`] fails the run if it stops covering exactly the
/// 87 characters the CLI maps, which is the same drift alarm C-015 pins in a
/// test. A bench target is not run by `cargo test`, so here the alarm lives at
/// startup.
fn cli_key_event(typed: char) -> Option<KeyEvent> {
    let (key, shift) = match typed {
        '\u{8}' => (Key::Backspace, false),
        '\n' => (Key::Enter, false),
        '\t' => (Key::Tab, false),
        ' ' => (Key::Space, false),
        'a'..='z' | 'A'..='Z' => (
            letter(typed.to_ascii_lowercase()),
            typed.is_ascii_uppercase(),
        ),
        '0'..='9' => (Key::Digit(typed as u8 - b'0'), false),
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

fn letter(typed: char) -> Key {
    match typed {
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
        other => unreachable!("letter({other:?}) is only called for a-z/A-Z"),
    }
}

/// The exact mapping the workload relies on: 4 controls + 26 upper + 26 lower
/// letters + 10 digits + 10 shifted digits + 11 punctuation = **87** entries.
///
/// C-015's `cli_key_event` pin and this table are two independent
/// transcriptions of `crates/openkey-cli/src/replay.rs:139-171`; nothing
/// compares them to each other, so a change to the CLI table has to be applied
/// to both. [`assert_mapping_is_pinned`] fails the run on any difference
/// between this table and [`cli_key_event`], and on any character that maps
/// without appearing here — a swapped key is a failure, not a smaller count.
const CLI_TABLE: [(char, Key, bool); 87] = [
    ('\u{8}', Key::Backspace, false),
    ('\n', Key::Enter, false),
    ('\t', Key::Tab, false),
    (' ', Key::Space, false),
    ('a', Key::A, false),
    ('b', Key::B, false),
    ('c', Key::C, false),
    ('d', Key::D, false),
    ('e', Key::E, false),
    ('f', Key::F, false),
    ('g', Key::G, false),
    ('h', Key::H, false),
    ('i', Key::I, false),
    ('j', Key::J, false),
    ('k', Key::K, false),
    ('l', Key::L, false),
    ('m', Key::M, false),
    ('n', Key::N, false),
    ('o', Key::O, false),
    ('p', Key::P, false),
    ('q', Key::Q, false),
    ('r', Key::R, false),
    ('s', Key::S, false),
    ('t', Key::T, false),
    ('u', Key::U, false),
    ('v', Key::V, false),
    ('w', Key::W, false),
    ('x', Key::X, false),
    ('y', Key::Y, false),
    ('z', Key::Z, false),
    ('A', Key::A, true),
    ('B', Key::B, true),
    ('C', Key::C, true),
    ('D', Key::D, true),
    ('E', Key::E, true),
    ('F', Key::F, true),
    ('G', Key::G, true),
    ('H', Key::H, true),
    ('I', Key::I, true),
    ('J', Key::J, true),
    ('K', Key::K, true),
    ('L', Key::L, true),
    ('M', Key::M, true),
    ('N', Key::N, true),
    ('O', Key::O, true),
    ('P', Key::P, true),
    ('Q', Key::Q, true),
    ('R', Key::R, true),
    ('S', Key::S, true),
    ('T', Key::T, true),
    ('U', Key::U, true),
    ('V', Key::V, true),
    ('W', Key::W, true),
    ('X', Key::X, true),
    ('Y', Key::Y, true),
    ('Z', Key::Z, true),
    ('0', Key::Digit(0), false),
    ('1', Key::Digit(1), false),
    ('2', Key::Digit(2), false),
    ('3', Key::Digit(3), false),
    ('4', Key::Digit(4), false),
    ('5', Key::Digit(5), false),
    ('6', Key::Digit(6), false),
    ('7', Key::Digit(7), false),
    ('8', Key::Digit(8), false),
    ('9', Key::Digit(9), false),
    ('!', Key::Digit(1), true),
    ('@', Key::Digit(2), true),
    ('#', Key::Digit(3), true),
    ('$', Key::Digit(4), true),
    ('%', Key::Digit(5), true),
    ('^', Key::Digit(6), true),
    ('&', Key::Digit(7), true),
    ('*', Key::Digit(8), true),
    ('(', Key::Digit(9), true),
    (')', Key::Digit(0), true),
    ('\'', Key::Quote, false),
    ('[', Key::LeftBracket, false),
    (']', Key::RightBracket, false),
    (';', Key::Semicolon, false),
    (',', Key::Comma, false),
    ('.', Key::Dot, false),
    ('/', Key::Slash, false),
    ('\\', Key::BackSlash, false),
    ('-', Key::Minus, false),
    ('=', Key::Equals, false),
    ('`', Key::Backquote, false),
];

/// Checks [`CLI_TABLE`] entry by entry against [`cli_key_event`], then proves
/// the table is closed: no other ASCII character maps, and the documented
/// refusals stay refused.
fn assert_mapping_is_pinned() -> Result<(), String> {
    let mut seen: Vec<char> = Vec::with_capacity(CLI_TABLE.len());
    for (typed, key, shift) in CLI_TABLE {
        let event = cli_key_event(typed)
            .ok_or_else(|| format!("{typed:?} is in the table but does not map"))?;
        let expected = KeyEvent {
            key,
            mods: Mods {
                shift,
                ..Mods::default()
            },
            phase: Phase::Down,
            text: Some(typed),
        };
        if event.key != expected.key
            || event.mods != expected.mods
            || event.phase != expected.phase
            || event.text != expected.text
        {
            return Err(format!(
                "mapping drifted for {typed:?}: {event:?} != {expected:?}"
            ));
        }
        if seen.contains(&typed) {
            return Err(format!("{typed:?} appears twice in the table"));
        }
        seen.push(typed);
    }
    for code in 0u32..=0x7F {
        let Some(typed) = char::from_u32(code) else {
            continue;
        };
        if cli_key_event(typed).is_some() && !seen.contains(&typed) {
            return Err(format!(
                "{typed:?} (U+{code:04X}) maps but is not in the table"
            ));
        }
    }
    for typed in ['\r', '\u{7F}', '~', 'ệ', 'Đ', '中', '😀'] {
        if cli_key_event(typed).is_some() {
            return Err(format!("{typed:?} must be refused"));
        }
    }
    Ok(())
}

fn profile_for(row: usize) -> Options {
    if row % 2 == 0 {
        Options::default()
    } else {
        Options {
            input_method: InputMethod::Vni,
            modern_orthography: true,
            ..Options::default()
        }
    }
}

fn build_stream(rows: &[Row], events: usize) -> Result<Stream, String> {
    // The hash covers the whole corpus once, so it identifies the workload
    // independently of `--events`; the baseline comparison relies on that.
    let mut fnv1a = FNV_OFFSET;
    for row in rows {
        hash_bytes(&mut fnv1a, row.input.as_bytes());
        hash_byte(&mut fnv1a, 0x1F);
    }
    let mut steps = Vec::with_capacity(events);
    let mut cycle = 0usize;
    'cycles: loop {
        for (index, row) in rows.iter().enumerate() {
            for typed in row.input.chars() {
                let event = cli_key_event(typed).ok_or_else(|| {
                    format!(
                        "{}: unmapped corpus character {typed:?} (U+{:04X}); the CLI refuses it",
                        row.source, typed as u32
                    )
                })?;
                steps.push(Step {
                    event,
                    row: cycle * rows.len() + index,
                });
                if steps.len() == events {
                    break 'cycles;
                }
            }
        }
        cycle += 1;
    }
    Ok(Stream {
        steps,
        cases: rows.len(),
        fnv1a,
    })
}

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

fn hash_byte(state: &mut u64, byte: u8) {
    *state ^= u64::from(byte);
    *state = state.wrapping_mul(FNV_PRIME);
}

fn hash_bytes(state: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        hash_byte(state, *byte);
    }
}

fn measure(stream: &Stream, warmup: usize, reps: usize) -> Result<Report, String> {
    for _ in 0..warmup {
        run_pass(stream);
    }
    let mut p50_per_rep = Vec::with_capacity(reps);
    let mut p99_per_rep = Vec::with_capacity(reps);
    let mut p999_per_rep = Vec::with_capacity(reps);
    let mut max_per_rep = Vec::with_capacity(reps);
    let mut rate_per_rep = Vec::with_capacity(reps);
    for _ in 0..reps {
        let deltas = run_pass(stream);
        if deltas.is_empty() {
            return Err("the stream produced no keystrokes".to_string());
        }
        let mut sorted = deltas.clone();
        sorted.sort_unstable();
        p50_per_rep.push(percentile_ms(&sorted, 0.50));
        p99_per_rep.push(percentile_ms(&sorted, 0.99));
        p999_per_rep.push(percentile_ms(&sorted, 0.999));
        max_per_rep.push(round6(sorted[sorted.len() - 1].as_secs_f64() * 1000.0));
        let total: Duration = deltas.iter().sum();
        rate_per_rep.push(round6(
            stream.steps.len() as f64 / total.as_secs_f64().max(f64::MIN_POSITIVE),
        ));
    }
    let p99_raw = median(&p99_per_rep);
    let index = index_of_median(&p99_per_rep);
    Ok(Report {
        p50_ms: round6(median(&p50_per_rep)),
        p99_ms: round6(p99_raw),
        p999_ms: round6(median(&p999_per_rep)),
        max_ms: median(&max_per_rep),
        keystrokes_per_s: rate_per_rep[index],
        p99_per_rep,
        p99_pass: p99_raw <= GATE_MS,
    })
}

/// One pass over the stream. Engine construction and `Options` clones happen
/// between rows and are outside the timed region; only `apply` is sampled.
fn run_pass(stream: &Stream) -> Vec<Duration> {
    let mut deltas = Vec::with_capacity(stream.steps.len());
    let mut current_row = usize::MAX;
    let mut engine = Engine::new(Options::default());
    for step in &stream.steps {
        if step.row != current_row {
            engine = Engine::new(profile_for(step.row));
            current_row = step.row;
        }
        let start = Instant::now();
        black_box(engine.apply(&step.event));
        deltas.push(start.elapsed());
    }
    deltas
}

/// Nearest-rank percentile: `sorted[ceil(n * q) - 1]`, in milliseconds.
fn percentile_ms(sorted: &[Duration], q: f64) -> f64 {
    let n = sorted.len();
    let rank = ((n as f64) * q).ceil().max(1.0) as usize;
    round6(sorted[rank.saturating_sub(1).min(n - 1)].as_secs_f64() * 1000.0)
}

fn round6(value: f64) -> f64 {
    (value * 1e6).round() / 1e6
}

fn median(values: &[f64]) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).expect("no NaN in a report"));
    sorted[sorted.len() / 2]
}

fn index_of_median(values: &[f64]) -> usize {
    let target = median(values);
    values
        .iter()
        .position(|value| *value == target)
        .unwrap_or(0)
}

fn render_json(stream: &Stream, args: &Args, report: &Report) -> String {
    let payload = serde_json::json!({
        "schema": SCHEMA,
        "commit": commit_id(),
        "runner": runner_name(),
        "os": env::consts::OS,
        "arch": env::consts::ARCH,
        "stream_events": stream.steps.len(),
        "stream_cases": stream.cases,
        "stream_fnv1a": format!("0x{:016x}", stream.fnv1a),
        "repetitions": args.reps,
        "warmup_passes": args.warmup,
        "p50_ms": report.p50_ms,
        "p99_ms": report.p99_ms,
        "p999_ms": report.p999_ms,
        "max_ms": report.max_ms,
        "keystrokes_per_s": report.keystrokes_per_s,
        "p99_per_rep_ms": report.p99_per_rep,
        "gate_ms": GATE_MS,
        "p99_pass": report.p99_pass,
    });
    payload.to_string()
}

fn commit_id() -> String {
    if let Ok(sha) = env::var("GITHUB_SHA") {
        if !sha.is_empty() {
            return sha;
        }
    }
    Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|sha| !sha.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

fn runner_name() -> String {
    env::var("RUNNER_NAME")
        .ok()
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}
