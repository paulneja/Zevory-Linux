// SPDX-License-Identifier: GPL-3.0-or-later

use crate::log;
use crate::sys;
use std::io;
use std::path::Path;

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
            log::warn(&format!("could not create {path}: {e}"));
        }
    }
}

pub fn attach_stdio() -> io::Result<()> {
    let fd = sys::open_rw("/dev/console")?;
    sys::dup2_stdio(fd)
}

pub fn take_ctty() {
    if let Err(e) = sys::take_ctty(libc::STDIN_FILENO) {
        log::warn(&format!("no controlling terminal ({e}), job control stays off"));
    }
}

#[cfg(test)]
mod tests {
    use super::NODES;

    #[test]
    fn numbers_match_the_kernel_ones() {
        assert_eq!(NODES[0], ("/dev/null", 0o666, 1, 3));
        assert_eq!(NODES[1], ("/dev/console", 0o600, 5, 1));
        assert_eq!(NODES[2], ("/dev/tty", 0o666, 5, 0));
    }

    #[test]
    fn modes_carry_no_type_bits() {
        for &(path, mode, _, _) in NODES {
            assert_eq!(mode & !0o777, 0, "{path} has more than permission bits");
        }
    }
}
