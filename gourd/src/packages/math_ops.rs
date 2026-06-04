//! Go's `math` package helpers.
//!
//! Provides absolute, sqrt, floor, ceil, round, min, max, exp, log, pow, sign,
//! and the Pi and E constants.

/// Returns the absolute value of an integer (Go `math.Abs`).
pub fn abs_i32(x: i32) -> i32 {
    x.abs()
}

/// Returns the absolute value of a 64-bit integer (Go `math.Abs` for int64).
pub fn abs_i64(x: i64) -> i64 {
    x.abs()
}

/// Returns the absolute value of a float (Go `math.Abs` for float64).
pub fn abs_f64(x: f64) -> f64 {
    x.abs()
}

/// Returns the square root of a float (Go `math.Sqrt`).
pub fn sqrt(x: f64) -> f64 {
    x.sqrt()
}

/// Returns the floor of a float (Go `math.Floor`).
pub fn floor(x: f64) -> f64 {
    x.floor()
}

/// Returns the ceiling of a float (Go `math.Ceil`).
pub fn ceil(x: f64) -> f64 {
    x.ceil()
}

/// Rounds a float to the nearest integer (Go `math.Round`).
pub fn round(x: f64) -> f64 {
    x.round()
}

/// Returns the minimum of two floats (Go `math.Min`).
pub fn min_f64(x: f64, y: f64) -> f64 {
    x.min(y)
}

/// Returns the maximum of two floats (Go `math.Max`).
pub fn max_f64(x: f64, y: f64) -> f64 {
    x.max(y)
}

/// Returns pi (Go `math.Pi`).
pub const PI: f64 = std::f64::consts::PI;

/// Returns e (Go `math.E`).
pub const E: f64 = std::f64::consts::E;

/// Returns the exponential of x (Go `math.Exp`).
pub fn exp(x: f64) -> f64 {
    x.exp()
}

/// Returns the natural logarithm of x (Go `math.Log`).
pub fn log(x: f64) -> f64 {
    x.ln()
}

/// Returns the base-10 logarithm of x (Go `math.Log10`).
pub fn log10(x: f64) -> f64 {
    x.log10()
}

/// Returns x raised to the power y (Go `math.Pow`).
pub fn pow(x: f64, y: f64) -> f64 {
    x.powf(y)
}

/// Returns the sign of x: -1, 0, or 1 (Go `math.Signbit` + sign logic).
pub fn sign(x: f64) -> f64 {
    if x > 0.0 { 1.0 } else if x < 0.0 { -1.0 } else { 0.0 }
}
