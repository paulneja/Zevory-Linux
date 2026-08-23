#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC="$ROOT/build/initramfs/root"
OUT="$ROOT/build/initramfs.cpio.zst"

if [ ! -d "$SRC" ]; then
  echo "ERROR: $SRC missing, run scripts/build-initramfs-root.sh first" >&2
  exit 1
fi

cd "$SRC"
find . | cpio -o -H newc --owner=0:0 | zstd -19 -T0 -f -o "$OUT"

echo "done: $OUT"
