//! Complex numbers and an in-place radix-2 FFT, without external crates.

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
        todo!()
    }

    pub fn norm(self) -> f64 {
        todo!()
    }

    pub fn scale(self, k: f64) -> Complex {
        let _ = k;
        todo!()
    }
}

impl Add for Complex {
    type Output = Complex;
    fn add(self, o: Complex) -> Complex {
        let _ = o;
        todo!()
    }
}

impl Sub for Complex {
    type Output = Complex;
    fn sub(self, o: Complex) -> Complex {
        let _ = o;
        todo!()
    }
}

impl Mul for Complex {
    type Output = Complex;
    fn mul(self, o: Complex) -> Complex {
        let _ = o;
        todo!()
    }
}

/// Forward transform: X[k] = sum x[t] e^(-2 pi i k t / n). Length must be a power of two.
pub fn fft(x: &mut [Complex]) {
    let _ = x;
    todo!()
}

/// Inverse transform including the 1/n factor.
pub fn ifft(x: &mut [Complex]) {
    let _ = x;
    todo!()
}
