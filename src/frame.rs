//! Cells of a frame, filled the way VARA's transmitter fills them.
//!
//! The coded bits pass the channel interleaver (x[i] = coded[A[i]]). Then, carrier by carrier,
//! symbol 0 is a reference point chosen by the caller (VARA draws it at random), a pilot repeats
//! the previous symbol, and a data cell multiplies the previous symbol by the next point.

use crate::fft::Complex;
use crate::mapping::{constellation, index_msb_first};
use crate::modes::{Modulation, SpeedLevel};
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

/// Applies the channel interleaver; coded output shorter than the interleaver is padded with 0.
pub fn channel_interleave(coded: &[u8], interleaver: &[u16]) -> Result<Vec<u8>, FrameError> {
    if coded.len() > interleaver.len() {
        return Err(FrameError::TooManyCodedBits { coded: coded.len(), interleaver: interleaver.len() });
    }
    Ok(interleaver.iter().map(|&i| coded.get(i as usize).copied().unwrap_or(0)).collect())
}

/// Cells indexed [symbol][carrier] for a differentially modulated level (BPSK, QPSK, 8PSK).
pub fn assemble(level: &SpeedLevel, map: &FrameMap, bits: &[u8], reference: &[usize]) -> Result<Vec<Vec<Complex>>, FrameError> {
    let per_cell = match level.modulation {
        Modulation::Fsk => return Err(FrameError::NotPsk { level: level.level }),
        Modulation::Qam16 | Modulation::Qam32 => return Err(FrameError::CoherentQam { level: level.level }),
        m => m.bits_per_symbol().unwrap_or(0) as usize,
    };
    let points = constellation(per_cell as u8).map_err(|_| FrameError::NotPsk { level: level.level })?;
    let carriers = level.carriers as usize;
    let expected = map.data_cells() * per_cell;
    if bits.len() != expected {
        return Err(FrameError::BitCount { expected, got: bits.len() });
    }
    if reference.len() != carriers {
        return Err(FrameError::ReferenceCount { expected: carriers, got: reference.len() });
    }
    if let Some((carrier, &index)) = reference.iter().enumerate().find(|(_, i)| **i >= points.len()) {
        return Err(FrameError::ReferenceIndex { carrier, index });
    }

    let mut grid = vec![vec![Complex::new(0.0, 0.0); carriers]; map.symbols()];
    let mut next = bits.chunks_exact(per_cell);
    for (c, &r) in reference.iter().enumerate() {
        grid[0][c] = points[r];
        for s in 1..map.symbols() {
            let prev = grid[s - 1][c];
            grid[s][c] = if map.is_pilot(s, c) {
                prev
            } else {
                // Bits were validated above: every chunk exists and holds only 0 and 1.
                let chunk = next.next().ok_or(FrameError::BitCount { expected, got: bits.len() })?;
                let index = index_msb_first(chunk).map_err(|_| FrameError::BitCount { expected, got: bits.len() })?;
                prev * points[index]
            };
        }
    }
    Ok(grid)
}
