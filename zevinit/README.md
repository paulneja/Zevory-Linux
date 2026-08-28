# zevinit

PID 1 for Zevory Linux. Rust, linked statically against musl, shipped as `/init` inside the
initramfs. `libc` is its only dependency.

It is deliberately small. Everything below is either something it does today or something it
refuses to do on purpose, and the second list matters more than the first.

---

## What it is responsible for

**Coming up.** Refuses to run when its pid is not 1. Mounts `/proc`, `/sys`, `/dev`, `/run` and
`/tmp`, all `nosuid`, and treats an already-mounted filesystem as success so a second run
changes nothing. Creates `/dev/null`, `/dev/console` and `/dev/tty` when devtmpfs did not.
Points its own stdio at `/dev/console` and takes the terminal.

**Being reachable.** Blocks the signals it cares about and reads them off a `signalfd` instead
of installing handlers, so nothing runs in a context where it cannot allocate. Takes
Ctrl+Alt+Del away from the kernel with `RB_DISABLE_CAD`, so the key arrives as SIGINT and goes
through the shutdown path instead of resetting the machine on the spot.

**Keeping a shell alive.** Starts one, in its own session, owning the terminal, so job control
works. Restarts it when it exits, no faster than once a second. After three deaths in a row it
moves to the next candidate; when the candidates run out it parks with a message saying what
happened, and re-enables Ctrl+Alt+Del on the way so there is still a way out.

**Reaping.** Every child, including orphans the kernel reparents to it. 300 orphans leave the
resident set and the descriptor count where they were.

**Going down.** Hangs up the terminal of each session it opened, then SIGTERM and SIGCONT to
everything, waits up to five seconds while reaping, SIGKILLs whatever is left, syncs, and hands
the machine to the kernel.

**Logging.** Three levels, written to `/dev/kmsg` with the priority the kernel understands, so
`quiet` hides the routine ones and still shows the errors. Falls back to stderr when `/dev/kmsg`
is not there yet. Asks the kernel to stop rate-limiting the device, because that limit is per
open file and a bad boot is exactly when messages start getting dropped.

**Never exiting.** Every path that cannot continue parks instead of returning. PID 1 exiting is
a kernel panic.

---

## What it deliberately does not do

Not "has not got to yet" in every case — some of these belong to other components by design.

| | |
|---|---|
| Service supervision | No dependencies, DAG, parallelism, restart policy, timers, socket or device activation, cgroups, or user services. That is the next stage, and it is what ZevInit is eventually for. |
| Configuration | There is no config file. SIGHUP is recognised and logged, and there is nothing to reread. |
| A logging daemon | It writes lines to the kernel ring buffer. Structured logging, rotation and correlation belong to ZevLog, which is a separate component and does not exist yet. Keeping it out of pid 1 is the point. |
| systemd compatibility | No UnitBridge, no `sd_notify`, no `/run/systemd`. Software that expects those will not find them. |
| Real filesystems | It mounts the virtual ones and nothing else. It never touches a block device, never mounts a root filesystem, never pivots out of the initramfs. |
| Networking, modules, hotplug | None. No udev, no firmware loading, no interface setup. |
| Login | No getty, no PAM, no users. The shell it starts runs as root because the initramfs has nothing else. |
| Remounting read-only on shutdown | Nothing is mounted from a disk, so there is nothing to flush beyond `sync`. This is a gap the moment a real root exists. |

---

## Signals

The mapping is the BusyBox and sysvinit one, because BusyBox is the userland in the initramfs
and `halt`, `poweroff` and `reboot` are its applets.

| signal | sent by | meaning |
|---|---|---|
| SIGCHLD | kernel | a child changed state, reap |
| SIGTERM | `reboot` | reboot |
| SIGINT | Ctrl+Alt+Del | reboot |
| SIGUSR2 | `poweroff` | power off |
| SIGUSR1 | `halt` | halt |
| SIGPWR | power failure | halt |
| SIGHUP | — | reload, nothing to reload |

Anything else is left alone. SIGKILL and SIGSTOP are not watched because they cannot be
blocked, and pretending otherwise would be a lie in the code.

When several arrive at once the kernel delivers the lowest-numbered one first, so a
simultaneous `halt` and `reboot` resolves to `halt`. That is inherent to standard signals.

---

## Building and testing

```sh
./scripts/build-zevinit.sh
```

Always build through that script, or from inside `zevinit/`. Cargo reads `.cargo/config.toml`
from the working directory, not from `--manifest-path`, so building from the repo root silently
produces a glibc binary that is not static. `main.rs` refuses to compile against anything but
musl to make that failure loud, and the script checks the output is `static-pie linked` before
it reports success.

Why musl and not glibc: a static glibc resolves `memcpy` and friends through ifunc at run time
and can pick an AVX-512 variant, which is an illegal instruction on machines that lack it, which
from pid 1 is a kernel panic. musl has no such mechanism.

```sh
cargo test                      # from inside zevinit/
./scripts/test-qemu-zevinit.sh  # boots it under qemu and drives it
```

The QEMU harness runs eleven cases and can run one at a time, for example
`./scripts/test-qemu-zevinit.sh reboot`. It needs the kernel and the initramfs tree built first.

---

## What is actually verified, and what is not

Worth reading before trusting any of the above.

Covered: 47 unit tests over the pure logic, and 42 assertions across 11 cases driven through a
real boot — ownership of the shell, orphan reaping without growth, all three shutdown paths,
Ctrl+Alt+Del arriving as a signal rather than resetting the machine, the fallback to a second
shell, parking when there is none, Ctrl+C reaching the foreground job, and log priorities.

Both suites were checked by mutation rather than trusted: seventeen deliberate breakages of the
pure logic, sixteen of which some unit test catches, and four of the boot behaviour. Every case
below that says it is not covered says so because a mutation proved it.

**Never run on real hardware.** Every result above comes from QEMU with KVM, plus containers for
the paths where the kernel denies permission. Stage 1 was tested on a real machine; zevinit has
not been.

**Two things pid 1 does for its children are not covered, and cannot be.** Before `exec`, the
child unblocks every signal, and when the spec asks for it, takes its own session and the
terminal. Deleting either changes nothing you can measure today: the only child is BusyBox's
interactive shell, which resets its own signal mask and sets up its own job control on startup,
so the checks pass either way. Both were confirmed by mutation, not assumed.

The code stays because it is correct for a child that does not do that for itself, which is every
service in the next stage. Until then, the two QEMU cases that look at signal masks and Ctrl+C
are asserting that the shell ends up usable, not that pid 1 is the reason. Treat those two lines
in `proc.rs` as unverified.

**Binaries are not reproducible across machines.** `Cargo.toml` pins a minimum Rust version, not
an exact one, so two developers on different toolchains get different bytes from the same source.
Same machine, same compiler, byte-identical. Pinning a toolchain is an open decision.
