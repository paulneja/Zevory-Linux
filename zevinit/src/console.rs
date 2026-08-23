// SPDX-License-Identifier: GPL-3.0-or-later

//! Device nodes and stdio. Without these PID 1 is blind, and so is whoever is
//! watching the screen.

use crate::kmsg;
use crate::sys;
use std::io;
use std::path::Path;

// devtmpfs normally hands us these. we only step in when it did not, which in
// practice means /dev failed to mount
const NODES: &[(&str, u32, u32, u32)] = &[
    ("/dev/null", 0o666, 1, 3),
    ("/dev/console", 0o600, 5, 1),
    ("/dev/tty", 0o666, 5, 0),
];

pub fn ensure_nodes() {
    for &(path, mode, major, minor) in NODES {
        if Path::new(path).exists() {
            continue;
        }
        if let Err(e) = sys::mknod_char(path, mode, major, minor) {
            kmsg::log(&format!("could not create {path}: {e}"));
        }
    }
}

/// Points stdin/stdout/stderr at /dev/console.
///
/// Which device that actually is depends on the last `console=` on the kernel
/// command line, not the first. Stage 1 had them the other way round and the
/// shell quietly ended up on the serial port.
pub fn attach_stdio() -> io::Result<()> {
    let fd = sys::open_rw("/dev/console")?;
    sys::dup2_stdio(fd)
}

#[cfg(test)]
mod tests {
    use super::NODES;

    #[test]
    fn numbers_match_the_kernel_ones() {
        // these live in Documentation/admin-guide/devices.txt, they are not
        // ours to pick
        assert_eq!(NODES[0], ("/dev/null", 0o666, 1, 3));
        assert_eq!(NODES[1], ("/dev/console", 0o600, 5, 1));
        assert_eq!(NODES[2], ("/dev/tty", 0o666, 5, 0));
    }

    #[test]
    fn modes_carry_no_type_bits() {
        // mknod_char ors S_IFCHR in, so anything extra here would fight it
        for &(path, mode, _, _) in NODES {
            assert_eq!(mode & !0o777, 0, "{path} has more than permission bits");
        }
    }
}
