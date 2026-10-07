//! Portable, bounded binomial sampling; no platform distribution implementations.
//!
//! Every transcendental call (`log`, `log1p`, `sqrt`, `floor`) goes through `libm`
//! so the seeded path is bit-identical natively and on `wasm32` (spec E4). All
//! draws are `u64`/`i64`/`f64`; no `usize` touches a random value. A Poisson
//! sampler is deliberately absent: every SEIR transition here has a finite source
//! pool and is drawn binomially, which cannot overshoot.

use rand_core::RngCore;

/// Uniform on the 2^52 midpoints of (0,1). Uses fixed-width draws and exact binary
/// arithmetic; neither endpoint can occur. All samplers use this same mapping.
pub(crate) fn uniform(rng: &mut impl RngCore) -> f64 {
    ((rng.next_u64() >> 12) as f64 + 0.5) * (1.0 / 4_503_599_627_370_496.0)
}

/// Below this value of n*min(p,1-p) the geometric waiting-time method is used;
/// at or above it BTPE is used. The same threshold as `rand_distr` and close to
/// numpy's 30: both methods are exact, so it only trades setup cost against the
/// O(n*p) loop. The threshold is part of the seeded path: changing it changes
/// trajectories, so it is fixed here and never configurable at run time.
const BTPE_THRESHOLD: f64 = 10.0;

/// Exact-distribution binomial draw in 0..=n (up to finite precision); never
/// overshoots n. Complements p > 1/2, then dispatches on n*min(p,1-p).
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
    let successes = if (n as f64) * probability < BTPE_THRESHOLD {
        binomial_geometric(rng, n, probability)
    } else {
        binomial_btpe(rng, n, probability)
    };
    if complement { n - successes } else { successes }
}

/// Binomial via geometric waiting times in n Bernoulli trials (p <= 1/2). A waiting
/// time is floor(ln(U) / ln(1-p)); stop when the next success is outside the n
/// trials. Cost O(1 + n*p); chosen for the sparse early-outbreak transitions.
fn binomial_geometric(rng: &mut impl RngCore, n: u64, p: f64) -> u64 {
    let log_failure = libm::log1p(-p);
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
    successes
}

/// BTPE (Binomial, Triangle, Parallelogram, Exponential) for n*p >= 10, p <= 1/2:
/// Kachitvichyanukul & Schmeiser (1988), "Binomial random variate generation",
/// Communications of the ACM 31(2):216-222, https://doi.org/10.1145/42372.42381.
/// Steps and constants follow the paper's Algorithm BTPE (the same ones numpy and
/// `rand_distr` implement); the squeeze in step 5.2 uses the paper's Stirling
/// series. Exact in distribution; cost is O(1) per draw on average.
fn binomial_btpe(rng: &mut impl RngCore, n: u64, p: f64) -> u64 {
    // Step 0: set-up. n <= 2^53 (populations are bounded by the engine), so n is
    // exact in f64 and every candidate y fits i64.
    let n_f = n as f64;
    let n_i = n as i64;
    let r = p;
    let q = 1.0 - r;
    let fm = n_f * r + r;
    let m = libm::floor(fm) as i64;
    let nrq = n_f * r * q;
    let p1 = libm::floor(2.195 * libm::sqrt(nrq) - 4.6 * q) + 0.5;
    let xm = m as f64 + 0.5;
    let xl = xm - p1;
    let xr = xm + p1;
    let c = 0.134 + 20.5 / (15.3 + m as f64);
    let a = (fm - xl) / (fm - xl * r);
    let lambda_l = a * (1.0 + a / 2.0);
    let a = (xr - fm) / (xr * q);
    let lambda_r = a * (1.0 + a / 2.0);
    let p2 = p1 * (1.0 + 2.0 * c);
    let p3 = p2 + c / lambda_l;
    let p4 = p3 + c / lambda_r;

    loop {
        // Step 1: region selection.
        let u = uniform(rng) * p4;
        let mut v = uniform(rng);
        let y: i64;
        if u <= p1 {
            // Triangular region: accept immediately.
            y = libm::floor(xm - p1 * v + u) as i64;
            return y as u64;
        }
        if u <= p2 {
            // Step 2: parallelogram region.
            let x = xl + (u - p1) / c;
            v = v * c + 1.0 - (m as f64 - x + 0.5).abs() / p1;
            if v > 1.0 {
                continue;
            }
            y = libm::floor(x) as i64;
        } else if u <= p3 {
            // Step 3: left exponential tail. uniform() never returns 0, so log(v) is finite.
            y = libm::floor(xl + libm::log(v) / lambda_l) as i64;
            if y < 0 {
                continue;
            }
            v *= (u - p2) * lambda_l;
        } else {
            // Step 4: right exponential tail.
            y = libm::floor(xr - libm::log(v) / lambda_r) as i64;
            if y > n_i {
                continue;
            }
            v *= (u - p3) * lambda_r;
        }

        // Step 5.0: acceptance/rejection comparison.
        let k = (y - m).abs();
        if k <= 20 || (k as f64) >= nrq / 2.0 - 1.0 {
            // Step 5.1: explicit evaluation of f(y)/f(m) by recursion.
            let s = r / q;
            let a = s * (n_f + 1.0);
            let mut f = 1.0;
            if m < y {
                let mut i = m + 1;
                while i <= y {
                    f *= a / i as f64 - s;
                    i += 1;
                }
            } else if m > y {
                let mut i = y + 1;
                while i <= m {
                    f /= a / i as f64 - s;
                    i += 1;
                }
            }
            if v <= f {
                return y as u64;
            }
            continue;
        }
        // Step 5.2: squeezing.
        let k_f = k as f64;
        let rho = (k_f / nrq) * ((k_f * (k_f / 3.0 + 0.625) + 1.0 / 6.0) / nrq + 0.5);
        let t = -k_f * k_f / (2.0 * nrq);
        let big_a = libm::log(v);
        if big_a < t - rho {
            return y as u64;
        }
        if big_a > t + rho {
            continue;
        }
        // Step 5.3: final acceptance/rejection test via Stirling's formula.
        let x1 = (y + 1) as f64;
        let f1 = (m + 1) as f64;
        let z = (n_i + 1 - m) as f64;
        let w = (n_i - y + 1) as f64;
        let x2 = x1 * x1;
        let f2 = f1 * f1;
        let z2 = z * z;
        let w2 = w * w;
        let stirling = |v: f64, v2: f64| {
            (13680.0 - (462.0 - (132.0 - (99.0 - 140.0 / v2) / v2) / v2) / v2) / v / 166320.0
        };
        let bound = xm * libm::log(f1 / x1)
            + (n_f - m as f64 + 0.5) * libm::log(z / w)
            + (y - m) as f64 * libm::log(w * r / (x1 * q))
            + stirling(f1, f2)
            + stirling(z, z2)
            + stirling(x1, x2)
            + stirling(w, w2);
        if big_a <= bound {
            return y as u64;
        }
    }
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
        for (n, p) in [
            (100_u64, 0.001),
            (100, 0.05),
            (100, 0.1),
            (100, 0.5),
            (100, 0.9),
            (100, 0.999),
            (6420, 0.09),
            (20_000, 0.0004),
            (1 << 40, 1e-9),
        ] {
            let count = 20_000_u32;
            let mut sum = 0.0;
            let mut square = 0.0;
            for _ in 0..count {
                let x = binomial(&mut rng, n, p);
                assert!(x <= n);
                sum += x as f64;
                square += (x as f64) * (x as f64);
            }
            let mean = sum / f64::from(count);
            let variance = square / f64::from(count) - mean * mean;
            let expected_variance = n as f64 * p * (1.0 - p);
            assert!(
                (mean - n as f64 * p).abs()
                    < 6.0 * libm::sqrt(expected_variance / f64::from(count)),
                "n={n} p={p} mean={mean}"
            );
            assert!(
                (variance - expected_variance).abs() < 0.08 * expected_variance + 0.005,
                "n={n} p={p} variance={variance} expected={expected_variance}"
            );
        }
    }

    /// Exact pmf by recursion from the mode (log-gamma for the mode's mass).
    fn exact_pmf(n: u64, p: f64) -> Vec<f64> {
        let n_f = n as f64;
        let mode = libm::floor((n_f + 1.0) * p) as u64;
        let log_mode = libm::lgamma(n_f + 1.0)
            - libm::lgamma(mode as f64 + 1.0)
            - libm::lgamma((n - mode) as f64 + 1.0)
            + mode as f64 * libm::log(p)
            + (n - mode) as f64 * libm::log1p(-p);
        let mut pmf = vec![0.0; n as usize + 1];
        pmf[mode as usize] = libm::exp(log_mode);
        for k in mode..n {
            pmf[k as usize + 1] = pmf[k as usize] * (n - k) as f64 / (k + 1) as f64 * p / (1.0 - p);
        }
        for k in (1..=mode).rev() {
            pmf[k as usize - 1] = pmf[k as usize] * k as f64 / (n - k + 1) as f64 * (1.0 - p) / p;
        }
        pmf
    }

    /// Chi-square goodness of fit against the exact pmf, with both the geometric
    /// (n*p < 10) and the BTPE (n*p >= 10, all five regions) paths exercised.
    /// Deterministic seed, so the thresholds are fixed, not flaky.
    #[test]
    fn samples_match_exact_pmf() {
        for (seed, n, p) in [
            (1_u8, 30_u64, 0.2),
            (2, 200, 0.03),
            (3, 100, 0.3),
            (4, 1_000, 0.5),
            (5, 6_420, 0.09),
            (6, 10_000, 0.002),
            (7, 500, 0.95),
        ] {
            let mut rng = ChaCha8Rng::from_seed([seed; 32]);
            let draws = 200_000_u32;
            let pmf = exact_pmf(n, p);
            let mut observed = vec![0_u32; n as usize + 1];
            for _ in 0..draws {
                let x = binomial(&mut rng, n, p);
                assert!(x <= n);
                observed[x as usize] += 1;
            }
            // Pool cells into bins with expected count >= 20, tails merged.
            let mut chi_square = 0.0;
            let mut bins = 0_u32;
            let (mut expected_bin, mut observed_bin) = (0.0, 0.0);
            for (k, mass) in pmf.iter().enumerate() {
                expected_bin += mass * f64::from(draws);
                observed_bin += f64::from(observed[k]);
                if expected_bin >= 20.0 {
                    chi_square += (observed_bin - expected_bin).powi(2) / expected_bin;
                    bins += 1;
                    expected_bin = 0.0;
                    observed_bin = 0.0;
                }
            }
            if expected_bin > 0.0 {
                chi_square += (observed_bin - expected_bin).powi(2) / expected_bin;
                bins += 1;
            }
            let degrees = f64::from(bins - 1);
            // Chi-square with d degrees of freedom has mean d and variance 2d; eight
            // standard deviations is a p-value far below 1e-6 for every case here.
            assert!(
                chi_square < degrees + 8.0 * libm::sqrt(2.0 * degrees),
                "n={n} p={p}: chi2={chi_square} bins={bins}"
            );
        }
    }
}
