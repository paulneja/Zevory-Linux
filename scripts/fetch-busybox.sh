#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

CURL=(curl -fL --retry 5 --retry-delay 3 --retry-all-errors)
# without a tty the progress bar turns a build log into a wall of noise
[ -t 1 ] || CURL+=(-sS)
VERSION="$(tr -d '[:space:]' < "$ROOT/busybox/VERSION")"
FPR="C9E9416F76E610DBD09D040F47B70C55ACC9965B"

BASE_URL="https://busybox.net/downloads"
TAR="busybox-$VERSION.tar.bz2"
SIG="busybox-$VERSION.tar.bz2.sig"

DL_DIR="$ROOT/sources/_dl"
GNUPGHOME="$ROOT/sources/.gnupg-busybox"
SRC_DIR="$ROOT/sources/busybox-$VERSION"

mkdir -p "$DL_DIR" "$GNUPGHOME"
chmod 700 "$GNUPGHOME"

if [ -d "$SRC_DIR" ]; then
  echo "$SRC_DIR already there, delete it by hand to re-fetch"
  exit 0
fi

gpg --homedir "$GNUPGHOME" --import "$ROOT/busybox/signing-key.asc"
gpg --homedir "$GNUPGHOME" --list-keys "$FPR" >/dev/null 2>&1 || {
  echo "ERROR: vendored key does not match the expected fingerprint" >&2
  exit 1
}

echo "fetching $TAR..."
"${CURL[@]}" -o "$DL_DIR/$TAR" "$BASE_URL/$TAR"
"${CURL[@]}" -o "$DL_DIR/$SIG" "$BASE_URL/$SIG"

gpg --homedir "$GNUPGHOME" --verify "$DL_DIR/$SIG" "$DL_DIR/$TAR"

mkdir -p "$ROOT/sources"
tar -xf "$DL_DIR/$TAR" -C "$ROOT/sources"
rm -f "$DL_DIR/$TAR" "$DL_DIR/$SIG"

echo "done: $SRC_DIR"
