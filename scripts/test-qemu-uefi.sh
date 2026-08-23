#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ISO="$ROOT/build/zevory-dev.iso"
LOG="$ROOT/build/test-uefi.log"
VARS="$ROOT/build/OVMF_VARS.test.fd"

OVMF_CODE="${OVMF_CODE:-/usr/share/edk2/x64/OVMF_CODE.4m.fd}"
OVMF_VARS_TEMPLATE="${OVMF_VARS_TEMPLATE:-/usr/share/edk2/x64/OVMF_VARS.4m.fd}"

[ -f "$ISO" ] || { echo "ERROR: $ISO missing, run scripts/build-iso.sh first" >&2; exit 1; }
[ -f "$OVMF_CODE" ] || { echo "ERROR: $OVMF_CODE missing (install edk2-ovmf, or point \$OVMF_CODE somewhere else)" >&2; exit 1; }

cp "$OVMF_VARS_TEMPLATE" "$VARS"

timeout 40 qemu-system-x86_64 \
  -drive if=pflash,format=raw,readonly=on,file="$OVMF_CODE" \
  -drive if=pflash,format=raw,file="$VARS" \
  -cdrom "$ISO" \
  -nographic -no-reboot -enable-kvm -cpu host -m 512 \
  < /dev/null > "$LOG" 2>&1 || true

"$ROOT/scripts/diagnose-boot.sh" "$LOG"
