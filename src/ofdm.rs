//! OFDM symbols at the sound card rate, and the pilot map of a frame.

use crate::dat::Record;
use crate::fft::Complex;
use crate::modes::SpeedLevel;
use std::ops::RangeInclusive;

pub const AUDIO_HZ: u32 = 48_000;
/// 64-point FFT of the 3000 Hz baseband, at 16 times the rate.
pub const FFT_LEN: usize = 1024;
/// 8-sample cyclic prefix of the baseband, at 16 times the rate.
pub const CP_LEN: usize = 128;
pub const SYMBOL_LEN: usize = FFT_LEN + CP_LEN;

#[derive(Debug, PartialEq, Eq)]
pub enum OfdmError {
    NotOfdm { level: u8 },
    /// Expected shape of the grid or mask.
    GridShape { symbols: usize, carriers: usize },
    SampleCount { expected: usize, got: usize },
}

#[derive(Debug, Clone)]
pub struct Ofdm {
    bins: RangeInclusive<u8>,
    frame_symbols: usize,
}

impl Ofdm {
    pub fn for_level(level: &SpeedLevel) -> Result<Ofdm, OfdmError> {
        let _ = level;
        todo!()
    }

    pub fn carriers(&self) -> usize {
        todo!()
    }

    pub fn frame_symbols(&self) -> usize {
        todo!()
    }

    /// One frame of real audio from cells indexed [symbol][carrier].
    pub fn modulate(&self, grid: &[Vec<Complex>]) -> Result<Vec<f64>, OfdmError> {
        let _ = (grid, &self.bins, self.frame_symbols);
        todo!()
    }

    /// Cells indexed [symbol][carrier] from one symbol-aligned frame of audio.
    pub fn demodulate(&self, samples: &[f64]) -> Result<Vec<Vec<Complex>>, OfdmError> {
        let _ = samples;
        todo!()
    }
}

/// Which cells of a frame carry pilots, indexed [symbol][carrier].
#[derive(Debug, Clone)]
pub struct FrameMap {
    pilot: Vec<Vec<bool>>,
}

impl FrameMap {
    pub fn from_record(level: &SpeedLevel, record: &Record) -> Result<FrameMap, OfdmError> {
        let _ = (level, record);
        todo!()
    }

    pub fn is_pilot(&self, symbol: usize, carrier: usize) -> bool {
        let _ = (symbol, carrier, &self.pilot);
        todo!()
    }

    pub fn pilot_count(&self) -> usize {
        todo!()
    }
}
