//! Run the forecast from every forecast date and score it against the final series.
//!
//! Protocol (fixed before any score was seen):
//! 1. For each forecast date `T`, the known series is [`weekly_from_vintages`] with
//!    `known_at = T`; the origin is its latest complete week.
//! 2. [`forecast_weekly`] projects `horizon_weeks` ahead from that origin with the
//!    pre-registered [`ForecastConfig`].
//! 3. The truth for a target week is the count in the series built from **every** version
//!    (`known_at = None`); a target with no reported truth is not scored.
//! 4. Per scored target: CRPS of the member ensemble ([`crps_counts`]), whether the 50% and
//!    90% central intervals of the published quantiles cover the truth, and the absolute
//!    error of the persistence baseline (the origin week's count carried forward: a point
//!    forecast, whose CRPS is its absolute error).
//! 5. Summaries are plain means over scored targets, per horizon and pooled, with `n`.
//!
//! Nothing is dropped, weighted or re-run on the basis of a score.

use chrono::{DateTime, Datelike, Duration, Utc, Weekday};
use koplik_contracts::v3::{CaseCount, GeoId, MmwrWeek};
use serde::Serialize;

use super::vintages::{ReportVintage, VintageError, weekly_from_vintages};
use super::{crps_counts, interval_covers};
use crate::forecast::{ForecastConfig, ForecastError, ProjectionStatus, forecast_weekly};

#[derive(Debug, thiserror::Error)]
pub enum BacktestError {
    #[error(transparent)]
    Vintage(#[from] VintageError),
    #[error(transparent)]
    Forecast(#[from] ForecastError),
    #[error("backtest needs at least one forecast date")]
    NoForecastDates,
}

/// What a backtest runs.
#[derive(Debug, Clone, PartialEq)]
pub struct BacktestConfig {
    pub forecast: ForecastConfig,
    /// Base seed of every forecast (members derive from it, the geography and the index).
    pub seed: u64,
    /// The geography the vintages describe.
    pub geography: GeoId,
    /// Forecast dates (information cutoffs), in order.
    pub forecast_dates: Vec<DateTime<Utc>>,
}

/// Every Wednesday 00:00 UTC in `[start, end]`: the pre-registered forecast-date rule.
/// Midweek, so a source that updates early in the week (Texas DSHS: Tuesdays and Fridays,
/// later Tuesdays) has usually made the previous MMWR week complete; what is actually known
/// is still decided by `first_seen_at`, never by the schedule.
pub fn wednesdays_between(start: DateTime<Utc>, end: DateTime<Utc>) -> Vec<DateTime<Utc>> {
    let mut day = start.date_naive();
    while day.weekday() != Weekday::Wed {
        day += Duration::days(1);
    }
    let mut out = Vec::new();
    loop {
        let t = day.and_hms_opt(0, 0, 0).expect("midnight").and_utc();
        if t < start {
            day += Duration::days(7);
            continue;
        }
        if t > end {
            break;
        }
        out.push(t);
        day += Duration::days(7);
    }
    out
}

/// One scored (or unscorable) target of one forecast.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HorizonScore {
    pub horizon: u32,
    pub target_week: MmwrWeek,
    /// The final count for the target week; `None` when it is missing, in which case
    /// nothing below is scored.
    pub observed: Option<u32>,
    pub median: f64,
    pub lower_50: f64,
    pub upper_50: f64,
    pub lower_90: f64,
    pub upper_90: f64,
    pub crps: Option<f64>,
    pub in_50: Option<bool>,
    pub in_90: Option<bool>,
    /// `|observed - origin week count|`.
    pub persistence_abs_error: Option<f64>,
}

/// The outcome at one forecast date.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OriginResult {
    pub forecast_date: DateTime<Utc>,
    pub known_versions: u32,
    /// Latest complete week at the forecast date; `None` when nothing was known.
    pub origin_week: Option<MmwrWeek>,
    /// `ok`, `insufficient_data` or `no_origin`.
    pub status: String,
    /// The renewal estimator's reason when `insufficient_data`.
    pub reason: Option<String>,
    pub cases_in_window: Option<u32>,
    pub r_mean: Option<f64>,
    pub r_lower_90: Option<f64>,
    pub r_upper_90: Option<f64>,
    /// The origin week's own count (the persistence forecast).
    pub origin_count: Option<u32>,
    pub scores: Vec<HorizonScore>,
}

/// Means over scored targets. `None` when `n == 0`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Summary {
    /// `0` for the pooled summary.
    pub horizon: u32,
    pub n: u32,
    pub mean_crps: Option<f64>,
    pub coverage_50: Option<f64>,
    pub coverage_90: Option<f64>,
    pub mean_persistence_abs_error: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BacktestReport {
    pub geography: GeoId,
    pub seed: u64,
    pub window_weeks: u32,
    pub max_lag_weeks: u32,
    pub min_cases: u32,
    pub horizon_weeks: u32,
    pub run_count: u32,
    pub versions: u32,
    /// The final series the truth was read from: `(week, count)`; missing weeks are absent.
    pub truth: Vec<(MmwrWeek, Option<u32>)>,
    pub origins: Vec<OriginResult>,
    pub by_horizon: Vec<Summary>,
    pub pooled: Summary,
}

fn summarise(horizon: u32, scores: &[&HorizonScore]) -> Summary {
    let scored: Vec<&HorizonScore> = scores
        .iter()
        .copied()
        .filter(|s| s.crps.is_some())
        .collect();
    let n = scored.len() as u32;
    let mean = |f: &dyn Fn(&HorizonScore) -> f64| {
        (n > 0).then(|| scored.iter().map(|s| f(s)).sum::<f64>() / f64::from(n))
    };
    let rate = |f: &dyn Fn(&HorizonScore) -> bool| {
        (n > 0).then(|| scored.iter().filter(|s| f(s)).count() as f64 / f64::from(n))
    };
    Summary {
        horizon,
        n,
        mean_crps: mean(&|s| s.crps.expect("scored")),
        coverage_50: rate(&|s| s.in_50.expect("scored")),
        coverage_90: rate(&|s| s.in_90.expect("scored")),
        mean_persistence_abs_error: mean(&|s| s.persistence_abs_error.expect("scored")),
    }
}

/// Run the protocol above.
pub fn run_backtest(
    vintages: &[ReportVintage],
    cfg: &BacktestConfig,
) -> Result<BacktestReport, BacktestError> {
    if cfg.forecast_dates.is_empty() {
        return Err(BacktestError::NoForecastDates);
    }
    cfg.forecast.validate()?;
    let truth_series = weekly_from_vintages(cfg.geography, vintages, None)?;
    let truth: Vec<(MmwrWeek, Option<u32>)> = truth_series
        .rows
        .iter()
        .map(|r| (r.week, r.cases.count()))
        .collect();
    let observed_at = |week: MmwrWeek| -> Option<u32> {
        truth.iter().find(|(w, _)| *w == week).and_then(|(_, c)| *c)
    };

    let mut origins = Vec::new();
    for &date in &cfg.forecast_dates {
        let known = weekly_from_vintages(cfg.geography, vintages, Some(date))?;
        let mut result = OriginResult {
            forecast_date: date,
            known_versions: known.known_versions,
            origin_week: known.last_complete_week,
            status: "no_origin".into(),
            reason: None,
            cases_in_window: None,
            r_mean: None,
            r_lower_90: None,
            r_upper_90: None,
            origin_count: None,
            scores: Vec::new(),
        };
        let Some(origin) = known.last_complete_week else {
            origins.push(result);
            continue;
        };
        result.origin_count =
            known
                .rows
                .iter()
                .find(|r| r.week == origin)
                .and_then(|r| match r.cases {
                    CaseCount::Reported { count } => Some(count),
                    CaseCount::Missing { .. } => None,
                });
        let forecasts = forecast_weekly(&known.rows, origin, &cfg.forecast, cfg.seed)?;
        let Some(forecast) = forecasts.into_iter().next() else {
            // No row at or before the origin (every known version is dated in the origin's
            // own week): nothing to forecast from.
            result.status = "insufficient_data".into();
            result.reason = Some("no complete week".into());
            origins.push(result);
            continue;
        };
        let projection = &forecast.projection;
        result.cases_in_window = projection.cases_in_window;
        match projection.status {
            ProjectionStatus::InsufficientData(reason) => {
                result.status = "insufficient_data".into();
                result.reason = Some(format!("{reason:?}"));
                origins.push(result);
                continue;
            }
            ProjectionStatus::Ok => result.status = "ok".into(),
        }
        let posterior = projection.r_posterior.expect("ok carries a posterior");
        result.r_mean = Some(posterior.mean());
        result.r_lower_90 = Some(posterior.quantile(0.05).map_err(ForecastError::from)?);
        result.r_upper_90 = Some(posterior.quantile(0.95).map_err(ForecastError::from)?);
        for (idx, row) in forecast.rows.iter().enumerate() {
            let horizon = idx as u32 + 1;
            let q = |level: f64| {
                row.quantiles
                    .iter()
                    .find(|q| (q.level - level).abs() < 1e-9)
                    .map(|q| q.value)
                    .expect("hub levels include the scored quantiles")
            };
            let observed = observed_at(row.target_week);
            let members = projection.at_horizon(horizon);
            result.scores.push(HorizonScore {
                horizon,
                target_week: row.target_week,
                observed,
                median: q(0.5),
                lower_50: q(0.25),
                upper_50: q(0.75),
                lower_90: q(0.05),
                upper_90: q(0.95),
                crps: observed.map(|y| crps_counts(&members, y)),
                in_50: observed.and_then(|y| interval_covers(&row.quantiles, 0.5, f64::from(y))),
                in_90: observed.and_then(|y| interval_covers(&row.quantiles, 0.9, f64::from(y))),
                persistence_abs_error: observed.and_then(|y| {
                    result
                        .origin_count
                        .map(|last| (f64::from(y) - f64::from(last)).abs())
                }),
            });
        }
        origins.push(result);
    }

    let all: Vec<&HorizonScore> = origins.iter().flat_map(|o| o.scores.iter()).collect();
    let by_horizon: Vec<Summary> = (1..=cfg.forecast.horizon_weeks)
        .map(|h| {
            let of_h: Vec<&HorizonScore> = all.iter().copied().filter(|s| s.horizon == h).collect();
            summarise(h, &of_h)
        })
        .collect();
    let pooled = summarise(0, &all);
    Ok(BacktestReport {
        geography: cfg.geography,
        seed: cfg.seed,
        window_weeks: cfg.forecast.renewal.window,
        max_lag_weeks: cfg.forecast.max_lag_weeks,
        min_cases: cfg.forecast.renewal.min_cases,
        horizon_weeks: cfg.forecast.horizon_weeks,
        run_count: cfg.forecast.run_count,
        versions: u32::try_from(vintages.len()).expect("fewer than 2^32 versions"),
        truth,
        origins,
        by_horizon,
        pooled,
    })
}
