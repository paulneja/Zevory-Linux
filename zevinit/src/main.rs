// SPDX-License-Identifier: GPL-3.0-or-later

#[cfg(not(target_env = "musl"))]
compile_error!(
    "zevinit must be built against musl. run cargo from inside zevinit/ so \
     .cargo/config.toml applies, or pass --target x86_64-unknown-linux-musl"
);

mod console;
mod log;
mod mount;
mod power;
mod proc;
mod shell;
mod signal;
mod sys;

use power::Action;
use proc::Table;
use shell::{Rescue, Verdict};
use signal::{Request, Signals, UNTIL_SOMETHING_ARRIVES};
use std::process;

const READY_MARKER: &str = "bootstrap OK, shell ready";

const RESPAWN_IS_TOO_FAST: u64 = 1;

const RESPAWN_BACKOFF_SECS: u64 = 1;

const BACKOFF_CHECK_MS: libc::c_int = 100;

fn main() {
    install_panic_hook();

    let pid = sys::getpid();
    if pid != 1 {
        eprintln!("zevinit: pid is {pid}, not 1, refusing to run.");
        eprintln!("zevinit is the init of a Zevory system, not a command you run by hand.");
        process::exit(1);
    }

    let failed = mount::mount_all();
    log::keep_every_message();
    console::ensure_nodes();

    match console::attach_stdio() {
        Ok(()) => console::take_ctty(),
        Err(e) => log::error(&format!("no console ({e}), carrying on without one")),
    }

    let signals = match Signals::install(signal::WATCHED) {
        Ok(s) => s,
        Err(e) => park(&format!("cannot watch for signals ({e})")),
    };

    if let Err(e) = sys::disable_ctrl_alt_del() {
        log::warn(&format!(
            "ctrl+alt+del stays with the kernel ({e}), it will reboot on the spot"
        ));
    }

    banner(failed);

    let Some(mut rescue) = Rescue::new(shell::CANDIDATES) else {
        park("no shell in the initramfs");
    };

    let mut table = Table::new();
    if let Err(e) = table.spawn(rescue.shell()) {
        park(&format!("cannot start {} ({e})", rescue.shell().path));
    }

    log::info(READY_MARKER);

    let action = supervise(&mut table, &signals, &mut rescue);
    let failure = power::execute(action, &signals, &mut table);
    park(&format!(
        "the kernel refused to {} ({failure})",
        action.name()
    ))
}

fn supervise(table: &mut Table, signals: &Signals, rescue: &mut Rescue) -> Action {
    let mut respawn_at = 0;

    loop {
        let idle = table.running() == 0;
        if idle && sys::monotonic_secs() >= respawn_at {
            if let Err(e) = table.spawn(rescue.shell()) {
                park(&format!("cannot start {} ({e})", rescue.shell().path));
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
            Request::ChildChanged => match signal::reap_all(table).shortest_life {
                None => {}
                Some(ran_for) if ran_for >= RESPAWN_IS_TOO_FAST => rescue.survived(),
                Some(_) => {
                    respawn_at = sys::monotonic_secs() + RESPAWN_BACKOFF_SECS;
                    give_up_or_fall_back(rescue);
                }
            },
            Request::Reload => log::info(&format!(
                "{} asked for a reload, zevinit has no configuration to reread",
                signal::name(signal)
            )),
            Request::Shutdown(action) => return action,
            Request::Unknown(other) => {
                log::info(&format!("nothing is wired to signal {other}"));
            }
        }
    }
}

fn give_up_or_fall_back(rescue: &mut Rescue) {
    let dying = rescue.shell().path;
    match rescue.failed() {
        Verdict::KeepTrying => {}
        Verdict::MovedOn(next) => {
            log::error(&format!(
                "{dying} keeps dying, falling back to {}",
                next.path
            ));
        }
        Verdict::OutOfOptions => park(&format!(
            "every shell we know about dies on startup, {dying} was the last one"
        )),
    }
}

fn banner(failed: usize) {
    log::to_console("\nZevory Linux\n\n");
    if failed > 0 {
        log::to_console(&format!(
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
    log::error(&format!("{why}, so there is nothing left to do. parked"));
    log::to_console(&format!(
        "\nzevinit: {why}, so there is nothing left to do.\n{way_out}\n"
    ));
    loop {
        sys::pause();
    }
}

fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        log::error(&format!("panic: {info}"));
    }));
}

#[cfg(test)]
mod tests {
    use super::READY_MARKER;

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
} #[cfg(test)]
mod integration {
    use crate::power::Action;
    use crate::signal::{self, Request, WATCHED};

    #[test]
    fn every_shutdown_signal_maps_to_an_action_with_a_name() {
        for &s in WATCHED {
            if let Request::Shutdown(action) = signal::classify(s) {
                let n = action.name();
                assert!(!n.is_empty(), "action for signal {s} has an empty name");
            }
        }
    }

    #[test]
    fn signal_classify_never_returns_unknown_for_watched() {
        for &s in WATCHED {
            assert_ne!(
                signal::classify(s),
                Request::Unknown(s),
                "signal {s} is watched but classify returns Unknown"
            );
        }
    }

    #[test]
    fn halt_poweroff_reboot_have_distinct_names() {
        let names: Vec<&str> = [Action::Halt, Action::PowerOff, Action::Reboot]
            .iter()
            .map(|a| a.name())
            .collect();
        let unique: std::collections::HashSet<&str> = names.iter().copied().collect();
        assert_eq!(names.len(), unique.len());
    }
}
