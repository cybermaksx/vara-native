use std::path::PathBuf;
use vara_native::crc::crc16;
use vara_native::dat::{parse, record_shapes, Bandwidth, LayoutError, RecordBody, RecordShape, Section, ShapeKind};
use vara_native::modes::levels;

const ALL: [Bandwidth; 3] = [Bandwidth::Hz500, Bandwidth::Hz2300, Bandwidth::Hz2750];

fn table_len(bw: Bandwidth) -> usize {
    match bw {
        Bandwidth::Hz500 => 0x2dc00,
        Bandwidth::Hz2300 => 0xaff4a,
        Bandwidth::Hz2750 => 0xd4f0c,
    }
}

fn words_of(shape: &RecordShape) -> usize {
    let n = shape.coded_bits as usize + shape.info_bits as usize;
    n + match shape.kind {
        ShapeKind::Fsk { frame_symbols } => 3 * frame_symbols as usize + 24,
        ShapeKind::Psk { carriers, frame_symbols, pilots_per_carrier } => {
            carriers as usize * (pilots_per_carrier as usize + frame_symbols as usize)
        }
    }
}

/// A table block VARA's parser would walk cleanly: identity permutations, pilots on the first symbols.
fn synthetic_words(bw: Bandwidth) -> Vec<u16> {
    let mut w = Vec::new();
    for s in record_shapes(bw) {
        w.extend(0..s.coded_bits as u16);
        w.extend(0..s.info_bits as u16);
        match s.kind {
            ShapeKind::Fsk { frame_symbols } => w.extend(std::iter::repeat_n(7, 3 * frame_symbols as usize + 24)),
            ShapeKind::Psk { carriers, frame_symbols, pilots_per_carrier } => {
                for _ in 0..carriers {
                    w.extend((0..pilots_per_carrier as u16).map(|p| p + 2));
                }
                for _ in 0..carriers {
                    w.extend((0..frame_symbols).map(|i| (i < pilots_per_carrier as u16) as u16));
                }
            }
        }
    }
    w
}

fn file_from_words(bw: Bandwidth, words: &[u16]) -> Vec<u8> {
    let mut f: Vec<u8> = words.iter().flat_map(|w| w.to_be_bytes()).collect();
    f.resize(bw.file_size(), 0);
    let n = table_len(bw);
    let crc = crc16(&f[..n - 1]);
    f[n..n + 2].copy_from_slice(&crc.to_be_bytes());
    f
}

fn real(bw: Bandwidth) -> Option<Vec<u8>> {
    let dir = PathBuf::from(std::env::var_os("VARA_DIR")?);
    Some(std::fs::read(dir.join(bw.file_name())).expect("VARA_DIR set but table file missing"))
}

// Every level, then the service frames that follow the levels in the files.
#[test]
fn shapes_start_with_the_levels() {
    for bw in ALL {
        let shapes = record_shapes(bw);
        assert_eq!(shapes.len(), 17, "{bw:?}: VARA keeps 17 records per bandwidth");
        for (s, l) in shapes.iter().zip(levels(bw)) {
            assert_eq!((s.info_bits, s.coded_bits), (l.info_bits, l.coded_bits), "{bw:?} L{}", l.level);
        }
    }
}

#[test]
fn service_frame_shapes() {
    let tail = |bw, from| -> Vec<RecordShape> { record_shapes(bw)[from..].to_vec() };
    let svc_11x50 = RecordShape {
        kind: ShapeKind::Psk { carriers: 11, frame_symbols: 50, pilots_per_carrier: 4 },
        info_bits: 1704,
        coded_bits: 2145,
    };
    assert_eq!(tail(Bandwidth::Hz2300, 16), [svc_11x50]);
    assert_eq!(tail(Bandwidth::Hz500, 13), [
        RecordShape { kind: ShapeKind::Psk { carriers: 2, frame_symbols: 450, pilots_per_carrier: 18 }, info_bits: 856, coded_bits: 1724 },
        RecordShape { kind: ShapeKind::Fsk { frame_symbols: 92 }, info_bits: 64, coded_bits: 204 },
        svc_11x50,
        svc_11x50,
    ]);
    assert!(tail(Bandwidth::Hz2750, 17).is_empty());
}

// The record layout accounts for every word of every table block, with nothing left over.
#[test]
fn layout_fills_table_block_exactly() {
    for bw in ALL {
        let total: usize = record_shapes(bw).iter().map(words_of).sum();
        assert_eq!(total * 2, table_len(bw), "{bw:?}");
    }
}

#[test]
fn parses_synthetic_tables() {
    for bw in ALL {
        let t = parse(bw, &file_from_words(bw, &synthetic_words(bw))).unwrap();
        let records = t.records().unwrap();
        assert_eq!(records.len(), 17);
        for (r, s) in records.iter().zip(record_shapes(bw)) {
            assert_eq!(r.channel_interleaver.len(), s.coded_bits as usize);
            assert_eq!(r.info_interleaver.len(), s.info_bits as usize);
            match (&r.body, s.kind) {
                (RecordBody::Psk { pilot_symbols, mask }, ShapeKind::Psk { carriers, frame_symbols, pilots_per_carrier }) => {
                    assert_eq!(pilot_symbols.len(), carriers as usize);
                    assert!(pilot_symbols.iter().all(|p| p.len() == pilots_per_carrier as usize));
                    assert_eq!(mask.len(), carriers as usize);
                    assert!(mask.iter().all(|m| m.len() == frame_symbols as usize));
                }
                (RecordBody::Fsk { map, pilots, pairs }, ShapeKind::Fsk { frame_symbols }) => {
                    assert_eq!((map.len(), pilots.len(), pairs.len()), (frame_symbols as usize, 24, frame_symbols as usize));
                }
                _ => panic!("{bw:?}: body kind does not match shape"),
            }
        }
    }
}

#[test]
fn rejects_non_permutation() {
    let bw = Bandwidth::Hz2300;
    let mut w = synthetic_words(bw);
    w[1] = 0; // channel interleaver of record 0 now holds 0 twice
    let t = parse(bw, &file_from_words(bw, &w)).unwrap();
    assert_eq!(t.records().unwrap_err(), LayoutError::NotPermutation { record: 0, section: Section::ChannelInterleaver });

    let mut w = synthetic_words(bw);
    w[300] = 1; // info interleaver of record 0
    let t = parse(bw, &file_from_words(bw, &w)).unwrap();
    assert_eq!(t.records().unwrap_err(), LayoutError::NotPermutation { record: 0, section: Section::InfoInterleaver });
}

#[test]
fn rejects_non_binary_mask() {
    let bw = Bandwidth::Hz2300;
    let shapes = record_shapes(bw);
    let mut w = synthetic_words(bw);
    // First word of the mask of record 4 (level 5).
    let at: usize = shapes[..4].iter().map(words_of).sum::<usize>()
        + (shapes[4].coded_bits + shapes[4].info_bits) as usize
        + 3 * 12;
    w[at] = 2;
    let t = parse(bw, &file_from_words(bw, &w)).unwrap();
    assert_eq!(t.records().unwrap_err(), LayoutError::MaskNotBinary { record: 4 });
}

#[test]
fn real_files_walk_cleanly() {
    for bw in ALL {
        let Some(f) = real(bw) else { return };
        let records = parse(bw, &f).unwrap().records().unwrap();
        for (r, s) in records.iter().zip(record_shapes(bw)) {
            assert_eq!((r.info_interleaver.len(), r.channel_interleaver.len()), (s.info_bits as usize, s.coded_bits as usize));
        }
    }
}

// Pilot positions are the 1s of the mask, as symbol numbers: the first mask entry is symbol 2.
#[test]
fn real_pilot_positions_are_the_mask_ones() {
    for bw in ALL {
        let Some(f) = real(bw) else { return };
        for (i, r) in parse(bw, &f).unwrap().records().unwrap().iter().enumerate() {
            let RecordBody::Psk { pilot_symbols, mask } = &r.body else { continue };
            for (c, (p, m)) in pilot_symbols.iter().zip(mask).enumerate() {
                let mut got = p.clone();
                got.sort_unstable();
                let want: Vec<u16> = m.iter().enumerate().filter(|(_, on)| **on).map(|(s, _)| s as u16 + 2).collect();
                assert_eq!(got, want, "{bw:?} record {i} carrier {c}");
            }
        }
    }
}
