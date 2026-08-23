#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
set -euo pipefail

PKGS=(
  base-devel  
  git
  bc
  flex
  bison
  openssl      
  elfutils    
  pahole       
  ncurses     
  cpio         
  xz
  zstd
  musl         
  nasm         
  libisoburn   
  mtools       
)

echo "installing: ${PKGS[*]}"
sudo pacman -S --needed "${PKGS[@]}"
