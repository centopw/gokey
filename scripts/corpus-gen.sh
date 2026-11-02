#!/usr/bin/env bash
# Corpus generator (C-014, plan §3 row 16, contract R2).
#
# Evidence tooling, not a gate: `scripts/parity.sh` remains the only judge. This
# script exists so the judged set can grow past the size a person can review by
# hand without giving up the corpus README's rule that no `expected` cell is
# hand-written. Every row it emits is the pinned harness's own output for that
# row, replayed by a fresh process (contract R6), and the whole block is then
# replayed in one process so the per-case baseline and the whole-file baseline
# are proved equal (the delta the contract expects to be zero).
#
# The rows land under `# >>> generated: <seed>` / `# <<< generated` markers in a
# file already selected by the block that owns them. The region is replaced in
# place, so re-running with the same seed and target is byte-identical and a
# second run leaves `git diff` empty.
#
# Usage: scripts/corpus-gen.sh --seed <n> [--target <judged cases>]
#   --target defaults to 2000 and counts both corpus rows and smoke.tsv, the
#   same set `openkey replay --scope` judges.
#
# Exit status: 0 rows written (or nothing to write), 2 refused — the pin did not
# verify, the harness could not be built, a candidate row was unpressable,
# duplicated an existing id, or would not classify into an active block, or the
# whole-file and per-case baselines could not be reconciled.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

exec python3 - "$@" <<'PY'
from __future__ import annotations

import argparse
import fnmatch
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path.cwd()
MASK64 = (1 << 64) - 1
HARNESS = ROOT / ".oracle" / "build" / "harness"
SMOKE = "scripts/parity/smoke.tsv"

# Keys `_characterMap` can press (Vietnamese.cpp:508-557) plus the corpus's own
# escapes. A key outside this set would make the harness refuse the row, which
# the contract calls a generator bug rather than a case.
ALLOWED = set("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789.-',!;")


def die(message: str) -> "None":
    print(f"corpus-gen: {message}", file=sys.stderr)
    sys.exit(2)


class Rng:
    """A 64-bit LCG: deterministic across Python versions, unlike `random`."""

    def __init__(self, seed: int) -> None:
        self.state = seed & MASK64

    def next64(self) -> int:
        self.state = (self.state * 6364136223846793005 + 1442695040888963407) & MASK64
        return self.state

    def below(self, n: int) -> int:
        if n <= 0:
            raise ValueError(n)
        return (self.next64() >> 33) % n

    def choice(self, seq):
        return seq[self.below(len(seq))]


# ---------------------------------------------------------------------------
# Word material. Every string is already a key sequence a person types: the
# base pools are plain Vietnamese syllables, and the encoded pools are the same
# words as the telex/vni keys that produce them.
# ---------------------------------------------------------------------------

BASES = """a an anh ao ba ban bao ben bi bo bu ca can cao cang chao che chi cho chu
chuyen co con cung da dang dao de di dieu do du duong gai ghe ghi gia giang gio
giua ha hai ham hanh hao hen ho hoa hoc hoi hom hop huong kha khac khach khan
khao khe kho khoe khong khu khuyen la lai lam lanh lao le lien lo long luon ma
mai man mang me mi minh mo moi mong mu mua muon na nai nam nao ne nen ngo ngon
ngu nguyen nguoi nhan nhieu nho nhu nhung no noi nong nu nua nuoc ong phai phan
phi pho phuc qua quang que quen quy quyen ra rao re ri ro ru ruou sa sai sang
sao se sen si so song su sung ta tai tan tang tao te ten thi tho thu thuong ti
tich to toi ton tong tra trai trang tre tri tro trong tru truong tu tua tuan
tung tuoi uong va vai van vang vao ve vi viec viet vo von vu vua vui xa xanh xe
xem xi xo xong xu xuan yeu""".split()

TELEX_ENCODED = """tieengs vieejt nguwowif dduwowcj truowngf khoer hoaf thuowng
nguyeenx chuyeenj quyeets nghieemj ddaay dduwowngf ddoongf moojt khoong phuowng
nguwowif""".split()

VNI_ENCODED = """vie65t nguo72i tie61ng d9u7o72ng tru7o72ng kho3e hoa2 thu7o7ng
nguye63n chuye63n quye61t nghie6m ha2nh phu1c gieo5 ra61t nhie62u la2m ho6m ye6u
d9a4 se4 cung4 giu74a ngoa2i nhu74ng vie65c""".split()

SIMPLE_ENCODED = """vieejt tieengs nguwowif dduwowcj truowngf khoer hoaf thuowng
nguyeenx chuyeenj quyeets nghieemj aa ee oo dd cc gg kk ng qu ph th uu w ww wow
wuw aw ew ow uw""".split()

TELEX_OPT_BASES = """hoaf hoafn hoafnn khoefn osang thoa hoa hoan khoa khoe quan
quang nguoi nguyen chuyen quyen thuong truong""".split()

ZFWJ = ["za", "fa", "ja", "zaa", "faa", "jaa"]

EDIT_OPTIONS = ["-", "spelling", "no-spelling", "restore-if-wrong-spelling",
                "temp-off-spelling", "allow-zfwj"]

OPT_OPTIONS = ["-", "quick-telex", "free-mark", "modern-orthography",
               "quick-start-consonant", "quick-end-consonant", "allow-zfwj",
               "upper-case-first", "no-spelling", "restore-if-wrong-spelling",
               "spelling", "temp-off-spelling"]

OPT_COMBOS = [
    "upper-case-first,quick-telex",
    "upper-case-first,no-spelling",
    "upper-case-first,modern-orthography",
    "restore-if-wrong-spelling,modern-orthography",
    "quick-telex,no-spelling",
    "quick-telex,modern-orthography",
    "quick-start-consonant,quick-telex",
    "quick-end-consonant,modern-orthography",
    "modern-orthography,spelling",
    "modern-orthography,no-spelling",
    "free-mark,modern-orthography",
    "allow-zfwj,quick-telex",
]

MACRO_OPTIONS = ["-", "auto-caps-macro", "upper-case-first,auto-caps-macro",
                 "no-macro", "english,macro-in-english", "english"]

MACROS = [
    ("bt", "bê tông"), ("vn", "Việt Nam"), ("ok", "được"),
    ("hcm", "Hồ Chí Minh"), ("tphcm", "Thành phố Hồ Chí Minh"),
    ("gokey", "GoKey"), ("hn", "Hà Nội"), ("nc", "nghiên cứu"),
    ("hp", "hạnh phúc"), ("mm", "mãi mãi"), ("hv", "học viên"),
    ("ns", "nhân sự"), ("hl", "hạnh lộc"), ("abc", "a"), ("xyz", "x"),
    ("d1", "dòng một"), ("k1", "khối một"), ("tv", "truyền hình"),
    ("qg", "quốc gia"), ("tl", "tài liệu"),
]


# ---------------------------------------------------------------------------
# Row shapes. Each returns (method, options, input, macros-or-None).
# ---------------------------------------------------------------------------

def gen_telex(rng: Rng):
    base = rng.choice(BASES + TELEX_ENCODED)
    if rng.below(10) < 2:
        base = base[:1].upper() + base[1:]
    if rng.below(10) < 6:
        base += rng.choice("sfrxjz")
    if rng.below(10) < 2:
        base += rng.choice([".", "!", ";", ","])
    if rng.below(10) < 2:
        base += r"\s" + rng.choice(BASES)
    if rng.below(10) < 1:
        base += r"\b"
    return "telex", "-", base, None


def gen_vni(rng: Rng):
    base = rng.choice(BASES + VNI_ENCODED)
    if rng.below(10) < 2:
        base = base[:1].upper() + base[1:]
    if rng.below(10) < 6:
        base += rng.choice("12345")
    elif rng.below(10) < 2:
        base += rng.choice("67890")
    if rng.below(10) < 2:
        base += rng.choice([".", "!", ";", ","])
    if rng.below(10) < 2:
        base += r"\s" + rng.choice(BASES)
    if rng.below(10) < 1:
        base += r"\b"
    return "vni", "-", base, None


def gen_simple(rng: Rng):
    base = rng.choice(SIMPLE_ENCODED + BASES)
    if rng.below(10) < 2:
        base = base[:1].upper() + base[1:]
    if rng.below(10) < 5:
        base += rng.choice("sfrxjz")
    if rng.below(10) < 2:
        base += r"\s" + rng.choice(SIMPLE_ENCODED)
    method = "simple_telex_1" if rng.below(10) < 8 else "simple_telex_2"
    return method, "-", base, None


def gen_edit(rng: Rng):
    method = "telex" if rng.below(10) < 8 else "vni"
    base = rng.choice(BASES + (TELEX_ENCODED if method == "telex" else VNI_ENCODED))
    if method == "telex":
        base += rng.choice("sfrxjz") if rng.below(10) < 7 else ""
    else:
        base += rng.choice("12345") if rng.below(10) < 7 else ""
    tokens = [base]
    for _ in range(1 + rng.below(2)):
        # Insert whole tokens, never a character index: splitting an escape
        # (`\` + `n`) would produce a literal backslash and fail validation.
        pos = 1 + rng.below(len(tokens))
        tokens.insert(pos, rng.choice([r"\b", r"\s", r"\n", r"\t"]))
    base = "".join(tokens)
    if rng.below(10) < 3:
        base += rng.choice(BASES)
    return method, rng.choice(EDIT_OPTIONS), base, None


def gen_opt_single(rng: Rng):
    options = rng.choice(OPT_OPTIONS)
    if options == "quick-telex":
        base = rng.choice(["cc", "gg", "kk", "ng", "qu", "ph", "th", "uu", "ch", "tr"])
    elif options == "free-mark":
        base = rng.choice(["hoaf", "hoafn", "hoafnn", "khoefn", "osang", "thoa"])
    elif options == "modern-orthography":
        base = rng.choice(TELEX_OPT_BASES)
    elif options == "quick-start-consonant":
        base = rng.choice(["ca", "cho", "chao", "gia", "gio", "quan", "qua"])
    elif options == "quick-end-consonant":
        base = rng.choice(["anh", "ach", "ang", "chinh", "minh", "ngon", "truong"])
    elif options == "allow-zfwj":
        base = rng.choice(ZFWJ)
    elif options == "upper-case-first":
        base = rng.choice(TELEX_OPT_BASES)
        base = base[:1].upper() + base[1:]
    else:
        base = rng.choice(BASES + TELEX_ENCODED)
    if rng.below(10) < 5:
        base += rng.choice("sfrxjz")
    if rng.below(10) < 3:
        base += rng.choice([".", "!", ";", ","])
    if rng.below(10) < 2:
        base += r"\s" + rng.choice(BASES)
    return "telex", options, base, None


def gen_opt_combo(rng: Rng):
    options = rng.choice(OPT_COMBOS)
    if "free-mark" in options:
        base = rng.choice(["hoaf", "hoafn", "hoafnn", "khoefn"])
    elif "quick-telex" in options:
        base = rng.choice(["cc", "gg", "kk", "ng", "qu", "ph", "th", "uu"])
    elif "upper-case-first" in options:
        base = rng.choice(TELEX_OPT_BASES)
        base = base[:1].upper() + base[1:]
    elif "allow-zfwj" in options:
        base = rng.choice(ZFWJ)
    elif "quick-start-consonant" in options:
        base = rng.choice(["ca", "cho", "chao", "gia", "gio", "quan", "qua"])
    elif "quick-end-consonant" in options:
        base = rng.choice(["anh", "ach", "ang", "chinh", "minh", "ngon", "truong"])
    else:
        base = rng.choice(BASES + TELEX_ENCODED)
    if rng.below(10) < 5:
        base += rng.choice("sfrxjz")
    if rng.below(10) < 2:
        base += r"\b"
    return "telex", options, base, None


def gen_macro(rng: Rng):
    name, content = rng.choice(MACROS)
    options = rng.choice(MACRO_OPTIONS)
    shape = rng.below(10)
    if shape < 5:
        typed = name + r"\s"
    elif shape < 7:
        typed = name.upper() + r"\s"
    elif shape < 8:
        typed = name + r"\n"
    elif shape < 9:
        typed = name + r"\b\s"
    else:
        typed = rng.choice("xyzq") + name + r"\s"
    return "telex", options, typed, f"{name}={content}"


# ---------------------------------------------------------------------------
# The plan: which file grows, by how much relative to the others, and what
# shape its rows take. All six blocks grow; no file is added and no seventh
# block exists (contract R1/R3).
# ---------------------------------------------------------------------------

PLAN = [
    ("tests/corpus/telex-basic.tsv", "telex", 100, gen_telex),
    ("tests/corpus/telex-tones.tsv", "telex", 100, gen_telex),
    ("tests/corpus/telex-words.tsv", "telex", 100, gen_telex),
    ("tests/corpus/telex-syllables.tsv", "telex", 100, gen_telex),
    ("tests/corpus/telex-clusters.tsv", "telex", 100, gen_telex),
    ("tests/corpus/telex-stops.tsv", "telex", 100, gen_telex),
    ("tests/corpus/telex-tone-cycle.tsv", "telex", 100, gen_telex),
    ("tests/corpus/vni-basic.tsv", "vni", 100, gen_vni),
    ("tests/corpus/vni-words.tsv", "vni", 100, gen_vni),
    ("tests/corpus/vni-tones.tsv", "vni", 100, gen_vni),
    ("tests/corpus/editing.tsv", "edit", 80, gen_edit),
    ("tests/corpus/editing-sequences.tsv", "edit", 80, gen_edit),
    ("tests/corpus/spelling-boundaries.tsv", "edit", 80, gen_edit),
    ("tests/corpus/options.tsv", "opt", 160, gen_opt_single),
    ("tests/corpus/options-combos.tsv", "opt", 80, gen_opt_combo),
    ("tests/corpus/simple-telex.tsv", "simple", 75, gen_simple),
    ("tests/corpus/simple-telex-modes.tsv", "simple", 75, gen_simple),
    ("tests/corpus/macros.tsv", "macro", 120, gen_macro),
]


# ---------------------------------------------------------------------------
# Corpus and scope readers.
# ---------------------------------------------------------------------------

ESCAPES = {"b": "\b", "n": "\n", "t": "\t", "r": "\r", "s": " ", "\\": "\\"}


def unescape(raw: str, where: str) -> str:
    """Decode one expected cell with the corpus escape vocabulary (`-` is empty)."""
    if raw == "-":
        return ""
    out = []
    index = 0
    while index < len(raw):
        ch = raw[index]
        index += 1
        if ch != "\\":
            out.append(ch)
            continue
        if index >= len(raw):
            die(f"{where}: trailing backslash")
        esc = raw[index]
        index += 1
        if esc not in ESCAPES:
            die(f"{where}: unknown escape \\{esc} (allowed: \\b \\n \\t \\r \\s \\\\)")
        out.append(ESCAPES[esc])
    return "".join(out)


def escape(document: str) -> str:
    """Encode a document the way the corpus stores it (`-` for an empty one)."""
    if not document:
        return "-"
    out = []
    for ch in document:
        if ch == "\\":
            out.append("\\\\")
        elif ch == "\b":
            out.append("\\b")
        elif ch == "\n":
            out.append("\\n")
        elif ch == "\t":
            out.append("\\t")
        elif ch == "\r":
            out.append("\\r")
        elif ch == " ":
            out.append("\\s")
        else:
            out.append(ch)
    return "".join(out)


def split_row(line: str, where: str):
    fields = line.split("\t")
    if len(fields) < 4 or len(fields) > 6:
        die(f"{where}: expected 4, 5 or 6 tab-separated fields, got {len(fields)}")
    row = {
        "id": fields[0],
        "method": fields[1],
        "options": fields[2],
        "input": fields[3],
        "expected": fields[4] if len(fields) >= 5 else None,
        "macros": fields[5] if len(fields) >= 6 else None,
    }
    if row["expected"] is None:
        die(f"{where}: {row['id']} has no expected column; the corpus requires five fields")
    row["expected"] = unescape(row["expected"], f"{where}: expected")
    return row


def base_rows(path: str):
    """Rows outside the generated region, in file order."""
    rows = []
    inside = False
    for lineno, line in enumerate(Path(path).read_text(encoding="utf-8").splitlines(), 1):
        if line.startswith("# >>> generated:"):
            inside = True
            continue
        if line == "# <<< generated":
            inside = False
            continue
        if inside or not line.strip() or line.startswith("#"):
            continue
        rows.append(split_row(line, f"{path}:{lineno}"))
    return rows


def parse_scope(path: str):
    blocks = []
    active = set()
    for line in Path(path).read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        if line.startswith("block:"):
            rest = line[len("block:"):].strip()
            if not rest:
                die(f"{path}: empty block line")
            name, _, selectors = rest.partition(" ")
            blocks.append((name, [sel.strip() for sel in selectors.split(",") if sel.strip()]))
        elif line.startswith("active:"):
            active.add(line[len("active:"):].strip())
    if not blocks:
        die(f"{path}: no block lines")
    return blocks, active


def classify(case_id: str, path: str, blocks) -> str:
    best, best_spec = [], -1
    for name, selectors in blocks:
        for selector in selectors:
            if selector.startswith("file:"):
                spec, hit = 0, path.endswith(selector[len("file:"):])
            elif selector.startswith("id:"):
                spec, hit = 1, fnmatch.fnmatchcase(case_id, selector[len("id:"):])
            else:
                spec, hit = 2, case_id == selector
            if not hit:
                continue
            if spec > best_spec:
                best, best_spec = [name], spec
            elif spec == best_spec:
                best.append(name)
    if not best:
        die(f"{case_id} ({path}) resolves to no block")
    if len(set(best)) > 1:
        die(f"{case_id} ({path}) resolves to several blocks at the same specificity: {sorted(set(best))}")
    return best[0]


# ---------------------------------------------------------------------------
# Input validation, harness invocation, and baseline equality.
# ---------------------------------------------------------------------------

def validate_input(raw: str, case_id: str) -> None:
    i = 0
    while i < len(raw):
        ch = raw[i]
        if ch == "\\":
            if i + 1 >= len(raw) or raw[i + 1] not in "bsnt":
                die(f"{case_id}: unknown escape in generated input {raw!r}")
            i += 2
            continue
        if ch not in ALLOWED:
            die(f"{case_id}: U+{ord(ch):04X} is not a key the harness can press")
        i += 1


def row_text(row) -> str:
    fields = [row["id"], row["method"], row["options"], row["input"]]
    if row["expected"] is not None or row["macros"] is not None:
        fields.append(escape(row["expected"]) if row["expected"] is not None else "-")
    if row["macros"] is not None:
        fields.append(row["macros"])
    return "\t".join(fields)


class Unrepresentable(Exception):
    """The pinned harness's document model refuses this case (exit 3)."""


def run_harness(rows, allow_refusal: bool = False) -> dict:
    """One harness process over `rows`; returns id -> final document (escaped)."""
    handle = tempfile.NamedTemporaryFile(
        "w", suffix=".tsv", delete=False, encoding="utf-8",
        dir=str(ROOT / "target"), prefix="corpus-gen-",
    ) if (ROOT / "target").is_dir() else tempfile.NamedTemporaryFile(
        "w", suffix=".tsv", delete=False, encoding="utf-8", prefix="corpus-gen-",
    )
    try:
        with handle as fh:
            fh.write("\n".join(row_text(row) for row in rows) + "\n")
        proc = subprocess.run([str(HARNESS), handle.name], capture_output=True, text=True)
    finally:
        os.unlink(handle.name)
    if proc.returncode != 0:
        if allow_refusal and len(rows) == 1:
            raise Unrepresentable(proc.stderr.strip())
        print(proc.stderr, file=sys.stderr, end="")
        die(f"the pinned harness refused a generated case (exit {proc.returncode})")
    finals = {}
    for line in proc.stdout.splitlines():
        if line.startswith("#"):
            continue
        fields = line.split("\t")
        if len(fields) != 7:
            die(f"harness output line has {len(fields)} fields, expected 7: {line!r}")
        if fields[1] == "final":
            finals[fields[0]] = unescape(fields[6], f"harness:{fields[0]}")
    if len(finals) != len(rows):
        die(f"the harness reported {len(finals)} final document(s) for {len(rows)} case(s)")
    return finals


def oracle_case(row) -> str:
    return run_harness([row], allow_refusal=True)[row["id"]]


def draw_case(rng, profile, case_id, path, block, blocks, active, attempts: int = 60) -> dict:
    """Draw a candidate the pinned harness can represent, deterministically.

    A harness exit 3 is a model refusal, not a behaviour disagreement: the case
    has no document to pin, so it cannot be a corpus row. The draw is retried
    with the same RNG stream, which keeps the run reproducible.
    """
    for _ in range(attempts):
        method, options, typed, macros = profile(rng)
        validate_input(typed, case_id)
        resolved = classify(case_id, path, blocks)
        if resolved != block or resolved not in active:
            die(f"{case_id} resolves to {resolved!r} (active={resolved in active}), "
                f"expected the active block {block!r}")
        row = {"id": case_id, "method": method, "options": options,
               "input": typed, "expected": None, "macros": macros}
        try:
            row["expected"] = oracle_case(row)
        except Unrepresentable:
            continue
        return row
    die(f"{case_id}: no representable candidate after {attempts} draws")


def build_harness() -> str:
    rustc = shutil.which("rustc")
    if rustc is None:
        die("rustc is not on PATH; the toolchain the nightly pins must be present")
    print(subprocess.run([rustc, "-V"], capture_output=True, text=True, check=False).stdout.strip())

    pin = subprocess.run(
        [sys.executable, "scripts/ci/check-oracle-pin.py",
         "--pin", "parity/oracle/pin.json",
         "--record", "docs/m0/ORACLE-PIN.json",
         "--dest", ".oracle/src"],
        capture_output=True, text=True,
    )
    sys.stdout.write(pin.stdout)
    if pin.returncode != 0:
        sys.stderr.write(pin.stderr)
        die("the oracle pin did not verify; nothing was generated")

    engine = ROOT / ".oracle" / "src" / "Sources" / "OpenKey" / "engine"
    sources = [engine / name for name in
               ("Engine.cpp", "Vietnamese.cpp", "Macro.cpp", "SmartSwitchKey.cpp", "ConvertTool.cpp")]
    sources.append(ROOT / "scripts" / "parity" / "harness.cpp")
    HARNESS.parent.mkdir(parents=True, exist_ok=True)
    stale = not HARNESS.exists() or any(
        src.stat().st_mtime > HARNESS.stat().st_mtime for src in sources)
    if stale:
        cxx = os.environ.get("CXX") or ("clang++" if shutil.which("clang++") else "g++")
        if shutil.which(cxx) is None:
            die(f"no C++ compiler: {cxx!r} not found (set CXX)")
        cmd = [cxx, "-std=c++17", "-O2", "-w", "-include", "algorithm", "-I", str(engine),
               "-o", str(HARNESS), str(ROOT / "scripts" / "parity" / "harness.cpp"),
               str(engine / "Engine.cpp"), str(engine / "Vietnamese.cpp"),
               str(engine / "Macro.cpp"), str(engine / "SmartSwitchKey.cpp"),
               str(engine / "ConvertTool.cpp")]
        proc = subprocess.run(cmd, capture_output=True, text=True)
        if proc.returncode != 0:
            sys.stderr.write(proc.stderr)
            die("the pinned harness could not be built")
        print(f"built {HARNESS.relative_to(ROOT)}")
    else:
        print(f"reusing {HARNESS.relative_to(ROOT)}")
    return str(HARNESS)


# ---------------------------------------------------------------------------
# Generation.
# ---------------------------------------------------------------------------

def generate(seed: int, target: int) -> None:
    build_harness()

    blocks, active = parse_scope("parity/scope.txt")
    base = {path: base_rows(path) for path, _, _, _ in PLAN}
    smoke_rows = base_rows(SMOKE)
    # Every scoped corpus file counts toward the judged set, including files the
    # plan does not grow (case-mixed.tsv shares the telex block). Counting only
    # the plan's files would overshoot the requested target.
    scoped_rows = [row for path in sorted(Path("tests/corpus").glob("*.tsv"))
                   for row in base_rows(path)]
    base_judged = len(scoped_rows) + len(smoke_rows)
    existing_ids = {row["id"] for row in scoped_rows}
    existing_ids.update(row["id"] for row in smoke_rows)

    additions = max(0, target - base_judged)
    total_weight = sum(weight for _, _, weight, _ in PLAN)
    quotas, remainders = [], []
    for index, (_, _, weight, _) in enumerate(PLAN):
        q, r = divmod(additions * weight, total_weight)
        quotas.append(q)
        remainders.append((r, index))
    for _, index in sorted(remainders, key=lambda item: (-item[0], item[1]))[:additions - sum(quotas)]:
        quotas[index] += 1

    generated = []
    for index, ((path, block, _, profile), quota) in enumerate(zip(PLAN, quotas)):
        if quota == 0:
            generated.append((path, []))
            continue
        rng = Rng((seed * 1_000_003 + index) & MASK64)
        stem = Path(path).stem
        rows = []
        for n in range(1, quota + 1):
            case_id = f"gen-{stem}-{n:04d}"
            if case_id in existing_ids:
                die(f"{case_id} already exists in the corpus; refusing to reuse an id")
            existing_ids.add(case_id)
            rows.append(draw_case(rng, profile, case_id, path, block, blocks, active))

        # Contract R6: the per-case value is the baseline, and the whole-file
        # invocation must agree. Regenerate a row whose whole-file document
        # differs rather than committing a baseline that depends on how the
        # harness was invoked; a committed row is never touched.
        committed = base[path]
        for attempt in range(40):
            documents = run_harness(committed + rows)
            wrong_base = [row["id"] for row in committed
                          if documents.get(row["id"]) != row["expected"]]
            if wrong_base:
                die(f"{path}: existing row(s) disagree with the pinned oracle: "
                    f"{', '.join(wrong_base[:5])}")
            wrong = [i for i, row in enumerate(rows)
                     if documents.get(row["id"]) != row["expected"]]
            if not wrong:
                break
            for position in wrong:
                row = rows[position]
                rows[position] = draw_case(rng, profile, row["id"], path, block,
                                           blocks, active)
        else:
            die(f"{path}: could not reconcile whole-file and per-case baselines "
                "after 40 attempts")
        generated.append((path, rows))
        print(f"  {path}: +{len(rows)} row(s)")

    for path, rows in generated:
        write_region(path, seed, rows)

    judged = base_judged + additions
    corpus_rows = judged - len(smoke_rows)
    print(f"seed={seed}")
    print(f"generated {additions} row(s); corpus {corpus_rows} rows + {len(smoke_rows)} smoke "
          f"= {judged} judged case(s)")
    print("whole-file vs per-case delta: 0 row(s)")


def write_region(path: str, seed: int, rows) -> None:
    marker = f"# >>> generated: {seed}"
    block = [marker] + [row_text(row) for row in rows] + ["# <<< generated"]
    text = Path(path).read_text(encoding="utf-8")
    lines = text.split("\n")
    starts = [i for i, line in enumerate(lines) if line.startswith("# >>> generated:")]
    if len(starts) > 1:
        die(f"{path}: more than one generated marker")
    if starts:
        start = starts[0]
        end = next((j for j in range(start + 1, len(lines))
                    if lines[j] == "# <<< generated"), None)
        if end is None:
            die(f"{path}: generated marker has no closing line")
        lines[start:end + 1] = block
        Path(path).write_text("\n".join(lines), encoding="utf-8")
        return
    if text and not text.endswith("\n"):
        text += "\n"
    Path(path).write_text(text + "\n".join(block) + "\n", encoding="utf-8")


def main() -> int:
    ap = argparse.ArgumentParser(
        prog="scripts/corpus-gen.sh",
        description="Generate oracle-derived corpus rows under a stable marker block.",
    )
    ap.add_argument("--seed", type=int, required=True,
                    help="deterministic seed; the same seed and target produce the same bytes")
    ap.add_argument("--target", type=int, default=2000,
                    help="judged cases wanted (corpus rows + smoke.tsv); default 2000")
    args = ap.parse_args()
    if args.seed < 0:
        die("--seed must be non-negative")
    if args.target < 0:
        die("--target must be non-negative")
    print(f"seed={args.seed}")
    generate(args.seed, args.target)
    return 0


if __name__ == "__main__":
    sys.exit(main())
PY
