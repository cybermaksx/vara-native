//! Complex numbers and an in-place radix-2 FFT, without external crates.

use std::f64::consts::PI;
use std::ops::{Add, Mul, Sub};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Complex {
    pub re: f64,
    pub im: f64,
}

impl Complex {
    pub const fn new(re: f64, im: f64) -> Complex {
        Complex { re, im }
    }

    pub fn conj(self) -> Complex {
        Complex::new(self.re, -self.im)
    }

    pub fn norm(self) -> f64 {
        self.re.hypot(self.im)
    }

    pub fn scale(self, k: f64) -> Complex {
        Complex::new(self.re * k, self.im * k)
    }

    /// e^(i a)
    pub fn from_angle(a: f64) -> Complex {
        Complex::new(a.cos(), a.sin())
    }
}

impl Add for Complex {
    type Output = Complex;
    fn add(self, o: Complex) -> Complex {
        Complex::new(self.re + o.re, self.im + o.im)
    }
}

impl Sub for Complex {
    type Output = Complex;
    fn sub(self, o: Complex) -> Complex {
        Complex::new(self.re - o.re, self.im - o.im)
    }
}

impl Mul for Complex {
    type Output = Complex;
    fn mul(self, o: Complex) -> Complex {
        Complex::new(self.re * o.re - self.im * o.im, self.re * o.im + self.im * o.re)
    }
}

/// Forward transform: X[k] = sum x[t] e^(-2 pi i k t / n). Length must be a power of two.
pub fn fft(x: &mut [Complex]) {
    transform(x, -1.0);
}

/// Inverse transform including the 1/n factor.
pub fn ifft(x: &mut [Complex]) {
    transform(x, 1.0);
    let k = 1.0 / x.len() as f64;
    x.iter_mut().for_each(|v| *v = v.scale(k));
}

/// Iterative Cooley-Tukey; `sign` is the sign of the exponent.
fn transform(x: &mut [Complex], sign: f64) {
    let n = x.len();
    assert!(n.is_power_of_two(), "FFT length {n} is not a power of two");
    if n < 2 {
        return;
    }
    let bits = n.trailing_zeros();
    for i in 0..n {
        let j = i.reverse_bits() >> (usize::BITS - bits);
        if i < j {
            x.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let half = len / 2;
        // Twiddles from exact angles rather than repeated multiplication, so errors do not accumulate.
        let twiddles: Vec<Complex> = (0..half).map(|k| Complex::from_angle(sign * 2.0 * PI * k as f64 / len as f64)).collect();
        for start in (0..n).step_by(len) {
            for (k, &w) in twiddles.iter().enumerate() {
                let a = x[start + k];
                let b = x[start + k + half] * w;
                x[start + k] = a + b;
                x[start + k + half] = a - b;
            }
        }
        len *= 2;
    }
}
