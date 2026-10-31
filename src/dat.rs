//! Loader for the VARAHF500/2300/2750.dat modem tables.

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
        todo!()
    }

    pub fn file_name(self) -> &'static str {
        todo!()
    }
}

pub fn parse(bandwidth: Bandwidth, bytes: &[u8]) -> Result<ModemTables, DatError> {
    let _ = (bandwidth, bytes);
    todo!()
}
