// SPDX-License-Identifier: GPL-3.0-or-later

use crate::kmsg;
use crate::sys::{self, Fork};
use std::io;
use std::path::Path;

pub struct Spec {
    pub name: &'static str,
    pub path: &'static str,
    pub argv: &'static [&'static str],
    pub wants_own_session: bool,
}

impl Spec {
    pub fn is_available(&self) -> bool {
        Path::new(self.path).exists()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum State {
    Running,
    Exited(libc::c_int),
    Killed(libc::c_int),
}

pub struct Entry {
    pub name: &'static str,
    pub pid: libc::pid_t,
    pub state: State,
    pub started_at: u64,
}

pub struct Table {
    entries: Vec<Entry>,
}

impl Table {
    pub fn new() -> Self {
        Table {
            entries: Vec::new(),
        }
    }

    pub fn spawn(&mut self, spec: &'static Spec) -> io::Result<libc::pid_t> {
        let pid = spawn(spec)?;
        self.entries.push(Entry {
            name: spec.name,
            pid,
            state: State::Running,
            started_at: sys::monotonic_secs(),
        });
        Ok(pid)
    }

    pub fn running(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| e.state == State::Running)
            .count()
    }

    pub fn record_exit(&mut self, pid: libc::pid_t, status: libc::c_int) -> Option<Departed> {
        let state = state_from(status);
        let entry = self
            .entries
            .iter_mut()
            .find(|e| e.pid == pid && e.state == State::Running)?;
        entry.state = state;
        Some(Departed {
            name: entry.name,
            pid,
            state,
            ran_for: sys::monotonic_secs().saturating_sub(entry.started_at),
        })
    }

    pub fn forget_finished(&mut self) {
        self.entries.retain(|e| e.state == State::Running);
    }
}

pub struct Departed {
    pub name: &'static str,
    pub pid: libc::pid_t,
    pub state: State,
    pub ran_for: u64,
}

fn state_from(status: libc::c_int) -> State {
    if let Some(code) = sys::exit_code(status) {
        return State::Exited(code);
    }
    if let Some(signal) = sys::termination_signal(status) {
        return State::Killed(signal);
    }
    State::Exited(-1)
}

fn spawn(spec: &Spec) -> io::Result<libc::pid_t> {
    match sys::fork()? {
        Fork::Parent(pid) => Ok(pid),
        Fork::Child => {
            let _ = sys::unblock_all_signals();
            if spec.wants_own_session {
                let _ = sys::new_session();
                let _ = sys::set_ctty(libc::STDIN_FILENO, true);
            }
            let failure = sys::exec(spec.path, spec.argv);
            kmsg::log(&format!("could not exec {}: {failure}", spec.path));
            sys::exit_child(127)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{State, Table, state_from};

    fn exited(code: i32) -> libc::c_int {
        code << 8
    }

    #[test]
    fn a_normal_exit_keeps_its_code() {
        assert_eq!(state_from(exited(0)), State::Exited(0));
        assert_eq!(state_from(exited(127)), State::Exited(127));
    }

    #[test]
    fn a_signal_death_is_not_an_exit() {
        assert_eq!(state_from(libc::SIGKILL), State::Killed(libc::SIGKILL));
    }

    #[test]
    fn an_unknown_pid_is_not_recorded() {
        let mut table = Table::new();
        assert!(table.record_exit(4242, exited(0)).is_none());
        assert_eq!(table.running(), 0);
    }
}
