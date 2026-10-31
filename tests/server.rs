//! The command server over real loopback sockets, on a clock 100 times faster than the captures.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::{Duration, Instant};
use vara_native::control::LineReader;
use vara_native::server::serve_client;
use vara_native::session::Timings;

const SCALE: f64 = 0.01;

fn start() -> (TcpStream, thread::JoinHandle<std::io::Result<()>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (stream, _) = listener.accept()?;
        serve_client(stream, Timings::observed().scaled(SCALE))
    });
    let client = TcpStream::connect(addr).unwrap();
    client.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    (client, server)
}

/// Reads lines until `stop` is seen; returns them with arrival times.
fn read_until(client: &mut TcpStream, stop: &str) -> Vec<(Duration, String)> {
    let t0 = Instant::now();
    let mut reader = LineReader::default();
    let mut out = Vec::new();
    let mut buf = [0u8; 256];
    loop {
        let n = client.read(&mut buf).expect("server went silent");
        assert!(n > 0, "server closed the connection");
        for line in reader.push(&buf[..n]) {
            let done = line == stop;
            out.push((t0.elapsed(), line));
            if done {
                return out;
            }
        }
    }
}

fn lines(v: &[(Duration, String)]) -> Vec<&str> {
    v.iter().map(|(_, l)| l.as_str()).collect()
}

#[test]
fn acknowledges_commands() {
    let (mut c, _server) = start();
    c.write_all(b"WINLINK SESSION\r").unwrap();
    assert_eq!(lines(&read_until(&mut c, "OK")), ["OK"]);
    c.write_all(b"BW2300\r").unwrap();
    assert_eq!(lines(&read_until(&mut c, "OK")), ["OK"]);
}

#[test]
fn runs_an_unanswered_call_to_the_end() {
    let (mut c, _server) = start();
    c.write_all(b"CONNECT N0CALL N1CALL\r").unwrap();
    let got = read_until(&mut c, "BUSY OFF");

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
    c.write_all(b"CONNECT N0CALL N1CALL\r").unwrap();
    read_until(&mut c, "BUSY ON");
    c.write_all(b"ABORT\r").unwrap();
    assert_eq!(lines(&read_until(&mut c, "BUSY OFF")), ["OK", "PTT OFF", "DISCONNECTED", "BUSY OFF"]);
}

#[test]
fn sends_keepalives() {
    let (mut c, _server) = start();
    let got = read_until(&mut c, "IAMALIVE");
    assert_eq!(lines(&got), ["IAMALIVE"]);
    let t = got[0].0.as_secs_f64();
    assert!((t - 60.0 * SCALE).abs() < 0.15, "first keepalive after {t:.3} s");
}

#[test]
fn handles_commands_split_across_writes() {
    let (mut c, _server) = start();
    c.write_all(b"WINLINK SES").unwrap();
    c.flush().unwrap();
    thread::sleep(Duration::from_millis(20));
    c.write_all(b"SION\rBW2300\r").unwrap();
    assert_eq!(lines(&read_until(&mut c, "OK")), ["OK"]);
    assert_eq!(lines(&read_until(&mut c, "OK")), ["OK"]);
}

#[test]
fn returns_when_the_client_leaves() {
    let (c, server) = start();
    drop(c);
    assert!(server.join().unwrap().is_ok());
}
