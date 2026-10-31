//! Command channel (TCP 8300): CR-terminated ASCII lines.

use crate::dat::Bandwidth;

/// Application to modem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    WinlinkSession,
    Connect { from: String, to: String },
    Abort,
    Bandwidth(Bandwidth),
    /// Not modelled yet; kept verbatim.
    Unknown(String),
}

/// Modem to application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Ok,
    PttOn,
    PttOff,
    BusyOn,
    BusyOff,
    Disconnected,
    IAmAlive,
    /// Not modelled yet; kept verbatim.
    Unknown(String),
}

impl Command {
    pub fn parse(line: &str) -> Command {
        let _ = line;
        todo!()
    }

    pub fn to_wire(&self) -> Vec<u8> {
        todo!()
    }
}

impl Event {
    pub fn parse(line: &str) -> Event {
        let _ = line;
        todo!()
    }

    /// Several events sent as one segment.
    pub fn encode(events: &[Event]) -> Vec<u8> {
        let _ = events;
        todo!()
    }
}

/// Splits an incoming byte stream into lines.
#[derive(Debug, Default)]
pub struct LineReader {
    buf: Vec<u8>,
}

impl LineReader {
    pub fn push(&mut self, bytes: &[u8]) -> Vec<String> {
        let _ = (bytes, &self.buf);
        todo!()
    }
}
