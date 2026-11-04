//! FSK levels: frequency-hopped MFSK. Each symbol has a base bin from the .dat map; data shifts the
//! tone by `spacing x d` (d = inverse Gray of the bits), wrapping around inside the band.

use std::path::PathBuf;
use vara_native::dat::{parse, Bandwidth, Record, RecordBody};
use vara_native::fsk::{gray_decode, tone_bins, FskError, FSK_PILOTS};
use vara_native::modes::{levels, Modulation, SpeedLevel};

const ALL: [Bandwidth; 3] = [Bandwidth::Hz500, Bandwidth::Hz2300, Bandwidth::Hz2750];

fn fsk_levels(bw: Bandwidth) -> impl Iterator<Item = &'static SpeedLevel> {
    levels(bw).iter().filter(|l| l.modulation == Modulation::Fsk)
}

fn bits(n: usize, seed: u64) -> Vec<u8> {
    let mut s = seed;
    (0..n)
        .map(|_| {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            (s & 1) as u8
        })
        .collect()
}

// VARA's own table of FSK symbol values, rows for 2, 3 and 4 bits.
#[test]
fn symbol_values_are_inverse_gray() {
    assert_eq!((0..4).map(gray_decode).collect::<Vec<_>>(), [0, 1, 3, 2]);
    assert_eq!((0..8).map(gray_decode).collect::<Vec<_>>(), [0, 1, 3, 2, 7, 6, 4, 5]);
    assert_eq!((0..16).map(gray_decode).collect::<Vec<_>>(), [0, 1, 3, 2, 7, 6, 4, 5, 15, 14, 12, 13, 8, 9, 11, 10]);
}

#[test]
fn fsk_parameters() {
    let p = |bw| -> Vec<(u8, u8, u8, u8)> {
        fsk_levels(bw).map(|l| { let f = l.fsk.unwrap(); (f.bits, f.first_bin, f.last_bin, f.spacing) }).collect()
    };
    assert_eq!(p(Bandwidth::Hz500), [(3, 54, 74, 2), (3, 27, 37, 1), (3, 27, 37, 1)]);
    assert_eq!(p(Bandwidth::Hz2300), [(3, 33, 96, 2), (3, 17, 48, 2), (4, 17, 48, 2), (4, 9, 24, 1)]);
    assert_eq!(p(Bandwidth::Hz2750), [(3, 24, 103, 2), (3, 12, 51, 2), (4, 12, 51, 2), (4, 6, 25, 1)]);
    for bw in ALL {
        assert!(levels(bw).iter().filter(|l| l.modulation != Modulation::Fsk).all(|l| l.fsk.is_none()));
    }
}

// The author's "carriers" for an FSK level are the tone positions: band bins / spacing, rounded up.
#[test]
fn tone_positions_match_the_published_carriers() {
    for bw in ALL {
        for l in fsk_levels(bw) {
            let f = l.fsk.unwrap();
            let band = (f.last_bin - f.first_bin + 1) as usize;
            assert_eq!(band.div_ceil(f.spacing as usize), l.carriers as usize, "{bw:?} L{}", l.level);
        }
    }
}

// Every frame symbol but the 24 pilots carries `bits` coded bits, filling N exactly.
#[test]
fn data_symbols_fill_the_coded_bits() {
    for bw in ALL {
        for l in fsk_levels(bw) {
            let f = l.fsk.unwrap();
            let data = l.frame_symbols.unwrap() as usize - FSK_PILOTS;
            assert_eq!(data * f.bits as usize, l.coded_bits as usize, "{bw:?} L{}", l.level);
        }
    }
}

fn fsk_record(map: Vec<u16>, pilots: &[(usize, u16)]) -> Record {
    let mut pairs = vec![(0u16, 0u16); map.len()];
    for &(t, d) in pilots {
        pairs[t] = (1, d);
    }
    Record { channel_interleaver: vec![], info_interleaver: vec![], body: RecordBody::Fsk { pilots: vec![0; FSK_PILOTS], map, pairs } }
}

#[test]
fn tone_formula_with_wraparound() {
    let l = &levels(Bandwidth::Hz2300)[0]; // 8 tones, bins 33..96, spacing 2
    let f = l.fsk.unwrap();
    let n = l.frame_symbols.unwrap() as usize;
    let mut map = vec![40u16; n];
    map[1] = 95; // 95 + 2*d wraps past 96 back to 33
    let pilots: Vec<(usize, u16)> = (0..FSK_PILOTS).map(|i| (2 + 5 * i, 6)).collect();
    let r = fsk_record(map, &pilots);
    let mut b = vec![0u8; (n - FSK_PILOTS) * 3];
    b[..6].copy_from_slice(&[0, 1, 1, 1, 0, 0]); // v = 3 -> d = 2; v = 4 -> d = 7
    let bins = tone_bins(l, &r, &b).unwrap();
    assert_eq!(bins.len(), n);
    assert_eq!(bins[0], 40 + 2 * 2);
    assert_eq!(bins[1], f.first_bin + ((95 + 2 * 7 - f.first_bin as u16) % 64) as u8); // 109 -> 45
    assert_eq!(bins[1], 45);
    assert_eq!(bins[2], 40 + 2 * 6, "pilot uses its own value");
}

#[test]
fn rejects_wrong_inputs() {
    let l = &levels(Bandwidth::Hz2300)[0];
    let n = l.frame_symbols.unwrap() as usize;
    let pilots: Vec<(usize, u16)> = (0..FSK_PILOTS).map(|i| (2 + 5 * i, 0)).collect();
    let r = fsk_record(vec![40; n], &pilots);
    let need = (n - FSK_PILOTS) * 3;
    assert_eq!(tone_bins(l, &r, &bits(need - 1, 1)).unwrap_err(), FskError::BitCount { expected: need, got: need - 1 });
    assert_eq!(tone_bins(l, &fsk_record(vec![40; n - 1], &pilots), &bits(need, 1)).unwrap_err(), FskError::RecordShape);
    assert_eq!(tone_bins(&levels(Bandwidth::Hz2300)[9], &r, &[]).unwrap_err(), FskError::NotFsk { level: 10 });
}

fn real_records(bw: Bandwidth) -> Option<Vec<Record>> {
    let dir = PathBuf::from(std::env::var_os("VARA_DIR")?);
    Some(parse(bw, &std::fs::read(dir.join(bw.file_name())).unwrap()).unwrap().records().unwrap())
}

// In VARA's tables: 24 flagged pilot symbols, at the listed positions (entry t is symbol t + 2).
#[test]
fn real_pilots_are_flagged_where_listed() {
    for bw in ALL {
        let Some(records) = real_records(bw) else { return };
        for (l, r) in levels(bw).iter().zip(&records).filter(|(l, _)| l.modulation == Modulation::Fsk) {
            let RecordBody::Fsk { pilots, pairs, .. } = &r.body else { panic!() };
            let flagged: Vec<u16> = pairs.iter().enumerate().filter(|(_, p)| p.0 == 1).map(|(t, _)| t as u16 + 2).collect();
            assert_eq!(&flagged, pilots, "{bw:?} L{}", l.level);
            assert!(pairs.iter().all(|p| p.0 <= 1));
        }
    }
}

#[test]
fn real_tables_give_tones_inside_the_band() {
    for bw in ALL {
        let Some(records) = real_records(bw) else { return };
        for (l, r) in levels(bw).iter().zip(&records).filter(|(l, _)| l.modulation == Modulation::Fsk) {
            let f = l.fsk.unwrap();
            let bins = tone_bins(l, r, &bits(l.coded_bits as usize, l.level as u64)).unwrap();
            assert_eq!(bins.len(), l.frame_symbols.unwrap() as usize);
            assert!(bins.iter().all(|b| (f.first_bin..=f.last_bin).contains(b)), "{bw:?} L{}", l.level);
        }
    }
}
