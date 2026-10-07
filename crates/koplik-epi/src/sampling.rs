//! Portable binomial and Poisson sampling; no platform distribution implementations.
//!
//! Every transcendental call (`log`, `log1p`, `exp`, `sqrt`, `floor`, `lgamma`) goes
//! through `libm` so the seeded path is bit-identical natively and on `wasm32` (spec
//! E4). All draws are `u64`/`i64`/`f64`; no `usize` touches a random value. Every SEIR
//! transition has a finite source pool and is drawn binomially, which cannot overshoot;
//! the Poisson sampler exists only for the renewal-equation forecast
//! (`crate::forecast`), whose offspring counts have no finite pool.

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
/// Steps and constants follow the paper's Algorithm BTPE as implemented by NumPy
/// (`random_binomial_btpe`) and `rand_distr`, including the corrected step 5.3
/// Stirling bound (see `btpe_log_ratio_bound`). Exact in distribution; cost is
/// O(1) per draw on average.
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
        if big_a <= btpe_log_ratio_bound(n_i, r, m, y) {
            return y as u64;
        }
    }
}

/// Stirling-series bound on ln(f(y)/f(m)) used by BTPE step 5.3, where f is the
/// binomial pmf with parameters (n, r) and m its mode. The corrected form: the
/// 1988 paper misprints the leading coefficient (13680) and the signs of the y
/// terms; the Stirling series for ln k! has remainder 1/(12k) - 1/(360k^3) +
/// 1/(1260k^5) - 1/(1680k^7) + 1/(1188k^9), i.e. (13860 - (462 - (132 - (99 -
/// 140/k^2)/k^2)/k^2)/k^2)/(166320 k), and ln f(y)/f(m) = ln m! + ln (n-m)! -
/// ln y! - ln (n-y)! + ..., so the m-side terms add and the y-side terms subtract.
/// Checked against NumPy `random_binomial_btpe` (numpy/random/src/distributions/
/// distributions.c, main, 2026-10) which uses 13860 and signs + + - -. The
/// regression test below compares this bound with the exact recurrence.
fn btpe_log_ratio_bound(n: i64, r: f64, m: i64, y: i64) -> f64 {
    let q = 1.0 - r;
    let n_f = n as f64;
    let xm = m as f64 + 0.5;
    let x1 = (y + 1) as f64;
    let f1 = (m + 1) as f64;
    let z = (n + 1 - m) as f64;
    let w = (n - y + 1) as f64;
    let stirling = |v: f64| {
        let v2 = v * v;
        (13860.0 - (462.0 - (132.0 - (99.0 - 140.0 / v2) / v2) / v2) / v2) / v / 166320.0
    };
    xm * libm::log(f1 / x1)
        + (n_f - m as f64 + 0.5) * libm::log(z / w)
        + (y - m) as f64 * libm::log(w * r / (x1 * q))
        + stirling(f1)
        + stirling(z)
        - stirling(x1)
        - stirling(w)
}

/// Below this mean the multiplication (Knuth) method is used; at or above it PTRS. The
/// same threshold as NumPy (`random_poisson`: `lam >= 10` → `random_poisson_ptrs`). Both
/// methods are exact, so it only trades the O(mean) loop against PTRS's set-up. Part of the
/// seeded path, so fixed here and never configurable at run time.
const PTRS_THRESHOLD: f64 = 10.0;

/// Exact-distribution Poisson draw with the given mean (finite, `>= 0`). Mean 0 gives 0.
///
/// `mean < 10`: Knuth's multiplication method (D. E. Knuth, *The Art of Computer
/// Programming* vol. 2, §3.4.1, algorithm Q): count uniforms until their product falls
/// below `exp(-mean)`. `mean >= 10`: PTRS, the transformed rejection method with squeeze
/// (W. Hörmann, *The transformed rejection method for generating Poisson random
/// variables*, Insurance: Mathematics and Economics 12 (1993) 39-45,
/// doi:10.1016/0167-6687(93)90997-4), constants and steps as in NumPy's
/// `random_poisson_ptrs` (numpy/random/src/distributions/distributions.c). Both paths
/// use only `libm` transcendentals and fixed-width integers.
pub(crate) fn poisson(rng: &mut impl RngCore, mean: f64) -> u64 {
    debug_assert!(mean.is_finite() && mean >= 0.0);
    if mean <= 0.0 {
        return 0;
    }
    if mean < PTRS_THRESHOLD {
        poisson_multiplication(rng, mean)
    } else {
        poisson_ptrs(rng, mean)
    }
}

/// Knuth's method: the number of exponential(1) arrivals before `mean`, found as the number
/// of uniforms whose running product stays above `exp(-mean)`. Cost O(1 + mean).
fn poisson_multiplication(rng: &mut impl RngCore, mean: f64) -> u64 {
    let threshold = libm::exp(-mean);
    let mut count = 0_u64;
    let mut product = 1.0_f64;
    loop {
        product *= uniform(rng);
        if product > threshold {
            count += 1;
        } else {
            return count;
        }
    }
}

/// PTRS (Hörmann 1993, algorithm PTRS; NumPy `random_poisson_ptrs`), for `mean >= 10`.
/// The candidate is `floor((2a/us + b)·U + mean + 0.43)`; the squeeze accepts it when
/// `us >= 0.07` and `V <= vr`, rejects it cheaply when `k < 0` or (`us < 0.013` and
/// `V > us`), and otherwise compares `log(V·α / (a/us² + b))` with the log pmf
/// `-mean + k·log(mean) - lgamma(k + 1)`.
fn poisson_ptrs(rng: &mut impl RngCore, mean: f64) -> u64 {
    let slam = libm::sqrt(mean);
    let loglam = libm::log(mean);
    let b = 0.931 + 2.53 * slam;
    let a = -0.059 + 0.02483 * b;
    let invalpha = 1.1239 + 1.1328 / (b - 3.4);
    let vr = 0.9277 - 3.6224 / (b - 2.0);
    loop {
        let u = uniform(rng) - 0.5;
        let v = uniform(rng);
        let us = 0.5 - u.abs();
        let k = libm::floor((2.0 * a / us + b) * u + mean + 0.43);
        if us >= 0.07 && v <= vr {
            return k as u64;
        }
        if k < 0.0 || (us < 0.013 && v > us) {
            continue;
        }
        if libm::log(v) + libm::log(invalpha) - libm::log(a / (us * us) + b)
            <= -mean + k * loglam - libm::lgamma(k + 1.0)
        {
            return k as u64;
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

    /// Exact ln(f(y)/f(m)) by the step 5.1 recurrence f(i)/f(i-1) = a/i - s.
    fn exact_log_ratio(n: i64, r: f64, m: i64, y: i64) -> f64 {
        let s = r / (1.0 - r);
        let a = s * (n as f64 + 1.0);
        let (lo, hi) = if m < y { (m + 1, y) } else { (y + 1, m) };
        let sum: f64 = (lo..=hi).map(|i| libm::log(a / i as f64 - s)).sum();
        if m < y { sum } else { -sum }
    }

    /// Regression for the step 5.3 bound (#1379): the misprinted 13680 / sign
    /// form is off by about 1e-4 at (1000, 0.5, 522); the corrected bound agrees
    /// with the exact recurrence to the Stirling remainder, far below 1e-9.
    #[test]
    fn btpe_step_53_bound_matches_exact_log_ratio() {
        for (n, r, y) in [
            (1_000_i64, 0.5, 522_i64),
            (1_000, 0.5, 478),
            (6_420, 0.09, 640),
            (6_420, 0.09, 520),
            (100_000, 0.002, 260),
            (50_000, 0.3, 14_800),
        ] {
            let m = libm::floor((n as f64) * r + r) as i64;
            let bound = btpe_log_ratio_bound(n, r, m, y);
            let exact = exact_log_ratio(n, r, m, y);
            assert!(
                (bound - exact).abs() < 1e-9,
                "n={n} r={r} m={m} y={y}: bound={bound} exact={exact}"
            );
        }
        // The uncorrected form is measurably wrong at the reviewer's case.
        let (n, r, y) = (1_000_i64, 0.5, 522_i64);
        let m = libm::floor((n as f64) * r + r) as i64;
        let misprinted = {
            let st = |v: f64| {
                let v2 = v * v;
                (13680.0 - (462.0 - (132.0 - (99.0 - 140.0 / v2) / v2) / v2) / v2) / v / 166320.0
            };
            let (x1, f1, z, w) = (
                (y + 1) as f64,
                (m + 1) as f64,
                (n + 1 - m) as f64,
                (n - y + 1) as f64,
            );
            (m as f64 + 0.5) * libm::log(f1 / x1)
                + (n as f64 - m as f64 + 0.5) * libm::log(z / w)
                + (y - m) as f64 * libm::log(w * r / (x1 * (1.0 - r)))
                + st(f1)
                + st(z)
                + st(x1)
                + st(w)
        };
        assert!((misprinted - exact_log_ratio(n, r, m, y)).abs() > 1e-6);
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

    /// Exact Poisson pmf (in log space, so large means do not underflow) up to a tail
    /// that holds less than 1e-12 of the mass.
    fn exact_poisson_pmf(mean: f64) -> Vec<f64> {
        let mut pmf = Vec::new();
        let mut k = 0.0;
        loop {
            let mass = libm::exp(-mean + k * libm::log(mean) - libm::lgamma(k + 1.0));
            pmf.push(mass);
            if k > mean && mass < 1e-12 {
                return pmf;
            }
            k += 1.0;
        }
    }

    /// Chi-square goodness of fit for the Poisson sampler against the exact pmf, with both
    /// the multiplication (mean < 10) and the PTRS (mean >= 10) paths exercised.
    /// Deterministic seed, so the thresholds are fixed, not flaky.
    #[test]
    fn poisson_samples_match_exact_pmf() {
        for (seed, mean) in [
            (11_u8, 0.3_f64),
            (12, 2.5),
            (13, 9.9),
            (14, 10.0),
            (15, 37.0),
            (16, 250.0),
            (17, 4_000.0),
        ] {
            let mut rng = ChaCha8Rng::from_seed([seed; 32]);
            let draws = 200_000_u32;
            let pmf = exact_poisson_pmf(mean);
            let mut observed = vec![0_u32; pmf.len()];
            for _ in 0..draws {
                let x = poisson(&mut rng, mean);
                let x = usize::try_from(x).unwrap().min(pmf.len() - 1);
                observed[x] += 1;
            }
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
            assert!(
                chi_square < degrees + 8.0 * libm::sqrt(2.0 * degrees),
                "mean={mean}: chi2={chi_square} bins={bins}"
            );
        }
    }

    #[test]
    fn poisson_zero_mean_is_zero_and_consumes_no_draws() {
        let mut rng = ChaCha8Rng::from_seed([9; 32]);
        let before = rng.next_u64();
        let mut rng = ChaCha8Rng::from_seed([9; 32]);
        assert_eq!(poisson(&mut rng, 0.0), 0);
        assert_eq!(rng.next_u64(), before);
    }
}
