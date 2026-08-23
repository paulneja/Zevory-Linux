#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
set -euo pipefail

PKGS=(
  base-devel
  git
  bc
  openssl
  elfutils
  pahole
  ncurses

  curl
  gnupg
  tar
  bzip2

  cpio
  xz
  zstd
  musl
  libisoburn
  mtools

  qemu-system-x86
  edk2-ovmf

  rustup
)

install() {
  if [ "$(id -u)" -eq 0 ]; then
    pacman "$@" "${PKGS[@]}"
  else
    sudo pacman "$@" "${PKGS[@]}"
  fi
}

echo "installing: ${PKGS[*]}"

ARGS=(-S --needed)
[ -t 0 ] || ARGS+=(--noconfirm)

if ! install "${ARGS[@]}"; then
  echo ""
  echo "pacman could not install that. the usual cause is a package database"
  echo "older than the mirrors, and -Syu is what fixes it. that upgrades the"
  echo "whole system though, which on a rolling distro is the normal thing to"
  echo "do but is still your call, not this script's."
  echo ""
  if [ -t 0 ]; then
    read -r -p "run pacman -Syu now? [y/N] " answer
    case "$answer" in
      [yY]*) install -Syu --needed ;;
      *) echo "nothing was installed. run 'sudo pacman -Syu' and try again."; exit 1 ;;
    esac
  else
    echo "no tty here, so nothing to ask. run 'pacman -Syu' and try again."
    exit 1
  fi
fi

if ! rustup show active-toolchain >/dev/null 2>&1; then
  echo "no rust toolchain yet, installing stable"
  rustup default stable
fi

echo "adding the rust target zevinit builds against"
rustup target add x86_64-unknown-linux-musl
