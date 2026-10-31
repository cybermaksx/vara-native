use std::path::PathBuf;
use vara_native::dat::{parse, Bandwidth};
use vara_native::modes::{levels, Modulation, SpeedLevel};
use Modulation::*;

const ALL: [Bandwidth; 3] = [Bandwidth::Hz500, Bandwidth::Hz2300, Bandwidth::Hz2750];

// Speed level table published by the VARA author ("VARA HF v4.3.0 Speed Levels.jpg"):
// (nominal baud, carriers, modulation, net bps) for levels 1, 2, 3, ...
const PUBLISHED_500: &[(u8, u8, Modulation, u16)] = &[
    (23, 11, Fsk, 18), (47, 11, Fsk, 41), (47, 11, Fsk, 61), (94, 2, Bpsk, 88),
    (94, 2, Qpsk, 177), (94, 3, Qpsk, 270), (42, 11, Qpsk, 441), (42, 11, Qpsk, 588),
    (42, 11, Qpsk, 705), (42, 11, Psk8, 884), (42, 11, Psk8, 1060), (42, 11, Qam16, 1286),
    (42, 11, Qam32, 1543),
];
const PUBLISHED_2300: &[(u8, u8, Modulation, u16)] = &[
    (23, 32, Fsk, 18), (47, 16, Fsk, 41), (47, 16, Fsk, 82), (94, 16, Fsk, 175),
    (94, 3, Qpsk, 270), (94, 4, Qpsk, 363), (94, 6, Qpsk, 549), (94, 8, Qpsk, 735),
    (94, 10, Qpsk, 922), (42, 49, Qpsk, 2011), (42, 49, Qpsk, 2682), (42, 49, Qpsk, 3219),
    (42, 49, Psk8, 4025), (42, 49, Psk8, 4830), (42, 49, Qam16, 5872), (42, 49, Qam32, 7050),
];
const PUBLISHED_2750: &[(u8, u8, Modulation, u16)] = &[
    (23, 40, Fsk, 18), (47, 20, Fsk, 41), (47, 20, Fsk, 82), (94, 20, Fsk, 175),
    (94, 3, Qpsk, 270), (94, 4, Qpsk, 363), (94, 6, Qpsk, 549), (94, 8, Qpsk, 735),
    (94, 10, Qpsk, 922), (94, 13, Qpsk, 1203), (42, 59, Qpsk, 2423), (42, 59, Qpsk, 3230),
    (42, 59, Qpsk, 3877), (42, 59, Psk8, 4848), (42, 59, Psk8, 5817), (42, 59, Qam16, 7074),
    (42, 59, Qam32, 8489),
];

fn published(bw: Bandwidth) -> &'static [(u8, u8, Modulation, u16)] {
    match bw {
        Bandwidth::Hz500 => PUBLISHED_500,
        Bandwidth::Hz2300 => PUBLISHED_2300,
        Bandwidth::Hz2750 => PUBLISHED_2750,
    }
}

fn psk_levels(bw: Bandwidth) -> impl Iterator<Item = &'static SpeedLevel> {
    levels(bw).iter().filter(|l| l.modulation != Fsk)
}

#[test]
fn level_counts() {
    assert_eq!(levels(Bandwidth::Hz500).len(), 13);
    assert_eq!(levels(Bandwidth::Hz2300).len(), 16);
    assert_eq!(levels(Bandwidth::Hz2750).len(), 17);
}

#[test]
fn levels_are_numbered_from_one() {
    for bw in ALL {
        for (i, l) in levels(bw).iter().enumerate() {
            assert_eq!(l.level as usize, i + 1, "{bw:?}");
        }
    }
}

#[test]
fn matches_published_speed_table() {
    for bw in ALL {
        let got: Vec<_> = levels(bw).iter().map(|l| (l.nominal_baud, l.carriers, l.modulation, l.net_bps)).collect();
        assert_eq!(got, published(bw), "{bw:?}");
    }
}

#[test]
fn net_rate_grows_with_level() {
    for bw in ALL {
        assert!(levels(bw).windows(2).all(|w| w[0].net_bps < w[1].net_bps), "{bw:?}");
    }
}

// Mode table field +0x01 in VARA.exe.
#[test]
fn bits_per_symbol() {
    assert_eq!(Fsk.bits_per_symbol(), None);
    assert_eq!(Bpsk.bits_per_symbol(), Some(1));
    assert_eq!(Qpsk.bits_per_symbol(), Some(2));
    assert_eq!(Psk8.bits_per_symbol(), Some(3));
    assert_eq!(Qam16.bits_per_symbol(), Some(4));
    assert_eq!(Qam32.bits_per_symbol(), Some(5));
}

#[test]
fn fsk_levels_carry_no_frame_map() {
    for bw in ALL {
        for l in levels(bw).iter().filter(|l| l.modulation == Fsk) {
            assert_eq!((l.frame_symbols, l.pilots_per_carrier, l.frame_cells(), l.pilots()), (None, None, None, None));
            assert!(l.carrier_freqs_hz.is_empty());
        }
    }
}

// Pilot masks found in the .dat files, in file order: (cells, pilots).
// The mode table in VARA.exe predicts exactly these: (frame symbols x carriers, pilots per carrier x carriers).
#[test]
fn frame_maps_match_dat_masks() {
    let expected: [(Bandwidth, &[(usize, usize)]); 3] = [
        (Bandwidth::Hz500, &[(786, 36), (788, 36), (1125, 36), (1639, 44), (1639, 44), (1639, 44),
                              (1639, 44), (1639, 44), (1694, 44), (1694, 44)]),
        (Bandwidth::Hz2300, &[(1125, 36), (1456, 36), (2124, 36), (2808, 40), (3480, 40), (7301, 49),
                               (7301, 49), (7301, 49), (7301, 49), (7301, 49), (7546, 49), (7546, 49)]),
        (Bandwidth::Hz2750, &[(1125, 36), (1456, 36), (2124, 36), (2808, 40), (3480, 40), (4524, 39),
                               (8791, 59), (8791, 59), (8791, 59), (8791, 59), (8791, 59), (9086, 59), (9086, 59)]),
    ];
    for (bw, masks) in expected {
        let got: Vec<_> = psk_levels(bw).map(|l| (l.frame_cells().unwrap(), l.pilots().unwrap())).collect();
        assert_eq!(got, masks, "{bw:?}");
    }
}

#[test]
fn single_carrier_frequencies_are_centred_and_even() {
    for bw in ALL {
        for l in psk_levels(bw) {
            let f = l.carrier_freqs_hz;
            if l.nominal_baud != 94 {
                assert!(f.is_empty(), "{bw:?} L{}: OFDM levels list no frequencies", l.level);
                continue;
            }
            assert_eq!(f.len(), l.carriers as usize, "{bw:?} L{}", l.level);
            assert_eq!(f[0] + f[f.len() - 1], 3000, "{bw:?} L{}: centred on 1500 Hz", l.level);
            let step = f[1] - f[0];
            assert!(f.windows(2).all(|w| w[1] - w[0] == step), "{bw:?} L{}: even spacing", l.level);
        }
    }
}

/// Runs of at least 256 words that are all 0 or 1: the pilot masks.
fn masks_in(words: &[u16]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < words.len() {
        let j = i + words[i..].iter().take_while(|&&w| w <= 1).count();
        if j - i >= 256 {
            out.push((j - i, words[i..j].iter().map(|&w| w as usize).sum()));
        }
        i = j.max(i + 1);
    }
    out
}

#[test]
fn real_files_hold_one_mask_per_psk_level() {
    for bw in ALL {
        let Some(dir) = std::env::var_os("VARA_DIR").map(PathBuf::from) else { return };
        let words = parse(bw, &std::fs::read(dir.join(bw.file_name())).unwrap()).unwrap().words;
        let found = masks_in(&words);
        let predicted: Vec<_> = psk_levels(bw).map(|l| (l.frame_cells().unwrap(), l.pilots().unwrap())).collect();
        assert_eq!(found[..predicted.len()], predicted[..], "{bw:?}");
        // What follows belongs to service frames outside the published levels:
        // 11 carriers x 50 symbols with 4 pilots each, and (500 Hz only) 2 carriers x 450 symbols with 18 each.
        let service = [(550, 44), (900, 36)];
        assert!(found[predicted.len()..].iter().all(|m| service.contains(m)), "{bw:?}: {:?}", &found[predicted.len()..]);
    }
}
