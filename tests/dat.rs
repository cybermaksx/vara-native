use std::path::PathBuf;
use vara_native::crc::crc16;
use vara_native::dat::{parse, Bandwidth, DatError};

const ALL: [Bandwidth; 3] = [Bandwidth::Hz500, Bandwidth::Hz2300, Bandwidth::Hz2750];

/// Builds a file VARA would accept: zeros with a valid CRC.
/// VARA's checksum skips the last data byte before the CRC (off-by-one in the original loop).
fn synthetic(bw: Bandwidth) -> Vec<u8> {
    let mut f = vec![0u8; bw.file_size()];
    let crc_at = crc_offset(bw);
    let crc = crc16(&f[..crc_at - 1]);
    f[crc_at..crc_at + 2].copy_from_slice(&crc.to_be_bytes());
    f
}

fn crc_offset(bw: Bandwidth) -> usize {
    match bw {
        Bandwidth::Hz500 => 0x2dc00,
        Bandwidth::Hz2300 => 0xaff4a,
        Bandwidth::Hz2750 => 0xd4f0c,
    }
}

/// Real VARA tables are not redistributed; point VARA_DIR at an installed VARA folder to run these.
fn real(bw: Bandwidth) -> Option<Vec<u8>> {
    let dir = PathBuf::from(std::env::var_os("VARA_DIR")?);
    Some(std::fs::read(dir.join(bw.file_name())).expect("VARA_DIR set but table file missing"))
}

#[test]
fn file_sizes_match_vara_loader() {
    assert_eq!(Bandwidth::Hz500.file_size(), 734_771);
    assert_eq!(Bandwidth::Hz2300.file_size(), 720_716);
    assert_eq!(Bandwidth::Hz2750.file_size(), 872_206);
}

#[test]
fn file_names() {
    assert_eq!(Bandwidth::Hz500.file_name(), "VARAHF500.dat");
    assert_eq!(Bandwidth::Hz2300.file_name(), "VARAHF2300.dat");
    assert_eq!(Bandwidth::Hz2750.file_name(), "VARAHF2750.dat");
}

#[test]
fn accepts_valid_file() {
    for bw in ALL {
        let t = parse(bw, &synthetic(bw)).unwrap();
        assert_eq!(t.bandwidth, bw);
        assert_eq!(t.words.len(), crc_offset(bw) / 2);
    }
}

#[test]
fn rejects_wrong_size() {
    for bw in ALL {
        let mut f = synthetic(bw);
        f.pop();
        assert_eq!(
            parse(bw, &f).unwrap_err(),
            DatError::WrongSize { expected: bw.file_size(), actual: bw.file_size() - 1 }
        );
    }
}

#[test]
fn rejects_bad_crc() {
    for bw in ALL {
        let mut f = synthetic(bw);
        f[0] ^= 0xFF;
        assert!(matches!(parse(bw, &f), Err(DatError::BadCrc { .. })));
    }
}

#[test]
fn last_data_byte_is_not_covered_by_crc() {
    for bw in ALL {
        let mut f = synthetic(bw);
        f[crc_offset(bw) - 1] ^= 0xFF;
        assert!(parse(bw, &f).is_ok());
    }
}

#[test]
fn words_are_big_endian() {
    let bw = Bandwidth::Hz2300;
    let mut f = synthetic(bw);
    f[0] = 0x01;
    f[1] = 0x02;
    let crc_at = crc_offset(bw);
    let crc = crc16(&f[..crc_at - 1]);
    f[crc_at..crc_at + 2].copy_from_slice(&crc.to_be_bytes());
    assert_eq!(parse(bw, &f).unwrap().words[0], 0x0102);
}

#[test]
fn only_500_has_extra_block() {
    assert_eq!(parse(Bandwidth::Hz500, &synthetic(Bandwidth::Hz500)).unwrap().extra.len(), 547_377);
    assert!(parse(Bandwidth::Hz2300, &synthetic(Bandwidth::Hz2300)).unwrap().extra.is_empty());
    assert!(parse(Bandwidth::Hz2750, &synthetic(Bandwidth::Hz2750)).unwrap().extra.is_empty());
}

#[test]
fn real_files_have_vara_crcs() {
    let expected = [(Bandwidth::Hz500, 0x1B72), (Bandwidth::Hz2300, 0xFBAC), (Bandwidth::Hz2750, 0x3DB1)];
    for (bw, crc) in expected {
        let Some(f) = real(bw) else { return };
        assert_eq!(crc16(&f[..crc_offset(bw) - 1]), crc);
        assert!(parse(bw, &f).is_ok());
    }
}

#[test]
fn real_files_start_with_two_permutations() {
    for bw in ALL {
        let Some(f) = real(bw) else { return };
        let w = parse(bw, &f).unwrap().words;
        assert!(is_permutation(&w[..300]), "{bw:?}: first 300 words");
        assert!(is_permutation(&w[300..396]), "{bw:?}: next 96 words");
    }
}

fn is_permutation(w: &[u16]) -> bool {
    let mut s = w.to_vec();
    s.sort_unstable();
    s.iter().enumerate().all(|(i, &v)| v as usize == i)
}
