//! The forecast stage (#1465): runs `koplik_epi::forecast::forecast_weekly` on the validated
//! weekly case series and writes v1 `Forecast` rows with a v5 `ForecastProvenance` companion.
//!
//! # The rule (stated before the first forecast was published)
//!
//! * **Method and parameters: the pre-registered defaults, unchanged.** [`forecast_config`] is
//!   `ForecastConfig::default()` (window 3 weeks, look-back 3 weeks, minimum 11 cases, 8 weeks
//!   ahead, 1,000 members, the 23 hub quantile levels); every default cites its source in
//!   `koplik_epi::forecast` and is repeated with its citation in the published companion. No
//!   value was chosen by looking at any score, here or in the backtest.
//! * **Seed.** [`FORECAST_SEED`], an explicit fixed constant, recorded on every row and in the
//!   companion. Members derive from it, the geography and the member index
//!   (`koplik_epi::forecast::derive_forecast_seed`), so two geographies never share draws.
//! * **Origin week.** The latest week with any row in the input, less the R_t module's
//!   provisional weeks ([`crate::rt_config`], two, spec E5): the most recent weeks are still
//!   being reported, and a forecast must not be anchored on a partial count. Nothing after the
//!   origin week is read (`forecast_weekly` discards it first).
//! * **Which series.** Every geography in the input, each under its own case definition; a
//!   geography whose rows mix definitions is an error, never mixed. A series is forecast only
//!   where the method's own minimum-count rule holds at the origin week (every count in the
//!   estimation window and the serial-interval look-back known, at least 11 cases in the window,
//!   some infectivity). Elsewhere the companion says `insufficient_data` and why, and no row is
//!   written: never an estimate. A series whose latest data is older than the origin week has a
//!   missing count in its window and so is `insufficient_data`.
//! * **Skill.** Two evaluations can be attached, each only from a committed report that was run with
//!   exactly this configuration. The report-vintage backtest scored one series (the Texas DSHS 2025
//!   outbreak total by report date; [`skill_from_report`], [`is_backtested`]). The pseudo-real-time
//!   backtest of the CDC NNDSS state series (#1503; [`series_backtest_from_report`]) scored every
//!   state series where the method's rule held and states, for each, whether its skill is measured
//!   (a floor of scored targets and origin weeks fixed before any score) or has insufficient data.
//!   A series gets a skill from an evaluation only through that evaluation's own entry for it
//!   ([`series_skill`]); every other series is published as `not backtested; no measured skill`,
//!   and no number is offered as evidence about a series it was not measured on.

use std::collections::{BTreeMap, BTreeSet};

use koplik_contracts::v1::{Forecast, GeoId, MmwrWeek, Sha256Hex};
use koplik_contracts::v3::{CaseDefinition, WeeklyCaseCount};
use koplik_contracts::v7::{
    BacktestSkill, FORECAST_PROVENANCE_VERSION, ForecastInput, ForecastProvenance, ForecastSeries,
    ForecastStatus, InformationBasis, InsufficientReason, MeasuredScores, ParameterProvenance,
    PooledScores, PublicationPolicy, SeriesBacktest, SeriesBacktestEntry, SeriesSkill,
    SkillByHorizon, WithheldReason,
};
use koplik_epi::forecast::{ForecastConfig, ForecastError, ProjectionStatus, forecast_weekly};
use koplik_epi::rt::{InsufficientReason as EpiReason, case_definitions};
use koplik_ingest::cdc;
use koplik_ingest::store::sha256_of;
use serde::Deserialize;
use serde_json::json;

use crate::{DSHS_REPORT_SOURCES, PipelineError, Result, rt_config};

/// Name of the series artifact that is forecast (`validate/weekly-cases.json`).
pub const ARTIFACT: &str = "weekly-cases";

/// Base seed of every published forecast. An arbitrary fixed constant chosen before any run (the
/// same one the backtest used, so the configuration that was scored is the one published); it
/// changes only the Monte Carlo draw, never the model, and is never adjusted to a result.
pub const FORECAST_SEED: u64 = 20_250_101;

/// Where the committed backtest report lives in the repository, as the companion cites it.
pub const BACKTEST_REPORT_PATH: &str = "data/reports/backtest/west-texas-2025.json";
/// The same report relative to the reports root (`Config::reports`).
pub const BACKTEST_REPORT_REL: &str = "backtest/west-texas-2025.json";

/// The pseudo-real-time NNDSS state-series report (#1503), as the companion cites it and relative
/// to the reports root.
pub use crate::nndss_backtest::{
    REPORT_PATH as SERIES_BACKTEST_REPORT_PATH, REPORT_REL as SERIES_BACKTEST_REPORT_REL,
};

/// The case definition of the backtested series: the Texas DSHS outbreak total counts confirmed
/// cases (`koplik_epi::backtest::vintages`; the report's scope says "confirmed cases").
const BACKTESTED_CASE_DEFINITION: CaseDefinition = CaseDefinition::Confirmed;

/// The backtested outbreak in a sentence ("In a backtest on the 2025 West Texas outbreak, ..."):
/// the committed report is `west-texas-2025.json`, the Texas DSHS 2025 outbreak.
const BACKTEST_NAME: &str = "the 2025 West Texas outbreak";

/// The configuration every forecast runs with: the pre-registered defaults, unchanged.
pub fn forecast_config() -> ForecastConfig {
    ForecastConfig::default()
}

/// The 90% interval coverage a series' measured skill must reach for its forecast to be published.
/// Fixed in `thoughts/shared/research/backtest-cdc-states.md` ("Publication policy") before the code
/// that applies it and never tuned: the nominal 0.90 less 0.15, the sampling noise of a coverage
/// estimate resting on about 10 independent origins (binomial standard error at 0.9 with 10 draws is
/// about 0.095; 1.5 of them is about 0.14).
pub const MINIMUM_COVERAGE_90: f64 = 0.75;
/// The most the measured mean CRPS may be, as a multiple of the persistence baseline's mean absolute
/// error (carrying the origin week's count forward): 1, "no worse than the number the reader already
/// has".
pub const MAXIMUM_CRPS_OVER_PERSISTENCE: f64 = 1.0;

/// The publication policy: a series' forecast is published only if its method has a measured skill on
/// that very series that meets the thresholds. Applied mechanically by [`build`], re-checked by the
/// contract's deserializer.
pub fn publication_policy() -> PublicationPolicy {
    PublicationPolicy {
        rule: format!(
            "A series' forecast is published only if its method has a measured skill on that very series (at least the evaluation's floor of scored targets and origin weeks) and that skill meets both criteria, pooled over the series' scored targets: 90% intervals contained the true count at least {:.0}% of the time (the nominal 90% less 15 points, the sampling noise of a coverage estimate resting on about 10 independent origins), and the mean CRPS is no more than {} times the mean absolute error of simply repeating the origin week's count. Otherwise the forecast is withheld and the page says why. The thresholds were fixed before the code that applies them and are never tuned to a score.",
            MINIMUM_COVERAGE_90 * 100.0,
            MAXIMUM_CRPS_OVER_PERSISTENCE
        ),
        minimum_coverage_90: MINIMUM_COVERAGE_90,
        maximum_crps_over_persistence: MAXIMUM_CRPS_OVER_PERSISTENCE,
    }
}

/// What [`build`] produces.
#[derive(Debug, Clone, PartialEq)]
pub struct Built {
    /// The rows that are published: only the series whose status is `forecast`.
    pub rows: Vec<Forecast>,
    /// The rows of the series the method forecast but the policy withheld. Kept for audit in the
    /// pipeline's work directory (`forecast/withheld.json`) and never published.
    pub withheld_rows: Vec<Forecast>,
    pub provenance: ForecastProvenance,
}

impl Built {
    /// Every forecast the method made, published or withheld, in geography then horizon order.
    pub fn all_rows(&self) -> Vec<Forecast> {
        let mut all: Vec<Forecast> = self
            .rows
            .iter()
            .chain(&self.withheld_rows)
            .cloned()
            .collect();
        all.sort_by_key(|r| (r.geography, r.target_week));
        all
    }
}

fn mmwr(e: koplik_contracts::v1::MmwrError) -> PipelineError {
    PipelineError::Forecast(koplik_epi::forecast::ForecastError::Mmwr(e))
}

fn reason(r: EpiReason) -> InsufficientReason {
    match r {
        EpiReason::IncompleteWindow => InsufficientReason::IncompleteWindow,
        EpiReason::MissingCount => InsufficientReason::MissingCount,
        EpiReason::BelowThreshold { .. } => InsufficientReason::BelowThreshold,
        EpiReason::NoInfectivity => InsufficientReason::NoInfectivity,
    }
}

/// Whether a series is the one the backtest scored: the same geography and case definition, and
/// every source record from the DSHS outbreak reports the backtest read.
pub fn is_backtested(
    skill: Option<&BacktestSkill>,
    geography: GeoId,
    definition: CaseDefinition,
    source_ids: &BTreeSet<&str>,
) -> bool {
    skill.is_some_and(|s| {
        s.geography == geography
            && s.case_definition == definition
            && !source_ids.is_empty()
            && source_ids.iter().all(|id| DSHS_REPORT_SOURCES.contains(id))
    })
}

/// Which evaluation, if any, measured a series' skill, and what it found. The report-vintage
/// backtest speaks only for the series it scored ([`is_backtested`]); the series backtest only
/// through its own entry for the series, and only for a series made entirely of NNDSS rows under
/// the case definition it scored. Anything else is not backtested.
pub fn series_skill(
    skill: Option<&BacktestSkill>,
    series_backtest: Option<&SeriesBacktest>,
    geography: GeoId,
    definition: CaseDefinition,
    source_ids: &BTreeSet<&str>,
) -> SeriesSkill {
    if is_backtested(skill, geography, definition, source_ids) {
        return SeriesSkill::Backtested;
    }
    let from_nndss = !source_ids.is_empty() && source_ids.iter().all(|id| *id == cdc::SOURCE_ID);
    if let Some(b) = series_backtest
        && from_nndss
        && definition == b.case_definition
        && let Some(entry) = b.by_series.iter().find(|e| e.geography == geography)
    {
        return if entry.measured.is_some() {
            SeriesSkill::Measured
        } else {
            SeriesSkill::InsufficientData
        };
    }
    SeriesSkill::NotBacktested
}

/// The measured scores `(coverage_90, mean CRPS, persistence mean absolute error)` a series' skill
/// rests on, when it has a measured skill.
fn measured_for(
    skill_status: SeriesSkill,
    skill: Option<&BacktestSkill>,
    series_backtest: Option<&SeriesBacktest>,
    geography: GeoId,
) -> Option<(f64, f64, f64)> {
    match skill_status {
        SeriesSkill::Backtested => {
            skill.map(|b| (b.coverage_90, b.mean_crps, b.mean_persistence_abs_error))
        }
        SeriesSkill::Measured => series_backtest
            .and_then(|b| b.by_series.iter().find(|e| e.geography == geography))
            .and_then(|e| e.measured.as_ref())
            .map(|m| (m.coverage_90, m.mean_crps, m.mean_persistence_abs_error)),
        SeriesSkill::InsufficientData | SeriesSkill::NotBacktested => None,
    }
}

/// Apply the publication policy to a series the method forecast: published, or withheld with the
/// reason its skill gives. Mechanical: nothing here looks at anything but the measured scores.
fn decide(
    policy: &PublicationPolicy,
    skill_status: SeriesSkill,
    scores: Option<(f64, f64, f64)>,
) -> (ForecastStatus, Option<WithheldReason>) {
    // Only a measured skill can be admitted: scores of any other series are never looked at.
    if matches!(
        skill_status,
        SeriesSkill::Measured | SeriesSkill::Backtested
    ) && let Some((coverage_90, mean_crps, persistence)) = scores
        && policy.admits(coverage_90, mean_crps, persistence)
    {
        return (ForecastStatus::Forecast, None);
    }
    let why = match skill_status {
        SeriesSkill::NotBacktested => WithheldReason::NotBacktested,
        SeriesSkill::InsufficientData => WithheldReason::InsufficientDataForSkill,
        SeriesSkill::Measured | SeriesSkill::Backtested => WithheldReason::SkillBelowPolicy,
    };
    (ForecastStatus::Withheld, Some(why))
}

/// One configuration value with the value this forecast ran with and its citation.
fn parameter(
    name: &str,
    value: serde_json::Value,
    source: &str,
    url: Option<&str>,
    note: &str,
) -> ParameterProvenance {
    ParameterProvenance {
        parameter: name.to_owned(),
        value,
        source: source.to_owned(),
        url: url.map(str::to_owned),
        note: note.to_owned(),
    }
}

/// Every configuration value, read from `cfg` (never retyped), with its source. The sources are
/// the ones cited in `koplik_epi::forecast` and `koplik_epi::rt`.
pub fn parameters(
    cfg: &ForecastConfig,
    seed: u64,
    provisional_weeks: u32,
) -> Vec<ParameterProvenance> {
    vec![
        parameter(
            "window_weeks",
            json!(cfg.renewal.window),
            "This project's pre-registered rule, fixed before any backtest score (koplik_epi::forecast::ForecastConfig::DEFAULT_WINDOW_WEEKS)",
            None,
            "Three weeks is about 1.8 mean measles serial intervals, so the window spans at least one generation of infectors and infectees while staying short enough to follow a change within a month. Not taken from the cited paper; never varied against a score (the backtest report lists windows 1, 2 and 4 beside it).",
        ),
        parameter(
            "max_lag_weeks",
            json!(cfg.max_lag_weeks),
            "This project's pre-registered rule: the smallest look-back whose dropped serial-interval mass is under 1%, the tolerance koplik_epi::rt documents for its own truncation",
            None,
            "Measured for the measles serial interval: 6.6% dropped with a 2-week look-back, 0.54% with 3. A longer look-back would postpone the first week a forecast can be made from a series that starts mid-outbreak.",
        ),
        parameter(
            "min_cases",
            json!(cfg.renewal.min_cases),
            "Cori A, Ferguson NM, Fraser C, Cauchemez S. Am J Epidemiol 2013;178(9):1505-1512, Web Appendix 2; EpiEstim estimate_R.R: ceiling(1 / cv_posterior^2 - a_prior) with cv_posterior 0.3",
            Some("https://doi.org/10.1093/aje/kwt133"),
            "Fewer cases than this in the estimation window gives insufficient_data and no forecast, never a prior-driven number.",
        ),
        parameter(
            "prior_mean",
            json!(cfg.renewal.prior.mean()),
            "EpiEstim make_config.R default mean_prior = 5, as in Cori et al. 2013",
            Some("https://doi.org/10.1093/aje/kwt133"),
            "Gamma prior on the reproduction number R; with at least min_cases cases in the window the data dominate it.",
        ),
        parameter(
            "prior_sd",
            json!(cfg.renewal.prior.sd()),
            "EpiEstim make_config.R default std_prior = 5, as in Cori et al. 2013",
            Some("https://doi.org/10.1093/aje/kwt133"),
            "Gamma prior on the reproduction number R.",
        ),
        parameter(
            "serial_interval_mean_days",
            json!(cfg.serial_interval.mean_days),
            "CDC, Nowcasting to Estimate Real-Time Measles Transmission Trends, MMWR 2026;75(33) (mm7533a1), citing Klinkenberg D, Nishiura H. J Theor Biol 2011;284:52-60; Vink MA, Bootsma MCJ, Wallinga J. Am J Epidemiol 2014;180(9):865-875",
            Some("https://www.cdc.gov/mmwr/volumes/75/wr/mm7533a1.htm"),
            "Gamma-distributed serial interval, discretised to MMWR weeks. CDC's figure is a generation interval; the serial interval is used as its observable proxy, with the same mean.",
        ),
        parameter(
            "serial_interval_sd_days",
            json!(cfg.serial_interval.sd_days),
            "CDC MMWR 2026;75(33): variance of 9 days squared",
            Some("https://www.cdc.gov/mmwr/volumes/75/wr/mm7533a1.htm"),
            "Standard deviation of the gamma serial interval.",
        ),
        parameter(
            "horizon_weeks",
            json!(cfg.horizon_weeks),
            "Koplik spec, Principle question 4: a 4-8 week outbreak forecast",
            None,
            "The far end of the stated range, so every horizon from 1 to 8 is published.",
        ),
        parameter(
            "run_count",
            json!(cfg.run_count),
            "Koplik spec E4: a 1,000-run simulated ensemble",
            None,
            "The 1% and 99% quantiles rest on about ten members each.",
        ),
        parameter(
            "quantile_levels",
            json!(cfg.levels),
            "The 23 quantile levels of the CDC FluSight / hubverse format; Bracher J, Ray EL, Gneiting T, Reich NG. PLoS Comput Biol 2021;17(2):e1008618",
            Some("https://doi.org/10.1371/journal.pcbi.1008618"),
            "Includes the 50% (0.25, 0.75) and 90% (0.05, 0.95) central intervals the backtest scored. Quantiles are Hyndman-Fan type 7 of the member counts.",
        ),
        parameter(
            "seed",
            json!(seed),
            "An arbitrary fixed constant chosen before any run; the backtest's base seed",
            None,
            "Changes only the Monte Carlo draw, never the model. Members derive their own seeds from it, the geography and the member index.",
        ),
        parameter(
            "provisional_weeks",
            json!(provisional_weeks),
            "Koplik spec E5: the most recent two weeks are provisional (reporting delay); the same rule as the published R_t",
            None,
            "These weeks are not read: the origin week is the latest week with a row, less this many weeks.",
        ),
    ]
}

/// The origin week: the latest week with any row, less the provisional weeks.
fn origin_of(latest: MmwrWeek, provisional_weeks: u32) -> Result<MmwrWeek> {
    let mut origin = latest;
    for _ in 0..provisional_weeks {
        origin = origin.prev().map_err(mmwr)?;
    }
    Ok(origin)
}

/// Forecast `cases` and write the companion. `None` when there are no rows at all (nothing to
/// forecast and no origin to name).
pub fn build(
    cases: &[WeeklyCaseCount],
    input: ForecastInput,
    skill: Option<BacktestSkill>,
    series_backtest: Option<SeriesBacktest>,
) -> Result<Option<Built>> {
    let cfg = forecast_config();
    cfg.validate().map_err(PipelineError::Forecast)?;
    let provisional_weeks = rt_config().provisional_weeks;
    // One definition per geography, or an error: a series is never mixed.
    let definitions = case_definitions(cases)?;
    let Some(latest) = cases.iter().map(|r| r.week).max() else {
        return Ok(None);
    };
    let origin = origin_of(latest, provisional_weeks)?;
    let policy = publication_policy();
    let mut sources: BTreeMap<GeoId, BTreeSet<&str>> = BTreeMap::new();
    let mut by_series_rows: BTreeMap<GeoId, Vec<WeeklyCaseCount>> = BTreeMap::new();
    for row in cases {
        sources.entry(row.geography).or_default().extend(
            row.provenance
                .as_slice()
                .iter()
                .map(|p| p.source_id.as_str()),
        );
        by_series_rows
            .entry(row.geography)
            .or_default()
            .push(row.clone());
    }

    let mut rows = Vec::new();
    let mut withheld_rows = Vec::new();
    let mut series = Vec::new();
    for (geography, definition) in &definitions {
        let skill_status = series_skill(
            skill.as_ref(),
            series_backtest.as_ref(),
            *geography,
            *definition,
            sources.get(geography).unwrap_or(&BTreeSet::new()),
        );
        // One series at a time, so a projection the method refuses (it grew past the limit) is that
        // series' own `projection_overflow` and never aborts the others (#1513). A series' forecast
        // does not depend on what else is in the call: members derive from its geography.
        let own = by_series_rows
            .get(geography)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let forecast = match forecast_weekly(own, origin, &cfg, FORECAST_SEED) {
            Ok(forecasts) => forecasts.into_iter().next(),
            Err(ForecastError::Overflow { .. }) => {
                series.push(ForecastSeries {
                    geography: *geography,
                    case_definition: *definition,
                    status: ForecastStatus::InsufficientData,
                    reason: Some(InsufficientReason::ProjectionOverflow),
                    withheld: None,
                    cases_in_window: None,
                    skill: skill_status,
                });
                continue;
            }
            Err(e) => return Err(PipelineError::Forecast(e)),
        };
        let entry = match forecast {
            Some(f) => match f.projection.status {
                ProjectionStatus::Ok => {
                    let scores = measured_for(
                        skill_status,
                        skill.as_ref(),
                        series_backtest.as_ref(),
                        *geography,
                    );
                    let (status, withheld) = decide(&policy, skill_status, scores);
                    if status == ForecastStatus::Forecast {
                        rows.extend(f.rows.iter().cloned());
                    } else {
                        withheld_rows.extend(f.rows.iter().cloned());
                    }
                    ForecastSeries {
                        geography: *geography,
                        case_definition: *definition,
                        status,
                        reason: None,
                        withheld,
                        cases_in_window: f.projection.cases_in_window,
                        skill: skill_status,
                    }
                }
                ProjectionStatus::InsufficientData(why) => ForecastSeries {
                    geography: *geography,
                    case_definition: *definition,
                    status: ForecastStatus::InsufficientData,
                    reason: Some(reason(why)),
                    withheld: None,
                    cases_in_window: f.projection.cases_in_window,
                    skill: skill_status,
                },
            },
            // Every row of this series is after the origin week: nothing at or before it.
            None => ForecastSeries {
                geography: *geography,
                case_definition: *definition,
                status: ForecastStatus::InsufficientData,
                reason: Some(InsufficientReason::IncompleteWindow),
                withheld: None,
                cases_in_window: None,
                skill: skill_status,
            },
        };
        series.push(entry);
    }

    let scope_note = scope_note(skill.as_ref(), series_backtest.as_ref(), &series);
    let provenance = ForecastProvenance {
        contract_version: FORECAST_PROVENANCE_VERSION,
        artifact: ARTIFACT.to_owned(),
        statement: format!(
            "Model projections from reported counts, not predictions of what will happen. Each of {} ensemble members draws one reproduction number from its posterior over the last {} complete weeks and projects weekly counts {} weeks ahead with Poisson noise, holding that number constant; the bands are quantiles of the members. A series with fewer than {} cases in those weeks is not forecast. A series' forecast is published only if the method has a measured skill on that very series that meets the publication policy; otherwise it is withheld and says why. Every forecast describes cases under its own case definition and never mixes definitions.",
            cfg.run_count, cfg.renewal.window, cfg.horizon_weeks, cfg.renewal.min_cases
        ),
        method: format!(
            "Renewal-equation projection (Nouvellet P, Cori A, Garske T, et al. A simple approach to measure transmissibility and forecast incidence. Epidemics 2018;22:29-35, doi:10.1016/j.epidem.2017.02.012; RECON projections package). R is the Cori et al. 2013 posterior over the last {} weeks ending at the origin week; each member's counts follow Poisson(R times the infectivity from the measles serial interval), observed counts before the origin and the member's own after it. There is no overdispersion, importation, seasonality or vaccination in this model.",
            cfg.renewal.window
        ),
        origin_week: origin,
        latest_data_week: latest,
        origin_rule: format!(
            "The origin week is the latest week with a row in the input (MMWR {latest}) less its {provisional_weeks} most recent weeks, which are provisional because of reporting delay (the same rule as the published R_t, spec E5). Nothing after the origin week is read."
        ),
        horizon_weeks: cfg.horizon_weeks,
        run_count: cfg.run_count,
        seed: FORECAST_SEED,
        levels: cfg.levels.clone(),
        input,
        parameters: parameters(&cfg, FORECAST_SEED, provisional_weeks),
        publication_policy: policy,
        series,
        scope_note,
        backtest: skill,
        series_backtest,
    };
    Ok(Some(Built {
        rows,
        withheld_rows,
        provenance,
    }))
}

/// What the attached evaluations and the publication policy do and do not say about the series the
/// method forecast. A series an evaluation did not score has no measured skill from it, and its
/// scores are not offered as evidence about any other series.
fn scope_note(
    skill: Option<&BacktestSkill>,
    series_backtest: Option<&SeriesBacktest>,
    series: &[ForecastSeries],
) -> String {
    let made: Vec<&ForecastSeries> = series
        .iter()
        .filter(|s| s.status != ForecastStatus::InsufficientData)
        .collect();
    let published = made
        .iter()
        .filter(|s| s.status == ForecastStatus::Forecast)
        .count();
    let withheld = |why: WithheldReason| made.iter().filter(|s| s.withheld == Some(why)).count();
    let n = made.len();
    if skill.is_none() && series_backtest.is_none() {
        return format!(
            "No backtest report for exactly this configuration is attached, so no skill has been measured and no forecast can meet the publication policy: {n} series the method forecast are withheld, none is published."
        );
    }
    if n == 0 {
        return "The method made no forecast for any series, so there is no forecast for an evaluation or the publication policy to speak to."
            .to_owned();
    }
    let mut parts = vec![format!(
        "Of the {n} series the method forecast, {published} {} published and {} withheld by the publication policy ({} with a measured skill below it, {} with insufficient data for a skill, {} not backtested). A series' forecast is published only if its own measured skill meets the policy; no other series' number is evidence about it.",
        if published == 1 { "is" } else { "are" },
        n - published,
        withheld(WithheldReason::SkillBelowPolicy),
        withheld(WithheldReason::InsufficientDataForSkill),
        withheld(WithheldReason::NotBacktested),
    )];
    if let Some(b) = series_backtest {
        parts.push(format!(
            "The backtest of {} is {}; a measured skill needs at least {} scored targets from at least {} origin weeks, a floor fixed before any score.",
            b.name,
            match b.basis {
                InformationBasis::PseudoRealTime => "pseudo-real-time (revised counts truncated at each forecast date), not real-time",
                InformationBasis::RealTimeByVintage => "real-time by report vintage",
            },
            b.minimum_targets,
            b.minimum_origin_weeks
        ));
    }
    if let Some(skill) = skill {
        parts.push(format!(
            "The report-vintage backtest scored one series only: {}; its scores are reported separately and are not a measure of any other series.",
            skill.series
        ));
    }
    parts.join(" ")
}

/// A share as a percentage with exactly one decimal (`62.5%`), the one precision the page and the
/// companion use. Half-up on the thousandths, the same arithmetic as `web/src/forecast.ts`.
pub fn percent(share: f64) -> String {
    let tenths = (share * 1000.0).round() as u64;
    format!("{}.{}%", tenths / 10, tenths % 10)
}

/// `" (30 of 48)"` when `share` is a whole number of `targets` (as a measured coverage is), else
/// empty.
pub fn out_of(share: f64, targets: u32) -> String {
    let count = share * f64::from(targets);
    if (count - count.round()).abs() < 1e-6 {
        format!(" ({} of {targets})", count.round() as u64)
    } else {
        String::new()
    }
}

// ---------------------------------------------------------------------------------------------
// The committed backtest report

#[derive(Deserialize)]
struct ReportSummary {
    horizon: u32,
    n: u32,
    mean_crps: Option<f64>,
    coverage_50: Option<f64>,
    coverage_90: Option<f64>,
    mean_persistence_abs_error: Option<f64>,
}

#[derive(Deserialize)]
struct ReportScore {
    /// The count the forecast was scored against; absent when the target was not reported.
    observed: Option<u32>,
    /// Present exactly when the target was scored.
    crps: Option<f64>,
}

#[derive(Deserialize)]
struct ReportOrigin {
    origin_week: Option<MmwrWeek>,
    status: String,
    #[serde(default)]
    scores: Vec<ReportScore>,
}

#[derive(Deserialize)]
struct ReportPrimary {
    geography: GeoId,
    seed: u64,
    window_weeks: u32,
    max_lag_weeks: u32,
    min_cases: u32,
    horizon_weeks: u32,
    run_count: u32,
    origins: Vec<ReportOrigin>,
    by_horizon: Vec<ReportSummary>,
    pooled: ReportSummary,
}

#[derive(Deserialize)]
struct Report {
    manifest_sha256: String,
    protocol: String,
    scope: String,
    primary: ReportPrimary,
}

/// The skill the committed backtest report measured, read exactly as written. `Err` carries the
/// reason no skill is attached: a report that does not parse, or that was run with a different
/// configuration than the one being published (its numbers would describe another method).
pub fn skill_from_report(
    bytes: &[u8],
    cfg: &ForecastConfig,
) -> std::result::Result<BacktestSkill, String> {
    let report: Report =
        serde_json::from_slice(bytes).map_err(|e| format!("the report does not parse: {e}"))?;
    let p = &report.primary;
    let ran_with = (
        p.window_weeks,
        p.max_lag_weeks,
        p.min_cases,
        p.horizon_weeks,
        p.run_count,
    );
    let publishing = (
        cfg.renewal.window,
        cfg.max_lag_weeks,
        cfg.renewal.min_cases,
        cfg.horizon_weeks,
        cfg.run_count,
    );
    if ran_with != publishing {
        return Err(format!(
            "the report was run with (window, look-back, min cases, horizon, members) = {ran_with:?}, not the {publishing:?} being published"
        ));
    }
    if p.seed != FORECAST_SEED {
        return Err(format!(
            "the report's seed {} is not the published seed {FORECAST_SEED}",
            p.seed
        ));
    }
    let need = |name: &str, v: Option<f64>| {
        v.ok_or_else(|| format!("the report's pooled {name} is empty"))
    };
    let by_horizon = p
        .by_horizon
        .iter()
        .map(|h| SkillByHorizon {
            horizon: h.horizon,
            n: h.n,
            mean_crps: h.mean_crps,
            coverage_50: h.coverage_50,
            coverage_90: h.coverage_90,
        })
        .collect();
    let made: Vec<&ReportOrigin> = p.origins.iter().filter(|o| o.status == "ok").collect();
    let origin_weeks: BTreeSet<MmwrWeek> = made.iter().filter_map(|o| o.origin_week).collect();
    // The range of the counts the forecasts were scored against: the scored targets only, not the
    // whole history (which includes weeks no forecast was scored on).
    let scored: Vec<u32> = made
        .iter()
        .flat_map(|o| o.scores.iter())
        .filter(|s| s.crps.is_some())
        .filter_map(|s| s.observed)
        .collect();
    if scored.len() != p.pooled.n as usize {
        return Err(format!(
            "the report lists {} scored targets but its pooled summary says {}",
            scored.len(),
            p.pooled.n
        ));
    }
    let (lowest, highest) = (
        scored.iter().min().copied().unwrap_or(0),
        scored.iter().max().copied().unwrap_or(0),
    );
    let coverage_50 = need("coverage_50", p.pooled.coverage_50)?;
    let coverage_90 = need("coverage_90", p.pooled.coverage_90)?;
    let limitations = vec![
        format!(
            "One series and one period: {}. This evaluation scored nothing else: no other outbreak and no county series (the CDC NNDSS state series have their own, separate evaluation).",
            report.scope
        ),
        format!(
            "A small sample: {} scored targets from {} forecast dates, which come from {} distinct origin weeks (a forecast date whose latest complete week had not advanced repeats the previous forecast, so those origins count twice).",
            p.pooled.n,
            made.len(),
            origin_weeks.len()
        ),
        format!(
            "Small counts: the {} weekly counts the forecasts were scored against ran from {lowest} to {highest} cases. The score is in cases, so it depends on the size of the series and is not comparable with a series of a different size.",
            scored.len()
        ),
        format!(
            "In this backtest the intervals were not well calibrated: 90% intervals contained the observed count {}{} of the time (nominal 90%) and 50% intervals {}{} (nominal 50%). The model has no overdispersion, which under-states the spread of real measles clusters.",
            percent(coverage_90),
            out_of(coverage_90, p.pooled.n),
            percent(coverage_50),
            out_of(coverage_50, p.pooled.n)
        ),
        "Counts are by report date, not symptom onset: they say when the source published the cases.".to_owned(),
    ];
    Ok(BacktestSkill {
        name: BACKTEST_NAME.to_owned(),
        // The first clause of the report's scope names the series; the rest (what was not
        // scored) is kept in full in the first limitation.
        series: report
            .scope
            .split(';')
            .next()
            .unwrap_or(&report.scope)
            .trim()
            .to_owned(),
        geography: p.geography,
        case_definition: BACKTESTED_CASE_DEFINITION,
        protocol: report.protocol.clone(),
        seed: p.seed,
        targets: p.pooled.n,
        forecast_dates: u32::try_from(made.len())
            .map_err(|_| "too many forecast dates".to_owned())?,
        origin_weeks: u32::try_from(origin_weeks.len())
            .map_err(|_| "too many origin weeks".to_owned())?,
        mean_crps: need("mean_crps", p.pooled.mean_crps)?,
        coverage_50,
        coverage_90,
        mean_persistence_abs_error: need(
            "mean_persistence_abs_error",
            p.pooled.mean_persistence_abs_error,
        )?,
        by_horizon,
        report_path: BACKTEST_REPORT_PATH.to_owned(),
        report_sha256: sha256_of(bytes),
        manifest_sha256: Sha256Hex::new(report.manifest_sha256).map_err(|e| e.to_string())?,
        limitations,
    })
}

// ---------------------------------------------------------------------------------------------
// The committed NNDSS state-series backtest report (#1503)

#[derive(Deserialize)]
struct StateReportSeries {
    geography: GeoId,
    origins_forecast: u32,
    origin_weeks_scored: u32,
    measured: bool,
    pooled: ReportSummary,
    by_horizon: Vec<ReportSummary>,
    not_forecast: BTreeMap<String, u32>,
}

#[derive(Deserialize)]
struct StateReportFloor {
    min_targets: u32,
    min_origin_weeks: u32,
}

#[derive(Deserialize)]
struct StateReportPrimary {
    seed: u64,
    window_weeks: u32,
    max_lag_weeks: u32,
    min_cases: u32,
    horizon_weeks: u32,
    run_count: u32,
    provisional_weeks: u32,
    floor: StateReportFloor,
    series: Vec<StateReportSeries>,
    series_scored: u32,
    forecasts_scored: u32,
    by_horizon: Vec<ReportSummary>,
    pooled: ReportSummary,
    pooled_measured: bool,
}

#[derive(Deserialize)]
struct StateReportInput {
    source_id: String,
    sha256: String,
}

#[derive(Deserialize)]
struct StateReport {
    input: StateReportInput,
    protocol: String,
    scope: String,
    primary: StateReportPrimary,
}

/// The label a pseudo-real-time backtest must carry (spec E5): a report without it is refused, so
/// a truncation of revised counts is never published as a real-time score.
const PSEUDO_REAL_TIME: &str = "pseudo-real-time (revised counts truncated at each forecast date)";

/// The name of the NNDSS state-series evaluation in a sentence ("the backtest of ...").
const SERIES_BACKTEST_NAME: &str = "the CDC NNDSS state series";

fn measured_scores(
    summary: &ReportSummary,
    by_horizon: &[ReportSummary],
) -> std::result::Result<MeasuredScores, String> {
    let need = |name: &str, v: Option<f64>| {
        v.ok_or_else(|| {
            format!(
                "a measured summary of {} targets lacks its {name}",
                summary.n
            )
        })
    };
    Ok(MeasuredScores {
        targets: summary.n,
        mean_crps: need("mean_crps", summary.mean_crps)?,
        coverage_50: need("coverage_50", summary.coverage_50)?,
        coverage_90: need("coverage_90", summary.coverage_90)?,
        mean_persistence_abs_error: need(
            "mean_persistence_abs_error",
            summary.mean_persistence_abs_error,
        )?,
        by_horizon: by_horizon
            .iter()
            .map(|h| SkillByHorizon {
                horizon: h.horizon,
                n: h.n,
                mean_crps: h.mean_crps,
                coverage_50: h.coverage_50,
                coverage_90: h.coverage_90,
            })
            .collect(),
    })
}

/// The evaluation the committed NNDSS state-series report measured, read exactly as written, or
/// the reason none is attached: the report does not parse, is not pseudo-real-time-labelled, or
/// was run with a different configuration than the one being published (its numbers would describe
/// another method).
pub fn series_backtest_from_report(
    bytes: &[u8],
    cfg: &ForecastConfig,
    provisional_weeks: u32,
) -> std::result::Result<SeriesBacktest, String> {
    let report: StateReport =
        serde_json::from_slice(bytes).map_err(|e| format!("the report does not parse: {e}"))?;
    let p = &report.primary;
    let ran_with = (
        p.window_weeks,
        p.max_lag_weeks,
        p.min_cases,
        p.horizon_weeks,
        p.run_count,
        p.provisional_weeks,
    );
    let publishing = (
        cfg.renewal.window,
        cfg.max_lag_weeks,
        cfg.renewal.min_cases,
        cfg.horizon_weeks,
        cfg.run_count,
        provisional_weeks,
    );
    if ran_with != publishing {
        return Err(format!(
            "the report was run with (window, look-back, min cases, horizon, members, provisional weeks) = {ran_with:?}, not the {publishing:?} being published"
        ));
    }
    if p.seed != FORECAST_SEED {
        return Err(format!(
            "the report's seed {} is not the published seed {FORECAST_SEED}",
            p.seed
        ));
    }
    if report.input.source_id != cdc::SOURCE_ID {
        return Err(format!(
            "the report was run on {}, not {}",
            report.input.source_id,
            cdc::SOURCE_ID
        ));
    }
    if !report.protocol.starts_with(PSEUDO_REAL_TIME) {
        return Err(format!(
            "the report's protocol does not carry the label \"{PSEUDO_REAL_TIME}\""
        ));
    }
    let mut entries = Vec::with_capacity(p.series.len());
    for s in &p.series {
        let reached =
            p.floor.min_targets <= s.pooled.n && p.floor.min_origin_weeks <= s.origin_weeks_scored;
        if reached != s.measured {
            return Err(format!(
                "series {}: the report's measured flag disagrees with its own floor",
                s.geography
            ));
        }
        entries.push(SeriesBacktestEntry {
            geography: s.geography,
            forecasts: s.origins_forecast,
            origin_weeks: s.origin_weeks_scored,
            targets: s.pooled.n,
            measured: if s.measured {
                Some(measured_scores(&s.pooled, &s.by_horizon)?)
            } else {
                None
            },
        });
    }
    let targets: u64 = p.series.iter().map(|s| u64::from(s.pooled.n)).sum();
    if targets != u64::from(p.pooled.n) {
        return Err(format!(
            "the report lists {targets} scored targets in its series but its pooled summary says {}",
            p.pooled.n
        ));
    }
    let pooled = if p.pooled_measured {
        Some(PooledScores {
            forecasts: p.forecasts_scored,
            series: p.series_scored,
            scores: measured_scores(&p.pooled, &p.by_horizon)?,
        })
    } else {
        None
    };

    let considered = p.series.len();
    let scored = p.series_scored;
    let measured = p.series.iter().filter(|s| s.measured).count();
    let forecasts: u32 = p.series.iter().map(|s| s.origins_forecast).sum();
    let refused: u32 = p
        .series
        .iter()
        .map(|s| {
            s.not_forecast
                .get("projection_overflow")
                .copied()
                .unwrap_or(0)
        })
        .sum();
    let origin_weeks = |s: &StateReportSeries| s.origin_weeks_scored;
    let most = p.series.iter().map(origin_weeks).max().unwrap_or(0);
    let least = p
        .series
        .iter()
        .map(origin_weeks)
        .filter(|n| *n > 0)
        .min()
        .unwrap_or(0);
    let limitations = {
        let mut list = vec![
            format!(
                "Scope: {}.",
                report.scope
            ),
            format!(
                "{PSEUDO_REAL_TIME}: CDC publishes no revision history and one retrieval of this source is held, so this is not a real-time backtest. Each forecast was given only the weeks up to its origin week, from the retrieved series; if CDC rewrote earlier weeks when it republished, a forecaster at the time saw different counts, and the effect on these scores cannot be measured from one retrieval."
            ),
            format!(
                "Only the series where the method's own minimum-count rule held were forecast and scored: {scored} of the {considered} state, territory and DC series ever were ({forecasts} forecasts), so the other {} are insufficient data for any skill. Only {measured} of the {scored} reach the floor of {} scored targets from {} origin weeks; the rest have insufficient data for a measured skill and no number is given for them.",
                considered as u32 - scored,
                p.floor.min_targets,
                p.floor.min_origin_weeks
            ),
            format!(
                "A small and correlated sample: the {} scored targets come from {} forecasts; the 8 horizons of one origin and neighbouring origins share data, so independent evidence is nearer the {least} to {most} origin weeks per scored series than the targets. No interval is given for any score; small differences between series mean nothing.",
                p.pooled.n, p.forecasts_scored
            ),
            "Mean CRPS is in cases and the forecast has no ceiling on growth, so it is dominated by the few forecasts that grew most and grows with the horizon by orders of magnitude; compare it with the persistence column of the same row, not across series of different size.".to_owned(),
            format!(
                "Scores are conditional on the method making a forecast: {refused} origin(s) where the projection passed the method's limit were refused and are not scored, and they are the origins where the method would have been furthest off."
            ),
            "Counts are by CDC report week (the growth of the published cumulative), not symptom onset, and combine confirmed and unknown-status cases.".to_owned(),
        ];
        if let (Some(c50), Some(c90)) = (p.pooled.coverage_50, p.pooled.coverage_90) {
            list.insert(
                3,
                format!(
                    "In this backtest the intervals were not well calibrated: pooled over every scored target, 90% intervals contained the observed count {}{} of the time (nominal 90%) and 50% intervals {}{} (nominal 50%). The model has no overdispersion and holds its growth rate constant over the horizon.",
                    percent(c90),
                    out_of(c90, p.pooled.n),
                    percent(c50),
                    out_of(c50, p.pooled.n)
                ),
            );
        }
        list
    };
    Ok(SeriesBacktest {
        name: SERIES_BACKTEST_NAME.to_owned(),
        series: report
            .scope
            .split(';')
            .next()
            .unwrap_or(&report.scope)
            .trim()
            .to_owned(),
        case_definition: CaseDefinition::ConfirmedOrUnknownStatus,
        basis: InformationBasis::PseudoRealTime,
        protocol: report.protocol.clone(),
        seed: p.seed,
        provisional_weeks: p.provisional_weeks,
        minimum_targets: p.floor.min_targets,
        minimum_origin_weeks: p.floor.min_origin_weeks,
        pooled,
        by_series: entries,
        report_path: SERIES_BACKTEST_REPORT_PATH.to_owned(),
        report_sha256: sha256_of(bytes),
        input_sha256: Sha256Hex::new(report.input.sha256).map_err(|e| e.to_string())?,
        limitations,
    })
}

/// The companion's input record for the series file read as `bytes`.
pub fn input_of(bytes: &[u8], rows: usize) -> ForecastInput {
    ForecastInput {
        artifact: ARTIFACT.to_owned(),
        sha256: sha256_of(bytes),
        rows: rows as u64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use koplik_contracts::v1::{Provenance, Provenances};
    use koplik_contracts::v3::CaseCount;

    fn provenance(source: &str) -> Provenances {
        Provenances::one(Provenance {
            source_id: source.to_owned(),
            url: "https://example.invalid/series".to_owned(),
            retrieved_at: "2026-10-07T02:19:30Z".parse().unwrap(),
            sha256: sha256_of(source.as_bytes()),
            licence_id: "test-terms".to_owned(),
        })
    }

    fn row(
        geography: &str,
        week: u8,
        count: u32,
        definition: CaseDefinition,
        source: &str,
    ) -> WeeklyCaseCount {
        WeeklyCaseCount {
            geography: geography.parse().unwrap(),
            week: MmwrWeek::new(2026, week).unwrap(),
            cases: CaseCount::Reported { count },
            case_definition: definition,
            provenance: provenance(source),
        }
    }

    /// Weeks 1..=last of a series counting `base` to `base + 2` cases a week.
    fn series(geography: &str, last: u8, base: u32) -> Vec<WeeklyCaseCount> {
        (1..=last)
            .map(|w| {
                row(
                    geography,
                    w,
                    base + u32::from(w % 3),
                    CaseDefinition::ConfirmedOrUnknownStatus,
                    "cdc-nndss-weekly-measles",
                )
            })
            .collect()
    }

    fn input() -> ForecastInput {
        input_of(b"weekly cases", 0)
    }

    fn built(cases: &[WeeklyCaseCount]) -> Built {
        build(cases, input(), None, None).unwrap().unwrap()
    }

    #[test]
    fn the_published_configuration_is_the_pre_registered_default() {
        assert_eq!(forecast_config(), ForecastConfig::default());
        let c = forecast_config();
        assert_eq!(
            (
                c.renewal.window,
                c.max_lag_weeks,
                c.renewal.min_cases,
                c.horizon_weeks,
                c.run_count
            ),
            (3, 3, 11, 8, 1000)
        );
    }

    #[test]
    fn the_origin_is_the_latest_week_less_the_provisional_weeks_and_nothing_later_is_read() {
        let mut cases = series("12", 20, 10);
        let a = built(&cases);
        assert_eq!(
            a.provenance.latest_data_week,
            MmwrWeek::new(2026, 20).unwrap()
        );
        assert_eq!(a.provenance.origin_week, MmwrWeek::new(2026, 18).unwrap());
        let (rows_a, rows_b) = (a.all_rows(), {
            let mut later = cases.clone();
            later[18].cases = CaseCount::Reported { count: 900 };
            later[19].cases = CaseCount::Reported { count: 0 };
            built(&later).all_rows()
        });
        assert_eq!(rows_a.len(), 8);
        assert_eq!(rows_a[0].target_week, MmwrWeek::new(2026, 19).unwrap());
        assert_eq!(rows_a[7].target_week, MmwrWeek::new(2026, 26).unwrap());
        // The two provisional weeks can be anything: they do not reach the forecast.
        cases[18].cases = CaseCount::Reported { count: 900 };
        cases[19].cases = CaseCount::Reported { count: 0 };
        let b = built(&cases);
        assert_eq!(rows_a, rows_b);
        assert_eq!(rows_b, b.all_rows());
        assert_eq!(a.provenance.series, b.provenance.series);
    }

    #[test]
    fn a_forecast_row_states_its_seed_members_provenance_and_ordered_quantiles() {
        let b = built(&series("12", 20, 10));
        assert!(!b.all_rows().is_empty());
        for r in &b.all_rows() {
            assert_eq!(r.seed, FORECAST_SEED);
            assert_eq!(r.run_count, 1000);
            assert_eq!(r.quantiles.len(), 23);
            assert!(r.quantiles.windows(2).all(|w| w[0].value <= w[1].value));
            assert_eq!(
                r.provenance.as_slice()[0].source_id,
                "cdc-nndss-weekly-measles"
            );
        }
        b.provenance.check_against(&b.rows).unwrap();
    }

    #[test]
    fn a_series_below_the_minimum_count_is_insufficient_data_with_no_row() {
        // One case a week: 3 in the window, far below the 11 the rule needs.
        let mut cases = series("12", 20, 10);
        cases.extend((1..=20).map(|w| {
            row(
                "13",
                w,
                1,
                CaseDefinition::ConfirmedOrUnknownStatus,
                "cdc-nndss-weekly-measles",
            )
        }));
        let b = built(&cases);
        let thin = b
            .provenance
            .series
            .iter()
            .find(|s| s.geography.to_string() == "13")
            .unwrap();
        assert_eq!(thin.status, ForecastStatus::InsufficientData);
        assert_eq!(thin.reason, Some(InsufficientReason::BelowThreshold));
        assert_eq!(thin.cases_in_window, Some(3));
        assert!(b.all_rows().iter().all(|r| r.geography.to_string() == "12"));
        assert!(!b.all_rows().is_empty());
        // Each series the method forecast (published or withheld) met the rule.
        for s in b
            .provenance
            .series
            .iter()
            .filter(|s| s.status != ForecastStatus::InsufficientData)
        {
            assert!(s.cases_in_window.unwrap() >= 11);
        }
        b.provenance.check_against(&b.rows).unwrap();
    }

    #[test]
    fn a_series_that_stopped_before_the_origin_has_a_missing_count_so_no_forecast() {
        let mut cases = series("12", 20, 10);
        cases.extend(series("13", 14, 10)); // last data in week 14, origin is week 18
        let b = built(&cases);
        let stale = b
            .provenance
            .series
            .iter()
            .find(|s| s.geography.to_string() == "13")
            .unwrap();
        assert_eq!(stale.status, ForecastStatus::InsufficientData);
        assert_eq!(stale.reason, Some(InsufficientReason::MissingCount));
        assert!(b.all_rows().iter().all(|r| r.geography.to_string() == "12"));
    }

    #[test]
    fn a_series_that_only_starts_after_the_origin_is_insufficient_not_dropped() {
        let mut cases = series("12", 20, 10);
        cases.push(row(
            "13",
            20,
            12,
            CaseDefinition::ConfirmedOrUnknownStatus,
            "x",
        ));
        let b = built(&cases);
        let late = b
            .provenance
            .series
            .iter()
            .find(|s| s.geography.to_string() == "13")
            .unwrap();
        assert_eq!(late.status, ForecastStatus::InsufficientData);
        assert_eq!(late.reason, Some(InsufficientReason::IncompleteWindow));
        assert_eq!(late.cases_in_window, None);
    }

    #[test]
    fn case_definitions_are_never_mixed() {
        let mut cases = series("12", 20, 10);
        cases[3].case_definition = CaseDefinition::Confirmed;
        assert!(build(&cases, input(), None, None).is_err());
        // Two geographies may each have their own definition; each is forecast under it.
        let mut cases = series("12", 20, 10);
        cases.extend(series("13", 20, 10).into_iter().map(|mut r| {
            r.case_definition = CaseDefinition::Confirmed;
            r
        }));
        let b = built(&cases);
        let definitions: Vec<_> = b
            .provenance
            .series
            .iter()
            .map(|s| s.case_definition)
            .collect();
        assert_eq!(
            definitions,
            [
                CaseDefinition::ConfirmedOrUnknownStatus,
                CaseDefinition::Confirmed
            ]
        );
    }

    #[test]
    fn no_rows_means_nothing_to_forecast() {
        assert!(build(&[], input(), None, None).unwrap().is_none());
    }

    #[test]
    fn the_same_input_gives_the_same_bytes() {
        let cases = series("12", 20, 10);
        let (a, b) = (built(&cases), built(&cases));
        assert_eq!(
            serde_json::to_vec(&a.all_rows()).unwrap(),
            serde_json::to_vec(&b.all_rows()).unwrap()
        );
        assert_eq!(
            serde_json::to_vec(&a.provenance).unwrap(),
            serde_json::to_vec(&b.provenance).unwrap()
        );
    }

    #[test]
    fn every_parameter_is_read_from_the_configuration_and_cited() {
        let cfg = forecast_config();
        let p = parameters(&cfg, FORECAST_SEED, 2);
        let value = |name: &str| {
            p.iter()
                .find(|x| x.parameter == name)
                .unwrap()
                .value
                .clone()
        };
        assert_eq!(value("window_weeks"), json!(3));
        assert_eq!(value("max_lag_weeks"), json!(3));
        assert_eq!(value("min_cases"), json!(11));
        assert_eq!(value("prior_mean"), json!(5.0));
        assert_eq!(value("prior_sd"), json!(5.0));
        assert_eq!(value("serial_interval_mean_days"), json!(11.7));
        assert_eq!(value("serial_interval_sd_days"), json!(3.0));
        assert_eq!(value("horizon_weeks"), json!(8));
        assert_eq!(value("run_count"), json!(1000));
        assert_eq!(value("seed"), json!(FORECAST_SEED));
        assert_eq!(value("provisional_weeks"), json!(2));
        assert_eq!(value("quantile_levels"), json!(cfg.levels));
        assert!(
            p.iter()
                .all(|x| !x.source.trim().is_empty() && !x.note.trim().is_empty())
        );
    }

    fn skill_json(edit: impl FnOnce(&mut serde_json::Value)) -> Vec<u8> {
        let mut v = json!({
            "manifest_sha256": "00ce05dd4730969da533bf1de684f07db8ae9485785912d28a9007f4f2b90a25",
            "protocol": "real-time by report vintage",
            "scope": "Texas DSHS outbreak total (all outbreak-associated confirmed cases), weekly by report date",
            "primary": {
                "geography": "48", "seed": FORECAST_SEED, "window_weeks": 3, "max_lag_weeks": 3,
                "min_cases": 11, "horizon_weeks": 8, "run_count": 1000,
                "truth": [[{"year": 2025, "week": 11}, 61], [{"year": 2025, "week": 12}, null], [{"year": 2025, "week": 13}, 0]],
                "origins": [
                    {"origin_week": {"year": 2025, "week": 21}, "status": "ok", "scores": [
                        {"observed": 3, "crps": 1.0}, {"observed": null, "crps": null}]},
                    {"origin_week": {"year": 2025, "week": 21}, "status": "ok", "scores": [
                        {"observed": 7, "crps": 1.0}]},
                    {"origin_week": null, "status": "no_origin", "scores": []}
                ],
                "by_horizon": [
                    {"horizon": 1, "n": 2, "mean_crps": 1.0, "coverage_50": 0.5, "coverage_90": 1.0, "mean_persistence_abs_error": 2.0}
                ],
                "pooled": {"horizon": 0, "n": 2, "mean_crps": 1.0, "coverage_50": 0.5, "coverage_90": 1.0, "mean_persistence_abs_error": 2.0}
            }
        });
        edit(&mut v);
        serde_json::to_vec(&v).unwrap()
    }

    #[test]
    fn skill_is_read_exactly_as_the_report_measured_it() {
        let bytes = skill_json(|_| {});
        let s = skill_from_report(&bytes, &forecast_config()).unwrap();
        assert_eq!((s.targets, s.forecast_dates, s.origin_weeks), (2, 2, 1));
        assert_eq!((s.mean_crps, s.coverage_50, s.coverage_90), (1.0, 0.5, 1.0));
        assert_eq!(s.report_sha256, sha256_of(&bytes));
        assert_eq!(s.case_definition, CaseDefinition::Confirmed);
        assert_eq!(s.geography.to_string(), "48");
        // The range is of the scored targets (3 and 7), never of the whole history (61, 0).
        let limitations = s.limitations.join("\n");
        assert!(limitations.contains("from 3 to 7 cases"), "{limitations}");
        assert!(!limitations.contains("61"), "{limitations}");
        // One precision, with counts, everywhere.
        assert!(limitations.contains(
            "100.0% (2 of 2) of the time (nominal 90%) and 50% intervals 50.0% (1 of 2)"
        ));
    }

    #[test]
    fn percentages_have_one_decimal_and_counts_when_whole() {
        assert_eq!(percent(0.625), "62.5%");
        assert_eq!(percent(30.0 / 48.0), "62.5%");
        assert_eq!(percent(23.0 / 48.0), "47.9%");
        assert_eq!(percent(0.5), "50.0%");
        assert_eq!(percent(1.0), "100.0%");
        assert_eq!(percent(0.0), "0.0%");
        assert_eq!(out_of(30.0 / 48.0, 48), " (30 of 48)");
        assert_eq!(out_of(0.5, 2), " (1 of 2)");
        assert_eq!(out_of(0.4, 3), "");
    }

    #[test]
    fn a_report_whose_scored_targets_disagree_with_its_summary_is_refused() {
        let e = skill_from_report(
            &skill_json(|v| v["primary"]["pooled"]["n"] = json!(3)),
            &forecast_config(),
        )
        .unwrap_err();
        assert!(e.contains("scored targets"), "{e}");
    }

    #[test]
    fn skill_is_not_attached_from_a_report_run_with_another_configuration() {
        for edit in [
            (|v: &mut serde_json::Value| v["primary"]["window_weeks"] = json!(2)) as fn(&mut _),
            |v| v["primary"]["min_cases"] = json!(5),
            |v| v["primary"]["run_count"] = json!(500),
            |v| v["primary"]["horizon_weeks"] = json!(4),
            |v| v["primary"]["max_lag_weeks"] = json!(8),
            |v| v["primary"]["seed"] = json!(1),
        ] {
            let e = skill_from_report(&skill_json(edit), &forecast_config()).unwrap_err();
            assert!(e.contains("report"), "{e}");
        }
        assert!(skill_from_report(b"not json", &forecast_config()).is_err());
        let e = skill_from_report(
            &skill_json(|v| v["primary"]["pooled"]["mean_crps"] = serde_json::Value::Null),
            &forecast_config(),
        )
        .unwrap_err();
        assert!(e.contains("mean_crps"), "{e}");
    }

    #[test]
    fn only_the_scored_series_is_marked_backtested() {
        let skill = skill_from_report(&skill_json(|_| {}), &forecast_config()).unwrap();
        let geography: GeoId = "48".parse().unwrap();
        let dshs: BTreeSet<&str> = ["dshs-measles-data-report-wayback"].into();
        let cdc: BTreeSet<&str> = ["cdc-nndss-weekly-measles"].into();
        let confirmed = CaseDefinition::Confirmed;
        assert!(is_backtested(Some(&skill), geography, confirmed, &dshs));
        // Same geography and definition from another source is another series.
        assert!(!is_backtested(Some(&skill), geography, confirmed, &cdc));
        assert!(!is_backtested(
            Some(&skill),
            geography,
            CaseDefinition::ConfirmedOrUnknownStatus,
            &dshs
        ));
        assert!(!is_backtested(
            Some(&skill),
            "12".parse().unwrap(),
            confirmed,
            &dshs
        ));
        assert!(!is_backtested(None, geography, confirmed, &dshs));
        assert!(!is_backtested(
            Some(&skill),
            geography,
            confirmed,
            &BTreeSet::new()
        ));
    }

    #[test]
    fn the_scope_note_says_what_each_evaluation_does_and_does_not_speak_to() {
        let skill = skill_from_report(&skill_json(|_| {}), &forecast_config()).unwrap();
        let mut cases = series("48", 20, 10);
        cases.extend(series("12", 20, 10));
        let b = build(&cases, input(), Some(skill), None).unwrap().unwrap();
        assert!(
            b.provenance
                .series
                .iter()
                .all(|s| s.skill == SeriesSkill::NotBacktested)
        );
        let note = &b.provenance.scope_note;
        assert!(
            note.contains("0 are published and 2 withheld by the publication policy"),
            "{note}"
        );
        assert!(note.contains("2 not backtested"), "{note}");
        assert!(note.contains("scored one series only"), "{note}");
        assert!(b.provenance.backtest.is_some());
        let none = build(&cases, input(), None, None).unwrap().unwrap();
        assert!(none.provenance.backtest.is_none());
        assert!(
            none.provenance
                .scope_note
                .contains("no skill has been measured")
        );
        assert!(none.rows.is_empty());
    }

    #[test]
    fn a_forecast_without_a_measured_skill_is_withheld_never_published() {
        let b = built(&series("12", 20, 10));
        let s = &b.provenance.series[0];
        assert_eq!(s.status, ForecastStatus::Withheld);
        assert_eq!(s.withheld, Some(WithheldReason::NotBacktested));
        assert_eq!(s.reason, None);
        assert_eq!(
            s.cases_in_window,
            Some(b.provenance.series[0].cases_in_window.unwrap())
        );
        // The forecast was made and is kept for audit; nothing of it is published.
        assert!(b.rows.is_empty());
        assert_eq!(b.withheld_rows.len(), 8);
        b.provenance.check_against(&b.rows).unwrap();
        // A companion that withholds a series does not describe that series' rows.
        assert!(b.provenance.check_against(&b.withheld_rows).is_err());
    }

    #[test]
    fn the_policy_is_the_registered_one_and_decides_only_from_the_measured_scores() {
        let policy = publication_policy();
        assert_eq!(policy.minimum_coverage_90, 0.75);
        assert_eq!(policy.maximum_crps_over_persistence, 1.0);
        assert!(policy.rule.contains("never tuned"));
        let measured = SeriesSkill::Measured;
        let (published, none) = decide(&policy, measured, Some((0.75, 4.0, 4.0)));
        assert_eq!((published, none), (ForecastStatus::Forecast, None));
        for (scores, why) in [
            (Some((0.7499, 1.0, 4.0)), WithheldReason::SkillBelowPolicy),
            (Some((0.95, 4.0001, 4.0)), WithheldReason::SkillBelowPolicy),
        ] {
            assert_eq!(
                decide(&policy, measured, scores),
                (ForecastStatus::Withheld, Some(why))
            );
        }
        assert_eq!(
            decide(&policy, SeriesSkill::InsufficientData, None),
            (
                ForecastStatus::Withheld,
                Some(WithheldReason::InsufficientDataForSkill)
            )
        );
        assert_eq!(
            decide(&policy, SeriesSkill::NotBacktested, None),
            (
                ForecastStatus::Withheld,
                Some(WithheldReason::NotBacktested)
            )
        );
        // A skill that is not measured cannot be admitted, whatever numbers someone supplies.
        assert_eq!(
            decide(&policy, SeriesSkill::NotBacktested, Some((1.0, 0.0, 1.0))).0,
            ForecastStatus::Withheld
        );
        assert_eq!(
            measured_for(
                SeriesSkill::NotBacktested,
                None,
                None,
                "12".parse().unwrap()
            ),
            None
        );
        assert_eq!(
            measured_for(
                SeriesSkill::InsufficientData,
                None,
                None,
                "12".parse().unwrap()
            ),
            None
        );
    }

    #[test]
    fn only_a_series_whose_own_measured_skill_meets_the_policy_is_published() {
        let cfg = forecast_config();
        let backtest = |edit: fn(&mut serde_json::Value)| {
            series_backtest_from_report(&state_report(edit), &cfg, 2).unwrap()
        };
        let mut cases = series("12", 20, 10);
        cases.extend(series("13", 20, 10));
        // "12" has a measured skill that meets the policy (90% coverage 1.0, CRPS 2.0 against 4.0);
        // "13" is in the backtest but below its floor.
        let b = build(&cases, input(), None, Some(backtest(|_| {})))
            .unwrap()
            .unwrap();
        let by = |g: &str| {
            b.provenance
                .series
                .iter()
                .find(|s| s.geography.to_string() == g)
                .unwrap()
        };
        assert_eq!(
            (by("12").status, by("12").skill),
            (ForecastStatus::Forecast, SeriesSkill::Measured)
        );
        assert_eq!(by("12").withheld, None);
        assert_eq!(
            (by("13").status, by("13").withheld, by("13").skill),
            (
                ForecastStatus::Withheld,
                Some(WithheldReason::InsufficientDataForSkill),
                SeriesSkill::InsufficientData
            )
        );
        assert_eq!(b.rows.len(), 8);
        assert!(b.rows.iter().all(|r| r.geography.to_string() == "12"));
        assert!(
            b.withheld_rows
                .iter()
                .all(|r| r.geography.to_string() == "13")
        );
        b.provenance.check_against(&b.rows).unwrap();
        // The companion survives the contract's own validation (which re-applies the policy).
        let json = serde_json::to_value(&b.provenance).unwrap();
        assert!(serde_json::from_value::<ForecastProvenance>(json).is_ok());

        // The same series with worse coverage, or an error above persistence, is withheld.
        for edit in [
            (|v: &mut serde_json::Value| {
                v["primary"]["series"][0]["pooled"]["coverage_90"] = json!(0.74)
            }) as fn(&mut _),
            |v| v["primary"]["series"][0]["pooled"]["mean_crps"] = json!(4.5),
        ] {
            let w = build(&cases, input(), None, Some(backtest(edit)))
                .unwrap()
                .unwrap();
            let s = w
                .provenance
                .series
                .iter()
                .find(|s| s.geography.to_string() == "12")
                .unwrap();
            assert_eq!(
                (s.status, s.withheld, s.skill),
                (
                    ForecastStatus::Withheld,
                    Some(WithheldReason::SkillBelowPolicy),
                    SeriesSkill::Measured
                )
            );
            assert!(w.rows.is_empty());
            assert!(
                serde_json::from_value::<ForecastProvenance>(
                    serde_json::to_value(&w.provenance).unwrap()
                )
                .is_ok()
            );
        }
    }

    #[test]
    fn a_refused_projection_is_that_series_alone_and_never_aborts_the_run() {
        // "14": one early case, then 40 in the origin week: the look-back infectivity is tiny, R is in
        // the hundreds and the projection passes the method's limit (#1513).
        let burst = |w: u8| -> u32 {
            match w {
                13 => 1,
                18 => 40,
                _ => 0,
            }
        };
        let mut cases = series("12", 20, 10);
        cases.extend((1..=20).map(|w| {
            row(
                "14",
                w,
                burst(w),
                CaseDefinition::ConfirmedOrUnknownStatus,
                "cdc-nndss-weekly-measles",
            )
        }));
        let b = built(&cases);
        let refused = b
            .provenance
            .series
            .iter()
            .find(|s| s.geography.to_string() == "14")
            .unwrap();
        assert_eq!(refused.status, ForecastStatus::InsufficientData);
        assert_eq!(refused.reason, Some(InsufficientReason::ProjectionOverflow));
        assert_eq!(refused.withheld, None);
        assert!(b.all_rows().iter().all(|r| r.geography.to_string() == "12"));
        // The other series is exactly what it would have been without the refused one.
        let alone = built(&series("12", 20, 10));
        assert_eq!(alone.all_rows(), b.all_rows());
        assert_eq!(alone.provenance.series[0], b.provenance.series[0]);
    }

    /// A state-series report with series "12" above the floor (3 horizons' worth of targets
    /// stand in for it: floor 4 targets from 2 origin weeks) and "13" below it.
    fn state_report(edit: impl FnOnce(&mut serde_json::Value)) -> Vec<u8> {
        let summary = |horizon: u32, n: u32| {
            if n == 0 {
                json!({"horizon": horizon, "n": 0, "mean_crps": null, "coverage_50": null,
                       "coverage_90": null, "mean_persistence_abs_error": null})
            } else {
                json!({"horizon": horizon, "n": n, "mean_crps": 2.0, "coverage_50": 0.5,
                       "coverage_90": 1.0, "mean_persistence_abs_error": 4.0})
            }
        };
        let horizons = |ns: [u32; 2]| json!([summary(1, ns[0]), summary(2, ns[1])]);
        let mut v = json!({
            "input": {"source_id": "cdc-nndss-weekly-measles", "sha256": "ab".repeat(32)},
            "protocol": "pseudo-real-time (revised counts truncated at each forecast date): one retrieval",
            "scope": "the CDC NNDSS weekly counts of each state; not the DSHS county series",
            "primary": {
                "seed": FORECAST_SEED, "window_weeks": 3, "max_lag_weeks": 3, "min_cases": 11,
                "horizon_weeks": 8, "run_count": 1000, "provisional_weeks": 2,
                "floor": {"min_targets": 4, "min_origin_weeks": 2},
                "series": [
                    {"geography": "12", "origins_forecast": 3, "origin_weeks_scored": 2, "measured": true,
                     "pooled": summary(0, 4), "by_horizon": horizons([2, 2]), "not_forecast": {"below_threshold": 9}},
                    {"geography": "13", "origins_forecast": 1, "origin_weeks_scored": 1, "measured": false,
                     "pooled": summary(0, 2), "by_horizon": horizons([1, 1]), "not_forecast": {"projection_overflow": 1}}
                ],
                "series_scored": 2, "forecasts_scored": 3,
                "by_horizon": horizons([3, 3]), "pooled": summary(0, 6), "pooled_measured": true
            }
        });
        edit(&mut v);
        serde_json::to_vec(&v).unwrap()
    }

    #[test]
    fn a_state_report_becomes_the_series_backtest_with_scores_only_above_the_floor() {
        let b = series_backtest_from_report(&state_report(|_| {}), &forecast_config(), 2).unwrap();
        assert_eq!(b.basis, InformationBasis::PseudoRealTime);
        assert_eq!((b.minimum_targets, b.minimum_origin_weeks), (4, 2));
        assert_eq!(b.by_series.len(), 2);
        let above = &b.by_series[0];
        assert_eq!(
            (above.targets, above.origin_weeks, above.forecasts),
            (4, 2, 3)
        );
        assert_eq!(above.measured.as_ref().unwrap().mean_crps, 2.0);
        // Below the floor: the counts, never the scores.
        let below = &b.by_series[1];
        assert_eq!((below.targets, below.origin_weeks), (2, 1));
        assert!(below.measured.is_none());
        let pooled = b.pooled.as_ref().unwrap();
        assert_eq!(
            (pooled.forecasts, pooled.series, pooled.scores.targets),
            (3, 2, 6)
        );
        assert_eq!(b.report_sha256, sha256_of(&state_report(|_| {})));
        assert!(
            b.limitations
                .join(" ")
                .contains("1 origin(s) where the projection passed")
        );
        // The companion contract accepts what the pipeline builds.
        let json = serde_json::to_value(&b).unwrap();
        assert!(serde_json::from_value::<SeriesBacktest>(json).is_ok());
    }

    #[test]
    fn a_state_report_that_does_not_fit_the_published_forecast_is_refused() {
        let attempt = |edit: fn(&mut serde_json::Value)| {
            series_backtest_from_report(&state_report(edit), &forecast_config(), 2).unwrap_err()
        };
        assert!(
            attempt(|v| v["primary"]["window_weeks"] = json!(2)).contains("report was run with")
        );
        assert!(
            attempt(|v| v["primary"]["provisional_weeks"] = json!(1))
                .contains("report was run with")
        );
        assert!(attempt(|v| v["primary"]["seed"] = json!(3)).contains("seed"));
        assert!(attempt(|v| v["input"]["source_id"] = json!("dshs")).contains("run on"));
        assert!(attempt(|v| v["protocol"] = json!("real-time")).contains("label"));
        // The report's own measured flag must agree with its floor, and its pooled numbers with its series.
        assert!(attempt(|v| v["primary"]["series"][1]["measured"] = json!(true)).contains("floor"));
        assert!(attempt(|v| v["primary"]["pooled"]["n"] = json!(7)).contains("pooled summary"));
        assert!(series_backtest_from_report(b"nope", &forecast_config(), 2).is_err());
        // Pooled below the floor is "no pooled result", not an invented one.
        let b = series_backtest_from_report(
            &state_report(|v| v["primary"]["pooled_measured"] = json!(false)),
            &forecast_config(),
            2,
        )
        .unwrap();
        assert!(b.pooled.is_none());
    }

    #[test]
    fn a_series_takes_its_skill_only_from_its_own_entry() {
        let b = series_backtest_from_report(&state_report(|_| {}), &forecast_config(), 2).unwrap();
        let cdc: BTreeSet<&str> = ["cdc-nndss-weekly-measles"].into();
        let dshs: BTreeSet<&str> = ["dshs-measles-data-report-wayback"].into();
        let unknown = CaseDefinition::ConfirmedOrUnknownStatus;
        let skill = |g: &str, d, src: &BTreeSet<&str>| {
            series_skill(None, Some(&b), g.parse().unwrap(), d, src)
        };
        assert_eq!(skill("12", unknown, &cdc), SeriesSkill::Measured);
        assert_eq!(skill("13", unknown, &cdc), SeriesSkill::InsufficientData);
        // Not in the report, another definition, or another source: not backtested.
        assert_eq!(skill("14", unknown, &cdc), SeriesSkill::NotBacktested);
        assert_eq!(
            skill("12", CaseDefinition::Confirmed, &cdc),
            SeriesSkill::NotBacktested
        );
        assert_eq!(skill("12", unknown, &dshs), SeriesSkill::NotBacktested);
        assert_eq!(
            skill("12", unknown, &BTreeSet::new()),
            SeriesSkill::NotBacktested
        );
        assert_eq!(
            series_skill(None, None, "12".parse().unwrap(), unknown, &cdc),
            SeriesSkill::NotBacktested
        );
    }
}
