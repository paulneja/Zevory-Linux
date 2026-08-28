#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
KERNEL="$ROOT/build/kernel/arch/x86/boot/bzImage"
ROOTFS="$ROOT/build/initramfs/root"
WORK="$ROOT/build/zevinit-tests"

[ -f "$KERNEL" ] || { echo "ERROR: $KERNEL missing, run scripts/build-kernel.sh first" >&2; exit 1; }
[ -x "$ROOTFS/init" ] || {
  echo "ERROR: $ROOTFS/init missing, run scripts/build-initramfs-root.sh first" >&2; exit 1; }
command -v qemu-system-x86_64 >/dev/null || {
  echo "ERROR: qemu-system-x86_64 not found, run scripts/deps.sh" >&2; exit 1; }

if [ -r /dev/kvm ] && [ -w /dev/kvm ]; then
  ACCEL=(-enable-kvm -cpu host)
  SLOW=1
else
  echo "warning: no usable /dev/kvm, falling back to tcg. this will take a few minutes" >&2
  ACCEL=(-cpu max)
  SLOW=5
fi

rm -rf "$WORK"
mkdir -p "$WORK"

pack() {
  local dir="$1" out="$2"
  ( cd "$dir" && find . | cpio -o -H newc --owner=0:0 2>/dev/null | zstd -3 -T0 -f -q -o "$out" )
}

prepare_images() {
  pack "$ROOTFS" "$WORK/normal.cpio.zst"

  cp -a "$ROOTFS" "$WORK/broken-sh"
  rm -f "$WORK/broken-sh/bin/sh"
  printf 'not an executable\n' > "$WORK/broken-sh/bin/sh"
  chmod 755 "$WORK/broken-sh/bin/sh"
  pack "$WORK/broken-sh" "$WORK/broken-sh.cpio.zst"

  cp -a "$ROOTFS" "$WORK/no-shell"
  rm -f "$WORK/no-shell/bin/sh" "$WORK/no-shell/bin/busybox"
  pack "$WORK/no-shell" "$WORK/no-shell.cpio.zst"
}

boot_driving_the_shell() {
  local image="$1" log="$2" settle="$3" linger="$4" script="$5"
  ( sleep "$((settle * SLOW))"; cat "$script"; sleep "$((linger * SLOW))" ) \
    | timeout "$(( (settle + linger + 20) * SLOW ))" qemu-system-x86_64 \
      -kernel "$KERNEL" -initrd "$image" \
      -append "console=tty0 console=ttyS0 loglevel=7" \
      -nographic -no-reboot "${ACCEL[@]}" -m 512 > "$log" 2>&1
}

boot_pressing_ctrl_alt_del() {
  local image="$1" log="$2" settle="$3"
  ( sleep "$((settle * SLOW))"; echo "sendkey ctrl-alt-delete"; sleep "$((6 * SLOW))"; echo quit ) \
    | timeout "$(( (settle + 20) * SLOW ))" qemu-system-x86_64 \
      -kernel "$KERNEL" -initrd "$image" \
      -append "console=tty0 console=ttyS0 loglevel=7" \
      -serial "file:$log" -display none -monitor stdio \
      -no-reboot "${ACCEL[@]}" -m 512 >/dev/null 2>&1
}

fail=0
current=""

case_begin() {
  current="$1"
  printf '\n%s\n' "$current"
}

booted() {
  local log="$1"
  if ! grep -q "Linux version" "$log"; then
    echo "  FAIL - the kernel never started, so nothing below would mean anything"
    [ -s "$log" ] && head -n 5 "$log" | sed 's/^/         /'
    fail=1
    return 1
  fi
  if grep -q "Kernel panic" "$log"; then
    echo "  FAIL - kernel panic"
    grep -A4 "Kernel panic" "$log" | sed 's/^/         /'
    fail=1
    return 1
  fi
  return 0
}

expect() {
  local desc="$1" log="$2" pattern="$3"
  if grep -qE -- "$pattern" "$log"; then
    echo "  OK   - $desc"
  else
    echo "  FAIL - $desc"
    fail=1
  fi
}

refuse() {
  local desc="$1" log="$2" pattern="$3"
  if grep -qE -- "$pattern" "$log"; then
    echo "  FAIL - $desc"
    fail=1
  else
    echo "  OK   - $desc"
  fi
}

test_the_shell_belongs_to_pid_one() {
  case_begin "a shell comes up owned by pid 1"
  local log="$WORK/ownership.log"
  cat > "$WORK/ownership.in" <<'EOF'
echo "PID1=$(cat /proc/1/comm) PPID=$(cut -d' ' -f4 /proc/$$/stat) TTY=$(readlink /proc/$$/fd/0)"
poweroff -f
EOF
  boot_driving_the_shell "$WORK/normal.cpio.zst" "$log" 6 4 "$WORK/ownership.in"
  booted "$log" || return
  expect "zevinit runs as pid 1"            "$log" "PID1=init"
  expect "the shell is a child of pid 1"    "$log" "PPID=1"
  expect "the shell owns a terminal"        "$log" "TTY=/dev/console"
  expect "the ready marker is logged"       "$log" "bootstrap OK, shell ready"
}

test_the_shell_can_receive_every_signal() {
  case_begin "pid 1 keeps its signals to itself and the shell can receive all of them"
  local log="$WORK/masks.log"
  cat > "$WORK/masks.in" <<'EOF'
echo "PID1BLK=$(grep SigBlk /proc/1/status | awk '{print $2}')"
echo "SHELLBLK=$(grep SigBlk /proc/$$/status | awk '{print $2}')"
poweroff -f
EOF
  boot_driving_the_shell "$WORK/normal.cpio.zst" "$log" 6 4 "$WORK/masks.in"
  booted "$log" || return
  refuse "pid 1 blocks the signals it watches"     "$log" "PID1BLK=0000000000000000"
  expect "the shell ends up blocking nothing"      "$log" "SHELLBLK=0000000000000000"
}

test_orphans_are_reaped_without_leaking() {
  case_begin "orphans are reaped and pid 1 does not grow"
  local log="$WORK/orphans.log"
  cat > "$WORK/orphans.in" <<'EOF'
BEFORE=$(awk '/VmRSS/{print $2}' /proc/1/status)
i=0; while [ $i -lt 300 ]; do sh -c '(exit 0) &' >/dev/null 2>&1; i=$((i+1)); done
sleep 3
echo "REAPED=$(dmesg | grep -c 'reaped orphan') ZOMBIES=$(ps -o stat | grep -c Z)"
AFTER=$(awk '/VmRSS/{print $2}' /proc/1/status)
echo "GREW=$((AFTER - BEFORE))k FDS=$(ls /proc/1/fd | wc -l)"
poweroff -f
EOF
  boot_driving_the_shell "$WORK/normal.cpio.zst" "$log" 6 10 "$WORK/orphans.in"
  booted "$log" || return
  expect "every orphan is reaped"          "$log" "REAPED=300"
  expect "no zombies are left behind"      "$log" "ZOMBIES=0"
  expect "pid 1 does not grow"             "$log" "GREW=0k"
  refuse "pid 1 does not leak descriptors" "$log" "FDS=(1[0-9]|[2-9][0-9])"
}

test_a_shutdown_request_brings_the_machine_down() {
  local command="$1" expected="$2"
  case_begin "$command hands the machine to the kernel"
  local log="$WORK/$command.log"
  printf '%s\n' "$command" > "$WORK/$command.in"
  boot_driving_the_shell "$WORK/normal.cpio.zst" "$log" 6 14 "$WORK/$command.in"
  booted "$log" || return
  expect "zevinit sees the request"      "$log" "zevinit: $command requested"
  expect "the console says what is up"   "$log" "going down for $command"
  expect "the kernel is asked to $command" "$log" "asking the kernel to $command"
  expect "the machine actually goes down"  "$log" "$expected"
  refuse "nothing had to be forced"        "$log" "outlived SIGKILL"
}

test_ctrl_alt_del_goes_through_zevinit() {
  case_begin "ctrl+alt+del reboots through zevinit, not behind its back"
  local log="$WORK/cad.log"
  boot_pressing_ctrl_alt_del "$WORK/normal.cpio.zst" "$log" 8
  booted "$log" || return
  expect "zevinit reads it as a reboot"  "$log" "zevinit: reboot requested"
  expect "the machine restarts"          "$log" "reboot: Restarting system"
}

test_a_broken_shell_falls_back_to_the_next_one() {
  case_begin "a shell that will not start falls back to the next candidate"
  local log="$WORK/fallback.log"
  cat > "$WORK/fallback.in" <<'EOF'
echo "ALIVE=$(cat /proc/1/comm) SHELLPPID=$(cut -d' ' -f4 /proc/$$/stat)"
poweroff -f
EOF
  boot_driving_the_shell "$WORK/broken-sh.cpio.zst" "$log" 12 4 "$WORK/fallback.in"
  booted "$log" || return
  expect "the broken shell is reported"   "$log" "could not exec /bin/sh"
  expect "zevinit moves to the next one"  "$log" "falling back to /bin/busybox"
  expect "a working shell comes up"       "$log" "ALIVE=init"
  expect "and it belongs to pid 1"        "$log" "SHELLPPID=1"
  local shown
  shown=$(grep -c 'could not exec /bin/sh' "$log")
  if [ "$shown" -eq 3 ]; then
    echo "  OK   - each failure is reported exactly once"
  else
    echo "  FAIL - each failure is reported exactly once, saw $shown lines for 3 attempts"
    fail=1
  fi
}

test_with_no_shell_at_all_zevinit_parks() {
  case_begin "with no shell at all zevinit parks instead of dying"
  local log="$WORK/park.log"
  boot_pressing_ctrl_alt_del "$WORK/no-shell.cpio.zst" "$log" 8
  booted "$log" || return
  expect "zevinit says what is wrong"        "$log" "no shell in the initramfs"
  expect "and how to get out"                "$log" "press ctrl\+alt\+del to reboot"
  expect "ctrl+alt+del still works there"    "$log" "reboot: Restarting system"
  refuse "pid 1 never dies"                  "$log" "Attempted to kill init"
}

test_log_lines_carry_a_priority_and_survive() {
  case_begin "log lines carry a priority and none get dropped"
  local log="$WORK/logging.log"
  cat > "$WORK/logging.in" <<'EOF'
echo "RATELIMIT=$(cat /proc/sys/kernel/printk_devkmsg)"
echo "INFO=$(dmesg -r | grep -c '^<14>.*zevinit:') ERRS=$(dmesg -r | grep -c '^<11>.*zevinit:')"
poweroff -f
EOF
  boot_driving_the_shell "$WORK/normal.cpio.zst" "$log" 6 6 "$WORK/logging.in"
  booted "$log" || return
  expect "the kernel stops rate limiting us" "$log" "RATELIMIT=on"
  refuse "our notes land at info priority"   "$log" "INFO=0"
  expect "a clean boot logs no errors"       "$log" "ERRS=0"
}

WANTED="${1:-all}"
run() {
  case "$WANTED" in
    all|"$1") shift; "$@" ;;
  esac
}

echo "booting zevinit under qemu, $(( SLOW == 1 ? 0 : 1 )) means no kvm: SLOW=$SLOW"
prepare_images

run ownership  test_the_shell_belongs_to_pid_one
run masks      test_the_shell_can_receive_every_signal
run orphans    test_orphans_are_reaped_without_leaking
run reboot     test_a_shutdown_request_brings_the_machine_down reboot "reboot: Restarting system"
run poweroff   test_a_shutdown_request_brings_the_machine_down poweroff "reboot: Power down"
run halt       test_a_shutdown_request_brings_the_machine_down halt "reboot: System halted"
run cad        test_ctrl_alt_del_goes_through_zevinit
run fallback   test_a_broken_shell_falls_back_to_the_next_one
run park       test_with_no_shell_at_all_zevinit_parks
run logging    test_log_lines_carry_a_priority_and_survive

echo ""
if [ "$fail" -eq 0 ]; then
  echo "result: PASS"
else
  echo "result: FAIL"
fi
exit "$fail"
