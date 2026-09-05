// SPDX-License-Identifier: GPL-3.0-or-later

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum State {
    Inactive,
    Starting,
    Running,
    Stopping,
    Failed,
    Restarting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Start,
    Stop,
    CameUp,
    ExitedCleanly,
    ExitedInError,
    Gone,
    RanOutOfTime,
    RetryScheduled,
    RetryDue,
}

impl State {
    pub fn settled(self) -> bool {
        matches!(self, State::Inactive | State::Running | State::Failed)
    }

    pub fn holds_a_process(self) -> bool {
        matches!(self, State::Starting | State::Running | State::Stopping)
    }

    pub fn after(self, event: Event) -> Option<State> {
        use Event::*;
        use State::*;
        let next = match (self, event) {
            (Inactive, Start) => Starting,
            (Inactive, Stop) => Inactive,

            (Starting, CameUp) => Running,
            (Starting, ExitedCleanly) => Inactive,
            (Starting, ExitedInError) => Failed,
            (Starting, RanOutOfTime) => Failed,
            (Starting, Stop) => Stopping,
            (Starting, Start) => Starting,

            (Running, ExitedCleanly) => Inactive,
            (Running, ExitedInError) => Failed,
            (Running, Stop) => Stopping,
            (Running, Start) => Running,

            (Stopping, Gone) => Inactive,
            (Stopping, ExitedCleanly) => Inactive,
            (Stopping, ExitedInError) => Inactive,
            (Stopping, RanOutOfTime) => Failed,
            (Stopping, Stop) => Stopping,

            (Failed, Start) => Starting,
            (Failed, RetryScheduled) => Restarting,
            (Failed, Stop) => Inactive,

            (Restarting, RetryDue) => Starting,
            (Restarting, Start) => Starting,
            (Restarting, Stop) => Inactive,

            _ => return None,
        };
        Some(next)
    }
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let word = match self {
            State::Inactive => "inactive",
            State::Starting => "starting",
            State::Running => "running",
            State::Stopping => "stopping",
            State::Failed => "failed",
            State::Restarting => "restarting",
        };
        f.write_str(word)
    }
}

#[cfg(test)]
mod tests {
    use super::{Event, State};

    const EVERY_STATE: [State; 6] = [
        State::Inactive,
        State::Starting,
        State::Running,
        State::Stopping,
        State::Failed,
        State::Restarting,
    ];

    const EVERY_EVENT: [Event; 9] = [
        Event::Start,
        Event::Stop,
        Event::CameUp,
        Event::ExitedCleanly,
        Event::ExitedInError,
        Event::Gone,
        Event::RanOutOfTime,
        Event::RetryScheduled,
        Event::RetryDue,
    ];

    #[test]
    fn a_clean_start_walks_inactive_to_running() {
        let started = State::Inactive.after(Event::Start).expect("start applies");
        assert_eq!(started, State::Starting);
        assert_eq!(started.after(Event::CameUp), Some(State::Running));
    }

    #[test]
    fn a_clean_stop_walks_running_back_to_inactive() {
        let stopping = State::Running.after(Event::Stop).expect("stop applies");
        assert_eq!(stopping, State::Stopping);
        assert_eq!(stopping.after(Event::Gone), Some(State::Inactive));
    }

    #[test]
    fn dying_badly_fails_from_either_side_of_coming_up() {
        assert_eq!(
            State::Starting.after(Event::ExitedInError),
            Some(State::Failed)
        );
        assert_eq!(
            State::Running.after(Event::ExitedInError),
            Some(State::Failed)
        );
    }

    #[test]
    fn exiting_cleanly_is_not_a_failure() {
        assert_eq!(
            State::Running.after(Event::ExitedCleanly),
            Some(State::Inactive)
        );
        assert_eq!(
            State::Starting.after(Event::ExitedCleanly),
            Some(State::Inactive)
        );
    }

    #[test]
    fn a_restart_goes_through_restarting_and_lands_on_starting() {
        let waiting = State::Failed
            .after(Event::RetryScheduled)
            .expect("a retry was scheduled");
        assert_eq!(waiting, State::Restarting);
        assert_eq!(waiting.after(Event::RetryDue), Some(State::Starting));
    }

    #[test]
    fn stopping_a_unit_that_is_waiting_to_retry_cancels_the_retry() {
        assert_eq!(State::Restarting.after(Event::Stop), Some(State::Inactive));
    }

    #[test]
    fn running_out_of_time_fails_on_the_way_up_and_on_the_way_down() {
        assert_eq!(
            State::Starting.after(Event::RanOutOfTime),
            Some(State::Failed)
        );
        assert_eq!(
            State::Stopping.after(Event::RanOutOfTime),
            Some(State::Failed)
        );
    }

    #[test]
    fn a_process_dying_while_we_stop_it_is_what_we_asked_for() {
        assert_eq!(
            State::Stopping.after(Event::ExitedInError),
            Some(State::Inactive)
        );
        assert_eq!(
            State::Stopping.after(Event::ExitedCleanly),
            Some(State::Inactive)
        );
    }

    #[test]
    fn asking_twice_changes_nothing() {
        assert_eq!(State::Running.after(Event::Start), Some(State::Running));
        assert_eq!(State::Inactive.after(Event::Stop), Some(State::Inactive));
        assert_eq!(State::Starting.after(Event::Start), Some(State::Starting));
        assert_eq!(State::Stopping.after(Event::Stop), Some(State::Stopping));
    }

    #[test]
    fn a_failed_unit_can_be_started_by_hand_or_given_up_on() {
        assert_eq!(State::Failed.after(Event::Start), Some(State::Starting));
        assert_eq!(State::Failed.after(Event::Stop), Some(State::Inactive));
    }

    #[test]
    fn events_that_make_no_sense_are_refused_rather_than_guessed() {
        assert_eq!(State::Inactive.after(Event::CameUp), None);
        assert_eq!(State::Inactive.after(Event::ExitedCleanly), None);
        assert_eq!(State::Running.after(Event::CameUp), None);
        assert_eq!(State::Running.after(Event::RetryDue), None);
        assert_eq!(State::Failed.after(Event::CameUp), None);
        assert_eq!(State::Restarting.after(Event::CameUp), None);
    }

    #[test]
    fn nothing_ever_leaves_the_six_states() {
        for state in EVERY_STATE {
            for event in EVERY_EVENT {
                if let Some(next) = state.after(event) {
                    assert!(
                        EVERY_STATE.contains(&next),
                        "{state} + {event:?} left the machine"
                    );
                }
            }
        }
    }

    #[test]
    fn every_state_can_be_reached_from_inactive() {
        let mut reached = std::collections::HashSet::from([State::Inactive]);
        loop {
            let grown: std::collections::HashSet<State> = reached
                .iter()
                .flat_map(|s| EVERY_EVENT.iter().filter_map(|e| s.after(*e)))
                .chain(reached.iter().copied())
                .collect();
            if grown.len() == reached.len() {
                break;
            }
            reached = grown;
        }
        for state in EVERY_STATE {
            assert!(
                reached.contains(&state),
                "{state} is unreachable, so it is dead weight"
            );
        }
    }

    #[test]
    fn every_state_can_get_back_to_inactive() {
        for state in EVERY_STATE {
            let mut seen = std::collections::HashSet::from([state]);
            let mut frontier = vec![state];
            while let Some(here) = frontier.pop() {
                for event in EVERY_EVENT {
                    if let Some(next) = here.after(event) {
                        if seen.insert(next) {
                            frontier.push(next);
                        }
                    }
                }
            }
            assert!(
                seen.contains(&State::Inactive),
                "{state} can never shut down"
            );
        }
    }

    #[test]
    fn the_settled_states_are_the_ones_without_a_pending_move() {
        assert!(State::Inactive.settled() && State::Running.settled() && State::Failed.settled());
        assert!(!State::Starting.settled());
        assert!(!State::Stopping.settled());
        assert!(!State::Restarting.settled());
    }

    #[test]
    fn only_the_states_around_a_live_process_claim_one() {
        assert!(State::Starting.holds_a_process());
        assert!(State::Running.holds_a_process());
        assert!(State::Stopping.holds_a_process());
        assert!(!State::Inactive.holds_a_process());
        assert!(!State::Failed.holds_a_process());
        assert!(!State::Restarting.holds_a_process());
    }

    #[test]
    fn the_names_are_the_ones_the_cli_will_print() {
        let printed: Vec<String> = EVERY_STATE.iter().map(|s| s.to_string()).collect();
        assert_eq!(
            printed,
            [
                "inactive",
                "starting",
                "running",
                "stopping",
                "failed",
                "restarting"
            ]
        );
    }
}
