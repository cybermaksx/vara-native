//! Constellations of VARA, as stored in its own table (f32 values).
//!
//! BPSK on the real axis, QPSK on the axes (not the diagonals), Gray-labelled 8PSK, a Gray square
//! 16QAM and a cross 32QAM. The first bit of a group is the most significant bit of the index.

use crate::fft::Complex;

#[derive(Debug, PartialEq, Eq)]
pub enum MappingError {
    UnsupportedBits(u8),
    NotABit,
}

pub fn constellation(bits: u8) -> Result<Vec<Complex>, MappingError> {
    let table: &[(f32, f32)] = match bits {
        1 => &BPSK,
        2 => &QPSK,
        3 => &PSK8,
        4 => &QAM16,
        5 => &QAM32,
        b => return Err(MappingError::UnsupportedBits(b)),
    };
    Ok(table.iter().map(|&(re, im)| Complex::new(re as f64, im as f64)).collect())
}

/// Point index from bits, the first bit being the most significant.
pub fn index_msb_first(bits: &[u8]) -> Result<usize, MappingError> {
    bits.iter().try_fold(0usize, |acc, &b| match b {
        0 | 1 => Ok(acc << 1 | b as usize),
        _ => Err(MappingError::NotABit),
    })
}

// Values copied bit-exactly from VARA's f32 table, including the rounding residue of sin(pi).
const BPSK: [(f32, f32); 2] = [(1.0, 0.0), (-1.0, 3.2310891e-15)];

const QPSK: [(f32, f32); 4] = [(1.0, 0.0), (1.6155446e-15, 1.0), (-4.624589e-15, -1.0), (-1.0, 3.2310891e-15)];

const PSK8: [(f32, f32); 8] = [
    (1.0, 0.0),
    (0.70710677, 0.70710677),
    (-0.70710677, 0.70710677),
    (1.6155446e-15, 1.0),
    (0.70710677, -0.70710677),
    (-4.624589e-15, -1.0),
    (-1.0, 3.2310891e-15),
    (-0.70710677, -0.70710677),
];

const A: f32 = 0.777_817_5; // 3 x 1.1 / sqrt(18)
const B: f32 = 0.259_272_5; // 1.1 / sqrt(18)
const QAM16: [(f32, f32); 16] = [
    (-A, -A), (-A, -B), (-A, A), (-A, B),
    (-B, -A), (-B, -B), (-B, A), (-B, B),
    (A, -A), (A, -B), (A, A), (A, B),
    (B, -A), (B, -B), (B, A), (B, B),
];

const P: f32 = 0.21213204; // 0.3 / sqrt(2)
const Q: f32 = 0.636_396_1; // 3 x P
const R: f32 = 1.0606601; // 5 x P
const QAM32: [(f32, f32); 32] = [
    (P, P), (Q, P), (Q, R), (R, P), (P, Q), (Q, Q), (P, R), (R, Q),
    (P, -P), (Q, -P), (Q, -R), (R, -P), (P, -Q), (Q, -Q), (P, -R), (R, -Q),
    (-P, P), (-Q, P), (-Q, R), (-R, P), (-P, Q), (-Q, Q), (-P, R), (-R, Q),
    (-P, -P), (-Q, -P), (-Q, -R), (-R, -P), (-P, -Q), (-Q, -Q), (-P, -R), (-R, -Q),
];
