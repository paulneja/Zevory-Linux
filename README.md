# zevory-system

Kernel, initramfs, filesystem, boot, and install backend for
[Zevory Linux](https://zevory.duckdns.org).

Stage 1 (bootstrap) works: firmware to Limine to kernel to initramfs to a shell, on QEMU
(BIOS and UEFI), VirtualBox (BIOS and EFI), and real hardware.

Pinned upstream: Linux 7.1.9, BusyBox 1.38.0 (static against musl), Limine 12.6.0. All three
are fetched from upstream and checked against pinned PGP fingerprints.

---

## What lives here

Kernel config, initramfs, filesystem, boot, and the install backend. Package management goes
in `zevory-packages`; our own userland bits like ZevInit and ZevPkg go in `zevory-core`,
admin tools in `zevory-tools`, and CI/repo infra in `zevory-infra`.

The repo only tracks the reproducible parts: version pins, config fragments, vendored public
keys, and the scripts. Everything else is generated:

- `sources/` — upstream tarballs, untouched and PGP-checked. gitignored.
- `build/` — everything we generate, out of tree. gitignored.

---

## Building

Install the host packages first, once, and this needs sudo:

```sh
./scripts/deps.sh
```

Then run the pipeline in this order:

```sh
./scripts/fetch-kernel.sh          # tarball + PGP check, lands in sources/
./scripts/configure-kernel.sh      # x86_64_defconfig + kernel/zevory.config
./scripts/build-kernel.sh          # build/kernel/arch/x86/boot/bzImage

./scripts/fetch-busybox.sh
./scripts/configure-busybox.sh     # static, built against musl
./scripts/build-busybox.sh         # build/busybox/busybox

./scripts/build-initramfs-root.sh  # tree + applet symlinks + /init
./scripts/build-initramfs.sh       # build/initramfs.cpio.zst

./scripts/fetch-limine.sh
./scripts/build-iso.sh             # build/zevory-dev.iso
```

Every script tells you what it needs if you run it out of order, so it's hard to get wrong.

---

## Testing

```sh
./scripts/test-qemu-bios.sh
./scripts/test-qemu-uefi.sh
```

Both boot the ISO, capture the serial console, and pass the log to `diagnose-boot.sh`, which
checks the boot stage by stage and exits 0 or 1. That script also works on its own against any
boot log:

```sh
./scripts/diagnose-boot.sh build/test-bios.log
```

`deps.sh` pulls in the OVMF firmware the UEFI test needs. If yours lives somewhere else, point
`$OVMF_CODE` and `$OVMF_VARS_TEMPLATE` at it.

Without KVM both tests still run, they just drop to TCG and take a couple of minutes instead of
seconds. Set `$TIMEOUT` if your machine needs longer.

One gotcha: VirtualBox and KVM fight over VT-x. If a VirtualBox VM is running, QEMU with
`-enable-kvm` dies with `KVM: entry failed, hardware error 0x0`. Not our bug.

---

## Real hardware

```sh
lsblk                                  # find the stick, read it twice
sudo dd if=build/zevory-dev.iso of=/dev/sdX bs=4M status=progress conv=fsync
```

`/dev/sdX` is the USB stick, not your disk. Writing to the wrong device wipes it.

The ISO is live. `/init` only mounts virtual filesystems (proc, sysfs, devtmpfs, tmpfs)
and never touches a disk, so booting it changes nothing on the machine.

---

## Known issues

Ctrl+C does not interrupt in the initramfs shell. Ctrl+Z suspends fine, so job control is
half working. Doesn't block anything for now.

---

## License

Our own code here (scripts, config fragments, `/init`) is GPL-3.0-or-later, see `LICENSE`.

What we ship inside the ISO keeps its own license: the kernel and BusyBox are GPL-2.0-only,
Limine is BSD-2-Clause, and musl (statically linked into BusyBox) is MIT. Those live next to
our stuff as separate works, so there is no conflict, but handing the ISO to anyone means the
GPLv2 source offer applies to the kernel and BusyBox.



P.S. Technically, the project is further along in terms of completed stages and tasks, but we’re still testing, fixing, implementing, and improving things. More updates are coming soon 😛😛
