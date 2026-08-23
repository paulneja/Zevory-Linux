#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="$(tr -d '[:space:]' < "$ROOT/busybox/VERSION")"
SRC_DIR="$ROOT/sources/busybox-$VERSION"
BUILD_DIR="$ROOT/build/busybox"
JOBS="${JOBS:-$(nproc)}"

if [ ! -f "$BUILD_DIR/.config" ]; then
  echo "ERROR: no .config in $BUILD_DIR, run scripts/configure-busybox.sh first" >&2
  exit 1
fi

make -C "$SRC_DIR" O="$BUILD_DIR" CC=musl-gcc -j"$JOBS"

echo "done: $BUILD_DIR/busybox"
