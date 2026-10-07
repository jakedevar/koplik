//! Pseudo-real-time backtest of the forecast on weekly case series: **revised counts truncated
//! at each forecast date** (#1503).
//!
//! Where a source publishes no revision history and only one retrieval of it is held, the
//! report-vintage backtest of [`super::run`] is not possible. This module is the honest
//! substitute and is labelled as such everywhere it is reported: at each forecast date the
//! forecaster is given the rows of the one retrieved series up to the origin week and nothing
//! after ([`forecast_weekly`] discards later rows first). It is **not** real-time: if the
//! source rewrote earlier weeks when it republished, the retrieved series carries those later
//! revisions, which a forecaster at the time did not have.
//!
//! Protocol (pre-registered in `thoughts/shared/research/backtest-cdc-states.md` before any
//! score on the CDC NNDSS series was computed; nothing here is chosen from a score):
//!
//! 1. **Series**: every geography in the input, each under its own case definition. A geography
//!    contributes a forecast at an origin only where the method's own minimum-count rule holds
//!    (the same `insufficient_data` rule the published forecast applies); elsewhere it
//!    contributes nothing, and the report counts why.
//! 2. **Origins**: for every week `L` of the series with `L` at most the last week, the origin
//!    is `O = L - provisional_weeks` (the published origin rule: the latest week with data less
//!    the provisional weeks), from the first week of the series to the last `O` with `L`
//!    inside the retrieved data.
//! 3. **Targets**: horizons `1..=horizon_weeks`, target week `O + h`. The truth is the count of
//!    the target week in the retrieved series; a target is scored only when that count is
//!    `reported` (a missing week, including a cumulative that fell, is never zero) and the
//!    target week is at most the last week less `provisional_weeks`: the newest weeks of the
//!    retrieved series are themselves still provisional and are not used as truth.
//! 4. **Scores**: [`super::run::score_rows`], the one scoring shared with the report-vintage
//!    backtest (CRPS of the members, inclusive 50% and 90% interval coverage, absolute error of
//!    persistence), summarised as plain means with `n` per series, per horizon and pooled.
//! 5. **Measured skill**: a series has a measured skill only with at least
//!    [`SkillFloor::min_targets`] scored targets from at least [`SkillFloor::min_origin_weeks`]
//!    distinct origin weeks (the origins, not the 8 horizons of one origin, are the unit of
//!    independent evidence). Below either floor the series has insufficient data for a skill,
//!    whatever its scores. The same floors, applied to scored targets and to distinct
//!    (series, origin) forecasts, decide whether the pooled result is measured.
//!
//! Nothing is dropped, weighted or re-run on the basis of a score.

use std::collections::BTreeMap;

use koplik_contracts::v3::{CaseDefinition, GeoId, MmwrWeek, WeeklyCaseCount};
use serde::Serialize;

use super::run::{BacktestError, HorizonScore, ScoredQuantiles, Summary, score_rows, summarise};
use crate::forecast::{ForecastConfig, ForecastError, ProjectionStatus, forecast_weekly};
use crate::rt::InsufficientReason;

/// How much scored evidence makes a skill "measured".
///
/// Pre-registered (`thoughts/shared/research/backtest-cdc-states.md`) from reasoning, not from
/// any count or score of the series: 40 scored targets is about the size of the first backtest
/// (#1361, 48 targets, which its own report calls too small to rank methods), and 10 distinct
/// origin weeks because the 8 horizons of one origin are not independent evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SkillFloor {
    pub min_targets: u32,
    pub min_origin_weeks: u32,
}

impl SkillFloor {
    /// The pre-registered floors.
    pub const PRE_REGISTERED: SkillFloor = SkillFloor {
        min_targets: 40,
        min_origin_weeks: 10,
    };

    /// Whether `targets` scored targets from `origin_weeks` distinct origin weeks reach the floor.
    pub fn met(self, targets: u32, origin_weeks: u32) -> bool {
        targets >= self.min_targets && origin_weeks >= self.min_origin_weeks
    }
}

/// What a pseudo-real-time backtest runs.
#[derive(Debug, Clone, PartialEq)]
pub struct TruncatedConfig {
    pub forecast: ForecastConfig,
    /// Base seed of every forecast (members derive from it, the geography and the index).
    pub seed: u64,
    /// Recent weeks that are provisional (reporting delay): the origin is the latest week less
    /// this many, and the same many newest weeks are not used as truth. The site's rule.
    pub provisional_weeks: u32,
    pub floor: SkillFloor,
}

/// One forecast of one series from one origin week.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TruncatedOrigin {
    /// The latest week of data at the (notional) forecast date: the origin week plus the
    /// provisional weeks. Nothing after the origin week is read.
    pub latest_week: MmwrWeek,
    pub origin_week: MmwrWeek,
    /// The origin week's own count (the persistence forecast).
    pub origin_count: Option<u32>,
    /// Cases summed over the estimation window.
    pub cases_in_window: Option<u32>,
    pub r_mean: Option<f64>,
    pub r_lower_90: Option<f64>,
    pub r_upper_90: Option<f64>,
    pub scores: Vec<HorizonScore>,
}

impl TruncatedOrigin {
    /// Whether at least one target of this forecast was scored.
    pub fn is_scored(&self) -> bool {
        self.scores.iter().any(|s| s.crps.is_some())
    }
}

/// The backtest of one geography's series.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SeriesBacktest {
    pub geography: GeoId,
    pub case_definition: CaseDefinition,
    /// Origins considered (every origin week from the first week of the series).
    pub origins_considered: u32,
    /// Origins at which a forecast was made (the minimum-count rule held).
    pub origins_forecast: u32,
    /// Why the other origins made no forecast, with how many origins each: the renewal
    /// estimator's reason (`incomplete_window`, `missing_count`, `below_threshold`,
    /// `no_infectivity`), `projection_overflow` (the method refuses a projection that passes
    /// `MAX_PROJECTED_MEAN`) or `no_rows`.
    pub not_forecast: BTreeMap<String, u32>,
    /// Distinct origin weeks with at least one scored target.
    pub origin_weeks_scored: u32,
    /// Whether the series reaches the [`SkillFloor`]: `false` is "insufficient data for a
    /// measured skill", whatever the scores are.
    pub measured: bool,
    /// All horizons pooled; `horizon` is 0.
    pub pooled: Summary,
    pub by_horizon: Vec<Summary>,
    /// `pooled.mean_crps / pooled.mean_persistence_abs_error`: below 1 the forecast's mean
    /// error is smaller than carrying the origin week's count forward. `None` when either is
    /// missing or the persistence error is zero.
    pub crps_over_persistence: Option<f64>,
    /// The retrieved series the truth was read from: `(week, count)`; `None` is missing.
    pub truth: Vec<(MmwrWeek, Option<u32>)>,
    /// The forecasts that were made (origins where the rule held), in origin order.
    pub origins: Vec<TruncatedOrigin>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TruncatedReport {
    pub seed: u64,
    pub window_weeks: u32,
    pub max_lag_weeks: u32,
    pub min_cases: u32,
    pub horizon_weeks: u32,
    pub run_count: u32,
    pub provisional_weeks: u32,
    pub floor: SkillFloor,
    /// First and last week of the retrieved series.
    pub first_week: MmwrWeek,
    pub last_week: MmwrWeek,
    /// The newest week used as truth: the last week less the provisional weeks.
    pub last_truth_week: MmwrWeek,
    /// Every geography of the input, in geography order.
    pub series: Vec<SeriesBacktest>,
    /// Series that reach the floor.
    pub series_measured: u32,
    /// Series with at least one scored target.
    pub series_scored: u32,
    /// (series, origin) forecasts with at least one scored target.
    pub forecasts_scored: u32,
    /// Every series pooled, per horizon 1..=horizon_weeks.
    pub by_horizon: Vec<Summary>,
    /// Every series pooled, all horizons; `horizon` is 0. Weights each target equally, so
    /// the series with the largest counts dominate a score in cases.
    pub pooled: Summary,
    pub crps_over_persistence: Option<f64>,
    /// Whether the pooled result reaches the floor (scored targets, and distinct (series,
    /// origin) forecasts in place of origin weeks).
    pub pooled_measured: bool,
}

/// The estimator's reason as a stable name (the same names as the published companion's
/// `InsufficientReason`), so origins that did not forecast can be counted by reason.
fn reason_name(reason: InsufficientReason) -> &'static str {
    match reason {
        InsufficientReason::IncompleteWindow => "incomplete_window",
        InsufficientReason::MissingCount => "missing_count",
        InsufficientReason::BelowThreshold { .. } => "below_threshold",
        InsufficientReason::NoInfectivity => "no_infectivity",
    }
}

fn ratio(summary: &Summary) -> Option<f64> {
    match (summary.mean_crps, summary.mean_persistence_abs_error) {
        (Some(crps), Some(persistence)) if persistence > 0.0 => Some(crps / persistence),
        _ => None,
    }
}

fn back(week: MmwrWeek, weeks: u32) -> Result<MmwrWeek, ForecastError> {
    let mut w = week;
    for _ in 0..weeks {
        w = w.prev()?;
    }
    Ok(w)
}

/// Run the protocol above on `rows` (any number of geographies, each with one case
/// definition; a geography that mixes definitions is an error, as in the forecast).
pub fn run_truncated_backtest(
    rows: &[WeeklyCaseCount],
    cfg: &TruncatedConfig,
) -> Result<TruncatedReport, BacktestError> {
    cfg.forecast.validate()?;
    let scored = ScoredQuantiles::resolve(&cfg.forecast.levels)?;
    let first_week = rows
        .iter()
        .map(|r| r.week)
        .min()
        .ok_or_else(|| BacktestError::Config("no rows to backtest".into()))?;
    let last_week = rows.iter().map(|r| r.week).max().expect("non-empty");
    let last_truth_week = back(last_week, cfg.provisional_weeks)?;

    let mut by_geography: BTreeMap<GeoId, Vec<WeeklyCaseCount>> = BTreeMap::new();
    for row in rows {
        by_geography
            .entry(row.geography)
            .or_default()
            .push(row.clone());
    }

    let mut series = Vec::with_capacity(by_geography.len());
    for (geography, geo_rows) in &by_geography {
        let definition = geo_rows[0].case_definition;
        let mut truth: BTreeMap<MmwrWeek, Option<u32>> = BTreeMap::new();
        for r in geo_rows {
            if r.case_definition != definition {
                return Err(
                    ForecastError::from(crate::rt::RtError::MixedCaseDefinition {
                        geography: *geography,
                    })
                    .into(),
                );
            }
            truth.insert(r.week, r.cases.count());
        }
        // The truth a target is scored against: a reported count of a week that is no longer
        // provisional in the retrieved series.
        let observed_at = |week: MmwrWeek| -> Option<u32> {
            if week > last_truth_week {
                return None;
            }
            truth.get(&week).copied().flatten()
        };
        let mut origins = Vec::new();
        let mut not_forecast: BTreeMap<String, u32> = BTreeMap::new();
        let mut considered = 0_u32;
        let mut origin_week = first_week;
        // Origin weeks O with L = O + provisional_weeks inside the retrieved data.
        while origin_week <= last_truth_week {
            considered += 1;
            let latest_week = {
                let mut l = origin_week;
                for _ in 0..cfg.provisional_weeks {
                    l = l.next().map_err(ForecastError::from)?;
                }
                l
            };
            let forecasts = match forecast_weekly(geo_rows, origin_week, &cfg.forecast, cfg.seed) {
                Ok(forecasts) => forecasts,
                // The method refuses to publish a projection that explodes past any meaningful
                // case count (`MAX_PROJECTED_MEAN`): it makes no forecast there, so there is
                // nothing to score. Counted and reported, never dropped silently, and never
                // scored as zero. (Pre-registration amendment 1, before any score was seen.)
                Err(ForecastError::Overflow { .. }) => {
                    *not_forecast
                        .entry("projection_overflow".into())
                        .or_default() += 1;
                    origin_week = origin_week.next().map_err(ForecastError::from)?;
                    continue;
                }
                Err(e) => return Err(e.into()),
            };
            match forecasts.into_iter().next() {
                // No row of this series at or before the origin.
                None => *not_forecast.entry("no_rows".into()).or_default() += 1,
                Some(forecast) => match forecast.projection.status {
                    ProjectionStatus::InsufficientData(reason) => {
                        *not_forecast.entry(reason_name(reason).into()).or_default() += 1;
                    }
                    ProjectionStatus::Ok => {
                        let posterior = forecast
                            .projection
                            .r_posterior
                            .expect("ok carries a posterior");
                        let origin_count = truth.get(&origin_week).copied().flatten();
                        let scores = score_rows(&forecast, scored, origin_count, &observed_at)?;
                        origins.push(TruncatedOrigin {
                            latest_week,
                            origin_week,
                            origin_count,
                            cases_in_window: forecast.projection.cases_in_window,
                            r_mean: Some(posterior.mean()),
                            r_lower_90: Some(
                                posterior.quantile(0.05).map_err(ForecastError::from)?,
                            ),
                            r_upper_90: Some(
                                posterior.quantile(0.95).map_err(ForecastError::from)?,
                            ),
                            scores,
                        });
                    }
                },
            }
            origin_week = origin_week.next().map_err(ForecastError::from)?;
        }
        let all: Vec<&HorizonScore> = origins.iter().flat_map(|o| o.scores.iter()).collect();
        let by_horizon: Vec<Summary> = (1..=cfg.forecast.horizon_weeks)
            .map(|h| {
                let of_h: Vec<&HorizonScore> =
                    all.iter().copied().filter(|s| s.horizon == h).collect();
                summarise(h, &of_h)
            })
            .collect();
        let pooled = summarise(0, &all);
        let origin_weeks_scored = origins.iter().filter(|o| o.is_scored()).count() as u32;
        series.push(SeriesBacktest {
            geography: *geography,
            case_definition: definition,
            origins_considered: considered,
            origins_forecast: origins.len() as u32,
            not_forecast,
            origin_weeks_scored,
            measured: cfg.floor.met(pooled.n, origin_weeks_scored),
            crps_over_persistence: ratio(&pooled),
            pooled,
            by_horizon,
            truth: truth.into_iter().collect(),
            origins,
        });
    }

    let all: Vec<&HorizonScore> = series
        .iter()
        .flat_map(|s| s.origins.iter())
        .flat_map(|o| o.scores.iter())
        .collect();
    let by_horizon: Vec<Summary> = (1..=cfg.forecast.horizon_weeks)
        .map(|h| {
            let of_h: Vec<&HorizonScore> = all.iter().copied().filter(|s| s.horizon == h).collect();
            summarise(h, &of_h)
        })
        .collect();
    let pooled = summarise(0, &all);
    let forecasts_scored = series
        .iter()
        .flat_map(|s| s.origins.iter())
        .filter(|o| o.is_scored())
        .count() as u32;
    Ok(TruncatedReport {
        seed: cfg.seed,
        window_weeks: cfg.forecast.renewal.window,
        max_lag_weeks: cfg.forecast.max_lag_weeks,
        min_cases: cfg.forecast.renewal.min_cases,
        horizon_weeks: cfg.forecast.horizon_weeks,
        run_count: cfg.forecast.run_count,
        provisional_weeks: cfg.provisional_weeks,
        floor: cfg.floor,
        first_week,
        last_week,
        last_truth_week,
        series_measured: series.iter().filter(|s| s.measured).count() as u32,
        series_scored: series.iter().filter(|s| s.pooled.n > 0).count() as u32,
        forecasts_scored,
        crps_over_persistence: ratio(&pooled),
        pooled_measured: cfg.floor.met(pooled.n, forecasts_scored),
        series,
        by_horizon,
        pooled,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use koplik_contracts::v3::{CaseCount, Provenance, Provenances};

    fn provenance() -> Provenances {
        Provenances::one(Provenance {
            source_id: "test-source".to_owned(),
            url: "https://example.invalid/series".to_owned(),
            retrieved_at: "2026-10-07T00:00:00Z".parse().unwrap(),
            sha256: koplik_contracts::v3::Sha256Hex::new("0".repeat(64)).unwrap(),
            licence_id: "test-terms".to_owned(),
        })
    }

    fn row(geography: &str, week: MmwrWeek, cases: CaseCount) -> WeeklyCaseCount {
        WeeklyCaseCount {
            geography: geography.parse().unwrap(),
            week,
            cases,
            case_definition: CaseDefinition::ConfirmedOrUnknownStatus,
            provenance: provenance(),
        }
    }

    /// `weeks` weekly counts from 2025-W1, `count(w)` for 1-based week `w`.
    fn series(geography: &str, weeks: u32, count: impl Fn(u32) -> u32) -> Vec<WeeklyCaseCount> {
        let mut week = MmwrWeek::new(2025, 1).unwrap();
        let mut out = Vec::new();
        for w in 1..=weeks {
            out.push(row(
                geography,
                week,
                CaseCount::Reported { count: count(w) },
            ));
            week = week.next().unwrap();
        }
        out
    }

    fn cfg() -> TruncatedConfig {
        TruncatedConfig {
            forecast: ForecastConfig {
                run_count: 200,
                ..ForecastConfig::default()
            },
            seed: 7,
            provisional_weeks: 2,
            floor: SkillFloor::PRE_REGISTERED,
        }
    }

    /// A steady series a forecaster can reach the 11-case rule on from the first possible week.
    fn steady(geography: &str, weeks: u32) -> Vec<WeeklyCaseCount> {
        series(geography, weeks, |w| 8 + (w % 3))
    }

    #[test]
    fn the_origin_is_the_latest_week_less_the_provisional_weeks() {
        let report = run_truncated_backtest(&steady("12", 30), &cfg()).unwrap();
        let s = &report.series[0];
        assert!(!s.origins.is_empty());
        for o in &s.origins {
            let mut l = o.origin_week;
            for _ in 0..2 {
                l = l.next().unwrap();
            }
            assert_eq!(o.latest_week, l);
            assert!(o.latest_week <= report.last_week);
        }
        // Origins run to the last week less the provisional weeks, never beyond.
        assert_eq!(report.last_truth_week, MmwrWeek::new(2025, 28).unwrap());
        assert_eq!(
            s.origins.last().unwrap().origin_week,
            report.last_truth_week
        );
        assert_eq!(s.origins_considered, 28);
    }

    /// Nothing after the origin week reaches the forecast: changing every later count changes
    /// what the forecast is scored against, never the forecast itself.
    #[test]
    fn the_forecast_never_reads_later_weeks() {
        let a = steady("12", 30);
        let mut b = a.clone();
        let cut = MmwrWeek::new(2025, 14).unwrap();
        for r in &mut b {
            if r.week > cut {
                r.cases = CaseCount::Reported { count: 500 };
            }
        }
        let ra = run_truncated_backtest(&a, &cfg()).unwrap();
        let rb = run_truncated_backtest(&b, &cfg()).unwrap();
        let forecast_of = |r: &TruncatedReport, week: MmwrWeek| {
            r.series[0]
                .origins
                .iter()
                .find(|o| o.origin_week == week)
                .map(|o| {
                    (
                        o.r_mean,
                        o.scores
                            .iter()
                            .map(|s| (s.median, s.lower_90, s.upper_90, s.lower_50, s.upper_50))
                            .collect::<Vec<_>>(),
                    )
                })
        };
        for w in 8..=14 {
            let week = MmwrWeek::new(2025, w).unwrap();
            let (fa, fb) = (forecast_of(&ra, week), forecast_of(&rb, week));
            assert!(fa.is_some(), "an origin at week {w}");
            assert_eq!(fa, fb, "origin week {w} must not see weeks after it");
        }
        // The later weeks are the truth, so the scores of those origins do differ.
        let week = MmwrWeek::new(2025, 14).unwrap();
        let obs = |r: &TruncatedReport| {
            r.series[0]
                .origins
                .iter()
                .find(|o| o.origin_week == week)
                .unwrap()
                .scores[0]
                .observed
        };
        assert_ne!(obs(&ra), obs(&rb));
    }

    /// The provisional tail of the retrieved series is never truth, and a missing week is not
    /// scored (never read as zero).
    #[test]
    fn provisional_and_missing_targets_are_not_scored() {
        let mut rows = steady("12", 30);
        // A missing count in the middle of the series, and absurd provisional tail counts.
        rows[17].cases = CaseCount::Missing {
            reason: koplik_contracts::v3::MissingReason::Ambiguous,
        };
        rows[28].cases = CaseCount::Reported { count: 900 };
        rows[29].cases = CaseCount::Reported { count: 900 };
        let report = run_truncated_backtest(&rows, &cfg()).unwrap();
        let missing_week = rows[17].week;
        let mut saw_missing_target = false;
        for o in &report.series[0].origins {
            for s in &o.scores {
                if s.target_week > report.last_truth_week {
                    assert_eq!(s.observed, None, "provisional {}", s.target_week);
                    assert_eq!(s.crps, None);
                }
                if s.target_week == missing_week {
                    saw_missing_target = true;
                    assert_eq!(s.observed, None);
                    assert_eq!(s.crps, None);
                    assert_eq!(s.in_50, None);
                    assert_eq!(s.persistence_abs_error, None);
                }
            }
        }
        assert!(saw_missing_target);
        // 900 never entered a score.
        for o in &report.series[0].origins {
            assert!(o.scores.iter().all(|s| s.observed != Some(900)));
        }
    }

    #[test]
    fn a_series_below_the_minimum_count_makes_no_forecast_and_says_why() {
        let mut rows = steady("12", 30);
        rows.extend(series("13", 30, |_| 1));
        let report = run_truncated_backtest(&rows, &cfg()).unwrap();
        let thin = report
            .series
            .iter()
            .find(|s| s.geography.to_string() == "13")
            .unwrap();
        assert_eq!(thin.origins_forecast, 0);
        assert!(thin.origins.is_empty());
        assert_eq!(thin.pooled.n, 0);
        assert!(!thin.measured);
        assert_eq!(thin.pooled.mean_crps, None);
        assert_eq!(thin.crps_over_persistence, None);
        // Every considered origin is accounted for by a reason.
        assert_eq!(
            thin.not_forecast.values().sum::<u32>(),
            thin.origins_considered
        );
        assert!(thin.not_forecast.contains_key("below_threshold"));
    }

    /// The floor is on both scored targets and distinct origin weeks, and a series that
    /// reaches neither stays "insufficient data" however its scores look.
    #[test]
    fn the_floor_needs_both_enough_targets_and_enough_origin_weeks() {
        let floor = SkillFloor::PRE_REGISTERED;
        assert!(floor.met(40, 10));
        assert!(!floor.met(39, 10));
        assert!(!floor.met(400, 9));
        assert!(floor.met(41, 11));
        // A long steady series reaches it; a short one does not.
        let long = run_truncated_backtest(&steady("12", 60), &cfg()).unwrap();
        assert!(long.series[0].measured);
        assert!(long.series[0].pooled.n >= 40);
        assert!(long.series[0].origin_weeks_scored >= 10);
        let short = run_truncated_backtest(&steady("12", 14), &cfg()).unwrap();
        assert!(!short.series[0].measured);
        assert!(short.series[0].pooled.n > 0);
    }

    #[test]
    fn summaries_are_plain_means_over_scored_targets_and_the_pooled_adds_up() {
        let mut rows = steady("12", 40);
        rows.extend(steady("13", 40));
        let r = run_truncated_backtest(&rows, &cfg()).unwrap();
        let n: u32 = r.series.iter().map(|s| s.pooled.n).sum();
        assert_eq!(r.pooled.n, n);
        assert_eq!(r.by_horizon.iter().map(|h| h.n).sum::<u32>(), n);
        // A series' pooled CRPS is the mean of its scored targets' CRPS.
        let s = &r.series[0];
        let crps: Vec<f64> = s
            .origins
            .iter()
            .flat_map(|o| o.scores.iter())
            .filter_map(|x| x.crps)
            .collect();
        let mean = crps.iter().sum::<f64>() / crps.len() as f64;
        assert_eq!(s.pooled.mean_crps, Some(mean));
        assert_eq!(s.pooled.n as usize, crps.len());
        assert_eq!(r.series_scored, 2);
        assert_eq!(
            r.forecasts_scored,
            r.series
                .iter()
                .flat_map(|s| s.origins.iter())
                .filter(|o| o.is_scored())
                .count() as u32
        );
        for s in &r.series {
            assert_eq!(s.measured, s.pooled.n >= 40 && s.origin_weeks_scored >= 10);
        }
        assert_eq!(
            r.pooled_measured,
            r.pooled.n >= 40 && r.forecasts_scored >= 10
        );
        // The ratio is crps / persistence.
        let expect = r.pooled.mean_crps.unwrap() / r.pooled.mean_persistence_abs_error.unwrap();
        assert_eq!(r.crps_over_persistence, Some(expect));
    }

    #[test]
    fn the_same_seed_and_rows_give_an_identical_report() {
        let rows = steady("12", 36);
        let a = run_truncated_backtest(&rows, &cfg()).unwrap();
        let b = run_truncated_backtest(&rows, &cfg()).unwrap();
        assert_eq!(a, b);
        let mut other = cfg();
        other.seed = 8;
        let c = run_truncated_backtest(&rows, &other).unwrap();
        assert_ne!(a.series[0].pooled.mean_crps, c.series[0].pooled.mean_crps);
    }

    /// A burst of cases after weeks of almost none gives a posterior for `R` so large that the
    /// projection passes the method's refusal limit. That origin makes no forecast (it is counted
    /// as `projection_overflow`, never scored and never turned into a number); the backtest does
    /// not abort, and the other origins of the series are unaffected.
    #[test]
    fn an_exploding_projection_is_counted_as_no_forecast() {
        // One early case, then 40 in a single week: the window's look-back infectivity is
        // tiny, so R is in the hundreds and the projection explodes within the horizon.
        let rows = series("12", 24, |w| match w {
            8 => 1,
            13 => 40,
            _ => 0,
        });
        let report = run_truncated_backtest(&rows, &cfg()).unwrap();
        let s = &report.series[0];
        assert!(
            s.not_forecast
                .get("projection_overflow")
                .copied()
                .unwrap_or(0)
                >= 1,
            "{:?}",
            s.not_forecast
        );
        assert_eq!(
            s.not_forecast.values().sum::<u32>() + s.origins_forecast,
            s.origins_considered
        );
        assert!(s.origins.iter().all(|o| o.r_mean.is_some()));
    }

    #[test]
    fn mixed_case_definitions_in_a_series_are_an_error() {
        let mut rows = steady("12", 30);
        rows[3].case_definition = CaseDefinition::Confirmed;
        assert!(run_truncated_backtest(&rows, &cfg()).is_err());
        assert!(run_truncated_backtest(&[], &cfg()).is_err());
    }
}
