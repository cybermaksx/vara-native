/// CRC-16 used by VARA: poly 0x1021, init 0xFFFF, xorout 0xFFFF, not reflected (CRC-16/GENIBUS).
pub fn crc16(data: &[u8]) -> u16 {
    let crc = data.iter().fold(0xFFFFu16, |c, &b| {
        (c << 8) ^ TABLE[((c >> 8) as u8 ^ b) as usize]
    });
    crc ^ 0xFFFF
}

const TABLE: [u16; 256] = build_table();

const fn build_table() -> [u16; 256] {
    let mut t = [0u16; 256];
    let mut i = 0;
    while i < 256 {
        let mut c = (i as u16) << 8;
        let mut bit = 0;
        while bit < 8 {
            c = if c & 0x8000 != 0 { (c << 1) ^ 0x1021 } else { c << 1 };
            bit += 1;
        }
        t[i] = c;
        i += 1;
    }
    t
}
