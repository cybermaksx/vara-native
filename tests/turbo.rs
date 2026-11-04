//! Turbo encoder: two recursive systematic encoders (feedback 1+D^2+D^3, output 1+D+D^3, the
//! 3GPP constituent code), an information interleaver, periodic puncturing and 12 tail bits.

use vara_native::dat::Bandwidth;
use vara_native::modes::levels;
use vara_native::turbo::{coded_len, encode, puncture_period, rsc_step, termination_input, TurboError};

/// Reference recursion on the register history, independent of the implementation's state packing:
/// w[t] = u[t] ^ w[t-2] ^ w[t-3], p[t] = w[t] ^ w[t-1] ^ w[t-3].
fn rsc_reference(u: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let mut w: Vec<u8> = vec![0, 0, 0];
    let mut p = Vec::new();
    for &bit in u {
        let n = w.len();
        let wt = bit ^ w[n - 2] ^ w[n - 3];
        p.push(wt ^ w[n - 1] ^ w[n - 3]);
        w.push(wt);
    }
    (w[3..].to_vec(), p)
}

fn run(u: &[u8]) -> (u8, Vec<u8>) {
    let mut s = 0;
    let mut p = Vec::new();
    for &b in u {
        let (next, parity) = rsc_step(s, b);
        p.push(parity);
        s = next;
    }
    (s, p)
}

fn bits(n: usize, seed: u64) -> Vec<u8> {
    let mut s = seed;
    (0..n)
        .map(|_| {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            (s & 1) as u8
        })
        .collect()
}

fn identity(k: usize) -> Vec<u16> {
    (0..k as u16).collect()
}

#[test]
fn impulse_response() {
    let mut u = vec![0u8; 8];
    u[0] = 1;
    let (w, p) = rsc_reference(&u);
    assert_eq!(w, [1, 0, 1, 1, 1, 0, 0, 1], "feedback 1+D^2+D^3 is primitive: period 7");
    assert_eq!(p, [1, 1, 1, 1, 0, 0, 1, 0]);
    assert_eq!(run(&u).1, p);
}

#[test]
fn step_matches_reference_on_random_input() {
    for seed in 1..20 {
        let u = bits(200, seed);
        assert_eq!(run(&u).1, rsc_reference(&u).1, "seed {seed}");
    }
}

#[test]
fn eight_states() {
    for s in 0..8 {
        for u in 0..2 {
            assert!(rsc_step(s, u).0 < 8);
        }
    }
}

#[test]
fn termination_reaches_zero_from_every_state() {
    for start in 0..8 {
        let mut s = start;
        for _ in 0..3 {
            s = rsc_step(s, termination_input(s)).0;
        }
        assert_eq!(s, 0, "from state {start}");
    }
}

#[test]
fn puncture_periods() {
    assert_eq!(puncture_period((1, 3)), Some(1));
    assert_eq!(puncture_period((1, 2)), Some(2));
    assert_eq!(puncture_period((2, 3)), Some(4));
    assert_eq!(puncture_period((3, 4)), Some(6));
    assert_eq!(puncture_period((4, 5)), Some(8));
    assert_eq!(puncture_period((5, 6)), Some(10));
    assert_eq!(puncture_period((7, 9)), None);
}

/// Systematic, parity 1 and parity 2 streams of the information part, with the reference encoder.
fn streams(u: &[u8], interleaver: &[u16]) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let permuted: Vec<u8> = interleaver.iter().map(|&i| u[i as usize]).collect();
    (u.to_vec(), rsc_reference(u).1, rsc_reference(&permuted).1)
}

#[test]
fn rate_one_third_sends_all_three_streams() {
    let u = bits(6, 3);
    let (s, p1, p2) = streams(&u, &identity(6));
    let out = encode(&u, &identity(6), (1, 3)).unwrap();
    let expected: Vec<u8> = (0..6).flat_map(|i| [s[i], p1[i], p2[i]]).collect();
    assert_eq!(out[..18], expected[..]);
}

#[test]
fn rate_one_half_alternates_parities() {
    let u = bits(6, 4);
    let (s, p1, p2) = streams(&u, &identity(6));
    let out = encode(&u, &identity(6), (1, 2)).unwrap();
    assert_eq!(out[..12], [s[0], p1[0], s[1], p2[1], s[2], p1[2], s[3], p2[3], s[4], p1[4], s[5], p2[5]]);
}

#[test]
fn rate_two_thirds_sends_one_parity_every_two_bits() {
    let u = bits(8, 5);
    let (s, p1, p2) = streams(&u, &identity(8));
    let out = encode(&u, &identity(8), (2, 3)).unwrap();
    assert_eq!(out[..12], [s[0], p1[0], s[1], s[2], p2[2], s[3], s[4], p1[4], s[5], s[6], p2[6], s[7]]);
}

#[test]
fn rate_five_sixths_pattern() {
    let u = bits(10, 6);
    let (s, p1, p2) = streams(&u, &identity(10));
    let out = encode(&u, &identity(10), (5, 6)).unwrap();
    assert_eq!(out[..12], [s[0], p1[0], s[1], s[2], s[3], s[4], s[5], p2[5], s[6], s[7], s[8], s[9]]);
}

// Tail: three steps per encoder, each sending sys1, p1, sys2, p2; both encoders end in state 0.
#[test]
fn tail_closes_both_encoders() {
    let k = 40;
    let u = bits(k, 7);
    let interleaver: Vec<u16> = (0..k as u16).rev().collect();
    let out = encode(&u, &interleaver, (1, 2)).unwrap();
    let tail = &out[out.len() - 12..];

    let permuted: Vec<u8> = interleaver.iter().map(|&i| u[i as usize]).collect();
    let (mut s1, _) = run(&u);
    let (mut s2, _) = run(&permuted);
    for j in 0..3 {
        let (x1, x2) = (termination_input(s1), termination_input(s2));
        let (n1, q1) = rsc_step(s1, x1);
        let (n2, q2) = rsc_step(s2, x2);
        assert_eq!(tail[4 * j..4 * j + 4], [x1, q1, x2, q2], "tail step {j}");
        (s1, s2) = (n1, n2);
    }
    assert_eq!((s1, s2), (0, 0));
}

// The second encoder sees u[B[i]], not the inverse permutation.
#[test]
fn second_encoder_reads_through_the_interleaver() {
    let k = 12;
    let u = bits(k, 8);
    let interleaver: Vec<u16> = vec![5, 0, 7, 2, 9, 4, 11, 6, 1, 8, 3, 10];
    let (_, _, p2) = streams(&u, &interleaver);
    let out = encode(&u, &interleaver, (1, 3)).unwrap();
    let got: Vec<u8> = (0..k).map(|i| out[3 * i + 2]).collect();
    assert_eq!(got, p2);
}

#[test]
fn length_formula() {
    for (k, rate) in [(96, (1, 3)), (1080, (1, 2)), (9596, (2, 3)), (11512, (4, 5)), (21710, (5, 6))] {
        let u = bits(k, k as u64);
        assert_eq!(encode(&u, &identity(k), rate).unwrap().len(), coded_len(k, rate).unwrap(), "K={k}");
    }
    assert_eq!(coded_len(96, (1, 3)), Some(300));
    assert_eq!(coded_len(7197, (1, 2)), Some(14406));
    assert_eq!(coded_len(9596, (2, 3)), Some(14406));
}

// Coded length against N, the channel interleaver length in the .dat files: exact for rates
// 1/3, 1/2 and 2/3; rates 4/5 and 5/6 fall 1..4 bits short of a whole number of data cells.
#[test]
fn coded_length_fills_the_frame() {
    for bw in [Bandwidth::Hz500, Bandwidth::Hz2300, Bandwidth::Hz2750] {
        for l in levels(bw) {
            let c = coded_len(l.info_bits as usize, l.code_rate).unwrap();
            let pad = l.coded_bits as usize - c;
            if matches!(l.code_rate, (1, 3) | (1, 2) | (2, 3)) {
                assert_eq!(pad, 0, "{bw:?} L{}", l.level);
            } else {
                assert!((1..=4).contains(&pad), "{bw:?} L{}: pad {pad}", l.level);
            }
        }
    }
}

#[test]
fn rejects_bad_input() {
    assert_eq!(encode(&[0, 1, 0], &identity(2), (1, 2)).unwrap_err(), TurboError::InterleaverLength { expected: 3, got: 2 });
    assert_eq!(encode(&[0, 2, 0], &identity(3), (1, 2)).unwrap_err(), TurboError::NotABit { index: 1 });
    assert_eq!(encode(&[0, 1, 0], &identity(3), (7, 9)).unwrap_err(), TurboError::UnsupportedRate((7, 9)));
    assert_eq!(encode(&[0, 1, 0], &[0, 0, 1], (1, 2)).unwrap_err(), TurboError::NotAPermutation);
}
