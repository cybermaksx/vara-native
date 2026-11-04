//! FSK levels: frequency-hopped MFSK tone sequence.

use crate::dat::Record;
use crate::modes::SpeedLevel;

/// Pilot symbols in every FSK frame.
pub const FSK_PILOTS: usize = 24;

#[derive(Debug, PartialEq, Eq)]
pub enum FskError {
    NotFsk { level: u8 },
    RecordShape,
    BitCount { expected: usize, got: usize },
}

/// Inverse Gray code: the tone offset carrying the label `v`.
pub fn gray_decode(v: u8) -> u8 {
    let _ = v;
    todo!()
}

/// FFT bin of every symbol of a frame, from the level's record and its interleaved coded bits.
pub fn tone_bins(level: &SpeedLevel, record: &Record, bits: &[u8]) -> Result<Vec<u8>, FskError> {
    let _ = (level, record, bits);
    todo!()
}
