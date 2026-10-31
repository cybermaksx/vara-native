//! Command channel (TCP 8300). Every byte string below was observed on the wire between
//! Winlink Express and VARA HF 4.9.0; callsigns are replaced with placeholders.

use vara_native::control::{Command, Event, LineReader};
use vara_native::dat::Bandwidth;

#[test]
fn parses_app_commands() {
    assert_eq!(Command::parse("WINLINK SESSION"), Command::WinlinkSession);
    assert_eq!(Command::parse("CONNECT N0CALL N1CALL"), Command::Connect { from: "N0CALL".into(), to: "N1CALL".into() });
    assert_eq!(Command::parse("ABORT"), Command::Abort);
    assert_eq!(Command::parse("BW2300"), Command::Bandwidth(Bandwidth::Hz2300));
    assert_eq!(Command::parse("BW500"), Command::Bandwidth(Bandwidth::Hz500));
    assert_eq!(Command::parse("BW2750"), Command::Bandwidth(Bandwidth::Hz2750));
}

#[test]
fn unknown_commands_are_kept_verbatim() {
    assert_eq!(Command::parse("MYCALL N0CALL"), Command::Unknown("MYCALL N0CALL".into()));
    assert_eq!(Command::parse("CONNECT N0CALL"), Command::Unknown("CONNECT N0CALL".into()));
    assert_eq!(Command::parse("BW1000"), Command::Unknown("BW1000".into()));
}

#[test]
fn commands_encode_to_observed_bytes() {
    assert_eq!(Command::WinlinkSession.to_wire(), b"WINLINK SESSION\r");
    assert_eq!(Command::Connect { from: "N0CALL".into(), to: "N1CALL".into() }.to_wire(), b"CONNECT N0CALL N1CALL\r");
    assert_eq!(Command::Abort.to_wire(), b"ABORT\r");
    assert_eq!(Command::Bandwidth(Bandwidth::Hz2300).to_wire(), b"BW2300\r");
}

#[test]
fn events_encode_to_observed_bytes() {
    assert_eq!(Event::encode(&[Event::Ok]), b"OK\r");
    assert_eq!(Event::encode(&[Event::PttOn]), b"PTT ON\r");
    assert_eq!(Event::encode(&[Event::PttOff]), b"PTT OFF\r");
    assert_eq!(Event::encode(&[Event::BusyOn]), b"BUSY ON\r");
    assert_eq!(Event::encode(&[Event::BusyOff]), b"BUSY OFF\r");
    assert_eq!(Event::encode(&[Event::IAmAlive]), b"IAMALIVE\r");
}

// VARA answers CONNECT with three messages in a single TCP segment, and ends a call with two.
#[test]
fn grouped_events_encode_to_observed_segments() {
    assert_eq!(Event::encode(&[Event::Ok, Event::PttOn, Event::BusyOn]), b"OK\rPTT ON\rBUSY ON\r");
    assert_eq!(Event::encode(&[Event::Disconnected, Event::BusyOff]), b"DISCONNECTED\rBUSY OFF\r");
}

#[test]
fn parses_modem_events() {
    for (line, ev) in [
        ("OK", Event::Ok),
        ("PTT ON", Event::PttOn),
        ("PTT OFF", Event::PttOff),
        ("BUSY ON", Event::BusyOn),
        ("BUSY OFF", Event::BusyOff),
        ("DISCONNECTED", Event::Disconnected),
        ("IAMALIVE", Event::IAmAlive),
    ] {
        assert_eq!(Event::parse(line), ev);
    }
    assert_eq!(Event::parse("CONNECTED N0CALL N1CALL 2300"), Event::Unknown("CONNECTED N0CALL N1CALL 2300".into()));
}

// Messages end with CR only; one segment may carry several, and one message may span segments.
#[test]
fn line_reader_splits_on_cr() {
    let mut r = LineReader::default();
    assert_eq!(r.push(b"OK\rPTT ON\rBUSY ON\r"), ["OK", "PTT ON", "BUSY ON"]);
    assert!(r.push(b"PTT O").is_empty());
    assert_eq!(r.push(b"FF\rDISCONN"), ["PTT OFF"]);
    assert_eq!(r.push(b"ECTED\r"), ["DISCONNECTED"]);
}

#[test]
fn line_reader_does_not_treat_lf_as_terminator() {
    let mut r = LineReader::default();
    assert!(r.push(b"OK\n").is_empty());
    assert_eq!(r.push(b"\r"), ["OK\n"]);
}

#[test]
fn line_reader_skips_empty_lines() {
    let mut r = LineReader::default();
    assert_eq!(r.push(b"\r\rOK\r\r"), ["OK"]);
}
