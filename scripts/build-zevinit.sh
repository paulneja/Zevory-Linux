#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CRATE="$ROOT/zevinit"
TARGET="x86_64-unknown-linux-musl"
OUT="$CRATE/target/$TARGET/release/zevinit"

command -v cargo >/dev/null || { echo "ERROR: cargo not found, run scripts/deps.sh" >&2; exit 1; }

cd "$CRATE"
cargo build --release --target "$TARGET"

if ! file "$OUT" | grep -q 'static-pie linked'; then
  echo "ERROR: $OUT is not static, it would not run as pid 1" >&2
  file "$OUT" >&2
  exit 1
fi

echo "done: $OUT"
