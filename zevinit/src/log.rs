// SPDX-License-Identifier: GPL-3.0-or-later

use crate::sys;
use std::sync::atomic::{AtomicI32, Ordering};

const NOT_OPEN_YET: libc::c_int = -1;

static KMSG: AtomicI32 = AtomicI32::new(NOT_OPEN_YET);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Level {
    Error,
    Warn,
    Info,
}

impl Level {
    fn priority(self) -> u8 {
        match self {
            Level::Error => 3,
            Level::Warn => 4,
            Level::Info => 6,
        }
    }
}

pub fn error(msg: &str) {
    emit(Level::Error, msg);
}

pub fn warn(msg: &str) {
    emit(Level::Warn, msg);
}

pub fn info(msg: &str) {
    emit(Level::Info, msg);
}

pub fn keep_every_message() {
    let knob = "/proc/sys/kernel/printk_devkmsg";
    if let Err(e) = std::fs::write(knob, "on\n") {
        warn(&format!(
            "{knob} is not writable ({e}), the kernel will drop our log lines in bursts"
        ));
    }
}

pub fn to_console(msg: &str) {
    sys::write_fd(libc::STDOUT_FILENO, msg.as_bytes());
}

fn emit(level: Level, msg: &str) {
    if write_to_kmsg(level, msg) {
        return;
    }
    sys::write_fd(libc::STDERR_FILENO, format!("zevinit: {msg}\n").as_bytes());
}

fn write_to_kmsg(level: Level, msg: &str) -> bool {
    let fd = kmsg();
    if fd == NOT_OPEN_YET {
        return false;
    }
    sys::write_fd(fd, line_for(level, msg).as_bytes())
}

fn line_for(level: Level, msg: &str) -> String {
    format!("<{}>zevinit: {msg}\n", level.priority())
}

fn kmsg() -> libc::c_int {
    let known = KMSG.load(Ordering::Relaxed);
    if known != NOT_OPEN_YET {
        return known;
    }
    let Ok(fd) = sys::open_append("/dev/kmsg") else {
        return NOT_OPEN_YET;
    };
    KMSG.store(fd, Ordering::Relaxed);
    fd
}

#[cfg(test)]
mod tests {
    use super::{Level, line_for};

    #[test]
    fn the_priorities_are_the_ones_the_kernel_understands() {
        assert_eq!(Level::Error.priority(), 3);
        assert_eq!(Level::Warn.priority(), 4);
        assert_eq!(Level::Info.priority(), 6);
    }

    #[test]
    fn quiet_hides_everything_except_errors() {
        let console_loglevel = 4;
        assert!(Level::Error.priority() < console_loglevel);
        assert!(Level::Warn.priority() >= console_loglevel);
        assert!(Level::Info.priority() >= console_loglevel);
    }

    #[test]
    fn every_line_carries_its_priority_and_who_wrote_it() {
        assert_eq!(line_for(Level::Error, "boom"), "<3>zevinit: boom\n");
        assert_eq!(line_for(Level::Info, "fine"), "<6>zevinit: fine\n");
    }

    #[test]
    fn a_line_ends_exactly_once() {
        let line = line_for(Level::Warn, "careful");
        assert!(line.ends_with('\n'));
        assert_eq!(line.matches('\n').count(), 1);
    }
}
