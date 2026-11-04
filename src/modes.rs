//! Speed levels per bandwidth.
//!
//! Rates and modulations follow the speed level table published by the VARA author; frame map
//! sizes, pilot counts and carrier frequencies follow the mode table built at startup by VARA.exe
//! and agree with the pilot masks in the .dat files. K and N are the lengths of the two
//! permutations stored per level in the .dat files (see doc/protocol.md, section 3).

use crate::dat::Bandwidth;
use std::ops::RangeInclusive;

/// Sample rate of the internal complex baseband.
pub const BASEBAND_HZ: f64 = 3000.0;
/// FFT length of the OFDM levels at the baseband rate.
pub const OFDM_FFT: u8 = 64;
/// Cyclic prefix of the OFDM levels, in baseband samples.
pub const OFDM_CP: u8 = 8;
/// Spacing of FFT bins: 3000 / 64.
pub const BIN_HZ: f64 = BASEBAND_HZ / OFDM_FFT as f64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modulation {
    Fsk,
    Bpsk,
    Qpsk,
    Psk8,
    Qam16,
    Qam32,
}

/// FSK levels only: tones hop over FFT bins first..=last at `spacing` bins per symbol value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FskParams {
    pub bits: u8,
    pub first_bin: u8,
    pub last_bin: u8,
    pub spacing: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpeedLevel {
    pub level: u8,
    pub modulation: Modulation,
    /// Tones for FSK, subcarriers otherwise.
    pub carriers: u8,
    pub nominal_baud: u8,
    /// Symbol length in samples of the 3000 Hz baseband.
    pub samples_per_symbol: u8,
    pub net_bps: u16,
    /// Information bits per frame (K).
    pub info_bits: u32,
    /// Coded bits per frame (N).
    pub coded_bits: u32,
    /// Turbo code rate as (numerator, denominator).
    pub code_rate: (u8, u8),
    /// Symbols per frame on each carrier.
    pub frame_symbols: Option<u16>,
    /// PSK/QAM only: pilot symbols per carrier per frame.
    pub pilots_per_carrier: Option<u8>,
    /// Single-carrier PSK levels only; empty for FSK and OFDM levels.
    pub carrier_freqs_hz: &'static [u16],
    /// OFDM levels only: FFT bin of the lowest subcarrier.
    first_bin: Option<u8>,
    pub fsk: Option<FskParams>,
}

impl Modulation {
    pub fn bits_per_symbol(self) -> Option<u8> {
        match self {
            Modulation::Fsk => None,
            Modulation::Bpsk => Some(1),
            Modulation::Qpsk => Some(2),
            Modulation::Psk8 => Some(3),
            Modulation::Qam16 => Some(4),
            Modulation::Qam32 => Some(5),
        }
    }
}

impl SpeedLevel {
    fn has_pilot_mask(&self) -> bool {
        self.modulation != Modulation::Fsk
    }

    /// Cells in the frame map (symbols x carriers); PSK/QAM only.
    pub fn frame_cells(&self) -> Option<usize> {
        self.has_pilot_mask()
            .then(|| self.frame_symbols.map(|s| s as usize * self.carriers as usize))
            .flatten()
    }

    pub fn pilots(&self) -> Option<usize> {
        self.pilots_per_carrier.map(|p| p as usize * self.carriers as usize)
    }

    pub fn symbol_rate(&self) -> f64 {
        BASEBAND_HZ / self.samples_per_symbol as f64
    }

    /// Cells carrying coded bits; PSK/QAM only.
    pub fn data_cells(&self) -> Option<usize> {
        let bits = self.modulation.bits_per_symbol()?;
        Some((self.coded_bits / bits as u32) as usize)
    }

    /// FFT bins of the OFDM subcarriers; None on FSK and single-carrier levels.
    pub fn ofdm_bins(&self) -> Option<RangeInclusive<u8>> {
        self.first_bin.map(|b| b..=b + self.carriers - 1)
    }
}

pub fn bin_frequency_hz(bin: u8) -> f64 {
    bin as f64 * BIN_HZ
}

pub fn levels(bandwidth: Bandwidth) -> &'static [SpeedLevel] {
    match bandwidth {
        Bandwidth::Hz500 => LEVELS_500,
        Bandwidth::Hz2300 => LEVELS_2300,
        Bandwidth::Hz2750 => LEVELS_2750,
    }
}

use Modulation::*;

/// Frame payload: (K, N, code rate).
type Fec = (u32, u32, (u8, u8));

/// FSK level; `tones` is the published carrier count, `p` the hopping parameters from the mode table.
const fn fsk(level: u8, tones: u8, nominal_baud: u8, net_bps: u16, frame_symbols: u16, fec: Fec, p: FskParams) -> SpeedLevel {
    let samples_per_symbol = match nominal_baud {
        23 => 128,
        47 => 64,
        _ => 32,
    };
    SpeedLevel {
        level,
        modulation: Fsk,
        carriers: tones,
        nominal_baud,
        samples_per_symbol,
        net_bps,
        info_bits: fec.0,
        coded_bits: fec.1,
        code_rate: fec.2,
        frame_symbols: Some(frame_symbols),
        pilots_per_carrier: None,
        carrier_freqs_hz: &[],
        first_bin: None,
        fsk: Some(p),
    }
}

/// (bits per symbol, first bin, last bin, spacing).
const fn hop(bits: u8, first_bin: u8, last_bin: u8, spacing: u8) -> FskParams {
    FskParams { bits, first_bin, last_bin, spacing }
}

/// Single-carrier PSK level at 94 baud, one tone per listed frequency.
const fn tones(level: u8, modulation: Modulation, net_bps: u16, frame_symbols: u16, pilots: u8, freqs: &'static [u16], fec: Fec) -> SpeedLevel {
    SpeedLevel {
        level,
        modulation,
        carriers: freqs.len() as u8,
        nominal_baud: 94,
        samples_per_symbol: 32,
        net_bps,
        info_bits: fec.0,
        coded_bits: fec.1,
        code_rate: fec.2,
        frame_symbols: Some(frame_symbols),
        pilots_per_carrier: Some(pilots),
        carrier_freqs_hz: freqs,
        first_bin: None,
        fsk: None,
    }
}

/// OFDM level at 42 baud, subcarriers on contiguous FFT bins centred on bin 32.
const fn ofdm(level: u8, modulation: Modulation, carriers: u8, net_bps: u16, frame_symbols: u16, pilots: u8, fec: Fec) -> SpeedLevel {
    SpeedLevel {
        level,
        modulation,
        carriers,
        nominal_baud: 42,
        samples_per_symbol: OFDM_FFT + OFDM_CP,
        net_bps,
        info_bits: fec.0,
        coded_bits: fec.1,
        code_rate: fec.2,
        frame_symbols: Some(frame_symbols),
        pilots_per_carrier: Some(pilots),
        carrier_freqs_hz: &[],
        first_bin: Some(OFDM_FFT / 2 - (carriers - 1) / 2),
        fsk: None,
    }
}

const R13: (u8, u8) = (1, 3);
const R12: (u8, u8) = (1, 2);
const R23: (u8, u8) = (2, 3);
const R45: (u8, u8) = (4, 5);
const R56: (u8, u8) = (5, 6);

const LEVELS_500: &[SpeedLevel] = &[
    fsk(1, 11, 23, 18, 124, (96, 300, R13), hop(3, 54, 74, 2)),
    fsk(2, 11, 47, 41, 228, (200, 612, R13), hop(3, 27, 37, 1)),
    fsk(3, 11, 47, 61, 226, (297, 606, R12), hop(3, 27, 37, 1)),
    tones(4, Bpsk, 88, 393, 18, &[1350, 1650], (368, 748, R12)),
    tones(5, Qpsk, 177, 394, 18, &[1350, 1650], (744, 1500, R12)),
    tones(6, Qpsk, 270, 375, 12, &[1320, 1500, 1680], (1080, 2172, R12)),
    ofdm(7, Qpsk, 11, 441, 149, 4, (1578, 3168, R12)),
    ofdm(8, Qpsk, 11, 588, 149, 4, (2104, 3168, R23)),
    ofdm(9, Qpsk, 11, 705, 149, 4, (2524, 3168, R45)),
    ofdm(10, Psk8, 11, 884, 149, 4, (3160, 4752, R23)),
    ofdm(11, Psk8, 11, 1060, 149, 4, (3792, 4752, R45)),
    ofdm(12, Qam16, 11, 1286, 154, 4, (4755, 5720, R56)),
    ofdm(13, Qam32, 11, 1543, 154, 4, (5708, 7150, R45)),
];

const LEVELS_2300: &[SpeedLevel] = &[
    fsk(1, 32, 23, 18, 124, (96, 300, R13), hop(3, 33, 96, 2)),
    fsk(2, 16, 47, 41, 228, (200, 612, R13), hop(3, 17, 48, 2)),
    fsk(3, 16, 47, 82, 228, (402, 816, R12), hop(4, 17, 48, 2)),
    fsk(4, 16, 94, 175, 395, (736, 1484, R12), hop(4, 9, 24, 1)),
    tones(5, Qpsk, 270, 375, 12, &[900, 1500, 2100], (1080, 2172, R12)),
    tones(6, Qpsk, 363, 364, 9, &[750, 1250, 1750, 2250], (1410, 2832, R12)),
    tones(7, Qpsk, 549, 354, 6, &[700, 1020, 1340, 1660, 1980, 2300], (2076, 4164, R12)),
    tones(8, Qpsk, 735, 351, 5, &[702, 930, 1158, 1386, 1614, 1842, 2070, 2298], (2754, 5520, R12)),
    tones(9, Qpsk, 922, 348, 4, &[555, 765, 975, 1185, 1395, 1605, 1815, 2025, 2235, 2445], (3424, 6860, R12)),
    ofdm(10, Qpsk, 49, 2011, 149, 1, (7197, 14406, R12)),
    ofdm(11, Qpsk, 49, 2682, 149, 1, (9596, 14406, R23)),
    ofdm(12, Qpsk, 49, 3219, 149, 1, (11512, 14406, R45)),
    ofdm(13, Psk8, 49, 4025, 149, 1, (14398, 21609, R23)),
    ofdm(14, Psk8, 49, 4830, 149, 1, (17276, 21609, R45)),
    ofdm(15, Qam16, 49, 5872, 154, 1, (21710, 26068, R56)),
    ofdm(16, Qam32, 49, 7050, 154, 1, (26056, 32585, R45)),
];

const LEVELS_2750: &[SpeedLevel] = &[
    fsk(1, 40, 23, 18, 124, (96, 300, R13), hop(3, 24, 103, 2)),
    fsk(2, 20, 47, 41, 228, (200, 612, R13), hop(3, 12, 51, 2)),
    fsk(3, 20, 47, 82, 228, (402, 816, R12), hop(4, 12, 51, 2)),
    fsk(4, 20, 94, 175, 395, (736, 1484, R12), hop(4, 6, 25, 1)),
    tones(5, Qpsk, 270, 375, 12, &[600, 1500, 2400], (1080, 2172, R12)),
    tones(6, Qpsk, 363, 364, 9, &[510, 1170, 1830, 2490], (1410, 2832, R12)),
    tones(7, Qpsk, 549, 354, 6, &[450, 870, 1290, 1710, 2130, 2550], (2076, 4164, R12)),
    tones(8, Qpsk, 735, 351, 5, &[380, 700, 1020, 1340, 1660, 1980, 2300, 2620], (2754, 5520, R12)),
    tones(9, Qpsk, 922, 348, 4, &[375, 625, 875, 1125, 1375, 1625, 1875, 2125, 2375, 2625], (3424, 6860, R12)),
    tones(10, Qpsk, 1203, 348, 3, &[240, 450, 660, 870, 1080, 1290, 1500, 1710, 1920, 2130, 2340, 2550, 2760], (4466, 8944, R12)),
    ofdm(11, Qpsk, 59, 2423, 149, 1, (8667, 17346, R12)),
    ofdm(12, Qpsk, 59, 3230, 149, 1, (11556, 17346, R23)),
    ofdm(13, Qpsk, 59, 3877, 149, 1, (13864, 17346, R45)),
    ofdm(14, Psk8, 59, 4848, 149, 1, (17338, 26019, R23)),
    ofdm(15, Psk8, 59, 5817, 149, 1, (20804, 26019, R45)),
    ofdm(16, Qam16, 59, 7074, 154, 1, (26145, 31388, R56)),
    ofdm(17, Qam32, 59, 8489, 154, 1, (31376, 39235, R45)),
];
