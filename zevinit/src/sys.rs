// SPDX-License-Identifier: GPL-3.0-or-later

//! Thin wrappers over the handful of syscalls PID 1 needs.
//!
//! Every `unsafe` block in zevinit lives in this file. If you find yourself
//! reaching for one somewhere else, add a wrapper here instead.

use std::ffi::CString;
use std::io;

fn cstr(s: &str) -> io::Result<CString> {
    CString::new(s).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "nul byte in string"))
}

fn check(rc: libc::c_int) -> io::Result<()> {
    if rc == 0 { Ok(()) } else { Err(io::Error::last_os_error()) }
}

pub fn getpid() -> i32 {
    // cannot fail
    unsafe { libc::getpid() }
}

/// Linux packs major/minor into dev_t with the minor split around the major.
/// Doing it here rather than through libc keeps it visible and testable.
pub fn makedev(major: u32, minor: u32) -> libc::dev_t {
    let major = major as u64;
    let minor = minor as u64;
    (((major & 0xffff_f000) << 32)
        | ((major & 0x0000_0fff) << 8)
        | ((minor & 0xffff_ff00) << 12)
        | (minor & 0x0000_00ff)) as libc::dev_t
}

pub struct MountOpts<'a> {
    pub source: &'a str,
    pub target: &'a str,
    pub fstype: &'a str,
    pub flags: libc::c_ulong,
    pub data: Option<&'a str>,
}

pub fn mount(o: &MountOpts) -> io::Result<()> {
    let source = cstr(o.source)?;
    let target = cstr(o.target)?;
    let fstype = cstr(o.fstype)?;
    let data = o.data.map(cstr).transpose()?;
    let data_ptr = data
        .as_ref()
        .map_or(std::ptr::null(), |d| d.as_ptr().cast::<libc::c_void>());

    check(unsafe {
        libc::mount(
            source.as_ptr(),
            target.as_ptr(),
            fstype.as_ptr(),
            o.flags,
            data_ptr,
        )
    })
}

pub fn mknod_char(path: &str, mode: u32, major: u32, minor: u32) -> io::Result<()> {
    let p = cstr(path)?;
    let mode = libc::S_IFCHR | mode;
    check(unsafe { libc::mknod(p.as_ptr(), mode, makedev(major, minor)) })
}

pub fn open_rw(path: &str) -> io::Result<libc::c_int> {
    let p = cstr(path)?;
    // no O_CLOEXEC, these are meant to survive into whatever we exec
    let fd = unsafe { libc::open(p.as_ptr(), libc::O_RDWR | libc::O_NOCTTY) };
    if fd < 0 { Err(io::Error::last_os_error()) } else { Ok(fd) }
}

/// Point stdin, stdout and stderr at `fd`, then drop the spare.
pub fn dup2_stdio(fd: libc::c_int) -> io::Result<()> {
    for target in [libc::STDIN_FILENO, libc::STDOUT_FILENO, libc::STDERR_FILENO] {
        if unsafe { libc::dup2(fd, target) } < 0 {
            return Err(io::Error::last_os_error());
        }
    }
    if fd > libc::STDERR_FILENO {
        unsafe { libc::close(fd) };
    }
    Ok(())
}

/// Sleeps until any signal shows up. Callers loop on it.
pub fn pause() {
    unsafe { libc::pause() };
}

/// Only returns if the exec failed, so the error is the whole return value.
pub fn exec(path: &str, args: &[&str]) -> io::Error {
    let Ok(cpath) = cstr(path) else {
        return io::Error::new(io::ErrorKind::InvalidInput, "nul byte in path");
    };
    let mut owned = Vec::with_capacity(args.len());
    for a in args {
        match cstr(a) {
            Ok(c) => owned.push(c),
            Err(e) => return e,
        }
    }
    let mut argv: Vec<*const libc::c_char> = owned.iter().map(|c| c.as_ptr()).collect();
    argv.push(std::ptr::null());

    unsafe { libc::execv(cpath.as_ptr(), argv.as_ptr()) };
    io::Error::last_os_error()
}

#[cfg(test)]
mod tests {
    use super::makedev;

    #[test]
    fn makedev_matches_known_nodes() {
        // the three nodes /init used to mknod by hand
        assert_eq!(makedev(1, 3), 0x0103); // /dev/null
        assert_eq!(makedev(5, 1), 0x0501); // /dev/console
        assert_eq!(makedev(5, 0), 0x0500); // /dev/tty
    }

    #[test]
    fn makedev_splits_wide_minor() {
        // a minor above 0xff moves into the high half instead of trampling the
        // major, which is the whole reason this is not just (major << 8) | minor
        assert_eq!(makedev(8, 0), 0x0800);
        assert_eq!(makedev(8, 256), 0x10_0800);
        assert_eq!(makedev(0, 0), 0);
    }
}
