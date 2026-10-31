use std::f64::consts::PI;
use std::path::PathBuf;
use vara_native::dat::{parse, Bandwidth, Record, RecordBody};
use vara_native::fft::{fft, Complex};
use vara_native::modes::{bin_frequency_hz, levels, Modulation, SpeedLevel, BIN_HZ};
use vara_native::ofdm::{FrameMap, Ofdm, OfdmError, AUDIO_HZ, CP_LEN, FFT_LEN, SYMBOL_LEN};

const ALL: [Bandwidth; 3] = [Bandwidth::Hz500, Bandwidth::Hz2300, Bandwidth::Hz2750];

fn ofdm_levels(bw: Bandwidth) -> impl Iterator<Item = &'static SpeedLevel> {
    levels(bw).iter().filter(|l| l.ofdm_bins().is_some())
}

fn level(bw: Bandwidth, n: u8) -> &'static SpeedLevel {
    &levels(bw)[n as usize - 1]
}

/// Deterministic constellation points for a whole frame (xorshift, no external crates).
fn random_grid(o: &Ofdm, points: u32, seed: u64) -> Vec<Vec<Complex>> {
    let mut s = seed;
    let mut next = || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        (s % points as u64) as f64
    };
    (0..o.frame_symbols())
        .map(|_| (0..o.carriers()).map(|_| Complex::from_angle(2.0 * PI * next() / points as f64)).collect())
        .collect()
}

fn zero_grid(o: &Ofdm) -> Vec<Vec<Complex>> {
    vec![vec![Complex::new(0.0, 0.0); o.carriers()]; o.frame_symbols()]
}

// 1024 + 128 samples at 48 kHz is the 64 + 8 at 3 kHz that reproduces the published rates.
#[test]
fn symbol_geometry() {
    assert_eq!((AUDIO_HZ, FFT_LEN, CP_LEN, SYMBOL_LEN), (48_000, 1024, 128, 1152));
    assert_eq!(AUDIO_HZ as f64 / FFT_LEN as f64, BIN_HZ);
    for bw in ALL {
        for l in ofdm_levels(bw) {
            assert_eq!(SYMBOL_LEN as f64 / AUDIO_HZ as f64, 1.0 / l.symbol_rate(), "{bw:?} L{}", l.level);
        }
    }
}

#[test]
fn only_ofdm_levels_are_accepted() {
    assert_eq!(Ofdm::for_level(level(Bandwidth::Hz2300, 1)).unwrap_err(), OfdmError::NotOfdm { level: 1 });
    assert_eq!(Ofdm::for_level(level(Bandwidth::Hz2300, 9)).unwrap_err(), OfdmError::NotOfdm { level: 9 });
    assert!(Ofdm::for_level(level(Bandwidth::Hz2300, 10)).is_ok());
}

#[test]
fn frame_length_in_samples() {
    for bw in ALL {
        for l in ofdm_levels(bw) {
            let o = Ofdm::for_level(l).unwrap();
            assert_eq!((o.carriers(), o.frame_symbols()), (l.carriers as usize, l.frame_symbols.unwrap() as usize));
            assert_eq!(o.modulate(&zero_grid(&o)).unwrap().len(), o.frame_symbols() * SYMBOL_LEN);
        }
    }
}

// A unit cell on one carrier is a cosine at that carrier's frequency, continuous through the cyclic prefix.
#[test]
fn single_carrier_is_a_cosine_at_its_bin_frequency() {
    let l = level(Bandwidth::Hz2300, 10);
    let o = Ofdm::for_level(l).unwrap();
    for (carrier, bin) in [(0, 8u8), (24, 32), (48, 56)] {
        let mut g = zero_grid(&o);
        g[0][carrier] = Complex::new(1.0, 0.0);
        let x = o.modulate(&g).unwrap();
        let f = bin_frequency_hz(bin);
        for (n, &v) in x[..SYMBOL_LEN].iter().enumerate() {
            let want = (2.0 * PI * f * (n as f64 - CP_LEN as f64) / AUDIO_HZ as f64).cos();
            assert!((v - want).abs() < 1e-12, "carrier {carrier} sample {n}");
        }
        assert!(x[SYMBOL_LEN..].iter().all(|v| v.abs() < 1e-12), "other symbols silent");
    }
}

#[test]
fn cyclic_prefix_repeats_the_symbol_tail() {
    let o = Ofdm::for_level(level(Bandwidth::Hz2750, 11)).unwrap();
    let x = o.modulate(&random_grid(&o, 4, 1)).unwrap();
    for sym in x.chunks_exact(SYMBOL_LEN) {
        assert_eq!(sym[..CP_LEN], sym[SYMBOL_LEN - CP_LEN..]);
    }
}

#[test]
fn energy_stays_on_the_level_bins() {
    for bw in ALL {
        let l = ofdm_levels(bw).next().unwrap();
        let o = Ofdm::for_level(l).unwrap();
        let bins = l.ofdm_bins().unwrap();
        let x = o.modulate(&random_grid(&o, 4, 2)).unwrap();
        let mut spec: Vec<Complex> = x[CP_LEN..SYMBOL_LEN].iter().map(|&v| Complex::new(v, 0.0)).collect();
        fft(&mut spec);
        let total: f64 = spec.iter().map(|v| v.norm().powi(2)).sum();
        let outside: f64 = spec
            .iter()
            .enumerate()
            .filter(|(k, _)| {
                // A real signal mirrors bin k at N - k: both are the same audio frequency.
                let f = (*k).min(FFT_LEN - k);
                !(f <= u8::MAX as usize && bins.contains(&(f as u8)))
            })
            .map(|(_, v)| v.norm().powi(2))
            .sum();
        assert!(outside < 1e-20 * total, "{bw:?}: {outside:e} of {total:e}");
    }
}

#[test]
fn demodulation_inverts_modulation() {
    for (bw, n, points) in [(Bandwidth::Hz2300, 13, 8), (Bandwidth::Hz2750, 17, 32), (Bandwidth::Hz500, 7, 4)] {
        let o = Ofdm::for_level(level(bw, n)).unwrap();
        let g = random_grid(&o, points, n as u64);
        let back = o.demodulate(&o.modulate(&g).unwrap()).unwrap();
        for (s, (a, b)) in g.iter().zip(&back).enumerate() {
            for (c, (x, y)) in a.iter().zip(b).enumerate() {
                assert!((*x - *y).norm() < 1e-9, "{bw:?} L{n} symbol {s} carrier {c}");
            }
        }
    }
}

#[test]
fn rejects_wrong_shapes() {
    let o = Ofdm::for_level(level(Bandwidth::Hz2300, 10)).unwrap();
    let mut g = zero_grid(&o);
    g.pop();
    assert_eq!(o.modulate(&g).unwrap_err(), OfdmError::GridShape { symbols: 149, carriers: 49 });
    let mut g = zero_grid(&o);
    g[5].pop();
    assert_eq!(o.modulate(&g).unwrap_err(), OfdmError::GridShape { symbols: 149, carriers: 49 });
    assert_eq!(o.demodulate(&[0.0; 10]).unwrap_err(), OfdmError::SampleCount { expected: 149 * SYMBOL_LEN, got: 10 });
}

fn mask_record(carriers: usize, symbols: usize, pilots: &[(usize, usize)]) -> Record {
    let mut mask = vec![vec![false; symbols]; carriers];
    for &(c, s) in pilots {
        mask[c][s] = true;
    }
    Record {
        channel_interleaver: vec![],
        info_interleaver: vec![],
        body: RecordBody::Psk { pilot_symbols: vec![vec![]; carriers], mask },
    }
}

// The .dat mask is stored carrier by carrier; the frame map is indexed [symbol][carrier].
#[test]
fn frame_map_transposes_the_mask() {
    let l = level(Bandwidth::Hz500, 7);
    let r = mask_record(11, 149, &[(0, 0), (3, 148), (10, 7)]);
    let m = FrameMap::from_record(l, &r).unwrap();
    assert_eq!(m.pilot_count(), 3);
    assert!(m.is_pilot(0, 0) && m.is_pilot(148, 3) && m.is_pilot(7, 10));
    assert!(!m.is_pilot(1, 0) && !m.is_pilot(148, 2));
}

#[test]
fn frame_map_rejects_mismatched_records() {
    let l = level(Bandwidth::Hz500, 7);
    assert_eq!(FrameMap::from_record(l, &mask_record(10, 149, &[])).unwrap_err(), OfdmError::GridShape { symbols: 149, carriers: 11 });
    let fsk = Record { channel_interleaver: vec![], info_interleaver: vec![], body: RecordBody::Fsk { map: vec![], pilots: vec![], pairs: vec![] } };
    assert_eq!(FrameMap::from_record(l, &fsk).unwrap_err(), OfdmError::GridShape { symbols: 149, carriers: 11 });
}

#[test]
fn real_frame_maps_hold_the_level_pilots() {
    for bw in ALL {
        let Some(dir) = std::env::var_os("VARA_DIR").map(PathBuf::from) else { return };
        let records = parse(bw, &std::fs::read(dir.join(bw.file_name())).unwrap()).unwrap().records().unwrap();
        for (l, r) in levels(bw).iter().zip(&records).filter(|(l, _)| l.ofdm_bins().is_some()) {
            let m = FrameMap::from_record(l, r).unwrap();
            assert_eq!(Some(m.pilot_count()), l.pilots(), "{bw:?} L{}", l.level);
            assert_ne!(l.modulation, Modulation::Fsk);
        }
    }
}
