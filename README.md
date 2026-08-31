# Zevory-Linux

Kernel, initramfs, filesystem, boot, and install backend for
[Zevory Linux](https://zevory.duckdns.org).

Stage 1 (bootstrap) works: firmware to Limine to kernel to initramfs to a shell, on QEMU
(BIOS and UEFI), VirtualBox (BIOS and EFI), and real hardware.

Stage 2 is most of the way there. `/init` is no longer a shell script: it is ZevInit, our own
pid 1, in `zevinit/`. See [zevinit/README.md](zevinit/README.md) for what it does and, more
usefully, what it refuses to do. It has not run on real hardware yet.

Pinned upstream: Linux 7.1.9, BusyBox 1.38.0 (static against musl), Limine 12.6.0. All three
are fetched from upstream and checked against pinned PGP fingerprints.

---

## What lives here

Kernel config, initramfs, filesystem, boot, the install backend, and for now ZevInit in
`zevinit/`. The long-term topology puts our own userland in `zevory-core`, package management
in `zevory-packages`, admin tools in `zevory-tools` and CI in `zevory-infra`. ZevInit sits here
instead because splitting a repo for one crate that nothing else depends on yet would cost more
than it buys; everything it needs is inside `zevinit/`, so moving it later is cheap.

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

./scripts/build-initramfs-root.sh  # tree + applet symlinks + zevinit as /init
./scripts/build-initramfs.sh       # build/initramfs.cpio.zst

./scripts/fetch-limine.sh
./scripts/build-iso.sh             # build/zevory-dev.iso
```

Every script tells you what it needs if you run it out of order, so it's hard to get wrong.
`build-initramfs-root.sh` is the one exception: it builds ZevInit itself rather than telling you
to, because a stale `/init` in the image is the kind of thing you debug for an hour. That build
needs the `x86_64-unknown-linux-musl` Rust target, which `deps.sh` installs.

---

## Testing

```sh
./scripts/test-qemu-bios.sh
./scripts/test-qemu-uefi.sh
./scripts/test-qemu-zevinit.sh
```

The first two boot the ISO, capture the serial console, and pass the log to `diagnose-boot.sh`,
which checks the boot stage by stage and exits 0 or 1. That script also works on its own against
any boot log:

```sh
./scripts/diagnose-boot.sh build/test-bios.log
```

The third one boots the kernel and initramfs directly and drives the shell over the serial
line, checking what ZevInit actually does: shutdown, reboot, halt, Ctrl+Alt+Del, orphan reaping,
the fallback to a second shell, and parking when there is no shell at all. Pass a case name to
run one of them, for example `./scripts/test-qemu-zevinit.sh reboot`.

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

The ISO is live. ZevInit only mounts virtual filesystems (proc, sysfs, devtmpfs, tmpfs) and
never touches a block device, so booting it changes nothing on the machine.

---

## Known issues

None blocking. Ctrl+C used to do nothing in the initramfs shell; ZevInit gives the shell its own
session and the controlling terminal, so job control works now.

Two things worth knowing: ZevInit has never booted on real hardware, and the Rust toolchain is
not pinned, so two machines on different `rustc` versions build different bytes from the same
source. Both are written up in [zevinit/README.md](zevinit/README.md).

---

## License

Our own code here (scripts, config fragments, and ZevInit) is GPL-3.0-or-later, see `LICENSE`.

What we ship inside the ISO keeps its own license: the kernel and BusyBox are GPL-2.0-only,
Limine is BSD-2-Clause, and musl (statically linked into BusyBox) is MIT. Those live next to
our stuff as separate works, so there is no conflict, but handing the ISO to anyone means the
GPLv2 source offer applies to the kernel and BusyBox.



P.S. Technically, the project is further along in terms of completed stages and tasks, but we’re still testing, fixing, implementing, and improving things. More updates are coming soon 😛😛
