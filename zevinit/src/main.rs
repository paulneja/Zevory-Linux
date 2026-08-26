// SPDX-License-Identifier: GPL-3.0-or-later

#[cfg(not(target_env = "musl"))]
compile_error!(
    "zevinit must be built against musl. run cargo from inside zevinit/ so \
     .cargo/config.toml applies, or pass --target x86_64-unknown-linux-musl"
);

mod console;
mod kmsg;
mod mount;
mod power;
mod proc;
mod signal;
mod sys;

use power::Action;
use proc::{Spec, Table};
use signal::{Request, Signals, UNTIL_SOMETHING_ARRIVES};
use std::process;

const READY_MARKER: &str = "bootstrap OK, shell ready";

const RESPAWN_IS_TOO_FAST: u64 = 1;

const RESPAWN_BACKOFF_SECS: u64 = 1;

const BACKOFF_CHECK_MS: libc::c_int = 100;

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

    let signals = match Signals::install(signal::WATCHED) {
        Ok(s) => s,
        Err(e) => park(&format!("cannot watch for signals ({e})")),
    };

    if let Err(e) = sys::disable_ctrl_alt_del() {
        kmsg::log(&format!(
            "ctrl+alt+del stays with the kernel ({e}), it will reboot on the spot"
        ));
    }

    banner(failed);

    let Some(shell) = SHELLS.iter().find(|s| s.is_available()) else {
        park("no shell in the initramfs");
    };

    let mut table = Table::new();
    if let Err(e) = table.spawn(shell) {
        park(&format!("cannot start {} ({e})", shell.path));
    }

    kmsg::log(READY_MARKER);

    let action = supervise(&mut table, &signals, shell);
    let failure = power::execute(action, &signals, &mut table);
    park(&format!(
        "the kernel refused to {} ({failure})",
        action.name()
    ))
}

fn supervise(table: &mut Table, signals: &Signals, shell: &'static Spec) -> Action {
    let mut respawn_at = 0;

    loop {
        let idle = table.running() == 0;
        if idle && sys::monotonic_secs() >= respawn_at {
            if let Err(e) = table.spawn(shell) {
                park(&format!("cannot start {} ({e})", shell.path));
            }
        }

        let how_long = if idle {
            BACKOFF_CHECK_MS
        } else {
            UNTIL_SOMETHING_ARRIVES
        };

        let signal = match signals.wait(how_long) {
            Ok(Some(signal)) => signal,
            Ok(None) => continue,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => park(&format!("lost the signal stream ({e})")),
        };

        match signal::classify(signal) {
            Request::ChildChanged => {
                let died_at_once = signal::reap_all(table)
                    .shortest_life
                    .is_some_and(|ran_for| ran_for < RESPAWN_IS_TOO_FAST);
                if died_at_once {
                    respawn_at = sys::monotonic_secs() + RESPAWN_BACKOFF_SECS;
                }
            }
            Request::Reload => kmsg::log(&format!(
                "{} asked for a reload, zevinit has no configuration to reread",
                signal::name(signal)
            )),
            Request::Shutdown(action) => return action,
            Request::Unknown(other) => {
                kmsg::log(&format!("nothing is wired to signal {other}"));
            }
        }
    }
}

fn banner(failed: usize) {
    kmsg::to_console("\nZevory Linux\n\n");
    if failed > 0 {
        kmsg::to_console(&format!(
            "  {failed} filesystem(s) did not mount, look above for which\n\n"
        ));
    }
}

fn park(why: &str) -> ! {
    let way_out = if sys::enable_ctrl_alt_del().is_ok() {
        "press ctrl+alt+del to reboot"
    } else {
        "power cycle the machine"
    };
    kmsg::log(&format!("{why}, so there is nothing left to do. parked"));
    kmsg::to_console(&format!(
        "\nzevinit: {why}, so there is nothing left to do.\n{way_out}\n"
    ));
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
