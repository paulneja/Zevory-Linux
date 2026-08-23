// SPDX-License-Identifier: GPL-3.0-or-later

//! Boot-progress output, nothing more.
//!
//! INIT1-010 turns this into real logging. All it has to do today is get a line
//! somewhere a human or a test can find it.

use std::fs::OpenOptions;
use std::io::Write;

/// Writing to /dev/kmsg reaches every console, not only the one that happens to
/// own /dev/console. In stage 1 the difference between those two was a whole
/// day of thinking the machine had hung.
pub fn log(msg: &str) {
    let line = format!("zevinit: {msg}\n");

    if let Ok(mut f) = OpenOptions::new().write(true).open("/dev/kmsg") {
        if f.write_all(line.as_bytes()).is_ok() {
            return;
        }
    }

    // before /dev is mounted, or if the kernel was built without /dev/kmsg
    eprint!("{line}");
}
