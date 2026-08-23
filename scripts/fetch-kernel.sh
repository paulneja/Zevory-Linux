#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="$(tr -d '[:space:]' < "$ROOT/kernel/VERSION")"
SERIES="v$(echo "$VERSION" | cut -d. -f1).x"

BASE_URL="https://cdn.kernel.org/pub/linux/kernel/$SERIES"
TAR_XZ="linux-$VERSION.tar.xz"
SIG="linux-$VERSION.tar.sign"

DL_DIR="$ROOT/sources/_dl"
GNUPGHOME="$ROOT/sources/.gnupg-kernel"
SRC_DIR="$ROOT/sources/linux-$VERSION"

LINUS_KEY="ABAF11C65A2970B130ABE3C479BE3E4300411886"
GREG_KEY="647F28654894E3BD457199BE38DBBDC86092693E"

mkdir -p "$DL_DIR" "$GNUPGHOME"
chmod 700 "$GNUPGHOME"

if [ -d "$SRC_DIR" ]; then
  echo "$SRC_DIR already there, delete it by hand to re-fetch"
  exit 0
fi

echo "importing kernel signing keys (wkd, the keyserver pool is flaky)..."
gpg --homedir "$GNUPGHOME" --auto-key-locate wkd --locate-keys \
  torvalds@kernel.org gregkh@kernel.org

for fpr in "$LINUS_KEY" "$GREG_KEY"; do
  gpg --homedir "$GNUPGHOME" --list-keys "$fpr" >/dev/null 2>&1 || {
    echo "ERROR: fingerprint $fpr did not show up, stopping here" >&2
    exit 1
  }
done

echo "fetching $TAR_XZ ($SERIES)..."
curl -fL --retry 5 --retry-delay 3 --retry-all-errors -o "$DL_DIR/$TAR_XZ" "$BASE_URL/$TAR_XZ"
curl -fL --retry 5 --retry-delay 3 --retry-all-errors -o "$DL_DIR/$SIG" "$BASE_URL/$SIG"

xz -dk "$DL_DIR/$TAR_XZ"

echo "checking signature..."
gpg --homedir "$GNUPGHOME" --verify "$DL_DIR/$SIG" "$DL_DIR/linux-$VERSION.tar"

echo "signature ok, extracting..."
mkdir -p "$ROOT/sources"
tar -xf "$DL_DIR/linux-$VERSION.tar" -C "$ROOT/sources"
rm -f "$DL_DIR/linux-$VERSION.tar"

echo "done: $SRC_DIR"
