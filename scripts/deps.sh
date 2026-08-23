#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
set -euo pipefail

PKGS=(
  # kernel and general build
  base-devel
  git
  bc
  openssl
  elfutils
  pahole
  ncurses

  # fetch and verify. these ride in with pacman anyway, but the fetch scripts
  # call them straight so we list them instead of leaning on that
  curl
  gnupg
  tar
  bzip2

  # initramfs and iso
  cpio
  xz
  zstd
  musl
  libisoburn
  mtools

  # testing
  qemu-system-x86
  edk2-ovmf
)

echo "installing: ${PKGS[*]}"
sudo pacman -S --needed "${PKGS[@]}"
