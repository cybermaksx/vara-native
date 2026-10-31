use vara_native::fft::{fft, ifft, Complex};

/// Deterministic test signal (xorshift), no external crates.
fn noise(n: usize, seed: u64) -> Vec<Complex> {
    let mut s = seed;
    let mut next = || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        (s >> 11) as f64 / (1u64 << 53) as f64 - 0.5
    };
    (0..n).map(|_| Complex::new(next(), next())).collect()
}

fn naive_dft(x: &[Complex]) -> Vec<Complex> {
    let n = x.len();
    (0..n)
        .map(|k| {
            x.iter().enumerate().fold(Complex::new(0.0, 0.0), |acc, (t, &v)| {
                let a = -2.0 * std::f64::consts::PI * (k * t % n) as f64 / n as f64;
                acc + v * Complex::new(a.cos(), a.sin())
            })
        })
        .collect()
}

fn max_err(a: &[Complex], b: &[Complex]) -> f64 {
    a.iter().zip(b).map(|(x, y)| (*x - *y).norm()).fold(0.0, f64::max)
}

#[test]
fn complex_arithmetic() {
    let a = Complex::new(1.0, 2.0);
    let b = Complex::new(3.0, -1.0);
    assert_eq!(a + b, Complex::new(4.0, 1.0));
    assert_eq!(a - b, Complex::new(-2.0, 3.0));
    assert_eq!(a * b, Complex::new(5.0, 5.0));
    assert_eq!(a.conj(), Complex::new(1.0, -2.0));
    assert_eq!(Complex::new(3.0, 4.0).norm(), 5.0);
    assert_eq!(a.scale(2.0), Complex::new(2.0, 4.0));
}

#[test]
fn impulse_gives_flat_spectrum() {
    let mut x = vec![Complex::new(0.0, 0.0); 64];
    x[0] = Complex::new(1.0, 0.0);
    fft(&mut x);
    assert!(x.iter().all(|v| (*v - Complex::new(1.0, 0.0)).norm() < 1e-12));
}

#[test]
fn constant_gives_dc_only() {
    let mut x = vec![Complex::new(1.0, 0.0); 64];
    fft(&mut x);
    assert!((x[0] - Complex::new(64.0, 0.0)).norm() < 1e-12);
    assert!(x[1..].iter().all(|v| v.norm() < 1e-12));
}

#[test]
fn matches_naive_dft() {
    for n in [1, 2, 8, 64, 1024] {
        let x = noise(n, n as u64 + 1);
        let mut y = x.clone();
        fft(&mut y);
        assert!(max_err(&y, &naive_dft(&x)) < 1e-9 * n as f64, "n = {n}");
    }
}

#[test]
fn inverse_restores_input() {
    for n in [8, 64, 1024] {
        let x = noise(n, 7);
        let mut y = x.clone();
        fft(&mut y);
        ifft(&mut y);
        assert!(max_err(&x, &y) < 1e-12, "n = {n}");
    }
}

#[test]
fn parseval() {
    let x = noise(1024, 3);
    let mut y = x.clone();
    fft(&mut y);
    let e_time: f64 = x.iter().map(|v| v.norm().powi(2)).sum();
    let e_freq: f64 = y.iter().map(|v| v.norm().powi(2)).sum::<f64>() / 1024.0;
    assert!((e_time - e_freq).abs() < 1e-9 * e_time);
}

#[test]
#[should_panic(expected = "power of two")]
fn rejects_lengths_that_are_not_powers_of_two() {
    fft(&mut vec![Complex::new(0.0, 0.0); 72]);
}
