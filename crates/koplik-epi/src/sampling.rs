//! Portable, bounded binomial sampling; no platform distribution implementations.

use rand_core::RngCore;

/// Uniform on the 2^52 midpoints of (0,1). Uses fixed-width draws and exact binary
/// arithmetic; neither endpoint can occur. All samplers use this same mapping.
pub(crate) fn uniform(rng: &mut impl RngCore) -> f64 {
    ((rng.next_u64() >> 12) as f64 + 0.5) * (1.0 / 4_503_599_627_370_496.0)
}

/// Binomial via geometric waiting times in n Bernoulli trials. A waiting time is
/// floor(ln(U) / ln(1-p)); stop when the next success is outside the n trials.
/// Complement p > 1/2. This is an exact-distribution construction (up to finite
/// precision), not a Poisson/normal approximation, and cannot overshoot n.
/// Cost O(1 + n*min(p,1-p)); chosen for sparse county-level transitions.
/// No Poisson sampler is needed: all SEIR transitions have finite source pools.
pub(crate) fn binomial(rng: &mut impl RngCore, n: u64, p: f64) -> u64 {
    debug_assert!(p.is_finite() && (0.0..=1.0).contains(&p));
    if n == 0 || p == 0.0 {
        return 0;
    }
    if p == 1.0 {
        return n;
    }
    let complement = p > 0.5;
    let probability = if complement { 1.0 - p } else { p };
    let log_failure = libm::log1p(-probability);
    let mut remaining = n;
    let mut successes = 0;
    while remaining > 0 {
        let failures = libm::floor(libm::log(uniform(rng)) / log_failure);
        if failures >= remaining as f64 {
            break;
        }
        remaining -= failures as u64 + 1;
        successes += 1;
    }
    if complement { n - successes } else { successes }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand_chacha::ChaCha8Rng;
    use rand_core::SeedableRng;

    #[test]
    fn boundaries_and_binomial_moments() {
        let mut rng = ChaCha8Rng::from_seed([19; 32]);
        assert_eq!(binomial(&mut rng, 0, 0.5), 0);
        assert_eq!(binomial(&mut rng, 91, 0.0), 0);
        assert_eq!(binomial(&mut rng, 91, 1.0), 91);
        for p in [0.001, 0.1, 0.5, 0.9, 0.999] {
            let n = 100_u64;
            let count = 20_000_u32;
            let mut sum = 0.0;
            let mut square = 0.0;
            for _ in 0..count {
                let x = binomial(&mut rng, n, p);
                assert!(x <= n);
                sum += x as f64;
                square += (x * x) as f64;
            }
            let mean = sum / f64::from(count);
            let variance = square / f64::from(count) - mean * mean;
            let expected_variance = n as f64 * p * (1.0 - p);
            assert!(
                (mean - n as f64 * p).abs()
                    < 6.0 * libm::sqrt(expected_variance / f64::from(count))
            );
            assert!((variance - expected_variance).abs() < 0.08 * expected_variance + 0.005);
        }
    }
}
