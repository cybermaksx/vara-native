//! OFDM symbols at the sound card rate, and the pilot map of a frame.
//!
//! Subcarrier k sits at k x 46.875 Hz. A symbol is 1024 samples plus a 128-sample cyclic prefix
//! at 48 kHz: the same 24 ms symbol as 64 + 8 samples of a 3000 Hz baseband. Each cell c on bin k
//! contributes Re(c e^(2 pi i k n / 1024)) to the audio, so a unit cell is a unit cosine.

use crate::dat::{Record, RecordBody};
use crate::fft::{fft, ifft, Complex};
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
    PilotBeyondFrame { carrier: usize, symbol: usize },
}

#[derive(Debug, Clone)]
pub struct Ofdm {
    bins: RangeInclusive<u8>,
    frame_symbols: usize,
}

impl Ofdm {
    pub fn for_level(level: &SpeedLevel) -> Result<Ofdm, OfdmError> {
        match (level.ofdm_bins(), level.frame_symbols) {
            (Some(bins), Some(symbols)) => Ok(Ofdm { bins, frame_symbols: symbols as usize }),
            _ => Err(OfdmError::NotOfdm { level: level.level }),
        }
    }

    pub fn carriers(&self) -> usize {
        self.bins.clone().count()
    }

    pub fn frame_symbols(&self) -> usize {
        self.frame_symbols
    }

    fn shape_error(&self) -> OfdmError {
        OfdmError::GridShape { symbols: self.frame_symbols, carriers: self.carriers() }
    }

    /// One frame of real audio from cells indexed [symbol][carrier].
    pub fn modulate(&self, grid: &[Vec<Complex>]) -> Result<Vec<f64>, OfdmError> {
        if grid.len() != self.frame_symbols || grid.iter().any(|s| s.len() != self.carriers()) {
            return Err(self.shape_error());
        }
        let mut out = Vec::with_capacity(self.frame_symbols * SYMBOL_LEN);
        let mut spec = vec![Complex::new(0.0, 0.0); FFT_LEN];
        for cells in grid {
            spec.fill(Complex::new(0.0, 0.0));
            // Hermitian pair, scaled so the inverse transform yields Re(c e^(i...)) per cell.
            let half = FFT_LEN as f64 / 2.0;
            for (k, &c) in self.bins.clone().zip(cells) {
                spec[k as usize] = c.scale(half);
                spec[FFT_LEN - k as usize] = c.conj().scale(half);
            }
            ifft(&mut spec);
            out.extend(spec[FFT_LEN - CP_LEN..].iter().map(|v| v.re));
            out.extend(spec.iter().map(|v| v.re));
        }
        Ok(out)
    }

    /// Cells indexed [symbol][carrier] from one symbol-aligned frame of audio.
    pub fn demodulate(&self, samples: &[f64]) -> Result<Vec<Vec<Complex>>, OfdmError> {
        let expected = self.frame_symbols * SYMBOL_LEN;
        if samples.len() != expected {
            return Err(OfdmError::SampleCount { expected, got: samples.len() });
        }
        let scale = 2.0 / FFT_LEN as f64;
        Ok(samples
            .chunks_exact(SYMBOL_LEN)
            .map(|sym| {
                let mut spec: Vec<Complex> = sym[CP_LEN..].iter().map(|&v| Complex::new(v, 0.0)).collect();
                fft(&mut spec);
                self.bins.clone().map(|k| spec[k as usize].scale(scale)).collect()
            })
            .collect())
    }
}

/// Which cells of a frame carry pilots, indexed [symbol][carrier].
#[derive(Debug, Clone)]
pub struct FrameMap {
    pilot: Vec<Vec<bool>>,
}

impl FrameMap {
    /// The record's mask is stored carrier by carrier; it must match the level's grid.
    pub fn from_record(level: &SpeedLevel, record: &Record) -> Result<FrameMap, OfdmError> {
        let carriers = level.carriers as usize;
        let symbols = level.frame_symbols.unwrap_or(0) as usize;
        let shape = OfdmError::GridShape { symbols, carriers };
        let RecordBody::Psk { mask, .. } = &record.body else { return Err(shape) };
        if mask.len() != carriers || mask.iter().any(|m| m.len() != symbols) {
            return Err(shape);
        }
        let pilot = (0..symbols).map(|s| mask.iter().map(|m| m[s]).collect()).collect();
        Ok(FrameMap { pilot })
    }

    pub fn is_pilot(&self, symbol: usize, carrier: usize) -> bool {
        self.pilot[symbol][carrier]
    }

    pub fn symbols(&self) -> usize {
        todo!()
    }

    /// Cells left for data: all cells minus the reference symbol and the pilots.
    pub fn data_cells(&self) -> usize {
        todo!()
    }

    pub fn pilot_count(&self) -> usize {
        self.pilot.iter().flatten().filter(|&&p| p).count()
    }
}
