// SPDX-License-Identifier: GPL-3.0-or-later

use crate::proc::Spec;

const STRIKES_BEFORE_MOVING_ON: u32 = 3;

pub const CANDIDATES: &[Spec] = &[
    Spec {
        name: "shell",
        path: "/bin/sh",
        argv: &["sh"],
        wants_own_session: true,
    },
    Spec {
        name: "shell",
        path: "/bin/busybox",
        argv: &["busybox", "sh"],
        wants_own_session: true,
    },
];

#[derive(PartialEq, Eq, Debug)]
pub enum Verdict {
    KeepTrying,
    MovedOn(&'static Spec),
    OutOfOptions,
}

pub struct Rescue {
    usable: Vec<&'static Spec>,
    at: usize,
    strikes: u32,
}

impl Rescue {
    pub fn new(candidates: &'static [Spec]) -> Option<Self> {
        Self::over(candidates.iter().filter(|s| s.is_available()).collect())
    }

    fn over(usable: Vec<&'static Spec>) -> Option<Self> {
        if usable.is_empty() {
            return None;
        }
        Some(Rescue {
            usable,
            at: 0,
            strikes: 0,
        })
    }

    pub fn shell(&self) -> &'static Spec {
        self.usable[self.at]
    }

    pub fn survived(&mut self) {
        self.strikes = 0;
    }

    pub fn failed(&mut self) -> Verdict {
        self.strikes += 1;
        if self.strikes < STRIKES_BEFORE_MOVING_ON {
            return Verdict::KeepTrying;
        }
        if self.at + 1 >= self.usable.len() {
            return Verdict::OutOfOptions;
        }
        self.at += 1;
        self.strikes = 0;
        Verdict::MovedOn(self.shell())
    }
}

#[cfg(test)]
mod tests {
    use super::{CANDIDATES, Rescue, STRIKES_BEFORE_MOVING_ON, Verdict};
    use crate::proc::Spec;

    static FIRST: Spec = Spec {
        name: "shell",
        path: "/first",
        argv: &["first"],
        wants_own_session: true,
    };
    static SECOND: Spec = Spec {
        name: "shell",
        path: "/second",
        argv: &["second"],
        wants_own_session: true,
    };

    fn two() -> Rescue {
        Rescue::over(vec![&FIRST, &SECOND]).expect("two candidates is not empty")
    }

    #[test]
    fn with_nothing_available_there_is_no_rescue() {
        assert!(Rescue::over(Vec::new()).is_none());
    }

    #[test]
    fn the_first_candidate_is_the_one_we_start_with() {
        assert_eq!(two().shell().path, "/first");
    }

    #[test]
    fn a_couple_of_failures_are_not_enough_to_give_up_on_a_shell() {
        let mut rescue = two();
        for _ in 1..STRIKES_BEFORE_MOVING_ON {
            assert_eq!(rescue.failed(), Verdict::KeepTrying);
        }
        assert_eq!(rescue.shell().path, "/first");
    }

    #[test]
    fn enough_failures_move_on_to_the_next_shell() {
        let mut rescue = two();
        let mut verdict = Verdict::KeepTrying;
        for _ in 0..STRIKES_BEFORE_MOVING_ON {
            verdict = rescue.failed();
        }
        assert_eq!(verdict, Verdict::MovedOn(&SECOND));
        assert_eq!(rescue.shell().path, "/second");
    }

    #[test]
    fn surviving_wipes_the_slate() {
        let mut rescue = two();
        for _ in 1..STRIKES_BEFORE_MOVING_ON {
            rescue.failed();
        }
        rescue.survived();
        for _ in 1..STRIKES_BEFORE_MOVING_ON {
            assert_eq!(rescue.failed(), Verdict::KeepTrying);
        }
        assert_eq!(rescue.shell().path, "/first");
    }

    #[test]
    fn running_out_of_shells_is_the_end_of_the_road() {
        let mut rescue = Rescue::over(vec![&FIRST]).expect("one candidate is not empty");
        let mut verdict = Verdict::KeepTrying;
        for _ in 0..STRIKES_BEFORE_MOVING_ON {
            verdict = rescue.failed();
        }
        assert_eq!(verdict, Verdict::OutOfOptions);
    }

    #[test]
    fn every_candidate_passes_its_own_name_as_argv0() {
        for shell in CANDIDATES {
            let argv0 = shell.argv.first().expect("a shell needs an argv[0]");
            assert!(
                shell.path.ends_with(argv0),
                "{} would be exec'd as {argv0:?}, which picks the wrong busybox applet",
                shell.path
            );
        }
    }

    #[test]
    fn every_candidate_gets_its_own_session() {
        for shell in CANDIDATES {
            assert!(shell.wants_own_session, "{} needs job control", shell.path);
        }
    }

    #[test]
    fn the_fallback_is_not_the_same_binary_as_the_first_choice() {
        assert!(
            CANDIDATES.len() > 1,
            "a single shell leaves nothing to fall back to"
        );
        assert_ne!(CANDIDATES[0].path, CANDIDATES[1].path);
    }
}
