//! The renewal-equation estimator on a regular grid of counts.

use koplik_contracts::v1::RtStatus;

use super::RtError;
use super::gamma::GammaDist;
use super::serial_interval::DiscreteSerialInterval;

/// What to assume about incidence before the first step of the series.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeforeSeries {
    /// Unknown (default): a look-back that reaches before the series makes the step
    /// `insufficient_data`. Honest for a series that starts mid-epidemic or at a calendar
    /// boundary (e.g. MMWR week 1).
    Unknown,
    /// Zero: the series starts at the beginning of the outbreak, so earlier incidence is
    /// zero. This is EpiEstim's implicit assumption (`overall_infectivity` sums only observed
    /// steps) and is needed to reproduce its examples.
    Zero,
}

/// Estimator configuration. Every default cites its source.
#[derive(Debug, Clone, PartialEq)]
pub struct RenewalConfig {
    /// Sliding window length in grid steps (`τ`). Default 1 step: on the weekly grid that is
    /// a 7-day window, EpiEstim's default (`make_config.R`: `t_start <- seq(2, T-6)`,
    /// `t_end <- seq(8, T)`, "weekly sliding windows"). Longer windows are smoother and
    /// narrower but less timely (Cori et al. 2013, Results).
    pub window: u32,
    /// Gamma prior on `R`. Default mean 5, SD 5 (shape 1, scale 5): EpiEstim `make_config.R`
    /// defaults `mean_prior = 5`, `std_prior = 5`, as in Cori et al. 2013.
    pub prior: GammaDist,
    /// Minimum cases summed over the window for an estimate to be published. Below it the
    /// step is `insufficient_data`. Default [`RenewalConfig::min_cases_for_cv`]`(0.3, prior)`
    /// = 11: the posterior shape is `a + ΣI`, so its coefficient of variation is
    /// `1/sqrt(a + ΣI)`; EpiEstim (`estimate_R.R`:
    /// `min_nb_cases_per_time_period <- ceiling(1 / cv_posterior^2 - a_prior)`, default
    /// `cv_posterior = 0.3`, Cori et al. 2013 Web Appendix 2) warns below it; here it is a
    /// hard threshold.
    pub min_cases: u32,
    /// Credible-interval levels in (0, 1), e.g. `[0.5, 0.95]`.
    pub levels: Vec<f64>,
    pub before_series: BeforeSeries,
}

impl RenewalConfig {
    /// EpiEstim's default prior: mean 5, SD 5.
    pub fn cori_default_prior() -> GammaDist {
        GammaDist::from_mean_sd(5.0, 5.0).expect("constant prior is valid")
    }

    /// Smallest `ΣI` with posterior coefficient of variation `<= cv`:
    /// `ceil(1/cv^2 - a_prior)`, at least 1 (EpiEstim `estimate_R.R`).
    pub fn min_cases_for_cv(cv: f64, prior: GammaDist) -> Result<u32, RtError> {
        if !(cv.is_finite() && cv > 0.0) {
            return Err(RtError::Config(format!(
                "cv must be finite and > 0, got {cv}"
            )));
        }
        let n = libm::ceil(1.0 / (cv * cv) - prior.shape);
        if n > f64::from(u32::MAX) {
            return Err(RtError::Config(format!(
                "cv {cv} needs more cases than fit in u32"
            )));
        }
        Ok(if n < 1.0 { 1 } else { n as u32 })
    }

    pub fn validate(&self) -> Result<(), RtError> {
        if self.window < 1 {
            return Err(RtError::Config("window must be >= 1 step".into()));
        }
        GammaDist::new(self.prior.shape, self.prior.scale)?;
        if self.levels.is_empty() {
            return Err(RtError::Config(
                "at least one credible level is required".into(),
            ));
        }
        for l in &self.levels {
            if !(l.is_finite() && *l > 0.0 && *l < 1.0) {
                return Err(RtError::Config(format!(
                    "credible level must be in (0, 1), got {l}"
                )));
            }
        }
        Ok(())
    }
}

impl Default for RenewalConfig {
    fn default() -> Self {
        let prior = Self::cori_default_prior();
        Self {
            window: 1,
            prior,
            min_cases: Self::min_cases_for_cv(0.3, prior).expect("constant cv is valid"),
            levels: vec![0.5, 0.95],
            before_series: BeforeSeries::Unknown,
        }
    }
}

/// Why a step is `insufficient_data`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsufficientReason {
    /// The window would start before the series, or includes its first step (whose
    /// infectivity `Λ` is undefined: there is nothing before it).
    IncompleteWindow,
    /// A count in the window, or in the look-back feeding `Λ`, is missing (or lies before
    /// the series under [`BeforeSeries::Unknown`]).
    MissingCount,
    /// Fewer cases in the window than `min_cases`.
    BelowThreshold { cases: u32 },
    /// The overall infectivity over the window is zero: no cases in the look-back, so the
    /// window's cases have no infectors in the data and `R` (cases per unit infectivity) is
    /// undefined. Publishing the posterior here would report an arbitrarily large `R` driven
    /// only by the prior; the honest output is `insufficient_data`. (Importation is not
    /// modelled: every case is treated as locally acquired, as in EpiEstim without an
    /// `imported` column.)
    NoInfectivity,
}

/// Equal-tailed credible interval.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CredibleInterval {
    pub level: f64,
    pub lower: f64,
    pub upper: f64,
}

/// The estimate for the window ending at `step`.
#[derive(Debug, Clone, PartialEq)]
pub struct StepEstimate {
    /// Index of the last step in the window (0-based).
    pub step: u32,
    pub status: RtStatus,
    pub reason: Option<InsufficientReason>,
    /// Cases summed over the window, when every count in it is known.
    pub cases_in_window: Option<u32>,
    /// Posterior of `R` when `status` is `Ok`.
    pub posterior: Option<GammaDist>,
    /// One interval per configured level, in configuration order; empty when insufficient.
    pub intervals: Vec<CredibleInterval>,
}

/// Estimate `R` for every step of `counts` (one window ending at each step).
///
/// `counts[s]` is the incidence at step `s`, `None` when missing. `si.weights()[k]` is the
/// serial-interval probability of a lag of `k` steps. The result has one entry per step.
pub fn estimate_series(
    counts: &[Option<u32>],
    si: &DiscreteSerialInterval,
    cfg: &RenewalConfig,
) -> Result<Vec<StepEstimate>, RtError> {
    cfg.validate()?;
    let n = u32::try_from(counts.len()).map_err(|_| RtError::SeriesTooLong(counts.len() as u64))?;
    let w = si.weights();
    let mut out = Vec::with_capacity(counts.len());
    for t in 0..n {
        out.push(estimate_step(counts, w, cfg, t)?);
    }
    Ok(out)
}

fn insufficient(step: u32, reason: InsufficientReason, cases: Option<u32>) -> StepEstimate {
    StepEstimate {
        step,
        status: RtStatus::InsufficientData,
        reason: Some(reason),
        cases_in_window: cases,
        posterior: None,
        intervals: Vec::new(),
    }
}

fn estimate_step(
    counts: &[Option<u32>],
    w: &[f64],
    cfg: &RenewalConfig,
    t: u32,
) -> Result<StepEstimate, RtError> {
    // Window [t - τ + 1, t] must lie inside the series and exclude step 0, whose Λ is
    // undefined (EpiEstim: lambda[1] <- NA, t_start >= 2).
    let Some(first) = (t + 1).checked_sub(cfg.window) else {
        return Ok(insufficient(t, InsufficientReason::IncompleteWindow, None));
    };
    if first == 0 {
        return Ok(insufficient(t, InsufficientReason::IncompleteWindow, None));
    }
    // Cases in the window: every count must be known.
    let mut cases: u64 = 0;
    for s in first..=t {
        match counts[s as usize] {
            Some(c) => cases += u64::from(c),
            None => return Ok(insufficient(t, InsufficientReason::MissingCount, None)),
        }
    }
    let cases_u32 = u32::try_from(cases).map_err(|_| RtError::SeriesTooLong(cases))?;
    // Overall infectivity Λ_s = Σ_{k>=1} w_k I_{s-k} for each step in the window.
    let mut lambda_sum = 0.0_f64;
    for s in first..=t {
        for (k, wk) in w.iter().enumerate().skip(1) {
            let k = k as u32; // bounded by w.len() which fits u32 (checked at construction)
            match s.checked_sub(k) {
                Some(idx) => match counts[idx as usize] {
                    Some(c) => lambda_sum += wk * f64::from(c),
                    None => {
                        return Ok(insufficient(
                            t,
                            InsufficientReason::MissingCount,
                            Some(cases_u32),
                        ));
                    }
                },
                None => match cfg.before_series {
                    BeforeSeries::Zero => {}
                    BeforeSeries::Unknown => {
                        return Ok(insufficient(
                            t,
                            InsufficientReason::MissingCount,
                            Some(cases_u32),
                        ));
                    }
                },
            }
        }
    }
    if cases_u32 < cfg.min_cases {
        return Ok(insufficient(
            t,
            InsufficientReason::BelowThreshold { cases: cases_u32 },
            Some(cases_u32),
        ));
    }
    // NaN-safe: a non-finite sum is also "no usable infectivity".
    if lambda_sum.is_nan() || lambda_sum <= 0.0 {
        return Ok(insufficient(
            t,
            InsufficientReason::NoInfectivity,
            Some(cases_u32),
        ));
    }
    // Posterior: Gamma(a + ΣI, 1 / (1/b + ΣΛ)) (Cori et al. 2013; EpiEstim `estimate_R.R`
    // `a_posterior <- a_prior + sum(incid[...])`, `b_posterior <- 1/(1/b_prior + sum(lambda[...]))`).
    let posterior = GammaDist::new(
        cfg.prior.shape + cases as f64,
        1.0 / (1.0 / cfg.prior.scale + lambda_sum),
    )?;
    let mut intervals = Vec::with_capacity(cfg.levels.len());
    for &level in &cfg.levels {
        let tail = (1.0 - level) / 2.0;
        intervals.push(CredibleInterval {
            level,
            lower: posterior.quantile(tail)?,
            upper: posterior.quantile(1.0 - tail)?,
        });
    }
    Ok(StepEstimate {
        step: t,
        status: RtStatus::Ok,
        reason: None,
        cases_in_window: Some(cases_u32),
        posterior: Some(posterior),
        intervals,
    })
}
