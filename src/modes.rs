//! Speed levels per bandwidth.

use crate::dat::Bandwidth;

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
    pub net_bps: u16,
    /// PSK/QAM only: symbols per frame on each carrier.
    pub frame_symbols: Option<u16>,
    /// PSK/QAM only: pilot symbols per carrier per frame.
    pub pilots_per_carrier: Option<u8>,
    /// Single-carrier PSK levels only; empty for FSK and OFDM levels.
    pub carrier_freqs_hz: &'static [u16],
}

impl Modulation {
    pub fn bits_per_symbol(self) -> Option<u8> {
        todo!()
    }
}

impl SpeedLevel {
    /// Cells in the frame map (symbols x carriers).
    pub fn frame_cells(&self) -> Option<usize> {
        todo!()
    }

    pub fn pilots(&self) -> Option<usize> {
        todo!()
    }
}

pub fn levels(bandwidth: Bandwidth) -> &'static [SpeedLevel] {
    let _ = bandwidth;
    todo!()
}
