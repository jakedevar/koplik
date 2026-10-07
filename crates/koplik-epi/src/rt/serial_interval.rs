//! The measles serial interval and its discretisation to a regular time grid.

use super::RtError;
use super::gamma::GammaDist;

/// A continuous gamma-distributed serial interval (symptom onset of infector to symptom
/// onset of infectee), in days.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SerialInterval {
    pub mean_days: f64,
    pub sd_days: f64,
}

impl SerialInterval {
    /// Measles default. Mean 11.7 d, SD 3.0 d (variance 9 d²), gamma.
    ///
    /// Source checked for this value: CDC, *Nowcasting to Estimate Real-Time Measles
    /// Transmission Trends*, MMWR Morb Mortal Wkly Rep 2026;75(33) (mm7533a1): "The time
    /// between date of infection for primary and secondary case pairs, assumed to be gamma
    /// distributed with a mean of 11.7 days and a variance of 9 days", citing Klinkenberg D,
    /// Nishiura H. J Theor Biol 2011;284:52-60. The same 11.7 d mean is the pooled measles
    /// estimate of the systematic review Vink MA, Bootsma MCJ, Wallinga J. *Serial intervals of
    /// respiratory infectious diseases: a systematic review and analysis.* Am J Epidemiol
    /// 2014;180(9):865-875 (abstract checked; the full text, with its SD, is paywalled).
    /// CDC's figure is a generation interval; following Cori et al. 2013 the serial interval
    /// is used as its observable proxy, which has the same mean.
    pub const MEASLES: SerialInterval = SerialInterval {
        mean_days: 11.7,
        sd_days: 3.0,
    };

    /// Reject a non-finite or non-positive mean or SD before any gamma parameter is
    /// derived from them (a negative SD would otherwise square away silently).
    pub fn validate(&self) -> Result<(), RtError> {
        if !(self.mean_days.is_finite() && self.mean_days > 0.0) {
            return Err(RtError::Config(format!(
                "serial interval mean must be finite and > 0 days, got {}",
                self.mean_days
            )));
        }
        if !(self.sd_days.is_finite() && self.sd_days > 0.0) {
            return Err(RtError::Config(format!(
                "serial interval SD must be finite and > 0 days, got {}",
                self.sd_days
            )));
        }
        Ok(())
    }

    /// The continuous distribution.
    pub fn gamma(&self) -> Result<GammaDist, RtError> {
        self.validate()?;
        GammaDist::from_mean_sd(self.mean_days, self.sd_days)
    }

    /// Discretise to the weekly grid, for weekly (MMWR) incidence.
    ///
    /// Method: an infector's onset falls uniformly within its week (position `u ~ U(0, 7)`
    /// days), the infectee's onset is `u + X` days later with `X ~ Gamma`, and the lag in
    /// weeks is `floor((u + X) / 7)`. Then `w_k = (1/7) ∫_0^7 [F(7k + 7 - u) - F(7k - u)] du`,
    /// which with `G(y) = ∫_0^y F = y·F_{a,b}(y) - a·b·F_{a+1,b}(y)` (the identity behind
    /// Cori et al. 2013, Web Appendix 11, and EpiEstim's `discr_si`) is the closed form
    /// `w_k = [G(7k + 7) - 2·G(7k) + G(7k - 7)] / 7`.
    ///
    /// The renewal equation needs `w_0 = 0` (EpiEstim: "si_distr should be so that
    /// si_distr[1] = 0"): same-week transmission cannot be attributed with weekly counts.
    /// That mass, and the tail beyond `max_weeks`, is removed and the remaining weights are
    /// renormalised to sum to 1; the removed mass is reported as [`DiscreteSerialInterval::dropped_mass`]
    /// so it can be checked, not hidden. For measles (mean 11.7 d, SD 3.0 d) it is well
    /// under 1% because the serial interval is rarely shorter than a week.
    pub fn discretize_weekly(&self, max_weeks: u32) -> Result<DiscreteSerialInterval, RtError> {
        if max_weeks < 1 {
            return Err(RtError::Config("max_weeks must be >= 1".into()));
        }
        let g = self.gamma()?;
        let mut raw = Vec::with_capacity(max_weeks as usize + 1);
        for k in 0..=max_weeks {
            let y = 7.0 * f64::from(k);
            let w = (integrated_cdf(g, y + 7.0) - 2.0 * integrated_cdf(g, y)
                + integrated_cdf(g, y - 7.0))
                / 7.0;
            raw.push(w.max(0.0));
        }
        let same_week = raw[0];
        let kept: f64 = raw[1..].iter().sum();
        if kept.is_nan() || kept <= 0.0 {
            return Err(RtError::Config(
                "serial interval has no mass at a lag of one week or more".into(),
            ));
        }
        let mut weights = vec![0.0];
        weights.extend(raw[1..].iter().map(|w| w / kept));
        // dropped = same-week mass + tail beyond max_weeks = 1 - kept.
        let dropped = (1.0 - kept).max(0.0);
        debug_assert!(dropped >= same_week - 1e-12);
        DiscreteSerialInterval::from_weights(weights, dropped)
    }

    /// EpiEstim's daily discretisation (`discr_si`, Cori et al. 2013 Web Appendix 11): a gamma
    /// shifted by one day, with `a = ((μ-1)/σ)^2`, `b = σ^2/(μ-1)`, and
    /// `w_k = k·F(k) + (k-2)·F(k-2) - 2(k-1)·F(k-1) + a·b·[2·F*(k-1) - F*(k-2) - F*(k)]`
    /// (`F*` has shape `a+1`), i.e. `G(k) - 2·G(k-1) + G(k-2)`. Weights for `k = 0..steps-1`,
    /// not renormalised, exactly as EpiEstim does before computing `Λ`. Used for daily data
    /// and for the agreement test against EpiEstim's published example.
    pub fn discretize_daily_cori(&self, steps: u32) -> Result<DiscreteSerialInterval, RtError> {
        if steps < 2 {
            return Err(RtError::Config("steps must be >= 2".into()));
        }
        self.validate()?;
        if self.mean_days <= 1.0 {
            return Err(RtError::Config(
                "Cori daily discretisation needs a mean serial interval > 1 day".into(),
            ));
        }
        let m = self.mean_days - 1.0;
        let g = GammaDist::new(
            (m / self.sd_days) * (m / self.sd_days),
            self.sd_days * self.sd_days / m,
        )?;
        let mut weights = Vec::with_capacity(steps as usize);
        for k in 0..steps {
            let k = f64::from(k);
            let w = integrated_cdf(g, k) - 2.0 * integrated_cdf(g, k - 1.0)
                + integrated_cdf(g, k - 2.0);
            weights.push(w.max(0.0));
        }
        let total: f64 = weights.iter().sum();
        DiscreteSerialInterval::from_weights(weights, (1.0 - total).max(0.0))
    }
}

/// `G(y) = ∫_0^y F_{a,b}(x) dx = y·F_{a,b}(y) - a·b·F_{a+1,b}(y)`, and 0 for `y <= 0`.
fn integrated_cdf(g: GammaDist, y: f64) -> f64 {
    if y <= 0.0 {
        return 0.0;
    }
    let shifted = GammaDist {
        shape: g.shape + 1.0,
        scale: g.scale,
    };
    y * g.cdf(y) - g.shape * g.scale * shifted.cdf(y)
}

/// A serial interval on a regular grid: `weights[k]` is the probability of a lag of `k`
/// steps, with `weights[0] == 0`.
#[derive(Debug, Clone, PartialEq)]
pub struct DiscreteSerialInterval {
    weights: Vec<f64>,
    dropped_mass: f64,
}

impl DiscreteSerialInterval {
    /// Validated constructor: at least two weights, `weights[0] == 0`, all finite and
    /// non-negative, total mass in (0, 1 + 1e-9]. `dropped_mass` records the probability
    /// mass of the continuous distribution not represented in `weights`.
    pub fn from_weights(weights: Vec<f64>, dropped_mass: f64) -> Result<Self, RtError> {
        if weights.len() < 2 {
            return Err(RtError::Config(
                "serial interval needs at least two weights".into(),
            ));
        }
        if weights[0] != 0.0 {
            return Err(RtError::Config(
                "serial interval weight at lag 0 must be 0".into(),
            ));
        }
        if weights.iter().any(|w| !w.is_finite() || *w < 0.0) {
            return Err(RtError::Config(
                "serial interval weights must be finite and >= 0".into(),
            ));
        }
        let total: f64 = weights.iter().sum();
        if !(total > 0.0 && total <= 1.0 + 1e-9) {
            return Err(RtError::Config(format!(
                "serial interval weights must sum to at most 1, got {total}"
            )));
        }
        if !(dropped_mass.is_finite() && dropped_mass >= 0.0) {
            return Err(RtError::Config(
                "dropped mass must be finite and >= 0".into(),
            ));
        }
        Ok(Self {
            weights,
            dropped_mass,
        })
    }

    pub fn weights(&self) -> &[f64] {
        &self.weights
    }

    /// Mass of the continuous serial interval that `weights` does not carry (same-step
    /// transmission and the truncated tail).
    pub fn dropped_mass(&self) -> f64 {
        self.dropped_mass
    }

    /// Largest lag with a weight (`weights.len() - 1`).
    pub fn max_lag(&self) -> u32 {
        (self.weights.len() - 1) as u32
    }

    /// Mean lag in grid steps under `weights`.
    pub fn mean_lag(&self) -> f64 {
        let total: f64 = self.weights.iter().sum();
        self.weights
            .iter()
            .enumerate()
            .map(|(k, w)| (k as u32) as f64 * w)
            .sum::<f64>()
            / total
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weekly_closed_form_matches_numerical_integration() {
        let si = SerialInterval::MEASLES;
        let g = si.gamma().unwrap();
        let d = si.discretize_weekly(8).unwrap();
        // Midpoint rule over the infector's position in the week, 20_000 points.
        let n = 20_000;
        let mut raw = [0.0; 9];
        for i in 0..n {
            let u = (f64::from(i) + 0.5) / f64::from(n) * 7.0;
            for (k, slot) in raw.iter_mut().enumerate() {
                let lo = 7.0 * k as f64 - u;
                let hi = lo + 7.0;
                *slot += (g.cdf(hi) - g.cdf(lo)) / f64::from(n);
            }
        }
        let kept: f64 = raw[1..].iter().sum();
        for (k, numeric) in raw.iter().enumerate().skip(1) {
            let expected = numeric / kept;
            assert!(
                (d.weights()[k] - expected).abs() < 1e-6,
                "k={k} closed={} numeric={expected}",
                d.weights()[k]
            );
        }
        // Same-week mass plus tail is what was dropped.
        assert!((d.dropped_mass() - (1.0 - kept)).abs() < 1e-6);
    }

    #[test]
    fn weekly_measles_weights_are_sane() {
        let d = SerialInterval::MEASLES.discretize_weekly(8).unwrap();
        let w = d.weights();
        assert_eq!(w[0], 0.0);
        assert!((w.iter().sum::<f64>() - 1.0).abs() < 1e-12);
        // Serial interval 11.7 ± 3 d: lags of one and two weeks carry nearly all the mass.
        assert!(w[1] + w[2] > 0.85, "w1={} w2={}", w[1], w[2]);
        assert!(
            w[2] > w[1],
            "a 12-day interval mostly spans a week boundary twice"
        );
        assert!(d.dropped_mass() < 0.01, "dropped {}", d.dropped_mass());
        // Mean lag in weeks: E[floor((u + X)/7)] ≈ (3.5 + 11.7)/7 - 1/2 (the floor drops
        // about half a week on average), slightly raised by renormalising away w_0.
        let expected = (3.5 + 11.7) / 7.0 - 0.5;
        assert!(
            (d.mean_lag() - expected).abs() < 0.1,
            "mean lag {} vs ≈{expected}",
            d.mean_lag()
        );
    }

    #[test]
    fn daily_cori_reproduces_epiestim_formula_shape() {
        // mean 2.6, sd 1.5 (the EpiEstim Flu2009 example): w_0 = 0, mode at lag 2, sums to ~1.
        let si = SerialInterval {
            mean_days: 2.6,
            sd_days: 1.5,
        };
        let d = si.discretize_daily_cori(32).unwrap();
        let w = d.weights();
        assert_eq!(w[0], 0.0);
        assert!(w[2] > w[1] && w[2] > w[3]);
        assert!((w.iter().sum::<f64>() - 1.0).abs() < 1e-6);
        // Mean of the shifted gamma is the requested mean.
        assert!((d.mean_lag() - 2.6).abs() < 1e-3);
    }

    #[test]
    fn rejects_negative_or_non_finite_mean_and_sd() {
        let bad_sd = SerialInterval {
            mean_days: 2.6,
            sd_days: -1.5,
        };
        assert!(bad_sd.validate().is_err());
        assert!(bad_sd.gamma().is_err());
        assert!(bad_sd.discretize_daily_cori(32).is_err());
        assert!(bad_sd.discretize_weekly(8).is_err());
        for (mean, sd) in [
            (0.0, 1.0),
            (-11.7, 3.0),
            (f64::NAN, 3.0),
            (11.7, 0.0),
            (11.7, f64::INFINITY),
        ] {
            let si = SerialInterval {
                mean_days: mean,
                sd_days: sd,
            };
            assert!(si.validate().is_err(), "mean={mean} sd={sd}");
            assert!(si.discretize_daily_cori(32).is_err(), "mean={mean} sd={sd}");
            assert!(si.discretize_weekly(8).is_err(), "mean={mean} sd={sd}");
        }
        // A mean of at most one day is valid in general but not for the shifted daily form.
        let short = SerialInterval {
            mean_days: 1.0,
            sd_days: 0.5,
        };
        assert!(short.validate().is_ok());
        assert!(short.discretize_daily_cori(32).is_err());
        assert!(SerialInterval::MEASLES.validate().is_ok());
    }

    #[test]
    fn rejects_invalid_weights() {
        assert!(DiscreteSerialInterval::from_weights(vec![0.1, 0.9], 0.0).is_err());
        assert!(DiscreteSerialInterval::from_weights(vec![0.0, 1.5], 0.0).is_err());
        assert!(DiscreteSerialInterval::from_weights(vec![0.0], 0.0).is_err());
        assert!(DiscreteSerialInterval::from_weights(vec![0.0, 0.5, 0.5], 0.0).is_ok());
    }
}
