// SPDX-License-Identifier: GPL-3.0-or-later

use crate::kmsg;
use crate::proc::{State, Table};
use crate::sys::{self, Reaped};
use std::io;

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

    pub fn wait(&self) -> io::Result<libc::c_int> {
        sys::read_signal(self.fd)
    }
}

pub fn reap_all(table: &mut Table) -> Option<u64> {
    let mut shortest_life = None;

    while let Reaped::Child(pid, status) = sys::reap_one() {
        let Some(gone) = table.record_exit(pid, status) else {
            kmsg::log(&format!("reaped orphan [{pid}]"));
            continue;
        };
        match gone.state {
            State::Exited(code) => kmsg::log(&format!(
                "{} [{}] exited with {code} after {}s",
                gone.name, gone.pid, gone.ran_for
            )),
            State::Killed(signal) => kmsg::log(&format!(
                "{} [{}] killed by signal {signal} after {}s",
                gone.name, gone.pid, gone.ran_for
            )),
            State::Running => {}
        }
        shortest_life = Some(shortest_life.map_or(gone.ran_for, |s: u64| s.min(gone.ran_for)));
    }

    table.forget_finished();
    shortest_life
}
