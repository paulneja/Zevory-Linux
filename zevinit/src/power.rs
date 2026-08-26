// SPDX-License-Identifier: GPL-3.0-or-later

use crate::log;
use crate::proc::Table;
use crate::signal::{self, Request, Signals};
use crate::sys;
use std::io;

const GRACE_SECS: u64 = 5;
const AFTER_KILL_SECS: u64 = 2;
const CHECK_EVERY_MS: libc::c_int = 100;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Action {
    Reboot,
    PowerOff,
    Halt,
}

impl Action {
    pub fn name(self) -> &'static str {
        match self {
            Action::Reboot => "reboot",
            Action::PowerOff => "poweroff",
            Action::Halt => "halt",
        }
    }

    fn kernel_command(self) -> libc::c_int {
        match self {
            Action::Reboot => libc::RB_AUTOBOOT,
            Action::PowerOff => libc::RB_POWER_OFF,
            Action::Halt => libc::RB_HALT_SYSTEM,
        }
    }
}

pub fn execute(action: Action, signals: &Signals, table: &mut Table) -> io::Error {
    log::to_console(&format!("\nzevory is going down for {}\n", action.name()));
    log::info(&format!("{} requested", action.name()));

    stop_everything(signals, table);
    sys::flush_filesystems();

    log::info(&format!("asking the kernel to {}", action.name()));
    sys::hand_over_to_kernel(action.kernel_command())
}

fn stop_everything(signals: &Signals, table: &mut Table) {
    if !signal::reap_all(table).children_left {
        return;
    }

    log::to_console("stopping everything\n");
    for leader in table.session_leaders() {
        sys::signal_group(leader, libc::SIGHUP);
    }
    sys::signal_everyone(libc::SIGTERM);
    sys::signal_everyone(libc::SIGCONT);
    if wait_for_silence(signals, table, GRACE_SECS) {
        return;
    }

    log::warn(&format!("still busy after {GRACE_SECS}s, sending SIGKILL"));
    log::to_console(&format!(
        "something ignored the first {GRACE_SECS}s, killing it\n"
    ));
    sys::signal_everyone(libc::SIGKILL);
    if !wait_for_silence(signals, table, AFTER_KILL_SECS) {
        log::error("something outlived SIGKILL, going down regardless");
    }
}

fn wait_for_silence(signals: &Signals, table: &mut Table, secs: u64) -> bool {
    let deadline = sys::monotonic_secs() + secs;

    loop {
        if !signal::reap_all(table).children_left {
            return true;
        }
        if sys::monotonic_secs() >= deadline {
            return false;
        }
        if let Ok(Some(arrived)) = signals.wait(CHECK_EVERY_MS)
            && signal::classify(arrived) != Request::ChildChanged
        {
            log::info(&format!(
                "{} arrived while shutting down, already on it",
                signal::name(arrived)
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Action;
    use crate::signal::{Request, WATCHED, classify};
    use std::collections::HashSet;

    #[test]
    fn each_action_asks_the_kernel_for_something_different() {
        let mut seen = HashSet::new();
        for action in [Action::Reboot, Action::PowerOff, Action::Halt] {
            assert!(
                seen.insert(action.kernel_command()),
                "{} shares its reboot(2) command with another action",
                action.name()
            );
        }
    }

    #[test]
    fn the_kernel_commands_are_the_ones_we_mean() {
        assert_eq!(Action::Reboot.kernel_command(), libc::RB_AUTOBOOT);
        assert_eq!(Action::PowerOff.kernel_command(), libc::RB_POWER_OFF);
        assert_eq!(Action::Halt.kernel_command(), libc::RB_HALT_SYSTEM);
    }

    #[test]
    fn nothing_but_a_shutdown_signal_brings_the_machine_down() {
        let down: Vec<libc::c_int> = WATCHED
            .iter()
            .copied()
            .filter(|&s| matches!(classify(s), Request::Shutdown(_)))
            .collect();
        assert_eq!(
            down,
            vec![
                libc::SIGTERM,
                libc::SIGINT,
                libc::SIGUSR1,
                libc::SIGUSR2,
                libc::SIGPWR
            ]
        );
    }

    #[test]
    fn every_action_is_reachable_from_a_signal() {
        let reached: HashSet<Action> = WATCHED
            .iter()
            .filter_map(|&s| match classify(s) {
                Request::Shutdown(a) => Some(a),
                _ => None,
            })
            .collect();
        for action in [Action::Reboot, Action::PowerOff, Action::Halt] {
            assert!(
                reached.contains(&action),
                "no signal ever asks for {}",
                action.name()
            );
        }
    }
}
