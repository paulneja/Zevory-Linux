// SPDX-License-Identifier: GPL-3.0-or-later

mod console;
mod kmsg;
mod mount;
mod proc;
mod signal;
mod sys;

use proc::{Spec, Table};
use signal::Signals;
use std::process;

const READY_MARKER: &str = "bootstrap OK, shell ready";

const RESPAWN_IS_TOO_FAST: u64 = 1;

const SHELLS: &[Spec] = &[
    Spec {
        name: "shell",
        path: "/bin/sh",
        argv: &["sh"],
        wants_own_session: true,
    },
    Spec {
        name: "shell",
        path: "/bin/busybox",
        argv: &["busybox", "sh"],
        wants_own_session: true,
    },
];

fn main() {
    install_panic_hook();

    let pid = sys::getpid();
    if pid != 1 {
        eprintln!("zevinit: pid is {pid}, not 1, refusing to run.");
        eprintln!("zevinit is the init of a Zevory system, not a command you run by hand.");
        process::exit(1);
    }

    let failed = mount::mount_all();
    console::ensure_nodes();

    match console::attach_stdio() {
        Ok(()) => console::take_ctty(),
        Err(e) => kmsg::log(&format!("no console ({e}), carrying on without one")),
    }

    let signals = match Signals::install(&[libc::SIGCHLD]) {
        Ok(s) => s,
        Err(e) => park(&format!("cannot watch for child exits ({e})")),
    };

    banner(failed);

    let Some(shell) = SHELLS.iter().find(|s| s.is_available()) else {
        park("no shell in the initramfs");
    };

    let mut table = Table::new();
    if let Err(e) = table.spawn(shell) {
        park(&format!("cannot start {} ({e})", shell.path));
    }

    kmsg::log(READY_MARKER);

    supervise(&mut table, &signals, shell)
}

fn supervise(table: &mut Table, signals: &Signals, shell: &'static Spec) -> ! {
    let mut last_exit_was_immediate = false;

    loop {
        if table.running() == 0 {
            if last_exit_was_immediate {
                sys::sleep_secs(1);
            }
            if let Err(e) = table.spawn(shell) {
                park(&format!("cannot start {} ({e})", shell.path));
            }
        }

        match signals.wait() {
            Ok(libc::SIGCHLD) => {
                last_exit_was_immediate = signal::reap_all(table)
                    .is_some_and(|ran_for| ran_for < RESPAWN_IS_TOO_FAST);
            }
            Ok(other) => kmsg::log(&format!("ignoring signal {other}")),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => park(&format!("lost the signal stream ({e})")),
        }
    }
}

fn banner(failed: usize) {
    println!();
    println!("Zevory Linux");
    println!();
    if failed > 0 {
        println!("  {failed} filesystem(s) did not mount, look above for which");
        println!();
    }
}

fn park(why: &str) -> ! {
    kmsg::log(&format!("{why}, so there is nothing left to do"));
    kmsg::log("parked. reboot with the power button, or fix the initramfs");
    loop {
        sys::pause();
    }
}

fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        kmsg::log(&format!("panic: {info}"));
    }));
}

#[cfg(test)]
mod tests {
    use super::{READY_MARKER, SHELLS};

    #[test]
    fn diagnose_boot_still_greps_for_our_marker() {
        let script =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../scripts/diagnose-boot.sh");
        let Ok(text) = std::fs::read_to_string(&script) else {
            eprintln!("skipped, {} is not here", script.display());
            return;
        };
        assert!(
            text.contains(READY_MARKER),
            "{} stopped grepping for {READY_MARKER:?}",
            script.display()
        );
    }

    #[test]
    fn every_shell_passes_its_own_name_as_argv0() {
        for shell in SHELLS {
            let argv0 = shell.argv.first().expect("a shell needs an argv[0]");
            assert!(
                shell.path.ends_with(argv0),
                "{} would be exec'd as {argv0:?}, which picks the wrong busybox applet",
                shell.path
            );
        }
    }

    #[test]
    fn shells_get_their_own_session() {
        for shell in SHELLS {
            assert!(shell.wants_own_session, "{} needs job control", shell.path);
        }
    }
}
