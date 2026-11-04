//! Turbo encoder of VARA.
//!
//! Two recursive systematic encoders with feedback 1+D^2+D^3 and output 1+D+D^3 (the 3GPP
//! constituent code, constraint length 4). The second one reads the information bits through the
//! level's information interleaver. Parities are punctured with period Pd: parity 1 at t = 0,
//! parity 2 at t = Pd/2; the systematic bit is always sent. Each encoder is then driven back to
//! state 0 in three steps, sent unpunctured as sys1, p1, sys2, p2 per step.

#[derive(Debug, PartialEq, Eq)]
pub enum TurboError {
    InterleaverLength { expected: usize, got: usize },
    NotAPermutation,
    NotABit { index: usize },
    UnsupportedRate((u8, u8)),
}

/// Tail bits: 3 steps x (sys1, p1, sys2, p2).
pub const TAIL_BITS: usize = 12;
const MEMORY: usize = 3;
const STATES: u8 = 1 << MEMORY;

/// Register bits of a state: r0 is the oldest feedback bit, r2 the newest.
fn register(state: u8) -> (u8, u8, u8) {
    ((state >> 2) & 1, (state >> 1) & 1, state & 1)
}

/// One step of the recursive systematic encoder: (next state, parity bit).
pub fn rsc_step(state: u8, input: u8) -> (u8, u8) {
    let (r0, r1, r2) = register(state);
    let w = input ^ r0 ^ r1;
    let parity = r0 ^ r2 ^ w;
    (((state << 1) | w) & (STATES - 1), parity)
}

/// Input bit that drives the feedback to zero, used to terminate the trellis.
pub fn termination_input(state: u8) -> u8 {
    let (r0, r1, _) = register(state);
    r0 ^ r1
}

pub fn puncture_period(rate: (u8, u8)) -> Option<usize> {
    match rate {
        (1, 3) => Some(1),
        (1, 2) => Some(2),
        (2, 3) => Some(4),
        (3, 4) => Some(6),
        (4, 5) => Some(8),
        (5, 6) => Some(10),
        _ => None,
    }
}

/// Whether information bit i carries parity 1 and parity 2.
fn parities_sent(i: usize, period: usize) -> (bool, bool) {
    let t = i % period;
    (t == 0, t == period / 2)
}

pub fn coded_len(k: usize, rate: (u8, u8)) -> Option<usize> {
    let period = puncture_period(rate)?;
    let parity: usize = (0..k)
        .map(|i| {
            let (a, b) = parities_sent(i, period);
            a as usize + b as usize
        })
        .sum();
    Some(k + parity + TAIL_BITS)
}

/// Parity stream of one encoder plus its termination: (parity, tail inputs, tail parities).
fn encode_one(bits: impl Iterator<Item = u8>) -> (Vec<u8>, [u8; MEMORY], [u8; MEMORY]) {
    let mut state = 0;
    let parity = bits
        .map(|b| {
            let (next, p) = rsc_step(state, b);
            state = next;
            p
        })
        .collect();
    let (mut xs, mut ps) = ([0; MEMORY], [0; MEMORY]);
    for j in 0..MEMORY {
        let x = termination_input(state);
        let (next, p) = rsc_step(state, x);
        (xs[j], ps[j], state) = (x, p, next);
    }
    (parity, xs, ps)
}

pub fn encode(info: &[u8], interleaver: &[u16], rate: (u8, u8)) -> Result<Vec<u8>, TurboError> {
    let k = info.len();
    if interleaver.len() != k {
        return Err(TurboError::InterleaverLength { expected: k, got: interleaver.len() });
    }
    if let Some(index) = info.iter().position(|&b| b > 1) {
        return Err(TurboError::NotABit { index });
    }
    let mut seen = vec![false; k];
    for &i in interleaver {
        match seen.get_mut(i as usize) {
            Some(s) if !*s => *s = true,
            _ => return Err(TurboError::NotAPermutation),
        }
    }
    let period = puncture_period(rate).ok_or(TurboError::UnsupportedRate(rate))?;

    let (p1, x1, q1) = encode_one(info.iter().copied());
    let (p2, x2, q2) = encode_one(interleaver.iter().map(|&i| info[i as usize]));

    let mut out = Vec::with_capacity(coded_len(k, rate).unwrap_or(0));
    for i in 0..k {
        out.push(info[i]);
        let (send1, send2) = parities_sent(i, period);
        if send1 {
            out.push(p1[i]);
        }
        if send2 {
            out.push(p2[i]);
        }
    }
    for j in 0..MEMORY {
        out.extend([x1[j], q1[j], x2[j], q2[j]]);
    }
    Ok(out)
}
