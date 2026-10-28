//! Deterministic pseudo-random number generator and sampling routines.
//!
//! Based on ChaCha8 with 53-bit IEEE-754 mantissa sampling and unbiased rejection sampling.
//! Outputs are locked across compiler versions to guarantee parallel-form retest fairness.

pub use rand_chacha::ChaCha8Rng;
use rand_core::{Rng, SeedableRng};

/// Create a ChaCha8Rng initialized with the given 64-bit seed.
pub fn create_seeded_rng(seed: u64) -> ChaCha8Rng {
    ChaCha8Rng::seed_from_u64(seed)
}

/// Generates a floating-point number in `[0.0, 1.0)` using 53 bits of precision.
///
/// Implements standard IEEE-754 float generation: `(next_u64() >> 11) * 2^(-53)`.
/// This guarantees full 53-bit resolution without ever returning 1.0.
#[inline]
pub fn sample_unit_f64(rng: &mut ChaCha8Rng) -> f64 {
    (rng.next_u64() >> 11) as f64 * (1.0 / ((1u64 << 53) as f64))
}

/// Generates a floating-point number in the half-open interval `[min, max)`.
#[inline]
pub fn sample_range_f64(rng: &mut ChaCha8Rng, min: f64, max: f64) -> f64 {
    debug_assert!(max >= min, "max ({max}) must be >= min ({min})");
    min + sample_unit_f64(rng) * (max - min)
}

/// Generates an integer in the half-open interval `[min, max)` (exclusive upper bound)
/// using unbiased rejection sampling to eliminate modulo bias.
pub fn sample_range_u32(rng: &mut ChaCha8Rng, min: u32, max: u32) -> u32 {
    debug_assert!(max >= min, "max ({max}) must be >= min ({min})");
    let range = max - min;
    if range <= 1 {
        return min;
    }
    let range_u64 = range as u64;
    let zone = u64::MAX - (u64::MAX % range_u64);
    loop {
        let val = rng.next_u64();
        if val < zone {
            return min + (val % range_u64) as u32;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn test_golden_seed_deadbeef_locked() {
        let mut rng = create_seeded_rng(0xDEADBEEF);

        let f0 = sample_unit_f64(&mut rng);
        let f1 = sample_range_f64(&mut rng, 10.0, 50.0);
        let f2 = sample_range_f64(&mut rng, -180.0, 180.0);
        let u0 = sample_range_u32(&mut rng, 0, 8);
        let u1 = sample_range_u32(&mut rng, 100, 200);

        // Lock exact deterministic values for seed 0xDEADBEEF with rand_chacha = "=0.10.0"
        assert!((f0 - 0.9961118994432799).abs() < 1e-15, "f0 was {f0}");
        assert!((f1 - 33.19052879865036).abs() < 1e-15, "f1 was {f1}");
        assert!((f2 - -177.89091746354038).abs() < 1e-15, "f2 was {f2}");
        assert_eq!(u0, 1, "u0 was {u0}");
        assert_eq!(u1, 134, "u1 was {u1}");
    }

    proptest! {
        #[test]
        fn test_sample_unit_f64_bounds(seed in any::<u64>()) {
            let mut rng = create_seeded_rng(seed);
            for _ in 0..100 {
                let v = sample_unit_f64(&mut rng);
                prop_assert!(v >= 0.0);
                prop_assert!(v < 1.0);
            }
        }

        #[test]
        fn test_sample_range_u32_bounds(seed in any::<u64>(), min in 0u32..5000u32, span in 1u32..5000u32) {
            let mut rng = create_seeded_rng(seed);
            let max = min + span;
            for _ in 0..50 {
                let val = sample_range_u32(&mut rng, min, max);
                prop_assert!(val >= min);
                prop_assert!(val < max);
            }
        }
    }
}
