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

use std::process;

/// diagnose-boot.sh greps for this exact string. There is a test down the file
/// that checks the two still agree, because a silent drift here turns the boot
/// tests green for the wrong reason.
const READY_MARKER: &str = "bootstrap OK, shell ready";

/// argv[0] is not decoration for busybox, it picks which applet runs.
const SHELLS: &[(&str, &[&str])] = &[("/bin/sh", &["sh"]), ("/bin/busybox", &["busybox", "sh"])];

fn main() {
    install_panic_hook();

    let pid = sys::getpid();
    if pid != 1 {
        // safe to exit here, we are not init
        eprintln!("zevinit: pid is {pid}, not 1, refusing to run.");
        eprintln!("zevinit is the init of a Zevory system, not a command you run by hand.");
        process::exit(1);
    }

    let failed = mount::mount_all();
    console::ensure_nodes();

    if let Err(e) = console::attach_stdio() {
        // not fatal on its own, kmsg still reaches the screen
        kmsg::log(&format!("no console ({e}), carrying on without one"));
    }

    banner(failed);
    kmsg::log(READY_MARKER);

    // INIT1-004 puts real process supervision here. Until it exists, handing
    // over to a shell is the honest placeholder: it makes the thing usable and
    // it does not pretend to supervise anything.
    //
    // Careful, exec replaces us, so the shell becomes PID 1 with all the signal
    // trouble that implies. That goes away once 004 to 009 land.
    for (path, argv) in SHELLS {
        let e = sys::exec(path, argv);
        kmsg::log(&format!("could not exec {path}: {e}"));
    }

    park("no shell left to hand over to");
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

/// PID 1 is not allowed to return. When it does the kernel panics with
/// "attempted to kill init", which tells whoever is staring at the screen
/// nothing useful at all. Sitting here with a readable message beats that.
fn park(why: &str) -> ! {
    kmsg::log(&format!("{why}, so there is nothing left to do"));
    kmsg::log("parked. reboot with the power button, or fix the initramfs");
    loop {
        sys::pause();
    }
}

/// A panic takes PID 1 with it and the kernel turns that into a panic of its
/// own, so the least we can do is leave the reason somewhere visible.
/// INIT1-011 turns this into an actual way out.
fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        kmsg::log(&format!("panic: {info}"));
    }));
}

#[cfg(test)]
mod tests {
    use super::READY_MARKER;

    /// Skips quietly when the script is not next to us, so moving this crate
    /// into another repo does not break the build.
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
}
