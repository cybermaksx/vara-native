use vara_native::crc::crc16;

// Standard check value for CRC-16/GENIBUS.
#[test]
fn check_value_123456789() {
    assert_eq!(crc16(b"123456789"), 0xD64E);
}

#[test]
fn empty_input() {
    assert_eq!(crc16(&[]), 0x0000);
}

// First 16 bytes of every VARAHF*.dat; the implementation must agree with a bitwise reference.
#[test]
fn matches_bitwise_reference() {
    let prefix = [
        0x00, 0x7a, 0x00, 0x21, 0x00, 0xc8, 0x01, 0x18, 0x00, 0x9a, 0x00, 0x16, 0x00, 0x30, 0x00, 0x9e,
    ];
    assert_eq!(crc16(&prefix), reference(&prefix));
}

// Bitwise reference, kept separate from the table-driven implementation under test.
fn reference(data: &[u8]) -> u16 {
    let mut c: u16 = 0xFFFF;
    for &b in data {
        c ^= (b as u16) << 8;
        for _ in 0..8 {
            c = if c & 0x8000 != 0 { (c << 1) ^ 0x1021 } else { c << 1 };
        }
    }
    c ^ 0xFFFF
}
