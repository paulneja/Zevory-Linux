// SPDX-License-Identifier: GPL-3.0-or-later

//! zevinit, PID 1 for Zevory.
//!
//! Deliberately small. Supervision, dependencies, timers, cgroups and the TOML
//! config all belong to later stages. What this does today is bring the
//! environment up and say so.

mod console;
mod kmsg;
mod mount;
mod sys;

use std::panic;
use std::process;

/// diagnose-boot.sh greps for this exact string. If you reword it, reword it
/// there too, or the boot tests start passing for the wrong reason.
const READY_MARKER: &str = "bootstrap OK, shell ready";

const SHELL: &str = "/bin/sh";

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

    if let Err(e) = console::attach_stdio() {
        // not fatal on its own. kmsg still reaches the screen
        kmsg::log(&format!("no console ({e}), carrying on without one"));
    }

    banner(failed);
    kmsg::log(READY_MARKER);

    // INIT1-004 puts real process supervision here. Until it exists, handing
    // over to a shell is the honest placeholder: it makes the thing usable and
    // it does not pretend to be an init that supervises anything.
    //
    // Careful, this replaces us, so the shell becomes PID 1 with all the signal
    // trouble that implies. That goes away once 004 to 009 land.
    let e = sys::exec(SHELL, &[SHELL]);
    kmsg::log(&format!("could not exec {SHELL}: {e}"));
    process::exit(1);
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

/// A panic here takes PID 1 with it and the kernel turns that into a panic of
/// its own, so the least we can do is say what happened somewhere visible.
/// INIT1-011 turns this into an actual way out.
fn install_panic_hook() {
    panic::set_hook(Box::new(|info| {
        kmsg::log(&format!("panic: {info}"));
    }));
}
