#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="$(tr -d '[:space:]' < "$ROOT/busybox/VERSION")"
SRC_DIR="$ROOT/sources/busybox-$VERSION"
BUILD_DIR="$ROOT/build/busybox"

if [ ! -d "$SRC_DIR" ]; then
  echo "ERROR: $SRC_DIR missing, run scripts/fetch-busybox.sh first" >&2
  exit 1
fi

mkdir -p "$BUILD_DIR"

make -C "$SRC_DIR" O="$BUILD_DIR" CC=musl-gcc defconfig

sed -i 's/^# CONFIG_STATIC is not set/CONFIG_STATIC=y/' "$BUILD_DIR/.config"
grep -q '^CONFIG_STATIC=y' "$BUILD_DIR/.config" || echo "CONFIG_STATIC=y" >> "$BUILD_DIR/.config"

sed -i 's/^CONFIG_TC=y/# CONFIG_TC is not set/' "$BUILD_DIR/.config"

sed -i 's|^CONFIG_EXTRA_CFLAGS=.*|CONFIG_EXTRA_CFLAGS="-march=x86-64 -idirafter /usr/include"|' "$BUILD_DIR/.config"

make -C "$SRC_DIR" O="$BUILD_DIR" CC=musl-gcc silentoldconfig

echo "done: $BUILD_DIR/.config"
