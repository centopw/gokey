#!/usr/bin/env bash
# Parity: verify the oracle pin, build the reference driver, replay every script.
#
# The CI parity job runs this the moment the file exists (ADR-0008 §1), so it has
# to pass on a clean ubuntu-24.04 runner with nothing but python3, git and
# clang++. It fetches the pinned upstream commit — network is available there —
# and refuses to build if the resolved commit is not the pin.
#
# The pin step reads `parity/oracle/pin.json` and fails if that copy and the
# human record `docs/m0/ORACLE-PIN.json` are not the same bytes (PORTING-MAP §5):
# the file CI verifies and the file a reader reads may not drift apart.
#
# Exit status: 0 all cases agree, 1 at least one disagrees, 2 the run could not
# start (pin, toolchain or checkout broken).
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

cache="${ORACLE_CACHE:-$root/.oracle/src}"
build="${ORACLE_BUILD:-$root/.oracle/build}"
cxx="${CXX:-clang++}"
pin="parity/oracle/pin.json"
record="docs/m0/ORACLE-PIN.json"

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

scripts=( scripts/parity/smoke.tsv )
while IFS= read -r script; do
    [ -n "$script" ] && scripts+=( "$script" )
done < <(find tests/corpus -name '*.tsv' -type f 2>/dev/null | sort)

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

if [ "$status" -ne 0 ]; then
    echo "::error::parity cases disagree with the reference engine" >&2
fi
exit "$status"
