//! Constellations of VARA, as stored in its own table (f32 values).

use crate::fft::Complex;

#[derive(Debug, PartialEq, Eq)]
pub enum MappingError {
    UnsupportedBits(u8),
    NotABit,
}

pub fn constellation(bits: u8) -> Result<Vec<Complex>, MappingError> {
    let _ = bits;
    todo!()
}

/// Point index from bits, the first bit being the most significant.
pub fn index_msb_first(bits: &[u8]) -> Result<usize, MappingError> {
    let _ = bits;
    todo!()
}
