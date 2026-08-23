#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ISO="$ROOT/build/zevory-dev.iso"
LOG="$ROOT/build/test-bios.log"

[ -f "$ISO" ] || { echo "ERROR: $ISO missing, run scripts/build-iso.sh first" >&2; exit 1; }
[ -c /dev/kvm ] || echo "warning: no /dev/kvm, running without acceleration" >&2


timeout 30 qemu-system-x86_64 \
  -cdrom "$ISO" -boot d \
  -nographic -no-reboot -enable-kvm -cpu host -m 512 \
  < /dev/null > "$LOG" 2>&1 || true

"$ROOT/scripts/diagnose-boot.sh" "$LOG"
