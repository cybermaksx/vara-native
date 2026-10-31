use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Command, Stdio};
use std::time::Duration;

#[test]
fn binary_serves_the_command_port() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_vara-native"))
        .args(["--bind", "127.0.0.1:0"])
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stderr = BufReader::new(child.stderr.take().unwrap());
    let mut line = String::new();
    stderr.read_line(&mut line).unwrap();
    let addr = line.trim().strip_prefix("listening on ").unwrap_or_else(|| panic!("unexpected: {line:?}")).to_string();

    let mut c = TcpStream::connect(&addr).unwrap();
    c.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    c.write_all(b"WINLINK SESSION\r").unwrap();
    let mut buf = [0u8; 3];
    c.read_exact(&mut buf).unwrap();
    assert_eq!(&buf, b"OK\r");

    child.kill().unwrap();
    child.wait().unwrap();
}

#[test]
fn rejects_unknown_arguments() {
    let out = Command::new(env!("CARGO_BIN_EXE_vara-native")).arg("--nope").output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("usage"));
}
