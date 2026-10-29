#!/usr/bin/env bash
# Fetch the pinned gitleaks binary and verify it against the release checksums.
#
# Usage: scripts/ci/fetch-gitleaks.sh <dest-dir>        -> <dest-dir>/gitleaks
#
# One place for the version and the digests, so the `secrets` job, the nightly history
# sweep and a developer's laptop cannot drift onto different binaries. The version is part of
# the argument in docs/adr/0009-secrets-gate-scope.md: on 8.30.1 a top-level `condition` is
# ignored by `gitleaks dir` and honoured by `gitleaks git`, which is why the shipped config
# attaches its AND to the rules instead. That behaviour is only stable because the binary is
# pinned by digest, and scripts/ci/test_gitleaks_config.py measures it in both modes.
set -euo pipefail

VERSION="8.30.1"
DEST="${1:?usage: fetch-gitleaks.sh <dest-dir>}"

case "$(uname -s)-$(uname -m)" in
  Darwin-arm64)   ASSET="darwin_arm64"; SHA="b40ab0ae55c505963e365f271a8d3846efbc170aa17f2607f13df610a9aeb6a5" ;;
  Darwin-x86_64)  ASSET="darwin_x64";   SHA="dfe101a4db2255fc85120ac7f3d25e4342c3c20cf749f2c20a18081af1952709" ;;
  Linux-x86_64)   ASSET="linux_x64";    SHA="551f6fc83ea457d62a0d98237cbad105af8d557003051f41f3e7ca7b3f2470eb" ;;
  Linux-aarch64|Linux-arm64) ASSET="linux_arm64"; SHA="e4a487ee7ccd7d3a7f7ec08657610aa3606637dab924210b3aee62570fb4b080" ;;
  *)
    echo "fetch-gitleaks: no pinned build for $(uname -s)-$(uname -m);" >&2
    echo "add its digest to this script from gitleaks_${VERSION}_checksums.txt" >&2
    exit 2
    ;;
esac

mkdir -p "$DEST"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

URL="https://github.com/gitleaks/gitleaks/releases/download/v${VERSION}/gitleaks_${VERSION}_${ASSET}.tar.gz"
curl -sSfL -o "$TMP/gitleaks.tar.gz" "$URL"

actual="$(sha256sum "$TMP/gitleaks.tar.gz" 2>/dev/null | cut -d' ' -f1 || shasum -a 256 "$TMP/gitleaks.tar.gz" | cut -d' ' -f1)"
if [ "$actual" != "$SHA" ]; then
  echo "fetch-gitleaks: checksum mismatch for ${ASSET}" >&2
  echo "  expected $SHA" >&2
  echo "  actual   $actual" >&2
  exit 1
fi

tar -xzf "$TMP/gitleaks.tar.gz" -C "$DEST" gitleaks
echo "$("$DEST/gitleaks" version) verified from ${ASSET} (sha256 ${SHA:0:16}…)"
