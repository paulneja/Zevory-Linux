#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
set -euo pipefail

LOG="${1:?usage: diagnose-boot.sh <boot-log>}"
[ -f "$LOG" ] || { echo "ERROR: $LOG does not exist" >&2; exit 1; }

fail=0

check() {
  local desc="$1" pattern="$2"
  if grep -q -- "$pattern" "$LOG"; then
    echo "OK   - $desc"
  else
    echo "FAIL - $desc"
    fail=1
  fi
}

echo "diagnosing $LOG"
echo ""

if ! grep -q "Linux version" "$LOG"; then
  echo "FAIL - the kernel never started, so the rest would just be noise"
  echo ""
  if [ -s "$LOG" ]; then
    echo "       first lines of the log:"
    head -n 10 "$LOG" | sed 's/^/         /'
  else
    echo "       $LOG is empty. is qemu-system-x86_64 installed?"
  fi
  echo ""
  echo "result: FAIL"
  exit 1
fi

if grep -q "Kernel panic" "$LOG"; then
  echo "FAIL - kernel panic:"
  grep -A6 "Kernel panic" "$LOG" | sed 's/^/       /'
  fail=1
fi

check "kernel boots"        "Linux version"
check "initramfs found /init" "Run /init as init process"
check "/init done, shell up" "bootstrap OK, shell ready"

echo ""
if [ "$fail" -eq 0 ]; then
  echo "result: PASS"
else
  echo "result: FAIL"
fi
exit "$fail"
