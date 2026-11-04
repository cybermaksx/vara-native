//! Cells of a frame, filled the way VARA's transmitter fills them.

use crate::fft::Complex;
use crate::modes::SpeedLevel;
use crate::ofdm::FrameMap;

#[derive(Debug, PartialEq, Eq)]
pub enum FrameError {
    TooManyCodedBits { coded: usize, interleaver: usize },
    BitCount { expected: usize, got: usize },
    ReferenceCount { expected: usize, got: usize },
    ReferenceIndex { carrier: usize, index: usize },
    NotPsk { level: u8 },
    CoherentQam { level: u8 },
}

pub fn channel_interleave(coded: &[u8], interleaver: &[u16]) -> Result<Vec<u8>, FrameError> {
    let _ = (coded, interleaver);
    todo!()
}

pub fn assemble(level: &SpeedLevel, map: &FrameMap, bits: &[u8], reference: &[usize]) -> Result<Vec<Vec<Complex>>, FrameError> {
    let _ = (level, map, bits, reference);
    todo!()
}
