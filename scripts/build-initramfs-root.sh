#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BB="$ROOT/build/busybox/busybox"
OUT="$ROOT/build/initramfs/root"

if [ ! -x "$BB" ]; then
  echo "ERROR: $BB missing, run scripts/build-busybox.sh first" >&2
  exit 1
fi

rm -rf "$OUT"
mkdir -p "$OUT"/bin "$OUT"/dev "$OUT"/proc "$OUT"/sys "$OUT"/etc "$OUT"/tmp "$OUT"/root "$OUT"/run "$OUT"/mnt
ln -s bin "$OUT/sbin"
mkdir -p "$OUT/usr"
ln -s ../bin "$OUT/usr/bin"
ln -s ../bin "$OUT/usr/sbin"

cp "$BB" "$OUT/bin/busybox"
cd "$OUT/bin"

for applet in $(./busybox --list); do
  [ "$applet" = busybox ] || ln -sf busybox "$applet"
done

cp "$ROOT/initramfs/init" "$OUT/init"
chmod +x "$OUT/init"

echo "done: $OUT"
