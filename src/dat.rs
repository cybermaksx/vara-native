//! Loader for the VARAHF500/2300/2750.dat modem tables.

use crate::crc::crc16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bandwidth {
    Hz500,
    Hz2300,
    Hz2750,
}

#[derive(Debug, PartialEq, Eq)]
pub enum DatError {
    WrongSize { expected: usize, actual: usize },
    BadCrc { stored: u16, computed: u16 },
}

#[derive(Debug)]
pub struct ModemTables {
    pub bandwidth: Bandwidth,
    /// Table block as big-endian u16 words, without the trailing CRC.
    pub words: Vec<u16>,
    /// Bytes after the CRC; only VARAHF500.dat carries this second block.
    pub extra: Vec<u8>,
}

impl Bandwidth {
    /// Exact file size VARA accepts.
    pub fn file_size(self) -> usize {
        match self {
            Bandwidth::Hz500 => 734_771,
            Bandwidth::Hz2300 => 720_716,
            Bandwidth::Hz2750 => 872_206,
        }
    }

    pub fn file_name(self) -> &'static str {
        match self {
            Bandwidth::Hz500 => "VARAHF500.dat",
            Bandwidth::Hz2300 => "VARAHF2300.dat",
            Bandwidth::Hz2750 => "VARAHF2750.dat",
        }
    }

    /// Length of the table block; the big-endian CRC follows it.
    fn table_len(self) -> usize {
        match self {
            Bandwidth::Hz500 => 0x2dc00,
            Bandwidth::Hz2300 => 0xaff4a,
            Bandwidth::Hz2750 => 0xd4f0c,
        }
    }
}

/// Size of one record in the table block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordShape {
    pub kind: ShapeKind,
    /// Length of the information interleaver (K).
    pub info_bits: u32,
    /// Length of the channel interleaver (N).
    pub coded_bits: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeKind {
    Fsk { frame_symbols: u16 },
    Psk { carriers: u8, frame_symbols: u16, pilots_per_carrier: u8 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    /// Permutation of the N coded bits.
    pub channel_interleaver: Vec<u16>,
    /// Permutation of the K information bits.
    pub info_interleaver: Vec<u16>,
    pub body: RecordBody,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordBody {
    /// Per carrier: pilot symbol numbers in file order, and the pilot mask over the frame.
    Psk { pilot_symbols: Vec<Vec<u16>>, mask: Vec<Vec<bool>> },
    /// FSK tables, kept raw until their meaning is known.
    Fsk { map: Vec<u16>, pilots: Vec<u16>, pairs: Vec<(u16, u16)> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    ChannelInterleaver,
    InfoInterleaver,
}

#[derive(Debug, PartialEq, Eq)]
pub enum LayoutError {
    NotPermutation { record: usize, section: Section },
    MaskNotBinary { record: usize },
    Truncated { record: usize },
    TrailingWords { count: usize },
}

/// The 17 records of a table block, in file order: the speed levels, then service frames.
pub fn record_shapes(bandwidth: Bandwidth) -> Vec<RecordShape> {
    let _ = bandwidth;
    todo!()
}

impl ModemTables {
    pub fn records(&self) -> Result<Vec<Record>, LayoutError> {
        todo!()
    }
}

pub fn parse(bandwidth: Bandwidth, bytes: &[u8]) -> Result<ModemTables, DatError> {
    let expected = bandwidth.file_size();
    if bytes.len() != expected {
        return Err(DatError::WrongSize { expected, actual: bytes.len() });
    }

    let n = bandwidth.table_len();
    let stored = u16::from_be_bytes([bytes[n], bytes[n + 1]]);
    // VARA's loop stops one byte short, so the last table byte is not covered.
    let computed = crc16(&bytes[..n - 1]);
    if stored != computed {
        return Err(DatError::BadCrc { stored, computed });
    }

    let words = bytes[..n]
        .chunks_exact(2)
        .map(|w| u16::from_be_bytes([w[0], w[1]]))
        .collect();
    let extra = bytes[n + 2..].to_vec();

    Ok(ModemTables { bandwidth, words, extra })
}
