//! Gamma distribution on `libm`: CDF by the regularized lower incomplete gamma function and
//! a deterministic quantile by bisection. No `std` transcendentals, so native and `wasm32`
//! agree to the last bit.

use libm::{exp, lgamma, log, sqrt};

use super::RtError;

/// Gamma distribution by shape and scale (R's `shape`/`scale` parametrisation, as EpiEstim).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GammaDist {
    pub shape: f64,
    pub scale: f64,
}

impl GammaDist {
    /// Validated constructor: both parameters finite and strictly positive.
    pub fn new(shape: f64, scale: f64) -> Result<Self, RtError> {
        if !(shape.is_finite() && shape > 0.0 && scale.is_finite() && scale > 0.0) {
            return Err(RtError::Config(format!(
                "gamma shape and scale must be finite and > 0, got shape={shape} scale={scale}"
            )));
        }
        Ok(Self { shape, scale })
    }

    /// Moment-matched gamma: `shape = (mean/sd)^2`, `scale = sd^2/mean` (EpiEstim
    /// `estimate_R.R`: `a_prior <- (mean_prior/std_prior)^2; b_prior <- std_prior^2/mean_prior`).
    pub fn from_mean_sd(mean: f64, sd: f64) -> Result<Self, RtError> {
        if !(mean.is_finite() && mean > 0.0 && sd.is_finite() && sd > 0.0) {
            return Err(RtError::Config(format!(
                "gamma mean and sd must be finite and > 0, got mean={mean} sd={sd}"
            )));
        }
        Self::new((mean / sd) * (mean / sd), sd * sd / mean)
    }

    pub fn mean(self) -> f64 {
        self.shape * self.scale
    }

    pub fn sd(self) -> f64 {
        sqrt(self.shape) * self.scale
    }

    /// `P(X <= x)`; 0 for `x <= 0`.
    pub fn cdf(self, x: f64) -> f64 {
        if x <= 0.0 {
            0.0
        } else {
            regularized_lower_gamma(self.shape, x / self.scale)
        }
    }

    /// Equal-tailed quantile for `p` in the open interval (0, 1), by bisection on [`cdf`]
    /// until the bracket cannot shrink further (a deterministic fixed point, not an
    /// iteration-count cutoff).
    pub fn quantile(self, p: f64) -> Result<f64, RtError> {
        if !(p.is_finite() && p > 0.0 && p < 1.0) {
            return Err(RtError::Config(format!(
                "quantile probability must be in (0, 1), got {p}"
            )));
        }
        // Bracket: widen the upper end geometrically until it covers p.
        let mut lo = 0.0_f64;
        let mut hi = (self.mean() + 10.0 * self.sd()).max(self.scale);
        let mut widen = 0u32;
        while self.cdf(hi) < p {
            hi *= 2.0;
            widen += 1;
            if widen > 2000 || !hi.is_finite() {
                return Err(RtError::Config(format!(
                    "gamma quantile bracket failed for shape={} scale={} p={p}",
                    self.shape, self.scale
                )));
            }
        }
        // Bisection to the floating-point fixed point: stops when the midpoint equals an end.
        for _ in 0..4000 {
            let mid = 0.5 * (lo + hi);
            if mid <= lo || mid >= hi {
                break;
            }
            if self.cdf(mid) < p {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        Ok(hi)
    }
}

/// Regularized lower incomplete gamma `P(a, x) = γ(a, x) / Γ(a)` for `a > 0`, `x >= 0`.
///
/// Series expansion for `x < a + 1`, Lentz's modified continued fraction for the complement
/// otherwise (Press et al., *Numerical Recipes*, 3rd ed., §6.2, `gser`/`gcf`). All
/// transcendentals via `libm`. Loops are bounded; a non-converged evaluation returns the
/// best iterate rather than looping forever (does not happen for the shapes used here).
pub fn regularized_lower_gamma(a: f64, x: f64) -> f64 {
    debug_assert!(a > 0.0 && x >= 0.0);
    if x <= 0.0 {
        return 0.0;
    }
    let eps = f64::EPSILON;
    let fpmin = f64::MIN_POSITIVE / eps;
    // ln of the prefactor x^a e^{-x} / Γ(a).
    let ln_pref = -x + a * log(x) - lgamma(a);
    if x < a + 1.0 {
        let mut ap = a;
        let mut del = 1.0 / a;
        let mut sum = del;
        for _ in 0..100_000 {
            ap += 1.0;
            del *= x / ap;
            sum += del;
            if del.abs() < sum.abs() * eps {
                break;
            }
        }
        sum * exp(ln_pref)
    } else {
        let mut b = x + 1.0 - a;
        let mut c = 1.0 / fpmin;
        let mut d = 1.0 / b;
        let mut h = d;
        let mut i = 1.0_f64;
        for _ in 0..100_000 {
            let an = -i * (i - a);
            b += 2.0;
            d = an * d + b;
            if d.abs() < fpmin {
                d = fpmin;
            }
            c = b + an / c;
            if c.abs() < fpmin {
                c = fpmin;
            }
            d = 1.0 / d;
            let del = d * c;
            h *= del;
            if (del - 1.0).abs() < eps {
                break;
            }
            i += 1.0;
        }
        1.0 - exp(ln_pref) * h
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    #[test]
    fn regularized_gamma_matches_closed_forms() {
        // P(1, x) = 1 - e^{-x}.
        assert!(close(
            regularized_lower_gamma(1.0, 1.0),
            1.0 - exp(-1.0),
            1e-14
        ));
        // P(3, 2) = 1 - e^{-2} (1 + 2 + 2).
        assert!(close(
            regularized_lower_gamma(3.0, 2.0),
            1.0 - exp(-2.0) * 5.0,
            1e-14
        ));
        // P(1/2, 1/2) = erf(sqrt(1/2)) = 0.6826894921370859 (two-sided normal 1σ mass).
        assert!(close(
            regularized_lower_gamma(0.5, 0.5),
            0.682_689_492_137_085_9,
            1e-13
        ));
        // Both branches, one each side of x = a + 1, agree with the exact series value of
        // P(2, x) = 1 - e^{-x}(1 + x).
        for x in [0.5, 2.5, 3.0, 3.5, 20.0] {
            assert!(close(
                regularized_lower_gamma(2.0, x),
                1.0 - exp(-x) * (1.0 + x),
                1e-13
            ));
        }
    }

    #[test]
    fn quantile_inverts_cdf() {
        let g = GammaDist::from_mean_sd(5.0, 5.0).unwrap(); // shape 1, scale 5: exponential
        for p in [0.025, 0.25, 0.5, 0.75, 0.975] {
            let q = g.quantile(p).unwrap();
            // Exponential quantile: -scale ln(1-p).
            assert!(close(q, -5.0 * log(1.0 - p), 1e-12), "p={p} q={q}");
            assert!(close(g.cdf(q), p, 1e-13));
        }
        let g = GammaDist::new(37.0, 0.047).unwrap();
        for p in [0.005, 0.5, 0.995] {
            assert!(close(g.cdf(g.quantile(p).unwrap()), p, 1e-12));
        }
    }

    #[test]
    fn rejects_bad_parameters() {
        assert!(GammaDist::new(0.0, 1.0).is_err());
        assert!(GammaDist::new(1.0, f64::NAN).is_err());
        assert!(GammaDist::from_mean_sd(1.0, 0.0).is_err());
        let g = GammaDist::new(1.0, 1.0).unwrap();
        assert!(g.quantile(0.0).is_err());
        assert!(g.quantile(1.0).is_err());
    }
}
