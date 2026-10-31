//! Command server (TCP 8300): one client at a time, one thread, driven by the session clock.

use crate::control::{Command, Event, LineReader};
use crate::session::{Session, Timings};
use std::io::{self, ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

/// Shortest socket wait; a zero read timeout would mean "block forever".
const MIN_WAIT: Duration = Duration::from_millis(1);

/// Serves clients one after another until accepting fails.
pub fn serve(listener: TcpListener, timings: Timings) -> io::Result<()> {
    for stream in listener.incoming() {
        if let Err(e) = serve_client(stream?, timings) {
            eprintln!("client error: {e}");
        }
    }
    Ok(())
}

/// Runs one client connection until the client closes it.
pub fn serve_client(mut stream: TcpStream, timings: Timings) -> io::Result<()> {
    stream.set_nodelay(true)?;
    let clock = Instant::now();
    let mut session = Session::new(timings, Duration::ZERO)
        .map_err(|e| io::Error::new(ErrorKind::InvalidInput, format!("invalid timings: {e:?}")))?;
    let mut reader = LineReader::default();
    let mut buf = [0u8; 1024];
    loop {
        let wait = session
            .next_deadline()
            .map_or(Duration::from_secs(3600), |d| d.saturating_sub(clock.elapsed()));
        stream.set_read_timeout(Some(wait.max(MIN_WAIT)))?;
        match stream.read(&mut buf) {
            Ok(0) => return Ok(()),
            Ok(n) => {
                let lines = reader
                    .push(&buf[..n])
                    .map_err(|_| io::Error::new(ErrorKind::InvalidData, "command line too long"))?;
                for line in lines {
                    let events = session.on_command(clock.elapsed(), Command::parse(&line));
                    send(&mut stream, &events)?;
                }
            }
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(e) => return Err(e),
        }
        // Fire timers at their scheduled times, not at the wake-up time, so the schedule never drifts.
        while let Some(d) = session.next_deadline().filter(|&d| d <= clock.elapsed()) {
            let events = session.on_timer(d);
            send(&mut stream, &events)?;
        }
    }
}

/// One batch of events goes out as one write, so it travels as one segment like VARA's.
fn send(stream: &mut TcpStream, events: &[Event]) -> io::Result<()> {
    if events.is_empty() {
        return Ok(());
    }
    stream.write_all(&Event::encode(events))
}
