// SPDX-License-Identifier: GPL-3.0-or-later

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

const TABLE: &[Vfs] = &[
    Vfs { source: "proc", target: "/proc", fstype: "proc", flags: RO_ISH, data: None },
    Vfs { source: "sysfs", target: "/sys", fstype: "sysfs", flags: RO_ISH, data: None },
    Vfs { source: "devtmpfs", target: "/dev", fstype: "devtmpfs", flags: libc::MS_NOSUID, data: Some("mode=0755") },
    Vfs { source: "tmpfs", target: "/run", fstype: "tmpfs", flags: TMPFS, data: Some("mode=0755") },
    Vfs { source: "tmpfs", target: "/tmp", fstype: "tmpfs", flags: TMPFS, data: None },
];

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
    std::fs::create_dir_all(v.target)?;

    match sys::mount(&MountOpts {
        source: v.source,
        target: v.target,
        fstype: v.fstype,
        flags: v.flags,
        data: v.data,
    }) {
        Ok(()) => Ok(()),
        Err(e) if e.raw_os_error() == Some(libc::EBUSY) => Ok(()),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::TABLE;
    use std::collections::HashSet;

    #[test]
    fn targets_are_absolute_and_unique() {
        let mut seen = HashSet::new();
        for v in TABLE {
            assert!(v.target.starts_with('/'), "{} is not absolute", v.target);
            assert!(seen.insert(v.target), "{} shows up twice", v.target);
        }
    }

    #[test]
    fn proc_and_sys_come_first() {
        assert_eq!(TABLE[0].target, "/proc");
        assert_eq!(TABLE[1].target, "/sys");
    }

    #[test]
    fn nothing_is_mounted_suid() {
        for v in TABLE {
            assert!(
                v.flags & libc::MS_NOSUID != 0,
                "{} would let setuid bits through",
                v.target
            );
        }
    }
}
