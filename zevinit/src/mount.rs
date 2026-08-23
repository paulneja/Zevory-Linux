// SPDX-License-Identifier: GPL-3.0-or-later

//! The virtual filesystems that have to exist before anything else works.

use crate::kmsg;
use crate::sys::{self, MountOpts};
use std::io;

struct Vfs {
    source: &'static str,
    target: &'static str,
    fstype: &'static str,
    flags: libc::c_ulong,
    data: Option<&'static str>,
}

const RO_ISH: libc::c_ulong = libc::MS_NOSUID | libc::MS_NODEV | libc::MS_NOEXEC;
const TMPFS: libc::c_ulong = libc::MS_NOSUID | libc::MS_NODEV;

// /proc and /sys go first so that whatever breaks after them has somewhere to
// complain from
const TABLE: &[Vfs] = &[
    Vfs { source: "proc", target: "/proc", fstype: "proc", flags: RO_ISH, data: None },
    Vfs { source: "sysfs", target: "/sys", fstype: "sysfs", flags: RO_ISH, data: None },
    Vfs { source: "devtmpfs", target: "/dev", fstype: "devtmpfs", flags: libc::MS_NOSUID, data: Some("mode=0755") },
    Vfs { source: "tmpfs", target: "/run", fstype: "tmpfs", flags: TMPFS, data: Some("mode=0755") },
    Vfs { source: "tmpfs", target: "/tmp", fstype: "tmpfs", flags: TMPFS, data: None },
];

/// Mounts everything in the table. Returns how many failed, because one missing
/// /tmp is not a reason to give up on the boot.
pub fn mount_all() -> usize {
    let mut failed = 0;
    for v in TABLE {
        if let Err(e) = mount_one(v) {
            kmsg::log(&format!("could not mount {} as {}: {e}", v.target, v.fstype));
            failed += 1;
        }
    }
    failed
}

fn mount_one(v: &Vfs) -> io::Result<()> {
    // the initramfs ships these already, but zevinit should not fall over on a
    // root that is missing one
    std::fs::create_dir_all(v.target)?;

    match sys::mount(&MountOpts {
        source: v.source,
        target: v.target,
        fstype: v.fstype,
        flags: v.flags,
        data: v.data,
    }) {
        Ok(()) => Ok(()),
        // already there, which happens if we come back through here after a
        // rescue shell. not an error
        Err(e) if e.raw_os_error() == Some(libc::EBUSY) => Ok(()),
        Err(e) => Err(e),
    }
}
