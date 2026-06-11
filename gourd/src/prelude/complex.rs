//! Complex number types for Go `complex64` and `complex128`.
//!
//! Provides lightweight complex number types that support the core
//! operations needed by Go transpilation: construction, arithmetic
//! (addition, subtraction, multiplication, real division), extraction
//! of real/imaginary parts, and convenience methods.

use std::ops::{Add, Sub, Mul, Div};

/// A 32-bit complex number (maps to Go `complex64`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Complex64 {
    pub real: f32,
    pub imag: f32,
}

impl Complex64 {
    /// Construct a complex number from real and imaginary parts.
    #[inline]
    pub fn new(real: f32, imag: f32) -> Self {
        Complex64 { real, imag }
    }

    /// Add two complex64 values.
    #[inline]
    pub fn add(self, other: Complex64) -> Self {
        Complex64::new(self.real + other.real, self.imag + other.imag)
    }

    /// Subtract another complex64 from this one.
    #[inline]
    pub fn sub(self, other: Complex64) -> Self {
        Complex64::new(self.real - other.real, self.imag - other.imag)
    }

    /// Multiply two complex64 values.
    #[inline]
    pub fn mul(self, other: Complex64) -> Self {
        // (a + bi)(c + di) = (ac - bd) + (ad + bc)i
        Complex64::new(
            self.real * other.real - self.imag * other.imag,
            self.real * other.imag + self.imag * other.real,
        )
    }

    /// Divide by another complex64.
    /// Uses the formula: (a+bi)/(c+di) = [(ac+bd) + (bc-ad)i] / (c²+d²)
    #[inline]
    pub fn div(self, other: Complex64) -> Self {
        let denom = other.real * other.real + other.imag * other.imag;
        if denom == 0.0 {
            panic!("complex64 division by zero");
        }
        Complex64::new(
            (self.real * other.real + self.imag * other.imag) / denom,
            (self.imag * other.real - self.real * other.imag) / denom,
        )
    }

    /// Divide by a real scalar.
    #[inline]
    pub fn div_real(self, divisor: f32) -> Self {
        Complex64::new(self.real / divisor, self.imag / divisor)
    }

    /// Negate this complex number.
    #[inline]
    pub fn neg(self) -> Self {
        Complex64::new(-self.real, -self.imag)
    }

    /// Conjugate: swap the sign of the imaginary part.
    #[inline]
    pub fn conjugate(self) -> Self {
        Complex64::new(self.real, -self.imag)
    }

    /// Absolute value (magnitude): sqrt(real² + imag²).
    #[inline]
    pub fn abs(self) -> f32 {
        (self.real * self.real + self.imag * self.imag).sqrt()
    }

    /// Squared magnitude: real² + imag² (avoids sqrt).
    #[inline]
    pub fn squared(self) -> f32 {
        self.real * self.real + self.imag * self.imag
    }

    /// Extract the real part.
    #[inline]
    pub fn real(self) -> f32 {
        self.real
    }

    /// Extract the imaginary part.
    #[inline]
    pub fn imag(self) -> f32 {
        self.imag
    }

    /// Equality comparison.
    #[inline]
    pub fn eq(self, other: Complex64) -> bool {
        (self.real - other.real).abs() < 1e-6 && (self.imag - other.imag).abs() < 1e-6
    }

    /// Check if this is the zero complex number.
    #[inline]
    pub fn is_zero(self) -> bool {
        (self.real == 0.0 || self.real.abs() < 1e-6) && (self.imag == 0.0 || self.imag.abs() < 1e-6)
    }
}

// Implement Display for debugging
impl std::fmt::Display for Complex64 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.imag >= 0.0 {
            write!(f, "{}+{}i", self.real, self.imag)
        } else {
            write!(f, "{}-{}i", self.real, self.imag.abs())
        }
    }
}

impl Add for Complex64 {
    type Output = Self;
    #[inline]
    fn add(self, other: Self) -> Self {
        self.add(other)
    }
}

impl Sub for Complex64 {
    type Output = Self;
    #[inline]
    fn sub(self, other: Self) -> Self {
        self.sub(other)
    }
}

impl Mul for Complex64 {
    type Output = Self;
    #[inline]
    fn mul(self, other: Self) -> Self {
        self.mul(other)
    }
}

impl Div for Complex64 {
    type Output = Self;
    #[inline]
    fn div(self, other: Self) -> Self {
        self.div(other)
    }
}

/// A 64-bit complex number (maps to Go `complex128`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Complex128 {
    pub real: f64,
    pub imag: f64,
}

impl Complex128 {
    /// Construct a complex number from real and imaginary parts.
    #[inline]
    pub fn new(real: f64, imag: f64) -> Self {
        Complex128 { real, imag }
    }

    /// Add two complex128 values.
    #[inline]
    pub fn add(self, other: Complex128) -> Self {
        Complex128::new(self.real + other.real, self.imag + other.imag)
    }

    /// Subtract another complex128 from this one.
    #[inline]
    pub fn sub(self, other: Complex128) -> Self {
        Complex128::new(self.real - other.real, self.imag - other.imag)
    }

    /// Multiply two complex128 values.
    #[inline]
    pub fn mul(self, other: Complex128) -> Self {
        Complex128::new(
            self.real * other.real - self.imag * other.imag,
            self.real * other.imag + self.imag * other.real,
        )
    }

    /// Divide by another complex128.
    #[inline]
    pub fn div(self, other: Complex128) -> Self {
        let denom = other.real * other.real + other.imag * other.imag;
        if denom == 0.0 {
            panic!("complex128 division by zero");
        }
        Complex128::new(
            (self.real * other.real + self.imag * other.imag) / denom,
            (self.imag * other.real - self.real * other.imag) / denom,
        )
    }

    /// Divide by a real scalar.
    #[inline]
    pub fn div_real(self, divisor: f64) -> Self {
        Complex128::new(self.real / divisor, self.imag / divisor)
    }

    /// Negate this complex number.
    #[inline]
    pub fn neg(self) -> Self {
        Complex128::new(-self.real, -self.imag)
    }

    /// Conjugate: swap the sign of the imaginary part.
    #[inline]
    pub fn conjugate(self) -> Self {
        Complex128::new(self.real, -self.imag)
    }

    /// Absolute value (magnitude): sqrt(real² + imag²).
    #[inline]
    pub fn abs(self) -> f64 {
        (self.real * self.real + self.imag * self.imag).sqrt()
    }

    /// Squared magnitude: real² + imag² (avoids sqrt).
    #[inline]
    pub fn squared(self) -> f64 {
        self.real * self.real + self.imag * self.imag
    }

    /// Extract the real part.
    #[inline]
    pub fn real(self) -> f64 {
        self.real
    }

    /// Extract the imaginary part.
    #[inline]
    pub fn imag(self) -> f64 {
        self.imag
    }

    /// Equality comparison.
    #[inline]
    pub fn eq(self, other: Complex128) -> bool {
        (self.real - other.real).abs() < 1e-12 && (self.imag - other.imag).abs() < 1e-12
    }

    /// Check if this is the zero complex number.
    #[inline]
    pub fn is_zero(self) -> bool {
        (self.real == 0.0 || self.real.abs() < 1e-12) && (self.imag == 0.0 || self.imag.abs() < 1e-12)
    }
}

// Implement Display for debugging
impl std::fmt::Display for Complex128 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.imag >= 0.0 {
            write!(f, "{}+{}i", self.real, self.imag)
        } else {
            write!(f, "{}-{}i", self.real, self.imag.abs())
        }
    }
}

impl Add for Complex128 {
    type Output = Self;
    #[inline]
    fn add(self, other: Self) -> Self {
        self.add(other)
    }
}

impl Sub for Complex128 {
    type Output = Self;
    #[inline]
    fn sub(self, other: Self) -> Self {
        self.sub(other)
    }
}

impl Mul for Complex128 {
    type Output = Self;
    #[inline]
    fn mul(self, other: Self) -> Self {
        self.mul(other)
    }
}

impl Div for Complex128 {
    type Output = Self;
    #[inline]
    fn div(self, other: Self) -> Self {
        self.div(other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_complex64_new() {
        let c = Complex64::new(3.0, 4.0);
        assert!((c.real - 3.0).abs() < 1e-6);
        assert!((c.imag - 4.0).abs() < 1e-6);
    }

    #[test]
    fn test_complex64_add() {
        let a = Complex64::new(1.0, 2.0);
        let b = Complex64::new(3.0, 4.0);
        let c = a.add(b);
        assert!((c.real - 4.0).abs() < 1e-6);
        assert!((c.imag - 6.0).abs() < 1e-6);
    }

    #[test]
    fn test_complex64_mul() {
        // (1+2i)(3+4i) = (3-8) + (4+6)i = -5+10i
        let a = Complex64::new(1.0, 2.0);
        let b = Complex64::new(3.0, 4.0);
        let c = a.mul(b);
        assert!((c.real - (-5.0)).abs() < 1e-6);
        assert!((c.imag - 10.0).abs() < 1e-6);
    }

    #[test]
    fn test_complex64_div_real() {
        let a = Complex64::new(6.0, 8.0);
        let c = a.div_real(2.0);
        assert!((c.real - 3.0).abs() < 1e-6);
        assert!((c.imag - 4.0).abs() < 1e-6);
    }

    #[test]
    fn test_complex64_abs() {
        let a = Complex64::new(3.0, 4.0);
        assert!((a.abs() - 5.0).abs() < 1e-6);
    }

    #[test]
    fn test_complex64_conjugate() {
        let a = Complex64::new(3.0, 4.0);
        let c = a.conjugate();
        assert!((c.real - 3.0).abs() < 1e-6);
        assert!((c.imag - (-4.0)).abs() < 1e-6);
    }

    #[test]
    fn test_complex128_mul() {
        // (1+2i)(3+4i) = -5+10i
        let a = Complex128::new(1.0, 2.0);
        let b = Complex128::new(3.0, 4.0);
        let c = a.mul(b);
        assert!((c.real - (-5.0)).abs() < 1e-12);
        assert!((c.imag - 10.0).abs() < 1e-12);
    }

    #[test]
    fn test_complex128_div() {
        // (3+4i)/(1+i) = (3+4i)(1-i) / 2 = (7+i)/2 = 3.5 + 0.5i
        let a = Complex128::new(3.0, 4.0);
        let b = Complex128::new(1.0, 1.0);
        let c = a.div(b);
        assert!((c.real - 3.5).abs() < 1e-12);
        assert!((c.imag - 0.5).abs() < 1e-12);
    }

    #[test]
    fn test_complex64_div() {
        // (3+4i)/(1+i) = (7+i)/2 = 3.5 + 0.5i
        let a = Complex64::new(3.0, 4.0);
        let b = Complex64::new(1.0, 1.0);
        let c = a.div(b);
        assert!((c.real - 3.5).abs() < 1e-6);
        assert!((c.imag - 0.5).abs() < 1e-6);
    }
}
