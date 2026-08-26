// SPDX-License-Identifier: GPL-3.0-or-later

use crate::log;
use crate::power::Action;
use crate::proc::{State, Table};
use crate::sys::{self, Reaped};
use std::io;

pub const WATCHED: &[libc::c_int] = &[
    libc::SIGCHLD,
    libc::SIGTERM,
    libc::SIGINT,
    libc::SIGUSR1,
    libc::SIGUSR2,
    libc::SIGPWR,
    libc::SIGHUP,
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Request {
    ChildChanged,
    Shutdown(Action),
    Reload,
    Unknown(libc::c_int),
}

pub fn classify(signal: libc::c_int) -> Request {
    match signal {
        libc::SIGCHLD => Request::ChildChanged,
        libc::SIGTERM | libc::SIGINT => Request::Shutdown(Action::Reboot),
        libc::SIGUSR2 => Request::Shutdown(Action::PowerOff),
        libc::SIGUSR1 | libc::SIGPWR => Request::Shutdown(Action::Halt),
        libc::SIGHUP => Request::Reload,
        other => Request::Unknown(other),
    }
}

pub fn name(signal: libc::c_int) -> &'static str {
    match signal {
        libc::SIGCHLD => "SIGCHLD",
        libc::SIGTERM => "SIGTERM",
        libc::SIGINT => "SIGINT",
        libc::SIGUSR1 => "SIGUSR1",
        libc::SIGUSR2 => "SIGUSR2",
        libc::SIGPWR => "SIGPWR",
        libc::SIGHUP => "SIGHUP",
        _ => "an unwatched signal",
    }
}

pub const UNTIL_SOMETHING_ARRIVES: libc::c_int = -1;

pub struct Signals {
    fd: libc::c_int,
}

impl Signals {
    pub fn install(signals: &[libc::c_int]) -> io::Result<Self> {
        sys::block_signals(signals)?;
        Ok(Signals {
            fd: sys::signal_fd(signals)?,
        })
    }

    pub fn wait(&self, millis: libc::c_int) -> io::Result<Option<libc::c_int>> {
        if sys::poll_readable(self.fd, millis)? {
            return sys::read_signal(self.fd).map(Some);
        }
        Ok(None)
    }
}

pub struct Harvest {
    pub shortest_life: Option<u64>,
    pub children_left: bool,
}

pub fn reap_all(table: &mut Table) -> Harvest {
    let mut shortest_life = None;

    let children_left = loop {
        let (pid, status) = match sys::reap_one() {
            Reaped::Child(pid, status) => (pid, status),
            Reaped::StillRunning => break true,
            Reaped::NoChildrenLeft => break false,
        };
        let Some(gone) = table.record_exit(pid, status) else {
            log::info(&format!("reaped orphan [{pid}]"));
            continue;
        };
        match gone.state {
            State::Exited(code) => log::info(&format!(
                "{} [{}] exited with {code} after {}s",
                gone.name, gone.pid, gone.ran_for
            )),
            State::Killed(signal) => log::warn(&format!(
                "{} [{}] killed by signal {signal} after {}s",
                gone.name, gone.pid, gone.ran_for
            )),
            State::Running => {}
        }
        shortest_life = Some(shortest_life.map_or(gone.ran_for, |s: u64| s.min(gone.ran_for)));
    };

    table.forget_finished();
    Harvest {
        shortest_life,
        children_left,
    }
}

#[cfg(test)]
mod tests {
    use super::{Request, WATCHED, classify, name};
    use crate::power::Action;
    use std::collections::HashSet;

    #[test]
    fn busybox_halt_poweroff_and_reboot_land_where_they_should() {
        assert_eq!(classify(libc::SIGUSR1), Request::Shutdown(Action::Halt));
        assert_eq!(classify(libc::SIGUSR2), Request::Shutdown(Action::PowerOff));
        assert_eq!(classify(libc::SIGTERM), Request::Shutdown(Action::Reboot));
    }

    #[test]
    fn ctrl_alt_del_reboots_and_power_failure_halts() {
        assert_eq!(classify(libc::SIGINT), Request::Shutdown(Action::Reboot));
        assert_eq!(classify(libc::SIGPWR), Request::Shutdown(Action::Halt));
    }

    #[test]
    fn every_watched_signal_means_something() {
        for &s in WATCHED {
            assert_ne!(
                classify(s),
                Request::Unknown(s),
                "{} is watched but nothing happens when it arrives",
                name(s)
            );
        }
    }

    #[test]
    fn every_watched_signal_has_a_name() {
        for &s in WATCHED {
            assert_ne!(
                name(s),
                "an unwatched signal",
                "signal {s} prints as a stranger"
            );
        }
    }

    #[test]
    fn children_are_still_watched() {
        assert!(
            WATCHED.contains(&libc::SIGCHLD),
            "without SIGCHLD nothing gets reaped and children pile up as zombies"
        );
    }

    #[test]
    fn no_signal_is_watched_twice() {
        let mut seen = HashSet::new();
        for &s in WATCHED {
            assert!(seen.insert(s), "{} shows up twice", name(s));
        }
    }

    #[test]
    fn the_unblockable_signals_are_left_alone() {
        for s in [libc::SIGKILL, libc::SIGSTOP] {
            assert!(
                !WATCHED.contains(&s),
                "signal {s} cannot be blocked, watching it would silently do nothing"
            );
        }
    }

    #[test]
    fn an_unwatched_signal_stays_unknown() {
        assert_eq!(classify(libc::SIGWINCH), Request::Unknown(libc::SIGWINCH));
    }
}
