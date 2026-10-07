//! 4-8 week forecast of weekly cases by renewal-equation projection.
//!
//! Method (Nouvellet P, Cori A, Garske T, et al. *A simple approach to measure
//! transmissibility and forecast incidence.* Epidemics 2018;22:29-35,
//! doi:10.1016/j.epidem.2017.02.012; reference implementation: the `projections` R package,
//! `project()`, RECON): the reproduction number `R` is estimated over the most recent
//! `window` weeks ending at the origin week with the Cori et al. 2013 renewal posterior
//! (`crate::rt`, `Gamma(a + ΣI, 1/(1/b + ΣΛ))`), and is held constant over the horizon.
//! Each ensemble member draws one `R` from that posterior (inverse transform through
//! [`GammaDist::quantile`]) and then, week by week, `I_{t+h} ~ Poisson(R · Λ_{t+h})` with
//! `Λ_{t+h} = Σ_{k≥1} w_k I_{t+h-k}`, where `w` is the measles serial interval discretised
//! to MMWR weeks ([`SerialInterval::discretize_weekly`]), observed counts stand in for
//! `I` at and before the origin and the member's own simulated counts after it. Quantiles
//! of the member counts at each horizon are the published forecast.
//!
//! Information cutoff: [`forecast_weekly`] discards every row after the origin week before
//! doing anything else, so a later week cannot leak in; which *vintage* of the counts at
//! or before the origin is used is the caller's responsibility (`crate::backtest::vintages`
//! does it by `first_seen_at`).
//!
//! Honesty rules, inherited from `crate::rt`: a missing count in the window or in the
//! serial-interval look-back, fewer than `min_cases` cases in the window, or zero
//! infectivity give `insufficient_data` and no forecast, never a prior-driven number.
//!
//! Pre-registration: every default below was fixed from the literature or by the stated
//! rule before any backtest score was computed, and none was changed after
//! (`thoughts/shared/research/backtest-2025-west-texas.md`). Nothing here is tuned.
//!
//! Determinism: the only stochastic draws are the member's `R` and its Poisson counts,
//! from a `ChaCha8Rng` seeded by [`derive_forecast_seed`]; every transcendental is from
//! `libm`; no `usize` enters a draw. The same seed and inputs give bit-identical members.

use std::collections::BTreeMap;

use koplik_contracts::v1::{
    Forecast, ForecastQuantile, GeoId, MmwrError, MmwrWeek, Provenance, ProvenanceError,
    Provenances,
};
use koplik_contracts::v3::CaseDefinition;
use rand_chacha::ChaCha8Rng;
use rand_core::SeedableRng;
use sha2::{Digest, Sha256};

use crate::rt::{
    DiscreteSerialInterval, GammaDist, InsufficientReason, RenewalConfig, RtError, SerialInterval,
    WeeklyCaseRow, estimate_series,
};
use crate::sampling::{poisson, uniform};

/// Errors from the forecast module. Data problems are reported, never silently repaired.
#[derive(Debug, thiserror::Error)]
pub enum ForecastError {
    #[error("invalid forecast configuration: {0}")]
    Config(String),
    #[error(transparent)]
    Rt(#[from] RtError),
    #[error(transparent)]
    Mmwr(#[from] MmwrError),
    #[error(transparent)]
    Provenance(#[from] ProvenanceError),
    /// A member's projected mean passed [`MAX_PROJECTED_MEAN`]: the projection has
    /// exploded past any meaningful case count and is refused rather than saturated.
    #[error("projected mean {mean} at horizon {horizon} exceeds {MAX_PROJECTED_MEAN}")]
    Overflow { horizon: u32, mean: f64 },
}

/// Largest weekly Poisson mean a member may reach (2^40). Above it the forecast is an
/// error, never a clipped number.
pub const MAX_PROJECTED_MEAN: f64 = 1_099_511_627_776.0;

/// The 23 quantile levels of the CDC FluSight / hubverse quantile format (FluSight
/// 2023-24 guidelines; Bracher J, Ray EL, Gneiting T, Reich NG. *Evaluating epidemic
/// forecasts in an interval format.* PLoS Comput Biol 2021;17(2):e1008618,
/// doi:10.1371/journal.pcbi.1008618). They contain the 50% (0.25, 0.75) and 90% (0.05,
/// 0.95) central intervals the backtest scores.
pub const HUB_QUANTILE_LEVELS: [f64; 23] = [
    0.01, 0.025, 0.05, 0.1, 0.15, 0.2, 0.25, 0.3, 0.35, 0.4, 0.45, 0.5, 0.55, 0.6, 0.65, 0.7, 0.75,
    0.8, 0.85, 0.9, 0.95, 0.975, 0.99,
];

/// Forecast configuration. Every default cites its source or its pre-registered rule.
#[derive(Debug, Clone, PartialEq)]
pub struct ForecastConfig {
    /// Default [`SerialInterval::MEASLES`] (mean 11.7 d, SD 3.0 d).
    pub serial_interval: SerialInterval,
    /// Longest lag kept when discretising the serial interval to weeks. Default
    /// [`ForecastConfig::DEFAULT_MAX_LAG_WEEKS`] by the rule: the smallest number of weeks
    /// whose dropped mass (same-week plus tail, [`DiscreteSerialInterval::dropped_mass`]) is
    /// below 1%, the same tolerance `crate::rt` documents for its own truncation. A shorter
    /// look-back than R_t's 8 weeks matters because every lag must be *known*: each extra
    /// week of lag postpones the first week a forecast can be made from a series that
    /// starts mid-outbreak. Checked by a test, not assumed.
    pub max_lag_weeks: u32,
    /// The renewal estimator for `R` at the origin. Default: [`RenewalConfig::default`]
    /// (EpiEstim prior mean 5, SD 5; `min_cases` 11 from the posterior-CV 0.3 rule;
    /// before-series unknown) with `window` = [`ForecastConfig::DEFAULT_WINDOW_WEEKS`].
    pub renewal: RenewalConfig,
    /// Weeks ahead to project, 1 to `horizon_weeks`. Default 8: the spec's "4-8 week
    /// forecast" at its far end, so every horizon from 1 to 8 is published and scored.
    pub horizon_weeks: u32,
    /// Ensemble members. Default 1,000: the spec's ensemble size (E4, "1,000-run
    /// simulated ensemble"), so the 1% and 99% quantiles rest on about ten members each.
    pub run_count: u32,
    /// Quantile levels to publish, strictly increasing in (0, 1). Default
    /// [`HUB_QUANTILE_LEVELS`].
    pub levels: Vec<f64>,
}

impl ForecastConfig {
    /// See [`ForecastConfig::max_lag_weeks`]. Measured for the measles serial interval
    /// (test `default_max_lag_follows_rule`): the dropped mass is 6.6% with a 2-week
    /// look-back and 0.54% with 3 weeks, of which 0.45% is same-week mass that no weekly
    /// look-back can keep (4 weeks: 0.45%). The rule therefore gives 3.
    pub const DEFAULT_MAX_LAG_WEEKS: u32 = 3;

    /// Estimation window in weeks, pre-registered at 3. Nouvellet et al. 2018 estimate
    /// transmissibility over a recent window whose length they choose per outbreak; here the
    /// rule is fixed before any score: three weeks is about 1.8 mean measles serial
    /// intervals (11.7 d), so the window spans at least one generation of infectors and
    /// their infectees, while staying short enough to follow a change in transmission
    /// within a month. The window is not chosen by, and was never varied against, the
    /// backtest score; the backtest report lists a window sensitivity next to the
    /// pre-registered primary, with every value shown.
    pub const DEFAULT_WINDOW_WEEKS: u32 = 3;

    pub fn validate(&self) -> Result<(), ForecastError> {
        self.renewal.validate()?;
        if self.max_lag_weeks < 1 {
            return Err(ForecastError::Config("max_lag_weeks must be >= 1".into()));
        }
        if self.horizon_weeks < 1 {
            return Err(ForecastError::Config("horizon_weeks must be >= 1".into()));
        }
        if self.run_count < 2 {
            return Err(ForecastError::Config("run_count must be >= 2".into()));
        }
        if self.levels.is_empty() {
            return Err(ForecastError::Config(
                "at least one quantile level is required".into(),
            ));
        }
        for l in &self.levels {
            if !(l.is_finite() && *l > 0.0 && *l < 1.0) {
                return Err(ForecastError::Config(format!(
                    "quantile level must be in (0, 1), got {l}"
                )));
            }
        }
        if self.levels.windows(2).any(|w| !(w[0] < w[1])) {
            return Err(ForecastError::Config(
                "quantile levels must increase strictly".into(),
            ));
        }
        Ok(())
    }
}

impl Default for ForecastConfig {
    fn default() -> Self {
        Self {
            serial_interval: SerialInterval::MEASLES,
            max_lag_weeks: Self::DEFAULT_MAX_LAG_WEEKS,
            renewal: RenewalConfig {
                window: Self::DEFAULT_WINDOW_WEEKS,
                ..RenewalConfig::default()
            },
            horizon_weeks: 8,
            run_count: 1_000,
            levels: HUB_QUANTILE_LEVELS.to_vec(),
        }
    }
}

/// Whether a projection was made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionStatus {
    Ok,
    /// No forecast: the reason is the renewal estimator's, see [`InsufficientReason`].
    InsufficientData(InsufficientReason),
}

/// A projection from one origin on a regular weekly grid.
#[derive(Debug, Clone, PartialEq)]
pub struct Projection {
    pub status: ProjectionStatus,
    /// Posterior of `R` over the window ending at the origin, when `status` is `Ok`.
    pub r_posterior: Option<GammaDist>,
    /// Cases summed over the window, when every count in it is known.
    pub cases_in_window: Option<u32>,
    /// `members[m][h]` is member `m`'s projected count `h + 1` weeks after the origin.
    /// Members are in index order; empty when `status` is not `Ok`.
    pub members: Vec<Vec<u64>>,
    /// Serial-interval mass dropped by the weekly discretisation (same week plus tail).
    pub dropped_si_mass: f64,
}

impl Projection {
    /// Projected counts of every member at horizon `h` (1-based), in member order.
    pub fn at_horizon(&self, h: u32) -> Vec<u64> {
        let idx = (h as usize).checked_sub(1).expect("horizon is 1-based");
        self.members.iter().map(|m| m[idx]).collect()
    }
}

/// Seed derivation v1 for a forecast member: SHA256(b"koplik-forecast-member-v1\0" ||
/// base_seed LE u64 || geography as its canonical FIPS string || 0u8 || member LE u32).
/// Including the geography keeps two geographies forecast under one base seed from
/// sharing their `R` and Poisson draws. No native-width values enter the hash.
pub fn derive_forecast_seed(base_seed: u64, geography: GeoId, member: u32) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"koplik-forecast-member-v1\0");
    hash.update(base_seed.to_le_bytes());
    hash.update(geography.to_string().as_bytes());
    hash.update([0u8]);
    hash.update(member.to_le_bytes());
    hash.finalize().into()
}

/// Project `counts` (a contiguous weekly grid whose last entry is the origin week, `None`
/// where a count is missing) `cfg.horizon_weeks` weeks ahead. `geography` only enters the
/// member seeds.
pub fn project_series(
    counts: &[Option<u32>],
    geography: GeoId,
    cfg: &ForecastConfig,
    seed: u64,
) -> Result<Projection, ForecastError> {
    cfg.validate()?;
    let si = cfg.serial_interval.discretize_weekly(cfg.max_lag_weeks)?;
    let insufficient = |reason, cases| Projection {
        status: ProjectionStatus::InsufficientData(reason),
        r_posterior: None,
        cases_in_window: cases,
        members: Vec::new(),
        dropped_si_mass: si.dropped_mass(),
    };
    if counts.is_empty() {
        return Ok(insufficient(InsufficientReason::IncompleteWindow, None));
    }
    let estimates = estimate_series(counts, &si, &cfg.renewal)?;
    let origin = estimates.last().expect("one estimate per step");
    let Some(posterior) = origin.posterior else {
        let reason = origin.reason.expect("insufficient carries a reason");
        return Ok(insufficient(reason, origin.cases_in_window));
    };
    let members = simulate_members(counts, &si, posterior, geography, cfg, seed)?;
    Ok(Projection {
        status: ProjectionStatus::Ok,
        r_posterior: Some(posterior),
        cases_in_window: origin.cases_in_window,
        members,
        dropped_si_mass: si.dropped_mass(),
    })
}

fn simulate_members(
    counts: &[Option<u32>],
    si: &DiscreteSerialInterval,
    posterior: GammaDist,
    geography: GeoId,
    cfg: &ForecastConfig,
    seed: u64,
) -> Result<Vec<Vec<u64>>, ForecastError> {
    let w = si.weights();
    let max_lag = w.len() - 1;
    // The last `max_lag` observed counts, oldest first. Every lag the projection reaches
    // was checked by the renewal estimator at the origin (its look-back covers
    // `origin - window + 1 - max_lag ..= origin - 1`, a superset of what horizon 1 needs,
    // and later horizons only reach into simulated weeks); a lag before the series is
    // therefore only reached under `BeforeSeries::Zero`, where it is zero.
    let recent: Vec<f64> = (0..max_lag)
        .map(|back| {
            counts
                .len()
                .checked_sub(max_lag - back)
                .and_then(|i| counts[i])
                .map_or(0.0, f64::from)
        })
        .collect();
    let horizon = cfg.horizon_weeks as usize;
    let mut members = Vec::with_capacity(cfg.run_count as usize);
    for m in 0..cfg.run_count {
        let mut rng = ChaCha8Rng::from_seed(derive_forecast_seed(seed, geography, m));
        // Inverse-transform draw of R from the gamma posterior: deterministic, libm-only.
        let r = posterior.quantile(uniform(&mut rng))?;
        let mut series = recent.clone();
        let mut member = Vec::with_capacity(horizon);
        for h in 0..horizon {
            let n = series.len();
            let mut lambda = 0.0_f64;
            for (k, wk) in w.iter().enumerate().skip(1) {
                lambda += wk * series[n - k];
            }
            let mean = r * lambda;
            if !(mean.is_finite() && mean <= MAX_PROJECTED_MEAN) {
                return Err(ForecastError::Overflow {
                    horizon: h as u32 + 1,
                    mean,
                });
            }
            let x = poisson(&mut rng, mean);
            member.push(x);
            series.push(x as f64);
        }
        members.push(member);
    }
    Ok(members)
}

/// Quantiles of a sample of counts at the given levels: linear interpolation at
/// `(n - 1) · p` (Hyndman & Fan 1996 type 7, the same rule as `crate::ensemble`). The
/// result has one entry per level in level order, so for strictly increasing levels the
/// values are non-decreasing, as the v1 `Forecast` contract requires.
pub fn sample_quantiles(values: &[u64], levels: &[f64]) -> Vec<ForecastQuantile> {
    assert!(!values.is_empty(), "quantiles of an empty sample");
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let n = u32::try_from(sorted.len()).expect("run_count is u32");
    levels
        .iter()
        .map(|&level| {
            let rank = f64::from(n - 1) * level;
            let lo = libm::floor(rank);
            let hi = libm::ceil(rank);
            let (vlo, vhi) = (sorted[lo as usize] as f64, sorted[hi as usize] as f64);
            ForecastQuantile {
                level,
                value: vlo + (vhi - vlo) * (rank - lo),
            }
        })
        .collect()
}

/// A forecast for one geography from one origin week.
#[derive(Debug, Clone, PartialEq)]
pub struct WeeklyForecast {
    pub geography: GeoId,
    pub origin_week: MmwrWeek,
    pub projection: Projection,
    /// v1 rows, one per horizon in horizon order; empty when the projection is
    /// `insufficient_data` (no number is published for an unforecastable origin).
    pub rows: Vec<Forecast>,
}

/// Forecast every geography in `rows` from `origin` (the last week of data allowed in).
///
/// Rows after `origin` are discarded first: that is the information cutoff. The remaining
/// rows are laid on a contiguous MMWR-week grid from the first week present to `origin`
/// (a week with no row is missing, like a row whose count is `missing`; a geography with
/// no row at `origin` is therefore `insufficient_data`). Two rows for one geography and
/// week are an error. As in `crate::rt::estimate_weekly`, each geography must keep one
/// case definition among the rows at or before the origin, which the forecast inherits
/// (rows after the origin are not consulted for this either). Output is in geography order; each
/// row's provenance is the union of the input rows' provenance in week order.
pub fn forecast_weekly<R: WeeklyCaseRow>(
    rows: &[R],
    origin: MmwrWeek,
    cfg: &ForecastConfig,
    seed: u64,
) -> Result<Vec<WeeklyForecast>, ForecastError> {
    cfg.validate()?;
    let mut by_geo: BTreeMap<GeoId, BTreeMap<MmwrWeek, &R>> = BTreeMap::new();
    let mut definitions: BTreeMap<GeoId, CaseDefinition> = BTreeMap::new();
    for row in rows {
        // Information cutoff: nothing after the origin week is looked at, not even its
        // case definition.
        if row.week() > origin {
            continue;
        }
        let found = *definitions
            .entry(row.geography())
            .or_insert(row.case_definition());
        if found != row.case_definition() {
            return Err(RtError::MixedCaseDefinition {
                geography: row.geography(),
            }
            .into());
        }
        if by_geo
            .entry(row.geography())
            .or_default()
            .insert(row.week(), row)
            .is_some()
        {
            return Err(RtError::DuplicateWeek {
                geography: row.geography(),
                week: row.week(),
            }
            .into());
        }
    }
    let mut out = Vec::new();
    for (geography, weeks) in &by_geo {
        let first = *weeks.keys().next().expect("group is non-empty");
        let mut counts = Vec::new();
        let mut week = first;
        loop {
            counts.push(weeks.get(&week).and_then(|r| r.cases().count()));
            if week == origin {
                break;
            }
            week = week.next()?;
        }
        let mut records: Vec<Provenance> = Vec::new();
        for row in weeks.values() {
            for p in row.provenance().as_slice() {
                if !records.contains(p) {
                    records.push(p.clone());
                }
            }
        }
        let provenance = Provenances::new(records)?;
        let projection = project_series(&counts, *geography, cfg, seed)?;
        let mut forecast_rows = Vec::new();
        if projection.status == ProjectionStatus::Ok {
            let mut target = origin;
            for h in 1..=cfg.horizon_weeks {
                target = target.next()?;
                forecast_rows.push(Forecast {
                    geography: *geography,
                    origin_week: origin,
                    target_week: target,
                    quantiles: sample_quantiles(&projection.at_horizon(h), &cfg.levels),
                    seed,
                    run_count: cfg.run_count,
                    provenance: provenance.clone(),
                });
            }
        }
        out.push(WeeklyForecast {
            geography: *geography,
            origin_week: origin,
            projection,
            rows: forecast_rows,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The default look-back is the smallest one with under 1% dropped serial-interval
    /// mass, as the field's documentation promises; the numbers quoted there are checked.
    #[test]
    fn default_max_lag_follows_rule() {
        let dropped = |weeks: u32| {
            SerialInterval::MEASLES
                .discretize_weekly(weeks)
                .unwrap()
                .dropped_mass()
        };
        let rule = (1..=8)
            .find(|&weeks| dropped(weeks) < 0.01)
            .expect("some lag qualifies");
        assert_eq!(rule, ForecastConfig::DEFAULT_MAX_LAG_WEEKS);
        assert!(
            (dropped(2) - 0.066).abs() < 0.002,
            "2 weeks: {}",
            dropped(2)
        );
        assert!(
            (dropped(3) - 0.0054).abs() < 0.0003,
            "3 weeks: {}",
            dropped(3)
        );
        assert!(
            (dropped(4) - 0.0045).abs() < 0.0003,
            "4 weeks: {}",
            dropped(4)
        );
    }

    #[test]
    fn quantiles_follow_type_7() {
        let q = sample_quantiles(&[1, 2, 3, 4, 5], &[0.1, 0.5, 0.9]);
        assert_eq!(q[0].value, 1.4);
        assert_eq!(q[1].value, 3.0);
        assert_eq!(q[2].value, 4.6);
        let q = sample_quantiles(&[7], &[0.01, 0.99]);
        assert_eq!((q[0].value, q[1].value), (7.0, 7.0));
    }

    #[test]
    fn config_validation() {
        let base = ForecastConfig::default();
        assert!(base.validate().is_ok());
        let mut c = base.clone();
        c.levels = vec![0.5, 0.5];
        assert!(c.validate().is_err());
        let mut c = base.clone();
        c.run_count = 1;
        assert!(c.validate().is_err());
        let mut c = base.clone();
        c.horizon_weeks = 0;
        assert!(c.validate().is_err());
        let mut c = base;
        c.levels = vec![0.0];
        assert!(c.validate().is_err());
    }
}
