//! Speed levels per bandwidth.
//!
//! Rates and modulations follow the speed level table published by the VARA author; frame map
//! sizes, pilot counts and carrier frequencies follow the mode table built at startup by VARA.exe
//! and agree with the pilot masks in the .dat files (see doc/protocol.md, section 3).

use crate::dat::Bandwidth;
use std::ops::RangeInclusive;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modulation {
    Fsk,
    Bpsk,
    Qpsk,
    Psk8,
    Qam16,
    Qam32,
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
    /// PSK/QAM only: symbols per frame on each carrier.
    pub frame_symbols: Option<u16>,
    /// PSK/QAM only: pilot symbols per carrier per frame.
    pub pilots_per_carrier: Option<u8>,
    /// Single-carrier PSK levels only; empty for FSK and OFDM levels.
    pub carrier_freqs_hz: &'static [u16],
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
    /// Cells in the frame map (symbols x carriers).
    pub fn frame_cells(&self) -> Option<usize> {
        self.frame_symbols.map(|s| s as usize * self.carriers as usize)
    }

    pub fn pilots(&self) -> Option<usize> {
        self.pilots_per_carrier.map(|p| p as usize * self.carriers as usize)
    }

    pub fn symbol_rate(&self) -> f64 {
        todo!()
    }

    pub fn data_cells(&self) -> Option<usize> {
        todo!()
    }

    /// FFT bins of the OFDM subcarriers; None on FSK and single-carrier levels.
    pub fn ofdm_bins(&self) -> Option<RangeInclusive<u8>> {
        todo!()
    }
}

pub fn bin_frequency_hz(bin: u8) -> f64 {
    let _ = bin;
    todo!()
}

pub fn levels(bandwidth: Bandwidth) -> &'static [SpeedLevel] {
    match bandwidth {
        Bandwidth::Hz500 => LEVELS_500,
        Bandwidth::Hz2300 => LEVELS_2300,
        Bandwidth::Hz2750 => LEVELS_2750,
    }
}

use Modulation::*;

const fn fsk(level: u8, tones: u8, nominal_baud: u8, net_bps: u16) -> SpeedLevel {
    SpeedLevel {
        level,
        modulation: Fsk,
        carriers: tones,
        nominal_baud,
        samples_per_symbol: 0,
        net_bps,
        info_bits: 0,
        coded_bits: 0,
        code_rate: (0, 0),
        frame_symbols: None,
        pilots_per_carrier: None,
        carrier_freqs_hz: &[],
    }
}

/// Single-carrier PSK level at 94 baud, one tone per listed frequency.
const fn tones(level: u8, modulation: Modulation, net_bps: u16, frame_symbols: u16, pilots: u8, freqs: &'static [u16]) -> SpeedLevel {
    SpeedLevel {
        level,
        modulation,
        carriers: freqs.len() as u8,
        nominal_baud: 94,
        samples_per_symbol: 0,
        net_bps,
        info_bits: 0,
        coded_bits: 0,
        code_rate: (0, 0),
        frame_symbols: Some(frame_symbols),
        pilots_per_carrier: Some(pilots),
        carrier_freqs_hz: freqs,
    }
}

/// OFDM level at 42 baud.
const fn ofdm(level: u8, modulation: Modulation, carriers: u8, net_bps: u16, frame_symbols: u16, pilots: u8) -> SpeedLevel {
    SpeedLevel {
        level,
        modulation,
        carriers,
        nominal_baud: 42,
        samples_per_symbol: 0,
        net_bps,
        info_bits: 0,
        coded_bits: 0,
        code_rate: (0, 0),
        frame_symbols: Some(frame_symbols),
        pilots_per_carrier: Some(pilots),
        carrier_freqs_hz: &[],
    }
}

const LEVELS_500: &[SpeedLevel] = &[
    fsk(1, 11, 23, 18),
    fsk(2, 11, 47, 41),
    fsk(3, 11, 47, 61),
    tones(4, Bpsk, 88, 393, 18, &[1350, 1650]),
    tones(5, Qpsk, 177, 394, 18, &[1350, 1650]),
    tones(6, Qpsk, 270, 375, 12, &[1320, 1500, 1680]),
    ofdm(7, Qpsk, 11, 441, 149, 4),
    ofdm(8, Qpsk, 11, 588, 149, 4),
    ofdm(9, Qpsk, 11, 705, 149, 4),
    ofdm(10, Psk8, 11, 884, 149, 4),
    ofdm(11, Psk8, 11, 1060, 149, 4),
    ofdm(12, Qam16, 11, 1286, 154, 4),
    ofdm(13, Qam32, 11, 1543, 154, 4),
];

const LEVELS_2300: &[SpeedLevel] = &[
    fsk(1, 32, 23, 18),
    fsk(2, 16, 47, 41),
    fsk(3, 16, 47, 82),
    fsk(4, 16, 94, 175),
    tones(5, Qpsk, 270, 375, 12, &[900, 1500, 2100]),
    tones(6, Qpsk, 363, 364, 9, &[750, 1250, 1750, 2250]),
    tones(7, Qpsk, 549, 354, 6, &[700, 1020, 1340, 1660, 1980, 2300]),
    tones(8, Qpsk, 735, 351, 5, &[702, 930, 1158, 1386, 1614, 1842, 2070, 2298]),
    tones(9, Qpsk, 922, 348, 4, &[555, 765, 975, 1185, 1395, 1605, 1815, 2025, 2235, 2445]),
    ofdm(10, Qpsk, 49, 2011, 149, 1),
    ofdm(11, Qpsk, 49, 2682, 149, 1),
    ofdm(12, Qpsk, 49, 3219, 149, 1),
    ofdm(13, Psk8, 49, 4025, 149, 1),
    ofdm(14, Psk8, 49, 4830, 149, 1),
    ofdm(15, Qam16, 49, 5872, 154, 1),
    ofdm(16, Qam32, 49, 7050, 154, 1),
];

const LEVELS_2750: &[SpeedLevel] = &[
    fsk(1, 40, 23, 18),
    fsk(2, 20, 47, 41),
    fsk(3, 20, 47, 82),
    fsk(4, 20, 94, 175),
    tones(5, Qpsk, 270, 375, 12, &[600, 1500, 2400]),
    tones(6, Qpsk, 363, 364, 9, &[510, 1170, 1830, 2490]),
    tones(7, Qpsk, 549, 354, 6, &[450, 870, 1290, 1710, 2130, 2550]),
    tones(8, Qpsk, 735, 351, 5, &[380, 700, 1020, 1340, 1660, 1980, 2300, 2620]),
    tones(9, Qpsk, 922, 348, 4, &[375, 625, 875, 1125, 1375, 1625, 1875, 2125, 2375, 2625]),
    tones(10, Qpsk, 1203, 348, 3, &[240, 450, 660, 870, 1080, 1290, 1500, 1710, 1920, 2130, 2340, 2550, 2760]),
    ofdm(11, Qpsk, 59, 2423, 149, 1),
    ofdm(12, Qpsk, 59, 3230, 149, 1),
    ofdm(13, Qpsk, 59, 3877, 149, 1),
    ofdm(14, Psk8, 59, 4848, 149, 1),
    ofdm(15, Psk8, 59, 5817, 149, 1),
    ofdm(16, Qam16, 59, 7074, 154, 1),
    ofdm(17, Qam32, 59, 8489, 154, 1),
];
