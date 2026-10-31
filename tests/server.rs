//! The command server over real loopback sockets, on a clock 100 times faster than the captures.

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::{Duration, Instant};
use vara_native::control::LineReader;
use vara_native::server::serve_client;
use vara_native::session::Timings;

const SCALE: f64 = 0.01;

/// Test client: one line buffer for the whole connection, so lines that arrive together are never lost.
struct Client {
    stream: TcpStream,
    reader: LineReader,
    pending: VecDeque<(Duration, String)>,
    t0: Instant,
}

impl Client {
    fn send(&mut self, bytes: &[u8]) {
        self.stream.write_all(bytes).unwrap();
    }

    /// Reads lines until `stop` is seen; returns them with arrival times. Fails after 5 s overall.
    fn read_until(&mut self, stop: &str) -> Vec<(Duration, String)> {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut out = Vec::new();
        let mut buf = [0u8; 256];
        loop {
            while let Some((t, line)) = self.pending.pop_front() {
                let done = line == stop;
                out.push((t, line));
                if done {
                    return out;
                }
            }
            assert!(Instant::now() < deadline, "no {stop:?} within 5 s; got {out:?}");
            let n = self.stream.read(&mut buf).expect("server went silent");
            assert!(n > 0, "server closed the connection");
            let t = self.t0.elapsed();
            self.pending.extend(self.reader.push(&buf[..n]).unwrap().into_iter().map(|l| (t, l)));
        }
    }
}

fn start() -> (Client, thread::JoinHandle<std::io::Result<()>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (stream, _) = listener.accept()?;
        serve_client(stream, Timings::observed().scaled(SCALE))
    });
    let stream = TcpStream::connect(addr).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let client = Client { stream, reader: LineReader::default(), pending: VecDeque::new(), t0: Instant::now() };
    (client, server)
}

fn lines(v: &[(Duration, String)]) -> Vec<&str> {
    v.iter().map(|(_, l)| l.as_str()).collect()
}

#[test]
fn acknowledges_commands() {
    let (mut c, _server) = start();
    c.send(b"WINLINK SESSION\r");
    assert_eq!(lines(&c.read_until("OK")), ["OK"]);
    c.send(b"BW2300\r");
    assert_eq!(lines(&c.read_until("OK")), ["OK"]);
}

#[test]
fn runs_an_unanswered_call_to_the_end() {
    let (mut c, _server) = start();
    c.send(b"CONNECT N0CALL N1CALL\r");
    let got = c.read_until("BUSY OFF");

    let mut expected = vec!["OK", "PTT ON", "BUSY ON", "PTT OFF"];
    for _ in 0..14 {
        expected.extend(["PTT ON", "PTT OFF"]);
    }
    expected.extend(["DISCONNECTED", "BUSY OFF"]);
    assert_eq!(lines(&got), expected);

    // 41.317 s in the capture; generous bound for a loaded test machine.
    let total = got.last().unwrap().0.as_secs_f64();
    assert!((total - 41.317 * SCALE).abs() < 0.15, "call took {total:.3} s");
}

#[test]
fn abort_ends_the_call() {
    let (mut c, _server) = start();
    c.send(b"CONNECT N0CALL N1CALL\r");
    c.read_until("BUSY ON");
    c.send(b"ABORT\r");
    assert_eq!(lines(&c.read_until("BUSY OFF")), ["OK", "PTT OFF", "DISCONNECTED", "BUSY OFF"]);
}

#[test]
fn sends_keepalives() {
    let (mut c, _server) = start();
    let got = c.read_until("IAMALIVE");
    assert_eq!(lines(&got), ["IAMALIVE"]);
    let t = got[0].0.as_secs_f64();
    assert!((t - 60.0 * SCALE).abs() < 0.15, "first keepalive after {t:.3} s");
}

#[test]
fn handles_commands_split_across_writes() {
    let (mut c, _server) = start();
    c.send(b"WINLINK SES");
    thread::sleep(Duration::from_millis(20));
    c.send(b"SION\rBW2300\r");
    assert_eq!(lines(&c.read_until("OK")), ["OK"]);
    assert_eq!(lines(&c.read_until("OK")), ["OK"]);
}

#[test]
fn returns_when_the_client_leaves() {
    let (c, server) = start();
    drop(c);
    assert!(server.join().unwrap().is_ok());
}

#[test]
fn closes_a_client_that_never_ends_its_line() {
    let (mut c, server) = start();
    c.send(&vec![b'A'; vara_native::control::MAX_LINE + 1]);
    let mut buf = [0u8; 16];
    assert_eq!(c.stream.read(&mut buf).unwrap(), 0, "server should close the connection");
    let err = server.join().unwrap().unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
}

#[test]
fn refuses_invalid_timings() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (stream, _) = listener.accept()?;
        serve_client(stream, Timings { keepalive: Duration::ZERO, ..Timings::observed() })
    });
    let _client = TcpStream::connect(addr).unwrap();
    assert_eq!(server.join().unwrap().unwrap_err().kind(), std::io::ErrorKind::InvalidInput);
}
