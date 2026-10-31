use std::net::TcpListener;
use std::process::ExitCode;
use vara_native::server::serve;
use vara_native::session::Timings;

const USAGE: &str = "usage: vara-native [--bind ADDR:PORT]   (default 127.0.0.1:8300)";

fn main() -> ExitCode {
    let mut bind = String::from("127.0.0.1:8300");
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match (arg.as_str(), args.next()) {
            ("--bind", Some(addr)) => bind = addr,
            _ => {
                eprintln!("{USAGE}");
                return ExitCode::FAILURE;
            }
        }
    }

    let listener = match TcpListener::bind(&bind) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("cannot listen on {bind}: {e}");
            return ExitCode::FAILURE;
        }
    };
    match listener.local_addr() {
        Ok(addr) => eprintln!("listening on {addr}"),
        Err(e) => eprintln!("listening (address unknown: {e})"),
    }
    match serve(listener, Timings::observed()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("server stopped: {e}");
            ExitCode::FAILURE
        }
    }
}
