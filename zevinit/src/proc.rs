// SPDX-License-Identifier: GPL-3.0-or-later

use crate::log;
use crate::sys::{self, Fork};
use std::io;
use std::path::Path;

#[derive(PartialEq, Eq, Debug)]
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
    pub owns_session: bool,
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
        self.track(spec.name, pid, spec.wants_own_session);
        Ok(pid)
    }

    fn track(&mut self, name: &'static str, pid: libc::pid_t, owns_session: bool) {
        self.entries.push(Entry {
            name,
            pid,
            state: State::Running,
            started_at: sys::monotonic_secs(),
            owns_session,
        });
    }

    #[cfg(test)]
    fn track_for_test(&mut self, name: &'static str, pid: libc::pid_t) {
        self.track(name, pid, false);
    }

    pub fn session_leaders(&self) -> Vec<libc::pid_t> {
        self.entries
            .iter()
            .filter(|e| e.owns_session && e.state == State::Running)
            .map(|e| e.pid)
            .collect()
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
            log::error(&format!("could not exec {}: {failure}", spec.path));
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

    fn killed(signal: i32) -> libc::c_int {
        signal
    }

    #[test]
    fn a_normal_exit_keeps_its_code() {
        assert_eq!(state_from(exited(0)), State::Exited(0));
        assert_eq!(state_from(exited(127)), State::Exited(127));
    }

    #[test]
    fn a_signal_death_is_not_an_exit() {
        assert_eq!(state_from(killed(libc::SIGKILL)), State::Killed(libc::SIGKILL));
        assert_eq!(state_from(killed(libc::SIGTERM)), State::Killed(libc::SIGTERM));
    }

    #[test]
    fn an_unknown_pid_is_not_recorded() {
        let mut table = Table::new();
        table.track_for_test("shell", 100);
        assert!(table.record_exit(4242, exited(0)).is_none());
        assert_eq!(table.running(), 1);
    }

    #[test]
    fn recording_an_exit_stops_counting_it_as_running() {
        let mut table = Table::new();
        table.track_for_test("shell", 100);
        assert_eq!(table.running(), 1);

        let gone = table.record_exit(100, exited(3)).expect("100 was tracked");
        assert_eq!(gone.name, "shell");
        assert_eq!(gone.pid, 100);
        assert_eq!(gone.state, State::Exited(3));
        assert_eq!(table.running(), 0);
    }

    #[test]
    fn the_same_pid_is_not_reaped_twice() {
        let mut table = Table::new();
        table.track_for_test("shell", 100);
        assert!(table.record_exit(100, exited(0)).is_some());
        assert!(table.record_exit(100, exited(0)).is_none());
    }

    #[test]
    fn a_recycled_pid_lands_on_the_live_entry() {
        let mut table = Table::new();
        table.track_for_test("first", 100);
        table.record_exit(100, exited(0));
        table.track_for_test("second", 100);

        let gone = table.record_exit(100, exited(9)).expect("the live 100");
        assert_eq!(gone.name, "second");
        assert_eq!(table.running(), 0);
    }

    #[test]
    fn several_children_are_tracked_independently() {
        let mut table = Table::new();
        for pid in 200..205 {
            table.track_for_test("worker", pid);
        }
        assert_eq!(table.running(), 5);

        for pid in 200..205 {
            assert!(table.record_exit(pid, exited(0)).is_some());
        }
        assert_eq!(table.running(), 0);
    }

    #[test]
    fn forgetting_drops_the_dead_and_keeps_the_living() {
        let mut table = Table::new();
        table.track_for_test("dead", 100);
        table.track_for_test("alive", 101);
        table.record_exit(100, exited(0));

        table.forget_finished();
        assert_eq!(table.running(), 1);
        assert!(table.record_exit(101, exited(0)).is_some());
        assert!(table.record_exit(100, exited(0)).is_none());
    }

    #[test]
    fn only_live_session_leaders_get_hung_up() {
        let mut table = Table::new();
        table.track("plain", 100, false);
        table.track("leader", 101, true);
        table.track("dead leader", 102, true);
        table.record_exit(102, exited(0));

        assert_eq!(table.session_leaders(), vec![101]);
    }

    #[test]
    fn a_table_with_no_sessions_hangs_nothing_up() {
        let mut table = Table::new();
        table.track_for_test("plain", 100);
        assert!(table.session_leaders().is_empty());
    }

    #[test]
    fn an_empty_table_has_nothing_running() {
        let mut table = Table::new();
        assert_eq!(table.running(), 0);
        table.forget_finished();
        assert_eq!(table.running(), 0);
    }
}
