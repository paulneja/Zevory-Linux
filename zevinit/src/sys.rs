// SPDX-License-Identifier: GPL-3.0-or-later

use std::ffi::CString;
use std::io;

fn cstr(s: &str) -> io::Result<CString> {
    CString::new(s).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "nul byte in string"))
}

fn check(rc: libc::c_int) -> io::Result<()> {
    if rc == 0 { Ok(()) } else { Err(io::Error::last_os_error()) }
}

pub fn getpid() -> i32 {
    unsafe { libc::getpid() }
}

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
    let fd = unsafe { libc::open(p.as_ptr(), libc::O_RDWR | libc::O_NOCTTY) };
    if fd < 0 { Err(io::Error::last_os_error()) } else { Ok(fd) }
}

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

pub fn new_session() -> io::Result<()> {
    if unsafe { libc::setsid() } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

pub fn set_ctty(fd: libc::c_int, steal: bool) -> io::Result<()> {
    let arg: libc::c_int = if steal { 1 } else { 0 };
    check(unsafe { libc::ioctl(fd, libc::TIOCSCTTY, arg) })
}

pub fn take_ctty(fd: libc::c_int) -> io::Result<()> {
    let _already_a_session_leader = new_session();
    set_ctty(fd, false)
}

pub fn pause() {
    unsafe { libc::pause() };
}

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
        assert_eq!(makedev(1, 3), 0x0103);
        assert_eq!(makedev(5, 1), 0x0501);
        assert_eq!(makedev(5, 0), 0x0500);
    }

    #[test]
    fn makedev_splits_wide_minor() {
        assert_eq!(makedev(8, 0), 0x0800);
        assert_eq!(makedev(8, 256), 0x10_0800);
        assert_eq!(makedev(0, 0), 0);
    }
}

pub enum Fork {
    Parent(libc::pid_t),
    Child,
}

pub fn fork() -> io::Result<Fork> {
    match unsafe { libc::fork() } {
        -1 => Err(io::Error::last_os_error()),
        0 => Ok(Fork::Child),
        pid => Ok(Fork::Parent(pid)),
    }
}

pub fn exit_child(code: libc::c_int) -> ! {
    unsafe { libc::_exit(code) }
}

pub enum Reaped {
    Child(libc::pid_t, libc::c_int),
    NothingPending,
}

pub fn reap_one() -> Reaped {
    let mut status: libc::c_int = 0;
    match unsafe { libc::waitpid(-1, &mut status, libc::WNOHANG) } {
        pid if pid > 0 => Reaped::Child(pid, status),
        _ => Reaped::NothingPending,
    }
}

pub fn exit_code(status: libc::c_int) -> Option<libc::c_int> {
    libc::WIFEXITED(status).then(|| libc::WEXITSTATUS(status))
}

pub fn termination_signal(status: libc::c_int) -> Option<libc::c_int> {
    libc::WIFSIGNALED(status).then(|| libc::WTERMSIG(status))
}

fn sigset_of(signals: &[libc::c_int]) -> libc::sigset_t {
    let mut set: libc::sigset_t = unsafe { std::mem::zeroed() };
    unsafe { libc::sigemptyset(&mut set) };
    for &s in signals {
        unsafe { libc::sigaddset(&mut set, s) };
    }
    set
}

pub fn block_signals(signals: &[libc::c_int]) -> io::Result<()> {
    let set = sigset_of(signals);
    check(unsafe { libc::sigprocmask(libc::SIG_BLOCK, &set, std::ptr::null_mut()) })
}

pub fn unblock_all_signals() -> io::Result<()> {
    let mut set: libc::sigset_t = unsafe { std::mem::zeroed() };
    unsafe { libc::sigfillset(&mut set) };
    check(unsafe { libc::sigprocmask(libc::SIG_UNBLOCK, &set, std::ptr::null_mut()) })
}

pub fn signal_fd(signals: &[libc::c_int]) -> io::Result<libc::c_int> {
    let set = sigset_of(signals);
    let fd = unsafe { libc::signalfd(-1, &set, libc::SFD_CLOEXEC) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(fd)
}

pub fn read_signal(fd: libc::c_int) -> io::Result<libc::c_int> {
    let mut info: libc::signalfd_siginfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of::<libc::signalfd_siginfo>();
    let n = unsafe { libc::read(fd, (&raw mut info).cast::<libc::c_void>(), size) };
    if n < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(info.ssi_signo as libc::c_int)
}

pub fn monotonic_secs() -> u64 {
    let mut ts: libc::timespec = unsafe { std::mem::zeroed() };
    unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) };
    ts.tv_sec as u64
}

pub fn sleep_secs(secs: libc::c_uint) {
    unsafe { libc::sleep(secs) };
}
