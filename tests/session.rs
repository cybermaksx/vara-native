//! Call behaviour of the command channel, replayed on a virtual clock against captured traffic
//! (Winlink Express <-> VARA HF 4.9.0). Times are seconds after VARA's reply to CONNECT.

use std::time::Duration;
use vara_native::control::{Command, Event};
use vara_native::session::{Session, Timings};

/// TCP and OS scheduling jitter in the captures; not a model parameter.
const TOLERANCE: f64 = 0.050;

// Call to a gateway that never answers (capture 2).
const PTT_ON: [f64; 15] = [
    0.000, 3.110, 5.820, 8.550, 11.261, 14.014, 16.719, 19.483, 22.198, 24.940, 27.671, 30.380, 33.134, 35.849, 38.547,
];
const PTT_OFF: [f64; 15] = [
    1.786, 4.540, 7.243, 9.985, 12.699, 15.437, 18.155, 20.912, 23.635, 26.380, 29.100, 31.801, 34.561, 37.284, 39.980,
];
const DISCONNECTED: f64 = 41.317;

fn secs(d: Duration) -> f64 {
    d.as_secs_f64()
}

fn at(t: f64) -> Duration {
    Duration::from_secs_f64(t)
}

/// Fires every timer up to `until`, collecting (time, events) per firing.
fn run_until(s: &mut Session, until: f64) -> Vec<(f64, Vec<Event>)> {
    let mut out = Vec::new();
    while let Some(d) = s.next_deadline() {
        if secs(d) > until {
            break;
        }
        let ev = s.on_timer(d);
        if !ev.is_empty() {
            out.push((secs(d), ev));
        }
    }
    out
}

fn connect() -> Command {
    Command::Connect { from: "N0CALL".into(), to: "N1CALL".into() }
}

fn assert_near(got: f64, want: f64, what: &str) {
    assert!((got - want).abs() <= TOLERANCE, "{what}: {got:.3} s, captured {want:.3} s");
}

#[test]
fn idle_commands_are_acknowledged() {
    let mut s = Session::new(Timings::observed(), at(0.0));
    assert_eq!(s.on_command(at(0.0), Command::WinlinkSession), [Event::Ok]);
    assert_eq!(s.on_command(at(0.0), Command::Bandwidth(vara_native::dat::Bandwidth::Hz2300)), [Event::Ok]);
}

#[test]
fn connect_answers_in_one_segment() {
    let mut s = Session::new(Timings::observed(), at(0.0));
    assert_eq!(s.on_command(at(0.0), connect()), [Event::Ok, Event::PttOn, Event::BusyOn]);
}

#[test]
fn unanswered_call_matches_capture() {
    let mut s = Session::new(Timings::observed(), at(0.0));
    s.on_command(at(0.0), connect());
    let fired = run_until(&mut s, 50.0);

    let ptt_on: Vec<f64> = fired.iter().filter(|(_, e)| e.contains(&Event::PttOn)).map(|(t, _)| *t).collect();
    let ptt_off: Vec<f64> = fired.iter().filter(|(_, e)| e.contains(&Event::PttOff)).map(|(t, _)| *t).collect();
    assert_eq!(ptt_on.len() + 1, 15, "15 attempts: the first PTT ON comes with OK");
    assert_eq!(ptt_off.len(), 15);
    for (i, (&got, &want)) in ptt_on.iter().zip(&PTT_ON[1..]).enumerate() {
        assert_near(got, want, &format!("PTT ON #{}", i + 2));
    }
    for (i, (&got, &want)) in ptt_off.iter().zip(&PTT_OFF).enumerate() {
        assert_near(got, want, &format!("PTT OFF #{}", i + 1));
    }

    let (t, last) = fired.last().unwrap();
    assert_eq!(last, &[Event::Disconnected, Event::BusyOff], "both in one segment");
    assert_near(*t, DISCONNECTED, "DISCONNECTED");
}

#[test]
fn call_events_alternate_strictly() {
    let mut s = Session::new(Timings::observed(), at(0.0));
    s.on_command(at(0.0), connect());
    let seq: Vec<Event> = run_until(&mut s, 50.0).into_iter().flat_map(|(_, e)| e).collect();
    let mut expected = vec![Event::PttOff];
    for _ in 0..14 {
        expected.extend([Event::PttOn, Event::PttOff]);
    }
    expected.extend([Event::Disconnected, Event::BusyOff]);
    assert_eq!(seq, expected);
}

// Capture 1: ABORT 0.503 s into a transmission. OK at once; the frame plays to its end; then hang up.
#[test]
fn abort_during_transmission_matches_capture() {
    let mut s = Session::new(Timings::observed(), at(0.0));
    s.on_command(at(0.0), connect());
    // Into attempt 4: fire timers until its PTT ON.
    let mut ptt_on_at = 0.0;
    for _ in 0..3 {
        loop {
            let d = s.next_deadline().unwrap();
            if s.on_timer(d).contains(&Event::PttOn) {
                ptt_on_at = secs(d);
                break;
            }
        }
    }
    assert_near(ptt_on_at, PTT_ON[3], "PTT ON #4");
    assert_eq!(s.on_command(at(ptt_on_at + 0.503), Command::Abort), [Event::Ok]);

    let fired = run_until(&mut s, ptt_on_at + 10.0);
    assert_eq!(fired.len(), 2, "{fired:?}");
    assert_eq!(fired[0].1, [Event::PttOff]);
    assert_near(fired[0].0 - ptt_on_at, 1.438, "frame length after ABORT");
    assert_eq!(fired[1].1, [Event::Disconnected, Event::BusyOff]);
    assert_near(fired[1].0 - fired[0].0, 1.305, "hang-up delay after ABORT");
}

#[test]
fn idle_after_call_accepts_a_new_call() {
    let mut s = Session::new(Timings::observed(), at(0.0));
    s.on_command(at(0.0), connect());
    run_until(&mut s, 50.0);
    assert_eq!(s.on_command(at(50.0), connect()), [Event::Ok, Event::PttOn, Event::BusyOn]);
}

// Capture 1: IAMALIVE every 60.0 s (59.985 .. 60.015) while idle.
#[test]
fn keepalive_every_60_s_when_idle() {
    let mut s = Session::new(Timings::observed(), at(0.0));
    let fired = run_until(&mut s, 300.0);
    let t: Vec<f64> = fired.iter().filter(|(_, e)| e == &[Event::IAmAlive]).map(|(t, _)| *t).collect();
    assert_eq!(t.len(), 5);
    assert!(t.windows(2).all(|w| (w[1] - w[0] - 60.0).abs() < 1e-9), "{t:?}");
}

// Assumption, not in the captures: none of the calls there overlapped a keepalive slot.
#[test]
fn no_keepalive_during_a_call() {
    let mut s = Session::new(Timings::observed(), at(0.0));
    run_until(&mut s, 59.0);
    s.on_command(at(59.0), connect());
    let fired = run_until(&mut s, 59.0 + DISCONNECTED - 0.5);
    assert!(fired.iter().all(|(_, e)| !e.contains(&Event::IAmAlive)), "{fired:?}");
}

// Assumption until a capture shows VARA's answer (its strings include WRONG).
#[test]
fn unknown_commands_get_no_reply() {
    let mut s = Session::new(Timings::observed(), at(0.0));
    assert!(s.on_command(at(0.0), Command::Unknown("MYCALL N0CALL".into())).is_empty());
}

#[test]
fn timings_scale_for_fast_tests() {
    let t = Timings::observed().scaled(0.01);
    let mut s = Session::new(t, at(0.0));
    s.on_command(at(0.0), connect());
    let fired = run_until(&mut s, 1.0);
    let (t, _) = fired.iter().find(|(_, e)| e.contains(&Event::Disconnected)).unwrap();
    assert_near(*t, DISCONNECTED * 0.01, "scaled DISCONNECTED");
}
