//! Command channel behaviour on a caller-supplied clock: no I/O, no threads.
//!
//! The caller feeds commands with `on_command`, sleeps until `next_deadline`, then calls
//! `on_timer`. Every returned batch of events is meant to go out as one TCP segment.

use crate::control::{Command, Event};
use std::time::Duration;

/// Call timing, fitted to captured traffic.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Timings {
    /// First call frame, PTT on to PTT off.
    pub first_tx: Duration,
    /// First PTT on to second PTT on.
    pub first_period: Duration,
    /// Later call frames, PTT on to PTT off.
    pub tx: Duration,
    /// Listening for an answer between later frames.
    pub listen: Duration,
    /// Last PTT off to DISCONNECTED.
    pub hangup: Duration,
    pub attempts: u8,
    pub keepalive: Duration,
}

impl Timings {
    /// Fitted to a 15-attempt unanswered call: every event within 38 ms of the capture.
    pub fn observed() -> Timings {
        Timings {
            first_tx: Duration::from_millis(1786),
            first_period: Duration::from_millis(3110),
            tx: Duration::from_millis(1431),
            listen: Duration::from_millis(1295),
            hangup: Duration::from_millis(1320),
            attempts: 15,
            keepalive: Duration::from_secs(60),
        }
    }

    /// Same behaviour on a faster clock, for tests over real sockets.
    pub fn scaled(self, factor: f64) -> Timings {
        let s = |d: Duration| d.mul_f64(factor);
        Timings {
            first_tx: s(self.first_tx),
            first_period: s(self.first_period),
            tx: s(self.tx),
            listen: s(self.listen),
            hangup: s(self.hangup),
            attempts: self.attempts,
            keepalive: s(self.keepalive),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimingsError {
    ZeroKeepalive,
    ZeroAttempts,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum State {
    Idle,
    /// PTT on, frame ends at `until`.
    Transmitting { attempt: u8, until: Duration, aborted: bool },
    /// PTT off, next frame starts at `until`.
    Listening { attempt: u8, until: Duration },
    HangingUp { until: Duration },
}

#[derive(Debug)]
pub struct Session {
    timings: Timings,
    state: State,
    next_keepalive: Duration,
}

impl Session {
    pub fn new(timings: Timings, now: Duration) -> Result<Session, TimingsError> {
        Ok(Session { timings, state: State::Idle, next_keepalive: now + timings.keepalive })
    }

    pub fn on_command(&mut self, now: Duration, command: Command) -> Vec<Event> {
        match command {
            Command::WinlinkSession | Command::Bandwidth(_) => vec![Event::Ok],
            Command::Connect { .. } if self.state == State::Idle => {
                self.state = State::Transmitting { attempt: 1, until: now + self.timings.first_tx, aborted: false };
                vec![Event::Ok, Event::PttOn, Event::BusyOn]
            }
            Command::Abort => {
                match self.state {
                    State::Transmitting { attempt, until, .. } => {
                        self.state = State::Transmitting { attempt, until, aborted: true };
                    }
                    // Not in the captures; hanging up as after the last frame.
                    State::Listening { .. } => self.state = State::HangingUp { until: now + self.timings.hangup },
                    State::Idle | State::HangingUp { .. } => {}
                }
                vec![Event::Ok]
            }
            Command::Connect { .. } | Command::Unknown(_) => Vec::new(),
        }
    }

    pub fn next_deadline(&self) -> Option<Duration> {
        let state = match self.state {
            State::Idle => None,
            State::Transmitting { until, .. } | State::Listening { until, .. } | State::HangingUp { until } => Some(until),
        };
        Some(state.map_or(self.next_keepalive, |s| s.min(self.next_keepalive)))
    }

    pub fn on_timer(&mut self, now: Duration) -> Vec<Event> {
        let mut out = Vec::new();
        while self.next_keepalive <= now {
            if self.state == State::Idle {
                out.push(Event::IAmAlive);
            }
            self.next_keepalive += self.timings.keepalive;
        }
        let t = self.timings;
        match self.state {
            State::Transmitting { attempt, until, aborted } if until <= now => {
                out.push(Event::PttOff);
                self.state = if aborted || attempt == t.attempts {
                    State::HangingUp { until: until + t.hangup }
                } else if attempt == 1 {
                    State::Listening { attempt, until: until - t.first_tx + t.first_period }
                } else {
                    State::Listening { attempt, until: until + t.listen }
                };
            }
            State::Listening { attempt, until } if until <= now => {
                out.push(Event::PttOn);
                self.state = State::Transmitting { attempt: attempt + 1, until: until + t.tx, aborted: false };
            }
            State::HangingUp { until } if until <= now => {
                out.extend([Event::Disconnected, Event::BusyOff]);
                self.state = State::Idle;
            }
            _ => {}
        }
        out
    }
}
