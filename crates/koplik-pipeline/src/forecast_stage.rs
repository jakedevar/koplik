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
//! * **Skill.** The measured backtest skill is attached only from a committed report that was
//!   run with exactly this configuration ([`skill_from_report`]); otherwise the companion says
//!   there is none. The backtest scored one series (the Texas DSHS 2025 outbreak total by
//!   report date), so a series is marked `backtested` only when it is that series
//!   ([`is_backtested`]); every other series is published as `not backtested; no measured skill`,
//!   and nothing about the backtest's calibration is said of it.

use std::collections::{BTreeMap, BTreeSet};

use koplik_contracts::v1::{Forecast, GeoId, MmwrWeek, Sha256Hex};
use koplik_contracts::v3::{CaseDefinition, WeeklyCaseCount};
use koplik_contracts::v5::{
    BacktestSkill, FORECAST_PROVENANCE_VERSION, ForecastInput, ForecastProvenance, ForecastSeries,
    ForecastStatus, InsufficientReason, ParameterProvenance, SeriesSkill, SkillByHorizon,
};
use koplik_epi::forecast::{ForecastConfig, ProjectionStatus, forecast_weekly};
use koplik_epi::rt::{InsufficientReason as EpiReason, case_definitions};
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

/// What [`build`] produces.
#[derive(Debug, Clone, PartialEq)]
pub struct Built {
    pub rows: Vec<Forecast>,
    pub provenance: ForecastProvenance,
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
    let forecasts =
        forecast_weekly(cases, origin, &cfg, FORECAST_SEED).map_err(PipelineError::Forecast)?;
    let by_geography: BTreeMap<GeoId, _> = forecasts.iter().map(|f| (f.geography, f)).collect();
    let mut sources: BTreeMap<GeoId, BTreeSet<&str>> = BTreeMap::new();
    for row in cases {
        sources.entry(row.geography).or_default().extend(
            row.provenance
                .as_slice()
                .iter()
                .map(|p| p.source_id.as_str()),
        );
    }

    let mut rows = Vec::new();
    let mut series = Vec::new();
    for (geography, definition) in &definitions {
        let skill_status = if is_backtested(
            skill.as_ref(),
            *geography,
            *definition,
            sources.get(geography).unwrap_or(&BTreeSet::new()),
        ) {
            SeriesSkill::Backtested
        } else {
            SeriesSkill::NotBacktested
        };
        let entry = match by_geography.get(geography) {
            Some(f) => match f.projection.status {
                ProjectionStatus::Ok => {
                    rows.extend(f.rows.iter().cloned());
                    ForecastSeries {
                        geography: *geography,
                        case_definition: *definition,
                        status: ForecastStatus::Forecast,
                        reason: None,
                        cases_in_window: f.projection.cases_in_window,
                        skill: skill_status,
                    }
                }
                ProjectionStatus::InsufficientData(why) => ForecastSeries {
                    geography: *geography,
                    case_definition: *definition,
                    status: ForecastStatus::InsufficientData,
                    reason: Some(reason(why)),
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
                cases_in_window: None,
                skill: skill_status,
            },
        };
        series.push(entry);
    }

    let forecast_count = series
        .iter()
        .filter(|s| s.status == ForecastStatus::Forecast)
        .count();
    let backtested_count = series
        .iter()
        .filter(|s| s.skill == SeriesSkill::Backtested)
        .count();
    let provenance = ForecastProvenance {
        contract_version: FORECAST_PROVENANCE_VERSION,
        artifact: ARTIFACT.to_owned(),
        statement: format!(
            "Model projections from reported counts, not predictions of what will happen. Each of {} ensemble members draws one reproduction number from its posterior over the last {} complete weeks and projects weekly counts {} weeks ahead with Poisson noise, holding that number constant; the bands are quantiles of the members. A series with fewer than {} cases in those weeks is not forecast. Every forecast describes cases under its own case definition and never mixes definitions.",
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
        series,
        scope_note: scope_note(skill.as_ref(), forecast_count, backtested_count),
        backtest: skill,
    };
    Ok(Some(Built { rows, provenance }))
}

/// What the attached skill does and does not say about the series forecast. A series the backtest
/// did not score has no measured skill, and the backtest's scores are not offered as evidence
/// about it.
fn scope_note(skill: Option<&BacktestSkill>, forecast: usize, backtested: usize) -> String {
    let Some(skill) = skill else {
        return format!(
            "No backtest report for exactly this configuration is attached, so no skill has been measured for these forecasts ({forecast} series forecast)."
        );
    };
    let unmeasured = forecast.saturating_sub(backtested);
    let which = match (forecast, unmeasured) {
        (0, _) => "No series was forecast.".to_owned(),
        (_, 0) => format!("Every one of the {forecast} series forecast here is that series."),
        (n, m) if n == m => format!(
            "None of the {n} series forecast here is that series: none was backtested and none has a measured skill."
        ),
        (n, m) => format!(
            "{m} of the {n} series forecast here are not that series: they were not backtested and have no measured skill."
        ),
    };
    format!(
        "The backtest scored one series only: {}. {which} Its scores are reported separately, as an evaluation of the method on that series, and are not a measure of the forecasts of any other series.",
        skill.series
    )
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
    truth: Vec<(MmwrWeek, Option<u32>)>,
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
            "One series and one period: {}. No state series, county series or other outbreak was scored.",
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
        build(cases, input(), None).unwrap().unwrap()
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
        assert_eq!(a.rows.len(), 8);
        assert_eq!(a.rows[0].target_week, MmwrWeek::new(2026, 19).unwrap());
        assert_eq!(a.rows[7].target_week, MmwrWeek::new(2026, 26).unwrap());
        // The two provisional weeks can be anything: they do not reach the forecast.
        cases[18].cases = CaseCount::Reported { count: 900 };
        cases[19].cases = CaseCount::Reported { count: 0 };
        let b = built(&cases);
        assert_eq!(a.rows, b.rows);
        assert_eq!(a.provenance.series, b.provenance.series);
    }

    #[test]
    fn a_forecast_row_states_its_seed_members_provenance_and_ordered_quantiles() {
        let b = built(&series("12", 20, 10));
        for r in &b.rows {
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
        assert!(b.rows.iter().all(|r| r.geography.to_string() == "12"));
        // Each forecast series met the rule.
        for s in b
            .provenance
            .series
            .iter()
            .filter(|s| s.status == ForecastStatus::Forecast)
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
        assert!(b.rows.iter().all(|r| r.geography.to_string() == "12"));
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
        assert!(build(&cases, input(), None).is_err());
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
        assert!(build(&[], input(), None).unwrap().is_none());
    }

    #[test]
    fn the_same_input_gives_the_same_bytes() {
        let cases = series("12", 20, 10);
        let (a, b) = (built(&cases), built(&cases));
        assert_eq!(
            serde_json::to_vec(&a.rows).unwrap(),
            serde_json::to_vec(&b.rows).unwrap()
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
    fn the_scope_note_says_the_forecast_series_were_not_scored() {
        let skill = skill_from_report(&skill_json(|_| {}), &forecast_config()).unwrap();
        let mut cases = series("48", 20, 10);
        cases.extend(series("12", 20, 10));
        let b = build(&cases, input(), Some(skill)).unwrap().unwrap();
        assert!(
            b.provenance
                .series
                .iter()
                .all(|s| s.skill == SeriesSkill::NotBacktested)
        );
        assert!(
            b.provenance
                .scope_note
                .contains("None of the 2 series forecast here is that series")
        );
        assert!(b.provenance.backtest.is_some());
        let none = build(&cases, input(), None).unwrap().unwrap();
        assert!(none.provenance.backtest.is_none());
        assert!(
            none.provenance
                .scope_note
                .contains("no skill has been measured")
        );
    }
}
