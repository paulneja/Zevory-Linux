// SPDX-License-Identifier: GPL-3.0-or-later

mod console;
mod kmsg;
mod mount;
mod sys;

use std::process;

const READY_MARKER: &str = "bootstrap OK, shell ready";

const SHELLS: &[(&str, &[&str])] = &[("/bin/sh", &["sh"]), ("/bin/busybox", &["busybox", "sh"])];

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

    banner(failed);
    kmsg::log(READY_MARKER);

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
}
