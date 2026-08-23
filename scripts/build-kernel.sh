#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="$(tr -d '[:space:]' < "$ROOT/kernel/VERSION")"
SRC_DIR="$ROOT/sources/linux-$VERSION"
BUILD_DIR="$ROOT/build/kernel"
JOBS="${JOBS:-$(nproc)}"

if [ ! -f "$BUILD_DIR/.config" ]; then
  echo "ERROR: no .config in $BUILD_DIR, run scripts/configure-kernel.sh first" >&2
  exit 1
fi

make -C "$SRC_DIR" O="$BUILD_DIR" ARCH=x86_64 -j"$JOBS"

echo "done: $BUILD_DIR/arch/x86/boot/bzImage"
