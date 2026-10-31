//! Command channel (TCP 8300): CR-terminated ASCII lines.

use crate::dat::Bandwidth;

const CR: u8 = b'\r';

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
        let words: Vec<&str> = line.split(' ').collect();
        match words.as_slice() {
            ["WINLINK", "SESSION"] => Command::WinlinkSession,
            ["ABORT"] => Command::Abort,
            ["CONNECT", from, to] => Command::Connect { from: from.to_string(), to: to.to_string() },
            ["BW500"] => Command::Bandwidth(Bandwidth::Hz500),
            ["BW2300"] => Command::Bandwidth(Bandwidth::Hz2300),
            ["BW2750"] => Command::Bandwidth(Bandwidth::Hz2750),
            _ => Command::Unknown(line.to_string()),
        }
    }

    pub fn to_wire(&self) -> Vec<u8> {
        let line = match self {
            Command::WinlinkSession => "WINLINK SESSION".to_string(),
            Command::Connect { from, to } => format!("CONNECT {from} {to}"),
            Command::Abort => "ABORT".to_string(),
            Command::Bandwidth(Bandwidth::Hz500) => "BW500".to_string(),
            Command::Bandwidth(Bandwidth::Hz2300) => "BW2300".to_string(),
            Command::Bandwidth(Bandwidth::Hz2750) => "BW2750".to_string(),
            Command::Unknown(s) => s.clone(),
        };
        terminated(&line)
    }
}

impl Event {
    pub fn parse(line: &str) -> Event {
        match line {
            "OK" => Event::Ok,
            "PTT ON" => Event::PttOn,
            "PTT OFF" => Event::PttOff,
            "BUSY ON" => Event::BusyOn,
            "BUSY OFF" => Event::BusyOff,
            "DISCONNECTED" => Event::Disconnected,
            "IAMALIVE" => Event::IAmAlive,
            _ => Event::Unknown(line.to_string()),
        }
    }

    fn as_str(&self) -> &str {
        match self {
            Event::Ok => "OK",
            Event::PttOn => "PTT ON",
            Event::PttOff => "PTT OFF",
            Event::BusyOn => "BUSY ON",
            Event::BusyOff => "BUSY OFF",
            Event::Disconnected => "DISCONNECTED",
            Event::IAmAlive => "IAMALIVE",
            Event::Unknown(s) => s,
        }
    }

    /// Several events sent as one segment.
    pub fn encode(events: &[Event]) -> Vec<u8> {
        events.iter().flat_map(|e| terminated(e.as_str())).collect()
    }
}

fn terminated(line: &str) -> Vec<u8> {
    let mut v = line.as_bytes().to_vec();
    v.push(CR);
    v
}

/// Splits an incoming byte stream into lines.
#[derive(Debug, Default)]
pub struct LineReader {
    buf: Vec<u8>,
}

impl LineReader {
    pub fn push(&mut self, bytes: &[u8]) -> Vec<String> {
        self.buf.extend_from_slice(bytes);
        let Some(last) = self.buf.iter().rposition(|&b| b == CR) else { return Vec::new() };
        let complete: Vec<u8> = self.buf.drain(..=last).collect();
        complete
            .split(|&b| b == CR)
            .filter(|l| !l.is_empty())
            .map(|l| String::from_utf8_lossy(l).into_owned())
            .collect()
    }
}
