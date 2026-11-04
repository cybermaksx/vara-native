//! Constellations and cell modulation. Points are VARA's own f32 table; the hard decisions below are
//! VARA's receive-side formulas (a separate function), so each test checks two sources against each other.

use std::f64::consts::PI;
use vara_native::fft::Complex;
use vara_native::mapping::{constellation, index_msb_first, MappingError};

fn hard_bits(bits: u8, s: Complex) -> Vec<u8> {
    // VARA's demapper: LLR > 0 means bit 1, most significant bit first.
    let llr: Vec<f64> = match bits {
        1 => vec![-s.re],
        2 => vec![-0.707 * (s.re + s.im), -0.707 * (s.re - s.im)],
        3 => {
            let z = s * Complex::from_angle(PI / 8.0);
            vec![-z.im, -z.re, 0.707 * (z.im.abs() - z.re.abs())]
        }
        _ => unreachable!(),
    };
    llr.iter().map(|&l| (l > 0.0) as u8).collect()
}

fn bits_of(index: usize, bits: u8) -> Vec<u8> {
    (0..bits).rev().map(|j| ((index >> j) & 1) as u8).collect()
}

#[test]
fn sizes() {
    for b in 1..=5u8 {
        assert_eq!(constellation(b).unwrap().len(), 1 << b);
    }
    assert_eq!(constellation(0).unwrap_err(), MappingError::UnsupportedBits(0));
    assert_eq!(constellation(6).unwrap_err(), MappingError::UnsupportedBits(6));
}

#[test]
fn psk_points_are_unit_circle_at_table_angles() {
    let deg = |b, i| {
        let p: Complex = constellation(b).unwrap()[i];
        assert!((p.norm() - 1.0).abs() < 1e-6);
        (p.im.atan2(p.re).to_degrees().round() as i32 + 360) % 360
    };
    assert_eq!([deg(1, 0), deg(1, 1)], [0, 180]);
    assert_eq!((0..4).map(|i| deg(2, i)).collect::<Vec<_>>(), [0, 90, 270, 180], "QPSK sits on the axes");
    assert_eq!((0..8).map(|i| deg(3, i)).collect::<Vec<_>>(), [0, 45, 135, 90, 315, 270, 180, 225]);
}

// Table builder and demapper are different functions in VARA; they agree point by point.
#[test]
fn psk_table_agrees_with_vara_demapper() {
    for b in 1..=3u8 {
        for (i, &p) in constellation(b).unwrap().iter().enumerate() {
            assert_eq!(hard_bits(b, p), bits_of(i, b), "{b} bits, point {i}");
        }
    }
}

fn hamming(a: usize, b: usize) -> u32 {
    (a ^ b).count_ones()
}

#[test]
fn psk_neighbours_differ_in_one_bit() {
    for b in 2..=3u8 {
        let pts = constellation(b).unwrap();
        let mut order: Vec<usize> = (0..pts.len()).collect();
        order.sort_by(|&x, &y| pts[x].im.atan2(pts[x].re).total_cmp(&pts[y].im.atan2(pts[y].re)));
        for w in 0..order.len() {
            let (a, c) = (order[w], order[(w + 1) % order.len()]);
            assert_eq!(hamming(a, c), 1, "{b} bits: {a} next to {c}");
        }
    }
}

#[test]
fn qam16_is_a_gray_square() {
    let pts = constellation(4).unwrap();
    let step = 1.1 / 18f64.sqrt();
    let level = |two_bits: usize| match two_bits {
        0b00 => -3.0,
        0b01 => -1.0,
        0b11 => 1.0,
        _ => 3.0,
    };
    for (i, p) in pts.iter().enumerate() {
        assert!((p.re - level(i >> 2) * step).abs() < 1e-6, "point {i} I");
        assert!((p.im - level(i & 3) * step).abs() < 1e-6, "point {i} Q");
    }
}

#[test]
fn qam32_is_a_cross() {
    let pts = constellation(5).unwrap();
    let step = 0.3 / 2f64.sqrt();
    let mut grid: Vec<(i32, i32)> = pts.iter().map(|p| ((p.re / step).round() as i32, (p.im / step).round() as i32)).collect();
    for (p, &(x, y)) in pts.iter().zip(&grid) {
        assert!((p.re - x as f64 * step).abs() < 1e-6 && (p.im - y as f64 * step).abs() < 1e-6);
    }
    grid.sort();
    let mut cross: Vec<(i32, i32)> = Vec::new();
    for x in [-5i32, -3, -1, 1, 3, 5] {
        for y in [-5i32, -3, -1, 1, 3, 5] {
            if x.abs() + y.abs() < 10 {
                cross.push((x, y));
            }
        }
    }
    assert_eq!(grid, cross);
}

#[test]
fn first_bit_is_most_significant() {
    assert_eq!(index_msb_first(&[1, 0, 0]), Ok(4));
    assert_eq!(index_msb_first(&[0, 1, 1]), Ok(3));
    assert_eq!(index_msb_first(&[1, 1, 0, 1, 0]), Ok(26));
    assert_eq!(index_msb_first(&[1, 2]), Err(MappingError::NotABit));
}
