#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="$(tr -d '[:space:]' < "$ROOT/boot/limine/VERSION")"

FPR="05D29860D0A0668AAEFB9D691F3C021BECA23821"
BASE_URL="https://github.com/limine-bootloader/limine/releases/download/v$VERSION"
TAR="limine-binary.tar.gz"
SIG="limine-binary.tar.gz.sig"

DL_DIR="$ROOT/sources/_dl"
GNUPGHOME="$ROOT/sources/.gnupg-limine"
OUT_DIR="$ROOT/sources/limine-$VERSION"

mkdir -p "$DL_DIR" "$GNUPGHOME"
chmod 700 "$GNUPGHOME"

if [ -d "$OUT_DIR" ]; then
  echo "$OUT_DIR already there, delete it by hand to re-fetch"
  exit 0
fi

gpg --homedir "$GNUPGHOME" --import "$ROOT/boot/limine/signing-key.asc"
gpg --homedir "$GNUPGHOME" --list-keys "$FPR" >/dev/null 2>&1 || {
  echo "ERROR: vendored key does not match the expected fingerprint" >&2
  exit 1
}

echo "fetching Limine $VERSION binaries..."
curl -fL --retry 5 --retry-delay 3 --retry-all-errors -o "$DL_DIR/$TAR" "$BASE_URL/$TAR"
curl -fL --retry 5 --retry-delay 3 --retry-all-errors -o "$DL_DIR/$SIG" "$BASE_URL/$SIG"

gpg --homedir "$GNUPGHOME" --verify "$DL_DIR/$SIG" "$DL_DIR/$TAR"

mkdir -p "$OUT_DIR"
tar -xf "$DL_DIR/$TAR" -C "$OUT_DIR"
rm -f "$DL_DIR/$TAR" "$DL_DIR/$SIG"

echo "done: $OUT_DIR"
