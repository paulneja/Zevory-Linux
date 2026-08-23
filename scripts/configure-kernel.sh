#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="$(tr -d '[:space:]' < "$ROOT/kernel/VERSION")"
SRC_DIR="$ROOT/sources/linux-$VERSION"
BUILD_DIR="$ROOT/build/kernel"

if [ ! -d "$SRC_DIR" ]; then
  echo "ERROR: $SRC_DIR missing, run scripts/fetch-kernel.sh first" >&2
  exit 1
fi

mkdir -p "$BUILD_DIR"

make -C "$SRC_DIR" O="$BUILD_DIR" ARCH=x86_64 x86_64_defconfig
"$SRC_DIR/scripts/kconfig/merge_config.sh" -O "$BUILD_DIR" -m "$BUILD_DIR/.config" "$ROOT/kernel/zevory.config"
make -C "$SRC_DIR" O="$BUILD_DIR" ARCH=x86_64 olddefconfig

echo "done: $BUILD_DIR/.config"
