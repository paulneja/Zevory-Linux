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

  # zevinit
  rustup
)

echo "installing: ${PKGS[*]}"

# -Syu and not -S: on a rolling distro installing against a stale database is
# how you end up with a half upgraded system. it does mean this upgrades yours
ARGS=(-Syu --needed)
# no tty means CI or a container, where stopping on a y/n just hangs
[ -t 0 ] || ARGS+=(--noconfirm)

# in a container or an install chroot you are already root and sudo is usually
# not even installed, so reaching for it there just breaks the script
if [ "$(id -u)" -eq 0 ]; then
  pacman "${ARGS[@]}" "${PKGS[@]}"
else
  sudo pacman "${ARGS[@]}" "${PKGS[@]}"
fi

# zevinit links static against musl for the same reason busybox does. rust ships
# its own musl for this target, so the musl package above is for busybox, not
# for this
if ! rustup show active-toolchain >/dev/null 2>&1; then
  echo "no rust toolchain yet, installing stable"
  rustup default stable
fi

echo "adding the rust target zevinit builds against"
rustup target add x86_64-unknown-linux-musl
