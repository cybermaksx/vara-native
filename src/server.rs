//! Command server (TCP 8300): one client at a time, one thread, driven by the session clock.

use crate::session::Timings;
use std::io;
use std::net::TcpStream;

pub fn serve_client(stream: TcpStream, timings: Timings) -> io::Result<()> {
    let _ = (stream, timings);
    todo!()
}
