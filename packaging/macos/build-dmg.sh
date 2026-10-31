#!/usr/bin/env bash
#
# Build an ad-hoc signed .app bundle and .dmg for the M0/M1 artefact channel (K-007, ADR-0008 tier 5).
#
# This is a SPIKE BUNDLE, not the shipping product: the executable it wraps is the
# headless `openkey` CLI. What it exercises is the part that always breaks late —
# bundle layout, Info.plist, the codesign flag path, and hdiutil.
#
# The ad-hoc signature is the *shipping* signature, not a placeholder (ADR-0010,
# D1/D2 2026-10-30: no paid Apple Developer Program, so there is no Developer ID
# certificate and Apple accepts no submission). MAC-006 replaces the payload with
# the InputMethodKit bundle; the signature only changes if a paid account appears,
# which is an ADR change. DL-022 freezes the identifier and the .app filename, so
# neither step renames anything.
#
# Usage:
#   build-dmg.sh --binary <path> --out <dir> [--version X.Y.Z] [--build N]
#                [--tag vX.Y.Z] [--toolchain STRING] [--offer-out <file>]
#
#   --tag         the release tag this build belongs to; written into SOURCE-OFFER.txt
#                 with the commit, and required to match --version (a semver pre-release
#                 suffix such as -alpha.1 is allowed). Omit for an untagged CI build.
#   --toolchain   the Rust version used, as a string (e.g. "rustc 1.99.0 (abc 2026-09-28)").
#                 Defaults to `rustc --version`, so a rebuild instruction names a toolchain.
#   --offer-out   also write SOURCE-OFFER.txt here, for upload next to the dmg.
#
# Environment overrides: APP_NAME, DISPLAY_NAME, BUNDLE_ID, MIN_MACOS, COPYRIGHT.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

BINARY=""
OUT_DIR=""
SHORT_VERSION="0.1.0"
BUILD_VERSION=""
TAG=""
TOOLCHAIN=""
OFFER_OUT=""
while [ $# -gt 0 ]; do
    case "$1" in
        --binary) BINARY="${2:?}"; shift 2 ;;
        --out) OUT_DIR="${2:?}"; shift 2 ;;
        --version) SHORT_VERSION="${2:?}"; shift 2 ;;
        --build) BUILD_VERSION="${2:?}"; shift 2 ;;
        --tag) TAG="${2:?}"; shift 2 ;;
        --toolchain) TOOLCHAIN="${2:?}"; shift 2 ;;
        --offer-out) OFFER_OUT="${2:?}"; shift 2 ;;
        -h|--help) awk 'NR > 1 && /^set -euo/{exit} NR > 1' "${BASH_SOURCE[0]}"; exit 0 ;;
        *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
done

[ -n "$BINARY" ] || { echo "--binary is required" >&2; exit 2; }
[ -f "$BINARY" ] || { echo "binary not found: $BINARY" >&2; exit 2; }
[ -n "$OUT_DIR" ] || { echo "--out is required" >&2; exit 2; }

# The tag and the version are two spellings of one claim; refuse to publish a build
# where they disagree, because SOURCE-OFFER.txt is a legal document and not a log.
# The version itself stays numeric (CFBundleShortVersionString is period-separated
# integers), so a semver pre-release tag like v0.1.0-alpha.1 is tagged from 0.1.0.
if [ -n "$TAG" ]; then
    case "$TAG" in
        "v$SHORT_VERSION"|"v$SHORT_VERSION"-*) ;;
        *)
            echo "--tag $TAG does not match --version $SHORT_VERSION" >&2
            exit 2
            ;;
    esac
fi

if [ -z "$TOOLCHAIN" ]; then
    TOOLCHAIN="$(rustc --version 2>/dev/null || echo unknown)"
fi

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
ARCHS="$(lipo -archs "$BINARY" 2>/dev/null || echo unknown)"

# What a reader needs to reproduce this download, in order of how they would look for it:
# which release, which commit, which machine it runs on, which compiler built it.
if [ -n "$TAG" ]; then
    TAG_LINE="$TAG"
    CHECKOUT_REF="$TAG"
    RELEASE_URL="${REMOTE%.git}/releases/tag/$TAG"
else
    TAG_LINE="(none - untagged CI build)"
    CHECKOUT_REF="$COMMIT"
    RELEASE_URL="(none - untagged CI build)"
fi

if [ "$(printf '%s' "$ARCHS" | wc -w | tr -d ' ')" -gt 1 ]; then
    REBUILD="  cargo build --release --locked -p openkey-cli --target aarch64-apple-darwin
  cargo build --release --locked -p openkey-cli --target x86_64-apple-darwin
  lipo -create -output openkey target/aarch64-apple-darwin/release/openkey target/x86_64-apple-darwin/release/openkey"
else
    REBUILD="  cargo build --release --locked -p openkey-cli"
fi

cat > "$APP/Contents/Resources/SOURCE-OFFER.txt" <<EOF
Corresponding source for this build
===================================

Application:    $DISPLAY_NAME ($APP_NAME.app)
Version:        $SHORT_VERSION ($BUILD_VERSION)
Tag:            $TAG_LINE
Commit:         $COMMIT
Architectures:  $ARCHS
Toolchain:      $TOOLCHAIN
Repository:     $REMOTE
Source archive: ${REMOTE%.git}/archive/$COMMIT.tar.gz
Release:        $RELEASE_URL

This program is free software under the GNU General Public License version 3.
The complete licence text is in LICENSE.txt next to this file.

Built from the revision named above with no local modifications outside it.
To rebuild this exact version:
  git checkout $CHECKOUT_REF
$REBUILD

Upstream: OpenKey (https://github.com/tuyenvm/OpenKey) by Mai Vu Tuyen, GPL-3.0.
This project is a modified port of the upstream engine, not an official OpenKey
build, and is not endorsed by the upstream authors.
EOF

if [ -n "$OFFER_OUT" ]; then
    install -m 0644 "$APP/Contents/Resources/SOURCE-OFFER.txt" "$OFFER_OUT"
fi

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
codesign --verify --strict --verbose=2 "$DMG"

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

# Verify the copy the user actually receives. The check above proves the staging
# bundle; a mount, a copy or a re-pack that dropped the signature would still pass it.
if codesign --verify --strict --verbose=2 "$MOUNT/$APP_NAME.app" 2>&1 | sed 's/^/    /'; then
    echo "    ok  mounted bundle signature verifies"
else
    echo "::error::the bundle inside the dmg does not verify: signature lost between signing and packaging"
    status=1
fi

hdiutil detach -quiet "$MOUNT" || true
[ "$status" -eq 0 ] || exit "$status"

shasum -a 256 "$DMG" | tee "$DMG.sha256"
echo "==> dmg ready: $DMG"
