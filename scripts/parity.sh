#!/usr/bin/env bash
# Parity: verify the oracle pin, build the reference driver, replay every script
# against it, then judge the same scripts with the Rust port (`openkey replay`).
#
# The CI parity job runs this the moment the file exists (ADR-0008 §1), so it has
# to pass on a clean ubuntu-24.04 runner with nothing but python3, git, clang++
# and the Rust toolchain. It fetches the pinned upstream commit — network is
# available there — and refuses to build if the resolved commit is not the pin.
#
# The pin step reads `parity/oracle/pin.json` and fails if that copy and the
# human record `docs/m0/ORACLE-PIN.json` are not the same bytes (PORTING-MAP §5):
# the file CI verifies and the file a reader reads may not drift apart.
#
# The two halves judge different things. `compare.py` checks the `.tsv` expected
# column against the reference engine — it says the baselines are right, nothing
# about the port. `openkey replay --diff ref-engine` replays the same script
# through the Rust engine and compares it to both the expected column and a
# fresh reference run: that is the half that can fail because of a port bug.
#
# Exit status (K-005 turn 3, Hiep/PA):
#   0 every in-scope case was judged and passed — with one declared-green
#     exception: a run whose every case sits in an inactive block judged nothing
#     because nothing was declared, and exits 0 naming the skipped count
#   1 at least one judged case disagrees (either half)
#   2 no verdict: could not start (pin, toolchain, checkout), could not judge
#     (corpus, scope or reference error), or an in-scope case was not judged
#     (pending, invalid, or a skipped in-scope row). Pending inside the declared
#     scope is never folded into green; pending outside it is skipped and named.
#
# Scope is `parity/scope.txt` (K-005 turn 3): every case resolves to exactly one
# of the five blocks, `active:` decides which are judged. The judged/in-scope/
# skipped counts are printed twice — a ::warning:: line and the job's step
# summary — so a green job never reads as full parity. `--full` selects the file
# set, never the judged set, or the nightly would go red again at C-002.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

full=0
for arg in "$@"; do
    case "$arg" in
        --full) full=1 ;;
        -h | --help)
            echo "usage: scripts/parity.sh [--full]"
            echo "  --full  replay the full file set (smoke + corpus); the judged set is always parity/scope.txt"
            exit 0
            ;;
        *)
            echo "::error::unknown parity argument '$arg'; only --full is accepted" >&2
            exit 2
            ;;
    esac
done

cache="${ORACLE_CACHE:-$root/.oracle/src}"
build="${ORACLE_BUILD:-$root/.oracle/build}"
cxx="${CXX:-clang++}"
pin="parity/oracle/pin.json"
record="docs/m0/ORACLE-PIN.json"
scope="parity/scope.txt"

if [ ! -f "$scope" ]; then
    echo "::error::missing $scope; the judge cannot name what it did not measure" >&2
    exit 2
fi

scripts=( scripts/parity/smoke.tsv )
while IFS= read -r script; do
    [ -n "$script" ] && scripts+=( "$script" )
done < <(find tests/corpus -name '*.tsv' -type f 2>/dev/null | sort)

# `--full` is parsed, never ignored: the nightly passes it. Today it selects the
# same file set as a PR run (there is no second stream yet), and it can never
# widen the judged set — that is `parity/scope.txt`, on every run.
if [ "$full" -eq 1 ]; then
    echo "== full file set: ${#scripts[@]} script(s); judged set is $scope =="
fi

# The global scope check: does every case resolve to exactly one block, and does
# every active block match at least one case *somewhere in the played set*? The
# per-file judge below cannot answer the second half — a VNI script contains no
# C-002 telex case by design — so this runs once over all scripts, before the
# network fetch, so a broken declaration fails fast. A selector that matches
# nothing is a broken gate, not a clean run.
echo "== scope =="
scope_args=()
for script in "${scripts[@]}"; do
    scope_args+=( --corpus "$script" )
done
rc=0
cargo run -q --locked -p openkey-cli -- scope --scope "$scope" "${scope_args[@]}" || rc=$?
case "$rc" in
    0) ;;
    2) echo "::error::the declared scope does not cover the played set; nothing was replayed" >&2; exit 2 ;;
    *) echo "::error::openkey scope could not start (cargo exit $rc)" >&2; exit 2 ;;
esac

if ! command -v "$cxx" >/dev/null 2>&1; then
    echo "::error::no C++ compiler: '$cxx' not found (set CXX)" >&2
    exit 2
fi

echo "== pin =="
# A pin that does not verify is "the run could not start" (exit 2), not "the
# cases disagree" (exit 1): nothing has been replayed yet, and reporting it as
# a behaviour disagreement would send the next reader looking for a wrong
# baseline. The checker prints which half failed — copy vs record, or a pinned
# file's digest.
if ! python3 scripts/ci/check-oracle-pin.py --pin "$pin" --record "$record" --dest "$cache"; then
    echo "::error::the oracle pin did not verify; nothing was replayed" >&2
    exit 2
fi

echo "== build =="
engine="$cache/Sources/OpenKey/engine"
mkdir -p "$build"
# No -DLINUX and no -D_WIN32: DataType.h picks platforms/mac.h by ifdef and the
# pin's normative_platform is macOS (DL-026). A -DLINUX build would use a
# different keycode convention and a different std::map iteration order (DEV-02),
# and would silently stop being the same oracle.
#
# -include algorithm: upstream ConvertTool.cpp:169 calls std::find without
# including <algorithm>, which libc++ happens to pull in transitively and
# libstdc++ does not. Forcing the header keeps the pinned tree byte-identical —
# patching it would break the digests this job just verified — and cannot change
# behaviour, because the overload libstdc++ picks is the same std::find.
"$cxx" -std=c++17 -O2 -w -include algorithm -I "$engine" \
    -o "$build/harness" \
    scripts/parity/harness.cpp \
    "$engine/Engine.cpp" "$engine/Vietnamese.cpp" "$engine/Macro.cpp" \
    "$engine/SmartSwitchKey.cpp" "$engine/ConvertTool.cpp"
echo "built $build/harness"

status=0
for script in "${scripts[@]}"; do
    [ -f "$script" ] || { echo "::error::missing script $script" >&2; exit 2; }
    out="$build/$(basename "${script%.tsv}").out"
    echo "== replay $script =="
    if ! "$build/harness" "$script" > "$out"; then
        echo "::error::the harness could not replay $script" >&2
        exit 2
    fi
    # The comparator's 2 is "could not judge", not "judged and disagreed".
    # Collapsing them costs the first person to debug a truncated run an hour
    # spent looking for a wrong baseline that does not exist.
    rc=0
    python3 scripts/parity/compare.py "$script" "$out" || rc=$?
    case "$rc" in
        0) ;;
        2) echo "::error::$script could not be judged (comparator exit 2)" >&2; exit 2 ;;
        *) status=1 ;;
    esac
done

# The Rust judge (K-005). It runs even when the oracle half is red, so a single
# run reports both sides; the exit code below is a combination, not a race.
echo "== judge (openkey replay) =="
judge_pass=0
judge_fail=0
judge_pending=0
judge_invalid=0
judge_skip=0
judge_no_verdict=0

# Count the per-case status lines `openkey replay` prints. The counts, not the
# exit code alone, are what name the "no verdict" state in the log: "12 pending
# in scope" and "ref engine failed" both exit 2 but are different problems.
count_rows() {
    local n
    n=$(grep -c "^$1 " "$2" 2>/dev/null || true)
    echo "${n:-0}"
}

for script in "${scripts[@]}"; do
    echo "== judge $script =="
    judge_out="$build/$(basename "${script%.tsv}").judge.txt"
    rc=0
    # `cargo run` forwards the binary's exit status. A build failure is 101 and
    # is handled below as "could not start", like a broken pin — not as a case
    # disagreement.
    cargo run -q --locked -p openkey-cli -- replay \
        --corpus "$script" --scope "$scope" --diff ref-engine --ref "$build/harness" \
        2>&1 | tee "$judge_out" || rc=$?
    judge_pass=$(( judge_pass + $(count_rows PASS "$judge_out") ))
    judge_fail=$(( judge_fail + $(count_rows FAIL "$judge_out") ))
    judge_pending=$(( judge_pending + $(count_rows PENDING "$judge_out") ))
    judge_invalid=$(( judge_invalid + $(count_rows INVALID "$judge_out") ))
    judge_skip=$(( judge_skip + $(count_rows SKIP "$judge_out") ))
    case "$rc" in
        0) ;;
        1) status=1 ;;
        2) judge_no_verdict=1 ;;
        *)
            echo "::error::openkey replay could not start on $script (cargo exit $rc)" >&2
            exit 2
            ;;
    esac
done

judged=$(( judge_pass + judge_fail ))
# Every row the CLI reported is either judged (pass/fail) or unjudged
# (pending/invalid); skipped rows are out of scope by construction, because CI
# never passes --allow-missing-expected (an in-scope skip would exit 2 below).
in_scope=$(( judge_pass + judge_fail + judge_pending + judge_invalid ))

green=0
if [ "$status" -eq 0 ] && [ "$judge_fail" -eq 0 ] && [ "$judge_no_verdict" -eq 0 ] \
    && [ "$in_scope" -le "$judged" ]; then
    green=1
fi

# Guardrail 2: a green job prints the counts twice — the ::warning:: line and
# the step summary — so green reads as "nothing declared is unjudged", not as
# parity. A red run prints the same line without the annotation.
counts="judged $judged / in-scope $in_scope / skipped $judge_skip"
line="parity scope: $counts — green means nothing declared is unjudged, never parity"
if [ "$green" -eq 1 ]; then
    echo "::warning::$line"
else
    echo "$line"
fi
if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then
    printf '%s\n' "$line" >> "$GITHUB_STEP_SUMMARY"
fi

if [ "$status" -ne 0 ]; then
    echo "::error::parity cases disagree with the reference engine" >&2
fi
if [ "$judge_fail" -gt 0 ]; then
    echo "::error::parity judge: $judge_fail judged case(s) disagree with the expected column or the reference engine" >&2
fi
if [ "$status" -ne 0 ] || [ "$judge_fail" -gt 0 ]; then
    exit 1
fi

# 0 means every *in-scope* case was judged and passed. The scope decides what
# "in scope" is, so an all-skipped run with nothing declared is green; an
# in-scope case that is not judged (pending, invalid, or a skipped in-scope row)
# keeps this at 2. `judge_no_verdict` covers a CLI run that could not produce a
# verdict at all (scope/corpus/reference error) — without it, zero rows would
# look like zero in-scope cases.
if [ "$judge_no_verdict" -ne 0 ] || [ "$in_scope" -gt "$judged" ]; then
    if [ "$judge_pending" -gt 0 ] || [ "$judge_invalid" -gt 0 ]; then
        reason="$(( in_scope - judged )) in-scope case(s) unjudged: $judge_pending pending, $judge_invalid invalid"
    else
        reason="no verdict — the corpus, the scope or the reference engine failed"
    fi
    echo "::error::parity judge: no verdict — $reason" >&2
    exit 2
fi

exit 0
