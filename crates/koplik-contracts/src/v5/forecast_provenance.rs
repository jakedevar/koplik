use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};

use super::{CaseDefinition, Forecast, GeoId, MmwrWeek, ParameterProvenance, Sha256Hex};

/// Tag every v5 forecast provenance carries.
pub const FORECAST_PROVENANCE_VERSION: u32 = 5;

/// Quantile levels a forecast must publish for the web to draw it and the backtest to have
/// scored it: the median and the 50% (0.25, 0.75) and 90% (0.05, 0.95) central intervals.
const BAND_LEVELS: [f64; 5] = [0.05, 0.25, 0.5, 0.75, 0.95];

/// Whether a series was forecast.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ForecastStatus {
    /// Forecast rows exist for the series.
    Forecast,
    /// The method's own minimum-count rule did not hold: no number is published.
    InsufficientData,
}

/// Why a series could not be forecast (the renewal estimator's reason at the origin week).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum InsufficientReason {
    /// The estimation window would start before the series, or include its first week.
    IncompleteWindow,
    /// A count in the estimation window or the serial-interval look-back is missing.
    MissingCount,
    /// Fewer cases in the window than the minimum-count threshold.
    BelowThreshold,
    /// No cases in the look-back, so the window's cases have no infectors in the data.
    NoInfectivity,
}

/// One series the pipeline considered: forecast, or not and why.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ForecastSeries {
    pub geography: GeoId,
    /// What the series counts; a forecast describes cases under this definition only.
    pub case_definition: CaseDefinition,
    pub status: ForecastStatus,
    /// Present exactly when `status` is `insufficient_data`.
    pub reason: Option<InsufficientReason>,
    /// Cases in the estimation window ending at the origin week, when every count in it is
    /// known.
    pub cases_in_window: Option<u32>,
    /// True only when this series is the one the backtest scored (same geography, case
    /// definition and source). Every other series is forecast by a method whose skill was not
    /// measured on it.
    pub backtested: bool,
}

/// The series file the forecast was made from, hashed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ForecastInput {
    /// Artifact name, e.g. `weekly-cases`.
    pub artifact: String,
    /// SHA-256 of the input file exactly as read.
    pub sha256: Sha256Hex,
    pub rows: u64,
}

/// Measured skill at one horizon.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SkillByHorizon {
    /// Weeks after the origin week (1-based).
    pub horizon: u32,
    /// Scored targets at this horizon.
    pub n: u32,
    /// Mean CRPS in cases; absent when `n` is 0.
    pub mean_crps: Option<f64>,
    /// Share of targets inside the central 50% interval; absent when `n` is 0.
    pub coverage_50: Option<f64>,
    /// Share of targets inside the central 90% interval; absent when `n` is 0.
    pub coverage_90: Option<f64>,
}

/// The measured skill of the method on its backtest, exactly as reported, with the scope it was
/// measured on. It is a statement about that series and period only.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BacktestSkill {
    /// The series that was forecast and scored, in words (non-empty).
    pub series: String,
    /// Geography of that series.
    pub geography: GeoId,
    /// What that series counts.
    pub case_definition: CaseDefinition,
    /// How the backtest respected the information cutoff, in words (non-empty).
    pub protocol: String,
    /// Base seed of the backtest.
    pub seed: u64,
    /// Scored targets (forecast week by horizon), pooled.
    #[schemars(range(min = 1))]
    pub targets: u32,
    /// Forecast dates that produced a forecast.
    pub forecast_dates: u32,
    /// Distinct origin weeks among them.
    pub origin_weeks: u32,
    /// Mean continuous ranked probability score over `targets`, in cases (lower is better).
    #[schemars(range(min = 0))]
    pub mean_crps: f64,
    /// Share of targets inside the central 50% interval (nominal 0.5).
    #[schemars(range(min = 0, max = 1))]
    pub coverage_50: f64,
    /// Share of targets inside the central 90% interval (nominal 0.9).
    #[schemars(range(min = 0, max = 1))]
    pub coverage_90: f64,
    /// Mean absolute error of carrying the origin week's count forward.
    #[schemars(range(min = 0))]
    pub mean_persistence_abs_error: f64,
    /// The same scores per horizon, horizons 1, 2, ... in order.
    pub by_horizon: Vec<SkillByHorizon>,
    /// Repository path of the committed report these numbers were read from.
    pub report_path: String,
    /// SHA-256 of that report file exactly as read.
    pub report_sha256: Sha256Hex,
    /// SHA-256 of the report-vintage manifest the backtest ran on.
    pub manifest_sha256: Sha256Hex,
    /// What the backtest does not show (non-empty list of non-empty statements).
    pub limitations: Vec<String>,
}

/// Companion of a published set of v1 [`Forecast`] rows. Rows are only published beside a
/// companion for which [`ForecastProvenance::check_against`] holds.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ForecastProvenance {
    /// Always [`FORECAST_PROVENANCE_VERSION`].
    pub contract_version: u32,
    /// Name of the series artifact that was forecast, e.g. `weekly-cases` (non-empty).
    pub artifact: String,
    /// What the forecast is and is not, in plain words (non-empty).
    pub statement: String,
    /// The method and its citation (non-empty).
    pub method: String,
    /// Last week of data the forecast was allowed to use.
    pub origin_week: MmwrWeek,
    /// Latest week with any row in the input series.
    pub latest_data_week: MmwrWeek,
    /// How the origin week was chosen from the data (non-empty).
    pub origin_rule: String,
    /// Weeks forecast ahead of the origin week.
    #[schemars(range(min = 1))]
    pub horizon_weeks: u32,
    /// Ensemble members behind every row's quantiles.
    #[schemars(range(min = 1))]
    pub run_count: u32,
    /// Base seed of every series' ensemble (members derive from it, the geography and the
    /// member index).
    pub seed: u64,
    /// Quantile levels of every row, strictly increasing in (0, 1).
    pub levels: Vec<f64>,
    pub input: ForecastInput,
    /// One entry per configuration value, with the value the forecast ran with and its
    /// citation (non-empty, unique).
    pub parameters: Vec<ParameterProvenance>,
    /// Every series considered, once each, in geography order.
    pub series: Vec<ForecastSeries>,
    /// The backtest skill, when a committed report exists for exactly this configuration.
    pub backtest: Option<BacktestSkill>,
    /// What the skill does and does not say about the series above (non-empty).
    pub scope_note: String,
}

fn non_empty(name: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!("{name} must not be empty"))
    } else {
        Ok(())
    }
}

fn fraction(name: &str, value: f64) -> Result<(), String> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(format!("{name} must be a share between 0 and 1"))
    }
}

fn non_negative(name: &str, value: f64) -> Result<(), String> {
    if value.is_finite() && value >= 0.0 {
        Ok(())
    } else {
        Err(format!("{name} must be finite and not negative"))
    }
}

impl<'de> Deserialize<'de> for ForecastSeries {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            geography: GeoId,
            case_definition: CaseDefinition,
            status: ForecastStatus,
            reason: Option<InsufficientReason>,
            cases_in_window: Option<u32>,
            backtested: bool,
        }
        use serde::de::Error;
        let r = Raw::deserialize(d)?;
        match (r.status, r.reason) {
            (ForecastStatus::Forecast, None) | (ForecastStatus::InsufficientData, Some(_)) => {}
            (ForecastStatus::Forecast, Some(_)) => {
                return Err(D::Error::custom("a forecast series has no reason"));
            }
            (ForecastStatus::InsufficientData, None) => {
                return Err(D::Error::custom("insufficient data needs a reason"));
            }
        }
        if r.status == ForecastStatus::Forecast && r.cases_in_window.is_none() {
            return Err(D::Error::custom(
                "a forecast series states the cases in its window",
            ));
        }
        Ok(Self {
            geography: r.geography,
            case_definition: r.case_definition,
            status: r.status,
            reason: r.reason,
            cases_in_window: r.cases_in_window,
            backtested: r.backtested,
        })
    }
}

impl<'de> Deserialize<'de> for BacktestSkill {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            series: String,
            geography: GeoId,
            case_definition: CaseDefinition,
            protocol: String,
            seed: u64,
            targets: u32,
            forecast_dates: u32,
            origin_weeks: u32,
            mean_crps: f64,
            coverage_50: f64,
            coverage_90: f64,
            mean_persistence_abs_error: f64,
            by_horizon: Vec<SkillByHorizon>,
            report_path: String,
            report_sha256: Sha256Hex,
            manifest_sha256: Sha256Hex,
            limitations: Vec<String>,
        }
        use serde::de::Error;
        let r = Raw::deserialize(d)?;
        let checked = || -> Result<(), String> {
            non_empty("backtest.series", &r.series)?;
            non_empty("backtest.protocol", &r.protocol)?;
            non_empty("backtest.report_path", &r.report_path)?;
            if r.targets == 0 {
                return Err("backtest.targets must be at least 1".into());
            }
            if r.origin_weeks > r.forecast_dates {
                return Err("backtest.origin_weeks cannot exceed forecast_dates".into());
            }
            non_negative("backtest.mean_crps", r.mean_crps)?;
            fraction("backtest.coverage_50", r.coverage_50)?;
            fraction("backtest.coverage_90", r.coverage_90)?;
            non_negative(
                "backtest.mean_persistence_abs_error",
                r.mean_persistence_abs_error,
            )?;
            if r.by_horizon.is_empty() {
                return Err("backtest.by_horizon must not be empty".into());
            }
            for (i, h) in r.by_horizon.iter().enumerate() {
                if h.horizon as usize != i + 1 {
                    return Err("backtest.by_horizon must list horizons 1, 2, ... in order".into());
                }
                for (name, v) in [
                    ("mean_crps", h.mean_crps),
                    ("coverage_50", h.coverage_50),
                    ("coverage_90", h.coverage_90),
                ] {
                    match (h.n, v) {
                        (0, Some(_)) => {
                            return Err(format!(
                                "horizon {} has no targets but a {name}",
                                h.horizon
                            ));
                        }
                        (n, None) if n > 0 => {
                            return Err(format!("horizon {} lacks its {name}", h.horizon));
                        }
                        _ => {}
                    }
                }
                if let Some(v) = h.mean_crps {
                    non_negative("by_horizon.mean_crps", v)?;
                }
                for v in [h.coverage_50, h.coverage_90].into_iter().flatten() {
                    fraction("by_horizon.coverage", v)?;
                }
            }
            if r.by_horizon.iter().map(|h| u64::from(h.n)).sum::<u64>() != u64::from(r.targets) {
                return Err("backtest.targets must equal the sum of the horizons' n".into());
            }
            if r.limitations.is_empty() {
                return Err("backtest.limitations must not be empty".into());
            }
            for l in &r.limitations {
                non_empty("a backtest limitation", l)?;
            }
            Ok(())
        };
        checked().map_err(D::Error::custom)?;
        Ok(Self {
            series: r.series,
            geography: r.geography,
            case_definition: r.case_definition,
            protocol: r.protocol,
            seed: r.seed,
            targets: r.targets,
            forecast_dates: r.forecast_dates,
            origin_weeks: r.origin_weeks,
            mean_crps: r.mean_crps,
            coverage_50: r.coverage_50,
            coverage_90: r.coverage_90,
            mean_persistence_abs_error: r.mean_persistence_abs_error,
            by_horizon: r.by_horizon,
            report_path: r.report_path,
            report_sha256: r.report_sha256,
            manifest_sha256: r.manifest_sha256,
            limitations: r.limitations,
        })
    }
}

impl<'de> Deserialize<'de> for ForecastProvenance {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            contract_version: u32,
            artifact: String,
            statement: String,
            method: String,
            origin_week: MmwrWeek,
            latest_data_week: MmwrWeek,
            origin_rule: String,
            horizon_weeks: u32,
            run_count: u32,
            seed: u64,
            levels: Vec<f64>,
            input: ForecastInput,
            parameters: Vec<ParameterProvenance>,
            series: Vec<ForecastSeries>,
            backtest: Option<BacktestSkill>,
            scope_note: String,
        }
        use serde::de::Error;
        let r = Raw::deserialize(d)?;
        if r.contract_version != FORECAST_PROVENANCE_VERSION {
            return Err(D::Error::custom(format!(
                "contract_version must be {FORECAST_PROVENANCE_VERSION}, got {}",
                r.contract_version
            )));
        }
        let checked = || -> Result<(), String> {
            non_empty("artifact", &r.artifact)?;
            non_empty("input.artifact", &r.input.artifact)?;
            non_empty("statement", &r.statement)?;
            non_empty("method", &r.method)?;
            non_empty("origin_rule", &r.origin_rule)?;
            non_empty("scope_note", &r.scope_note)?;
            if r.horizon_weeks == 0 {
                return Err("horizon_weeks must be at least 1".into());
            }
            if r.run_count == 0 {
                return Err("run_count must be at least 1".into());
            }
            if r.latest_data_week < r.origin_week {
                return Err("latest_data_week cannot be before origin_week".into());
            }
            if r.levels.is_empty()
                || r.levels
                    .iter()
                    .any(|l| !(l.is_finite() && *l > 0.0 && *l < 1.0))
                || r.levels.windows(2).any(|w| !(w[0] < w[1]))
            {
                return Err("levels must increase strictly inside (0, 1)".into());
            }
            if r.parameters.is_empty() {
                return Err("parameters must not be empty".into());
            }
            let mut names = BTreeSet::new();
            for p in &r.parameters {
                non_empty("parameter name", &p.parameter)?;
                non_empty(&format!("{} source", p.parameter), &p.source)?;
                non_empty(&format!("{} note", p.parameter), &p.note)?;
                if !names.insert(p.parameter.as_str()) {
                    return Err(format!("parameter {} is cited twice", p.parameter));
                }
            }
            let mut geographies = BTreeSet::new();
            for s in &r.series {
                if !geographies.insert(s.geography) {
                    return Err(format!("series {} is listed twice", s.geography));
                }
            }
            if let Some(b) = &r.backtest {
                for s in r.series.iter().filter(|s| s.backtested) {
                    if (s.geography, s.case_definition) != (b.geography, b.case_definition) {
                        return Err(format!(
                            "series {} is marked backtested but is not the backtested series",
                            s.geography
                        ));
                    }
                }
            } else if r.series.iter().any(|s| s.backtested) {
                return Err("a series is marked backtested but there is no backtest".into());
            }
            Ok(())
        };
        checked().map_err(D::Error::custom)?;
        Ok(Self {
            contract_version: r.contract_version,
            artifact: r.artifact,
            statement: r.statement,
            method: r.method,
            origin_week: r.origin_week,
            latest_data_week: r.latest_data_week,
            origin_rule: r.origin_rule,
            horizon_weeks: r.horizon_weeks,
            run_count: r.run_count,
            seed: r.seed,
            levels: r.levels,
            input: r.input,
            parameters: r.parameters,
            series: r.series,
            backtest: r.backtest,
            scope_note: r.scope_note,
        })
    }
}

impl ForecastProvenance {
    /// Whether this companion describes `rows`: every row has this seed, run count, origin week
    /// and quantile levels; the rows cover exactly the series whose status is `forecast`, each
    /// with one row for every week from 1 to `horizon_weeks` after the origin; and no series
    /// that was not forecast has a row. Rows are only published beside a companion that agrees
    /// with them.
    pub fn check_against(&self, rows: &[Forecast]) -> Result<(), String> {
        let differs = |what: &str| {
            Err(format!(
                "the provenance companion does not describe the forecast rows beside it ({what})"
            ))
        };
        let mut by_geography: BTreeMap<GeoId, Vec<&Forecast>> = BTreeMap::new();
        for row in rows {
            if row.seed != self.seed {
                return differs("seed");
            }
            if row.run_count != self.run_count {
                return differs("run_count");
            }
            if row.origin_week != self.origin_week {
                return differs("origin week");
            }
            if row.quantiles.len() != self.levels.len()
                || !row
                    .quantiles
                    .iter()
                    .zip(&self.levels)
                    .all(|(q, l)| q.level == *l)
            {
                return differs("quantile levels");
            }
            by_geography.entry(row.geography).or_default().push(row);
        }
        let forecast: BTreeSet<GeoId> = self
            .series
            .iter()
            .filter(|s| s.status == ForecastStatus::Forecast)
            .map(|s| s.geography)
            .collect();
        if by_geography.keys().copied().collect::<BTreeSet<_>>() != forecast {
            return differs("the series that were forecast");
        }
        for list in by_geography.values() {
            if list.len() != self.horizon_weeks as usize {
                return differs("horizons");
            }
            let mut target = self.origin_week;
            for row in list {
                target = target.next().map_err(|e| e.to_string())?;
                if row.target_week != target {
                    return differs("target weeks");
                }
            }
        }
        if !forecast.is_empty()
            && !BAND_LEVELS
                .iter()
                .all(|b| self.levels.iter().any(|l| (l - b).abs() < 1e-9))
        {
            return differs("quantile levels (the 50% and 90% bands are missing)");
        }
        Ok(())
    }
}
