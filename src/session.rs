//! Command channel behaviour on a caller-supplied clock: no I/O, no threads.

use crate::control::{Command, Event};
use std::time::Duration;

/// Call timing, fitted to captured traffic.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Timings {
    pub first_tx: Duration,
    pub first_period: Duration,
    pub tx: Duration,
    pub listen: Duration,
    pub hangup: Duration,
    pub attempts: u8,
    pub keepalive: Duration,
}

impl Timings {
    pub fn observed() -> Timings {
        todo!()
    }

    pub fn scaled(self, factor: f64) -> Timings {
        let _ = factor;
        todo!()
    }
}

#[derive(Debug)]
pub struct Session {
    timings: Timings,
}

impl Session {
    pub fn new(timings: Timings, now: Duration) -> Session {
        let _ = now;
        Session { timings }
    }

    pub fn on_command(&mut self, now: Duration, command: Command) -> Vec<Event> {
        let _ = (now, command, self.timings);
        todo!()
    }

    pub fn next_deadline(&self) -> Option<Duration> {
        todo!()
    }

    pub fn on_timer(&mut self, now: Duration) -> Vec<Event> {
        let _ = now;
        todo!()
    }
}
