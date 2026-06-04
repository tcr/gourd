//! Go's `math/rand` package helpers.
//!
//! Provides simple pseudo-random number generation.

/// Go's `rand` package — pseudo-random number generation.
///
/// Mirrors Go's `math/rand` for simple random operations.
#[derive(Debug)]
pub struct GoRand {
    seed: u32,
}

impl GoRand {
    /// Creates a new random number generator with a fixed seed.
    pub fn new(seed: i64) -> Self {
        GoRand {
            seed: if seed < 0 { (-seed) as u32 } else { seed as u32 },
        }
    }

    /// Returns a random integer in [0, max).
    pub fn intn(&mut self, max: i64) -> i64 {
        self.next_u32();
        let range_size = if max <= 0 { 1 } else { max as u32 };
        let next = self.next_u32();
        (next % range_size) as i64
    }

    /// Returns a random float64 in [0.0, 1.0).
    pub fn float64(&mut self) -> f64 {
        self.next_u32();
        (self.next_u32() as f64) / (u32::MAX as f64)
    }

    /// Returns a random boolean.
    pub fn bool(&mut self) -> bool {
        self.next_u32() % 2 == 0
    }

    fn next_u32(&mut self) -> u32 {
        // Simple LCG-based generator
        self.seed = self.seed.wrapping_mul(1103515245).wrapping_add(12345);
        self.seed
    }
}
