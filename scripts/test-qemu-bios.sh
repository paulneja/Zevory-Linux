#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ISO="$ROOT/build/zevory-dev.iso"
LOG="$ROOT/build/test-bios.log"

[ -f "$ISO" ] || { echo "ERROR: $ISO missing, run scripts/build-iso.sh first" >&2; exit 1; }
command -v qemu-system-x86_64 >/dev/null || {
  echo "ERROR: qemu-system-x86_64 not found, run scripts/deps.sh" >&2; exit 1; }

# -cpu host flat out refuses to run without kvm, so the flags have to change
# together with it. -c /dev/kvm is not enough of a check either, you also need
# to be in the kvm group or the open fails later
if [ -r /dev/kvm ] && [ -w /dev/kvm ]; then
  ACCEL=(-enable-kvm -cpu host)
  TIMEOUT="${TIMEOUT:-30}"
else
  echo "warning: no usable /dev/kvm, falling back to tcg (much slower)" >&2
  ACCEL=(-cpu max)
  TIMEOUT="${TIMEOUT:-120}"
fi

timeout "$TIMEOUT" qemu-system-x86_64 \
  -cdrom "$ISO" -boot d \
  -nographic -no-reboot "${ACCEL[@]}" -m 512 \
  < /dev/null > "$LOG" 2>&1 || true

"$ROOT/scripts/diagnose-boot.sh" "$LOG"
