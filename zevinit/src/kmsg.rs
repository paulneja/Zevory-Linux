// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs::OpenOptions;
use std::io::Write;

pub fn log(msg: &str) {
    let line = format!("zevinit: {msg}\n");

    if let Ok(mut f) = OpenOptions::new().write(true).open("/dev/kmsg") {
        if f.write_all(line.as_bytes()).is_ok() {
            return;
        }
    }

    eprint!("{line}");
}
