#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LIMINE_VERSION="$(tr -d '[:space:]' < "$ROOT/boot/limine/VERSION")"
LB="$ROOT/sources/limine-$LIMINE_VERSION/limine-binary"
KERNEL="$ROOT/build/kernel/arch/x86/boot/bzImage"
INITRAMFS="$ROOT/build/initramfs.cpio.zst"
ISOROOT="$ROOT/build/iso-root"
OUT="$ROOT/build/zevory-dev.iso"

for f in "$KERNEL" "$INITRAMFS" "$LB/limine.c" "$ROOT/boot/limine/limine.conf"; do
  [ -e "$f" ] || { echo "ERROR: missing $f" >&2; exit 1; }
done

if [ ! -x "$LB/limine" ]; then
  make -C "$LB"
fi

rm -rf "$ISOROOT"
mkdir -p "$ISOROOT/boot/limine" "$ISOROOT/boot/zevory" "$ISOROOT/EFI/BOOT"

cp "$KERNEL" "$ISOROOT/boot/zevory/vmlinuz"
cp "$INITRAMFS" "$ISOROOT/boot/zevory/initramfs.cpio.zst"
cp "$ROOT/boot/limine/limine.conf" "$ISOROOT/boot/limine/limine.conf"
cp "$LB/limine-bios.sys" "$LB/limine-bios-cd.bin" "$LB/limine-uefi-cd.bin" "$ISOROOT/boot/limine/"
cp "$LB/BOOTX64.EFI" "$ISOROOT/EFI/BOOT/BOOTX64.EFI"
cp "$LB/BOOTIA32.EFI" "$ISOROOT/EFI/BOOT/BOOTIA32.EFI"

xorriso -as mkisofs -R -r -J -b boot/limine/limine-bios-cd.bin \
    -no-emul-boot -boot-load-size 4 -boot-info-table -hfsplus \
    -apm-block-size 2048 --efi-boot boot/limine/limine-uefi-cd.bin \
    -efi-boot-part --efi-boot-image --protective-msdos-label \
    "$ISOROOT" -o "$OUT"

"$LB/limine" bios-install "$OUT"

echo "done: $OUT"
