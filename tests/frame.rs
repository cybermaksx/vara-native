//! Cells of a frame, as VARA's transmitter fills them: channel interleaving, then per carrier a
//! random reference symbol, pilots repeating the previous symbol, and data multiplying it by a point.

use std::path::PathBuf;
use vara_native::dat::{parse, Bandwidth, Record, RecordBody};
use vara_native::fft::Complex;
use vara_native::frame::{assemble, channel_interleave, FrameError};
use vara_native::mapping::constellation;
use vara_native::modes::{levels, Modulation, SpeedLevel};
use vara_native::ofdm::FrameMap;

fn level(bw: Bandwidth, n: u8) -> &'static SpeedLevel {
    &levels(bw)[n as usize - 1]
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

/// Frame map with pilots at the given (carrier, symbol) cells.
fn map(l: &SpeedLevel, pilots: &[(usize, usize)]) -> FrameMap {
    let (c, f) = (l.carriers as usize, l.frame_symbols.unwrap() as usize);
    let mut mask = vec![vec![false; f]; c];
    for &(carrier, symbol) in pilots {
        mask[carrier][symbol - 2] = true;
    }
    let r = Record { channel_interleaver: vec![], info_interleaver: vec![], body: RecordBody::Psk { pilot_symbols: vec![vec![]; c], mask } };
    FrameMap::from_record(l, &r).unwrap()
}

fn close(a: Complex, b: Complex) -> bool {
    (a - b).norm() < 1e-9
}

#[test]
fn channel_interleaver_reads_through_the_permutation() {
    let coded = [1, 0, 1, 1, 0];
    let a = [4u16, 0, 3, 1, 2];
    assert_eq!(channel_interleave(&coded, &a).unwrap(), [0, 1, 1, 0, 1]);
}

// Coded output can be up to 4 bits shorter than the interleaver; the missing bits are taken as 0.
#[test]
fn channel_interleaver_pads_with_zeros() {
    let coded = [1, 1, 1];
    let a = [4u16, 3, 2, 1, 0];
    assert_eq!(channel_interleave(&coded, &a).unwrap(), [0, 0, 1, 1, 1]);
    assert_eq!(channel_interleave(&[1; 6], &a).unwrap_err(), FrameError::TooManyCodedBits { coded: 6, interleaver: 5 });
}

#[test]
fn reference_symbols_open_every_carrier() {
    let l = level(Bandwidth::Hz2300, 5); // 3 carriers, QPSK
    let m = map(l, &[]);
    let qpsk = constellation(2).unwrap();
    let g = assemble(l, &m, &bits(m.data_cells() * 2, 1), &[3, 0, 2]).unwrap();
    assert!(close(g[0][0], qpsk[3]) && close(g[0][1], qpsk[0]) && close(g[0][2], qpsk[2]));
}

#[test]
fn pilots_repeat_the_previous_symbol() {
    let l = level(Bandwidth::Hz2300, 5);
    let m = map(l, &[(1, 5), (1, 6), (2, 300)]);
    let g = assemble(l, &m, &bits(m.data_cells() * 2, 2), &[0, 1, 2]).unwrap();
    assert!(close(g[5][1], g[4][1]) && close(g[6][1], g[5][1]) && close(g[300][2], g[299][2]));
}

// Bits go carrier by carrier, each carrier from symbol 1 to the end, first bit most significant.
#[test]
fn data_multiplies_the_previous_symbol_carrier_by_carrier() {
    let l = level(Bandwidth::Hz2300, 5);
    let m = map(l, &[]);
    let qpsk = constellation(2).unwrap();
    let mut b = vec![0u8; m.data_cells() * 2];
    b[..4].copy_from_slice(&[1, 0, 0, 1]); // carrier 0: symbol 1 gets point 2, symbol 2 point 1
    let per_carrier = (l.frame_symbols.unwrap() as usize - 1) * 2;
    b[per_carrier..per_carrier + 2].copy_from_slice(&[1, 1]); // carrier 1, symbol 1: point 3
    let g = assemble(l, &m, &b, &[0, 0, 0]).unwrap();
    assert!(close(g[1][0], g[0][0] * qpsk[2]));
    assert!(close(g[2][0], g[1][0] * qpsk[1]));
    assert!(close(g[3][0], g[2][0] * qpsk[0]));
    assert!(close(g[1][1], g[0][1] * qpsk[3]));
}

/// Undoes the differential modulation: nearest point to sym[s] / sym[s-1], then its bits.
fn decode(l: &SpeedLevel, m: &FrameMap, g: &[Vec<Complex>]) -> Vec<u8> {
    let b = l.modulation.bits_per_symbol().unwrap();
    let pts = constellation(b).unwrap();
    let mut out = Vec::new();
    for c in 0..l.carriers as usize {
        for s in 1..g.len() {
            if m.is_pilot(s, c) {
                continue;
            }
            let (cur, prev) = (g[s][c], g[s - 1][c]);
            let d = prev.norm().powi(2);
            let q = cur * prev.conj();
            let ratio = Complex::new(q.re / d, q.im / d);
            let idx = (0..pts.len()).min_by(|&x, &y| (pts[x] - ratio).norm().total_cmp(&(pts[y] - ratio).norm())).unwrap();
            out.extend((0..b).rev().map(|j| ((idx >> j) & 1) as u8));
        }
    }
    out
}

#[test]
fn differential_decoding_recovers_the_bits() {
    for (bw, n) in [(Bandwidth::Hz500, 4), (Bandwidth::Hz2300, 9), (Bandwidth::Hz2300, 10), (Bandwidth::Hz2750, 14)] {
        let l = level(bw, n);
        let m = map(l, &[(0, 3), (1, 10)]);
        let b = l.modulation.bits_per_symbol().unwrap() as usize;
        let input = bits(m.data_cells() * b, n as u64);
        let refs: Vec<usize> = (0..l.carriers as usize).map(|c| c % (1 << b)).collect();
        let g = assemble(l, &m, &input, &refs).unwrap();
        assert_eq!(decode(l, &m, &g), input, "{bw:?} L{n}");
    }
}

#[test]
fn rejects_wrong_inputs() {
    let l = level(Bandwidth::Hz2300, 5);
    let m = map(l, &[]);
    let n = m.data_cells() * 2;
    assert_eq!(assemble(l, &m, &bits(n - 1, 1), &[0, 0, 0]).unwrap_err(), FrameError::BitCount { expected: n, got: n - 1 });
    assert_eq!(assemble(l, &m, &bits(n, 1), &[0, 0]).unwrap_err(), FrameError::ReferenceCount { expected: 3, got: 2 });
    assert_eq!(assemble(l, &m, &bits(n, 1), &[0, 4, 0]).unwrap_err(), FrameError::ReferenceIndex { carrier: 1, index: 4 });
    let fsk = level(Bandwidth::Hz2300, 1);
    assert_eq!(assemble(fsk, &m, &[], &[]).unwrap_err(), FrameError::NotPsk { level: 1 });
}

// 16QAM and 32QAM are modulated coherently with extra reference cells VARA keeps in a second
// mask; until that mask is known, assembling them must fail rather than guess.
#[test]
fn qam_levels_are_not_assembled_yet() {
    for n in [15, 16] {
        let l = level(Bandwidth::Hz2300, n);
        let m = map(l, &[]);
        let b = l.modulation.bits_per_symbol().unwrap() as usize;
        let refs = vec![0; l.carriers as usize];
        assert_eq!(assemble(l, &m, &bits(m.data_cells() * b, 1), &refs).unwrap_err(), FrameError::CoherentQam { level: n });
    }
}

// With VARA's own tables: coded bits after interleaving fill every data cell of the differential levels exactly.
#[test]
fn real_levels_consume_exactly_the_coded_bits() {
    for bw in [Bandwidth::Hz500, Bandwidth::Hz2300, Bandwidth::Hz2750] {
        let Some(dir) = std::env::var_os("VARA_DIR").map(PathBuf::from) else { return };
        let records = parse(bw, &std::fs::read(dir.join(bw.file_name())).unwrap()).unwrap().records().unwrap();
        let differential = |l: &&SpeedLevel| matches!(l.modulation, Modulation::Bpsk | Modulation::Qpsk | Modulation::Psk8);
        for (l, r) in levels(bw).iter().zip(&records).filter(|(l, _)| differential(l)) {
            let m = FrameMap::from_record(l, r).unwrap();
            let coded = bits(l.coded_bits as usize - 3, 9);
            let x = channel_interleave(&coded, &r.channel_interleaver).unwrap();
            let refs = vec![0; l.carriers as usize];
            assert!(assemble(l, &m, &x, &refs).is_ok(), "{bw:?} L{}", l.level);
        }
    }
}
