//! FSK levels: frequency-hopped MFSK tone sequence.
//!
//! Each frame symbol t has a base bin `map[t]` from the level's record. A data symbol takes the
//! next `bits` coded bits (first bit most significant) as v and sends d = inverse Gray of v; a
//! pilot symbol sends the d stored in the record. The tone is
//! `first + ((map[t] + spacing * d - first) mod band)`, band = last - first + 1.

use crate::dat::{Record, RecordBody};
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
    let mut d = v;
    let mut shift = v >> 1;
    while shift != 0 {
        d ^= shift;
        shift >>= 1;
    }
    d
}

/// FFT bin of every symbol of a frame, from the level's record and its interleaved coded bits.
pub fn tone_bins(level: &SpeedLevel, record: &Record, bits: &[u8]) -> Result<Vec<u8>, FskError> {
    let p = level.fsk.ok_or(FskError::NotFsk { level: level.level })?;
    let symbols = level.frame_symbols.unwrap_or(0) as usize;
    let RecordBody::Fsk { map, pairs, .. } = &record.body else { return Err(FskError::RecordShape) };
    if map.len() != symbols || pairs.len() != symbols {
        return Err(FskError::RecordShape);
    }
    let pilots = pairs.iter().filter(|p| p.0 == 1).count();
    let per_symbol = p.bits as usize;
    let expected = (symbols - pilots) * per_symbol;
    if bits.len() != expected || bits.iter().any(|&b| b > 1) {
        return Err(FskError::BitCount { expected, got: bits.len() });
    }

    let band = (p.last_bin - p.first_bin + 1) as i32;
    let first = p.first_bin as i32;
    let mut next = bits.chunks_exact(per_symbol);
    Ok(map
        .iter()
        .zip(pairs)
        .map(|(&base, &(is_pilot, pilot_value))| {
            let d = if is_pilot == 1 {
                pilot_value as i32
            } else {
                // Length and bit values were checked above, so a chunk is always there.
                let v = next.next().map_or(0, |c| c.iter().fold(0u8, |acc, &b| acc << 1 | b));
                gray_decode(v) as i32
            };
            (first + (base as i32 + p.spacing as i32 * d - first).rem_euclid(band)) as u8
        })
        .collect())
}
