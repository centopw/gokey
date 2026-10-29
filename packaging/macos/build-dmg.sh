#!/usr/bin/env bash
#
# Build an ad-hoc signed .app bundle and .dmg for the M0/M1 artefact channel (K-007, G5).
#
# This is a SPIKE BUNDLE, not the shipping product: the executable it wraps is the
# headless `openkey` CLI. What it exercises is the part that always breaks late —
# bundle layout, Info.plist, the codesign flag path, and hdiutil. MAC-006 replaces
# the payload with the InputMethodKit bundle and the ad-hoc signature with
# Developer ID + notarisation (DL-022 freezes the identifier and the .app filename,
# so neither of those changes at M3).
#
# Usage:
#   build-dmg.sh --binary <path> --out <dir> [--version X.Y.Z] [--build N]
#
# Environment overrides: APP_NAME, DISPLAY_NAME, BUNDLE_ID, MIN_MACOS, COPYRIGHT.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

BINARY=""
OUT_DIR=""
SHORT_VERSION="0.1.0"
BUILD_VERSION=""
while [ $# -gt 0 ]; do
    case "$1" in
        --binary) BINARY="${2:?}"; shift 2 ;;
        --out) OUT_DIR="${2:?}"; shift 2 ;;
        --version) SHORT_VERSION="${2:?}"; shift 2 ;;
        --build) BUILD_VERSION="${2:?}"; shift 2 ;;
        -h|--help) sed -n '2,20p' "${BASH_SOURCE[0]}"; exit 0 ;;
        *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
done

[ -n "$BINARY" ] || { echo "--binary is required" >&2; exit 2; }
[ -f "$BINARY" ] || { echo "binary not found: $BINARY" >&2; exit 2; }
[ -n "$OUT_DIR" ] || { echo "--out is required" >&2; exit 2; }

# Frozen literals for 1.0 (DL-022). Changing any of these invalidates the
# Accessibility grant of every existing user, so they are not build knobs —
# they are constants with an env override only so the spike can be renamed on a
# throwaway branch.
APP_NAME="${APP_NAME:-GoKey}"                       # -> /Applications/GoKey.app
DISPLAY_NAME="${DISPLAY_NAME:-Gõ Key}"              # shown in consent dialogs, About, menu bar
BUNDLE_ID="${BUNDLE_ID:-com.centopw.gokey}"         # keys settings dir, TCC, update channel
EXECUTABLE="openkey"
MIN_MACOS="${MIN_MACOS:-13.0}"
COPYRIGHT="${COPYRIGHT:-GPL-3.0-only. Port of OpenKey by Mai Vu Tuyen.}"

if [ -z "$BUILD_VERSION" ]; then
    if [ -n "${GITHUB_RUN_NUMBER:-}" ]; then
        BUILD_VERSION="$GITHUB_RUN_NUMBER"
    else
        BUILD_VERSION="$(git -C "$ROOT" rev-list --count HEAD 2>/dev/null || echo 1)"
    fi
fi

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

STAGE="$WORK/dmgroot"
APP="$STAGE/$APP_NAME.app"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources" "$OUT_DIR"

echo "==> assembling $APP_NAME.app (display name: $DISPLAY_NAME, id: $BUNDLE_ID)"
install -m 0755 "$BINARY" "$APP/Contents/MacOS/$EXECUTABLE"

export DISPLAY_NAME EXECUTABLE BUNDLE_ID SHORT_VERSION BUILD_VERSION MIN_MACOS COPYRIGHT

python3 - "$ROOT/packaging/macos/Info.plist.in" "$APP/Contents/Info.plist" <<'PY'
import sys
from xml.sax.saxutils import escape

template, destination = sys.argv[1], sys.argv[2]
values = {
    "@DISPLAY_NAME@": escape(__import__("os").environ["DISPLAY_NAME"]),
    "@EXECUTABLE@": escape(__import__("os").environ["EXECUTABLE"]),
    "@BUNDLE_ID@": escape(__import__("os").environ["BUNDLE_ID"]),
    "@SHORT_VERSION@": escape(__import__("os").environ["SHORT_VERSION"]),
    "@BUILD_VERSION@": escape(__import__("os").environ["BUILD_VERSION"]),
    "@MIN_MACOS@": escape(__import__("os").environ["MIN_MACOS"]),
    "@COPYRIGHT@": escape(__import__("os").environ["COPYRIGHT"]),
}
text = open(template, encoding="utf-8").read()
for key, value in values.items():
    text = text.replace(key, value)
leftover = [k for k in values if k in text]
if leftover:
    sys.exit(f"unsubstituted placeholders: {leftover}")
open(destination, "w", encoding="utf-8").write(text)
PY

plutil -lint "$APP/Contents/Info.plist"

# GPL-3.0 obligations travel with the payload, not with the repository: an Actions
# artefact on a public repo is a distribution (ADR-0008 §3).
install -m 0644 "$ROOT/LICENSE" "$APP/Contents/Resources/LICENSE.txt"
install -m 0644 "$ROOT/NOTICE.md" "$APP/Contents/Resources/NOTICE.md"

COMMIT="$(git -C "$ROOT" rev-parse HEAD 2>/dev/null || echo "${GITHUB_SHA:-unknown}")"
REMOTE="$(git -C "$ROOT" config --get remote.origin.url 2>/dev/null || echo "https://github.com/centopw/gokey")"
cat > "$APP/Contents/Resources/SOURCE-OFFER.txt" <<EOF
Corresponding source for this build
===================================

Application:  $DISPLAY_NAME ($APP_NAME.app)
Version:      $SHORT_VERSION ($BUILD_VERSION)
Commit:       $COMMIT
Repository:   $REMOTE
Source archive: ${REMOTE%.git}/archive/$COMMIT.tar.gz

This program is free software under the GNU General Public License version 3.
The complete licence text is in LICENSE.txt next to this file.

Built from the commit above with no local modifications outside that commit.
To rebuild this exact version: git checkout $COMMIT && cargo build --release --locked

Upstream: OpenKey (https://github.com/tuyenvm/OpenKey) by Mai Vu Tuyen, GPL-3.0.
This project is a modified port of the upstream engine, not an official OpenKey
build, and is not endorsed by the upstream authors.
EOF

# Copy the licence files to the top level of the disk image too: a user who
# opens the dmg before installing should not have to mount the bundle to find them.
install -m 0644 "$ROOT/LICENSE" "$STAGE/LICENSE.txt"
install -m 0644 "$ROOT/NOTICE.md" "$STAGE/NOTICE.md"
install -m 0644 "$APP/Contents/Resources/SOURCE-OFFER.txt" "$STAGE/SOURCE-OFFER.txt"
ln -s /Applications "$STAGE/Applications"

echo "==> codesign (ad-hoc, inside-out)"
codesign --force --sign - --options runtime --timestamp=none "$APP/Contents/MacOS/$EXECUTABLE"
codesign --force --sign - --options runtime --timestamp=none "$APP"
codesign --verify --strict --verbose=2 "$APP"
codesign --display --verbose=2 "$APP" 2>&1 | sed 's/^/    /'

DMG="$OUT_DIR/$APP_NAME-$SHORT_VERSION-$BUILD_VERSION.dmg"
echo "==> hdiutil create $DMG"
hdiutil create -quiet -volname "$DISPLAY_NAME" -srcfolder "$STAGE" -fs HFS+ -format UDZO -ov "$DMG"
hdiutil verify "$DMG"
codesign --force --sign - --timestamp=none "$DMG"

# Assert the artefact, not just its existence: a dmg that mounts but lacks the app
# or the licence files is the failure mode this job exists to catch.
echo "==> verifying dmg contents"
MOUNT="$WORK/mnt"
mkdir -p "$MOUNT"
hdiutil attach -quiet -nobrowse -readonly -mountpoint "$MOUNT" "$DMG"
status=0
for path in \
    "$MOUNT/$APP_NAME.app/Contents/MacOS/$EXECUTABLE" \
    "$MOUNT/$APP_NAME.app/Contents/Info.plist" \
    "$MOUNT/$APP_NAME.app/Contents/Resources/LICENSE.txt" \
    "$MOUNT/$APP_NAME.app/Contents/Resources/NOTICE.md" \
    "$MOUNT/$APP_NAME.app/Contents/Resources/SOURCE-OFFER.txt" \
    "$MOUNT/LICENSE.txt" \
    "$MOUNT/NOTICE.md" \
    "$MOUNT/SOURCE-OFFER.txt"
do
    if [ -e "$path" ]; then
        echo "    ok  ${path#"$MOUNT"/}"
    else
        echo "::error::missing from dmg: ${path#"$MOUNT"/}"
        status=1
    fi
done

actual_id="$(plutil -extract CFBundleIdentifier raw "$MOUNT/$APP_NAME.app/Contents/Info.plist" 2>/dev/null || echo "")"
if [ "$actual_id" != "$BUNDLE_ID" ]; then
    echo "::error::CFBundleIdentifier is '$actual_id', expected '$BUNDLE_ID' (DL-022)"
    status=1
fi
actual_display="$(plutil -extract CFBundleDisplayName raw "$MOUNT/$APP_NAME.app/Contents/Info.plist" 2>/dev/null || echo "")"
if [ "$actual_display" != "$DISPLAY_NAME" ]; then
    echo "::error::CFBundleDisplayName is '$actual_display', expected '$DISPLAY_NAME' (DL-022)"
    status=1
fi

hdiutil detach -quiet "$MOUNT" || true
[ "$status" -eq 0 ] || exit "$status"

shasum -a 256 "$DMG" | tee "$DMG.sha256"
echo "==> dmg ready: $DMG"
