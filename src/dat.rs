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

/// Pilot entries per FSK record (mode table field shared by every FSK record).
const FSK_PILOTS: usize = 24;

const SERVICE_11X50: RecordShape = RecordShape {
    kind: ShapeKind::Psk { carriers: 11, frame_symbols: 50, pilots_per_carrier: 4 },
    info_bits: 1704,
    coded_bits: 2145,
};

/// Records after the speed levels; their purpose is not known yet.
fn service_shapes(bandwidth: Bandwidth) -> &'static [RecordShape] {
    match bandwidth {
        Bandwidth::Hz500 => &[
            RecordShape {
                kind: ShapeKind::Psk { carriers: 2, frame_symbols: 450, pilots_per_carrier: 18 },
                info_bits: 856,
                coded_bits: 1724,
            },
            RecordShape { kind: ShapeKind::Fsk { frame_symbols: 92 }, info_bits: 64, coded_bits: 204 },
            SERVICE_11X50,
            SERVICE_11X50,
        ],
        Bandwidth::Hz2300 => &[SERVICE_11X50],
        Bandwidth::Hz2750 => &[],
    }
}

/// The 17 records of a table block, in file order: the speed levels, then service frames.
pub fn record_shapes(bandwidth: Bandwidth) -> Vec<RecordShape> {
    let levels = crate::modes::levels(bandwidth).iter().map(|l| {
        let frame_symbols = l.frame_symbols.expect("every level has a frame length");
        let kind = match l.pilots_per_carrier {
            None => ShapeKind::Fsk { frame_symbols },
            Some(pilots_per_carrier) => ShapeKind::Psk { carriers: l.carriers, frame_symbols, pilots_per_carrier },
        };
        RecordShape { kind, info_bits: l.info_bits, coded_bits: l.coded_bits }
    });
    levels.chain(service_shapes(bandwidth).iter().copied()).collect()
}

/// Sequential reader over the table words, in VARA's parse order.
struct Cursor<'a> {
    words: &'a [u16],
    pos: usize,
    record: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u16], LayoutError> {
        let end = self.pos + n;
        let out = self.words.get(self.pos..end).ok_or(LayoutError::Truncated { record: self.record })?;
        self.pos = end;
        Ok(out)
    }

    fn permutation(&mut self, n: u32, section: Section) -> Result<Vec<u16>, LayoutError> {
        let p = self.take(n as usize)?;
        let mut seen = vec![false; p.len()];
        for &v in p {
            match seen.get_mut(v as usize) {
                Some(s) if !*s => *s = true,
                _ => return Err(LayoutError::NotPermutation { record: self.record, section }),
            }
        }
        Ok(p.to_vec())
    }
}

impl ModemTables {
    /// Walks the table block record by record; every word must belong to a record.
    pub fn records(&self) -> Result<Vec<Record>, LayoutError> {
        let mut cur = Cursor { words: &self.words, pos: 0, record: 0 };
        let mut records = Vec::new();
        for (i, shape) in record_shapes(self.bandwidth).into_iter().enumerate() {
            cur.record = i;
            let channel_interleaver = cur.permutation(shape.coded_bits, Section::ChannelInterleaver)?;
            let info_interleaver = cur.permutation(shape.info_bits, Section::InfoInterleaver)?;
            let body = match shape.kind {
                ShapeKind::Psk { carriers, frame_symbols, pilots_per_carrier } => {
                    let pilot_symbols = (0..carriers)
                        .map(|_| cur.take(pilots_per_carrier as usize).map(<[u16]>::to_vec))
                        .collect::<Result<_, _>>()?;
                    let mask = (0..carriers)
                        .map(|_| {
                            cur.take(frame_symbols as usize)?
                                .iter()
                                .map(|&w| match w {
                                    0 => Ok(false),
                                    1 => Ok(true),
                                    _ => Err(LayoutError::MaskNotBinary { record: i }),
                                })
                                .collect()
                        })
                        .collect::<Result<_, _>>()?;
                    RecordBody::Psk { pilot_symbols, mask }
                }
                ShapeKind::Fsk { frame_symbols } => {
                    let n = frame_symbols as usize;
                    let map = cur.take(n)?.to_vec();
                    let pilots = cur.take(FSK_PILOTS)?.to_vec();
                    let pairs = cur.take(2 * n)?.chunks_exact(2).map(|p| (p[0], p[1])).collect();
                    RecordBody::Fsk { map, pilots, pairs }
                }
            };
            records.push(Record { channel_interleaver, info_interleaver, body });
        }
        match self.words.len() - cur.pos {
            0 => Ok(records),
            count => Err(LayoutError::TrailingWords { count }),
        }
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
