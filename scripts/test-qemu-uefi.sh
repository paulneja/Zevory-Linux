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
command -v qemu-system-x86_64 >/dev/null || {
  echo "ERROR: qemu-system-x86_64 not found, run scripts/deps.sh" >&2; exit 1; }

for f in "$OVMF_CODE" "$OVMF_VARS_TEMPLATE"; do
  [ -f "$f" ] || {
    echo "ERROR: $f missing (install edk2-ovmf, or point \$OVMF_CODE and \$OVMF_VARS_TEMPLATE somewhere else)" >&2
    exit 1; }
done

# same story as the bios test, see the comment there
if [ -r /dev/kvm ] && [ -w /dev/kvm ]; then
  ACCEL=(-enable-kvm -cpu host)
  TIMEOUT="${TIMEOUT:-40}"
else
  echo "warning: no usable /dev/kvm, falling back to tcg (much slower)" >&2
  ACCEL=(-cpu max)
  TIMEOUT="${TIMEOUT:-150}"
fi

cp "$OVMF_VARS_TEMPLATE" "$VARS"

timeout "$TIMEOUT" qemu-system-x86_64 \
  -drive if=pflash,format=raw,readonly=on,file="$OVMF_CODE" \
  -drive if=pflash,format=raw,file="$VARS" \
  -cdrom "$ISO" \
  -nographic -no-reboot "${ACCEL[@]}" -m 512 \
  < /dev/null > "$LOG" 2>&1 || true

"$ROOT/scripts/diagnose-boot.sh" "$LOG"
