use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};

use super::{
    BacktestSkill, CaseDefinition, Forecast, ForecastInput, ForecastStatus, GeoId,
    InsufficientReason, MmwrWeek, ParameterProvenance, Sha256Hex, SkillByHorizon,
};

/// Tag every v7 forecast provenance carries.
pub const FORECAST_PROVENANCE_VERSION: u32 = 7;

/// Quantile levels a forecast must publish for the web to draw it and the backtests to have
/// scored it: the median and the 50% (0.25, 0.75) and 90% (0.05, 0.95) central intervals.
const BAND_LEVELS: [f64; 5] = [0.05, 0.25, 0.5, 0.75, 0.95];

/// Whether a series has a measured skill, and which evaluation says so. A forecast method is only
/// as good as its tests on the data it is run on: a series no backtest scored has no measured
/// skill, and says so.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum SeriesSkill {
    /// The series is the one the report-vintage backtest scored (same geography, case definition
    /// and source); its measured skill is in `backtest`.
    #[serde(rename = "backtested")]
    Backtested,
    /// The series was scored in `series_backtest` with enough targets from enough origin weeks to
    /// state a skill; its numbers are that entry's `measured`.
    #[serde(rename = "measured")]
    Measured,
    /// The series was part of `series_backtest`, but too few targets or origin weeks were scored
    /// for it to state a skill. Nothing is measured about it, and no other series' number is
    /// evidence about it.
    #[serde(rename = "insufficient data for a measured skill")]
    InsufficientData,
    /// No backtest scored this series. Nothing has been measured about how this forecast will do.
    #[serde(rename = "not backtested; no measured skill")]
    NotBacktested,
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
    /// Which backtest, if any, measured this series' skill.
    pub skill: SeriesSkill,
}

/// How a backtest respected the information cutoff on *revisions* of the counts. A backtest on a
/// series whose revision history is not held is pseudo-real-time and must never be described as
/// real-time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum InformationBasis {
    /// Every forecast used only the report versions first seen by its forecast date.
    #[serde(rename = "real-time by report vintage")]
    RealTimeByVintage,
    /// One retrieval of the (revised) series is held: each forecast was given only the weeks up to
    /// its origin, taken from that one retrieval. Revisions made after a forecast date are in the
    /// counts the forecast saw.
    #[serde(rename = "pseudo-real-time (revised counts truncated at each forecast date)")]
    PseudoRealTime,
}

/// The numbers of a measured evaluation, exactly as measured.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MeasuredScores {
    /// Scored targets (forecast week by horizon), pooled.
    #[schemars(range(min = 1))]
    pub targets: u32,
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
    /// The same scores per horizon, horizons 1, 2, ... in order; the `n` sum to `targets`.
    pub by_horizon: Vec<SkillByHorizon>,
}

/// What the evaluation scored for one series. The scores are present exactly when the series
/// reached the evaluation's floor: below it the series has insufficient data for a skill, however
/// its scores would have looked.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SeriesBacktestEntry {
    pub geography: GeoId,
    /// Origins at which the method forecast this series (its minimum-count rule held).
    pub forecasts: u32,
    /// Distinct origin weeks with at least one scored target.
    pub origin_weeks: u32,
    /// Scored targets.
    pub targets: u32,
    /// The measured skill; present exactly when `targets` and `origin_weeks` reach the floor.
    pub measured: Option<MeasuredScores>,
}

/// All series pooled: every scored target of every series, weighted equally, so the series with
/// the largest counts dominate a score in cases. A statement about the forecasts made where the
/// method's minimum-count rule held, never about any one series.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PooledScores {
    /// (series, origin) forecasts with at least one scored target.
    pub forecasts: u32,
    /// Series with at least one scored target.
    pub series: u32,
    pub scores: MeasuredScores,
}

/// A backtest of the forecast over a family of series (#1503), with the scope and information
/// basis it was measured on. It is a statement about those series and that period only.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SeriesBacktest {
    /// Short name of what was backtested, for a sentence, e.g. `the CDC NNDSS state series`
    /// (non-empty).
    pub name: String,
    /// What was forecast and scored, in words, including what was not scored (non-empty).
    pub series: String,
    /// What every series scored counts.
    pub case_definition: CaseDefinition,
    /// Whether the information cutoff covered revisions of the counts.
    pub basis: InformationBasis,
    /// How the information cutoff was respected, and what that may hide, in words (non-empty).
    pub protocol: String,
    /// Base seed of the backtest (and of the published forecast).
    pub seed: u64,
    /// Recent weeks treated as provisional: the origin is the latest week less these, and the same
    /// newest weeks are never used as truth.
    pub provisional_weeks: u32,
    /// The floor for a measured skill: at least this many scored targets...
    #[schemars(range(min = 1))]
    pub minimum_targets: u32,
    /// ...from at least this many distinct origin weeks. Fixed before any score was computed.
    #[schemars(range(min = 1))]
    pub minimum_origin_weeks: u32,
    /// Every series pooled; absent when even the pooled result is below the floor.
    pub pooled: Option<PooledScores>,
    /// Every series the backtest ran on, once each, in geography order.
    pub by_series: Vec<SeriesBacktestEntry>,
    /// Repository path of the committed report these numbers were read from.
    pub report_path: String,
    /// SHA-256 of that report file exactly as read.
    pub report_sha256: Sha256Hex,
    /// SHA-256 of the source snapshot the backtest ran on.
    pub input_sha256: Sha256Hex,
    /// What this evaluation does not show (non-empty list of non-empty statements).
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
    /// The report-vintage backtest (the Texas DSHS 2025 outbreak total), when a committed report
    /// exists for exactly this configuration.
    pub backtest: Option<BacktestSkill>,
    /// The backtest over the CDC NNDSS state series, when a committed report exists for exactly
    /// this configuration.
    pub series_backtest: Option<SeriesBacktest>,
    /// What the skills do and do not say about the series above (non-empty).
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
            skill: SeriesSkill,
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
            skill: r.skill,
        })
    }
}

/// Horizons 1, 2, ... in order, each with its scores exactly when it has targets, summing to
/// `targets`.
fn check_by_horizon(by_horizon: &[SkillByHorizon], targets: u32) -> Result<(), String> {
    if by_horizon.is_empty() {
        return Err("by_horizon must not be empty".into());
    }
    for (i, h) in by_horizon.iter().enumerate() {
        if h.horizon as usize != i + 1 {
            return Err("by_horizon must list horizons 1, 2, ... in order".into());
        }
        for (name, v) in [
            ("mean_crps", h.mean_crps),
            ("coverage_50", h.coverage_50),
            ("coverage_90", h.coverage_90),
        ] {
            match (h.n, v) {
                (0, Some(_)) => {
                    return Err(format!("horizon {} has no targets but a {name}", h.horizon));
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
    if by_horizon.iter().map(|h| u64::from(h.n)).sum::<u64>() != u64::from(targets) {
        return Err("targets must equal the sum of the horizons' n".into());
    }
    Ok(())
}

impl MeasuredScores {
    /// The rules every set of measured scores obeys.
    fn check(&self) -> Result<(), String> {
        if self.targets == 0 {
            return Err("measured scores need at least 1 target".into());
        }
        non_negative("mean_crps", self.mean_crps)?;
        fraction("coverage_50", self.coverage_50)?;
        fraction("coverage_90", self.coverage_90)?;
        non_negative(
            "mean_persistence_abs_error",
            self.mean_persistence_abs_error,
        )?;
        check_by_horizon(&self.by_horizon, self.targets)
    }
}

impl<'de> Deserialize<'de> for MeasuredScores {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            targets: u32,
            mean_crps: f64,
            coverage_50: f64,
            coverage_90: f64,
            mean_persistence_abs_error: f64,
            by_horizon: Vec<SkillByHorizon>,
        }
        use serde::de::Error;
        let r = Raw::deserialize(d)?;
        let scores = Self {
            targets: r.targets,
            mean_crps: r.mean_crps,
            coverage_50: r.coverage_50,
            coverage_90: r.coverage_90,
            mean_persistence_abs_error: r.mean_persistence_abs_error,
            by_horizon: r.by_horizon,
        };
        scores.check().map_err(D::Error::custom)?;
        Ok(scores)
    }
}

impl<'de> Deserialize<'de> for PooledScores {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            forecasts: u32,
            series: u32,
            scores: MeasuredScores,
        }
        use serde::de::Error;
        let r = Raw::deserialize(d)?;
        if r.series == 0 || r.forecasts == 0 {
            return Err(D::Error::custom(
                "pooled scores come from at least one series and one forecast",
            ));
        }
        if r.forecasts > r.scores.targets || r.series > r.forecasts {
            return Err(D::Error::custom(
                "pooled scores: series <= forecasts <= targets",
            ));
        }
        Ok(Self {
            forecasts: r.forecasts,
            series: r.series,
            scores: r.scores,
        })
    }
}

impl<'de> Deserialize<'de> for SeriesBacktestEntry {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            geography: GeoId,
            forecasts: u32,
            origin_weeks: u32,
            targets: u32,
            measured: Option<MeasuredScores>,
        }
        use serde::de::Error;
        let r = Raw::deserialize(d)?;
        if r.origin_weeks > r.forecasts {
            return Err(D::Error::custom(
                "origin_weeks cannot exceed the forecasts made",
            ));
        }
        if (r.targets == 0) != (r.origin_weeks == 0) {
            return Err(D::Error::custom(
                "a series has scored targets exactly when it has scored origin weeks",
            ));
        }
        if let Some(m) = &r.measured
            && m.targets != r.targets
        {
            return Err(D::Error::custom(
                "measured scores must be over all the series' scored targets",
            ));
        }
        Ok(Self {
            geography: r.geography,
            forecasts: r.forecasts,
            origin_weeks: r.origin_weeks,
            targets: r.targets,
            measured: r.measured,
        })
    }
}

impl<'de> Deserialize<'de> for SeriesBacktest {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            name: String,
            series: String,
            case_definition: CaseDefinition,
            basis: InformationBasis,
            protocol: String,
            seed: u64,
            provisional_weeks: u32,
            minimum_targets: u32,
            minimum_origin_weeks: u32,
            pooled: Option<PooledScores>,
            by_series: Vec<SeriesBacktestEntry>,
            report_path: String,
            report_sha256: Sha256Hex,
            input_sha256: Sha256Hex,
            limitations: Vec<String>,
        }
        use serde::de::Error;
        let r = Raw::deserialize(d)?;
        let checked = || -> Result<(), String> {
            non_empty("series_backtest.name", &r.name)?;
            non_empty("series_backtest.series", &r.series)?;
            non_empty("series_backtest.protocol", &r.protocol)?;
            non_empty("series_backtest.report_path", &r.report_path)?;
            if r.minimum_targets == 0 || r.minimum_origin_weeks == 0 {
                return Err("the floor for a measured skill must be at least 1".into());
            }
            if r.basis == InformationBasis::PseudoRealTime
                && !r.protocol.contains("pseudo-real-time")
            {
                return Err("a pseudo-real-time backtest must say so in its protocol".into());
            }
            if r.limitations.is_empty() {
                return Err("series_backtest.limitations must not be empty".into());
            }
            for l in &r.limitations {
                non_empty("a series_backtest limitation", l)?;
            }
            let mut seen = BTreeSet::new();
            let (mut targets, mut forecasts, mut scored_series) = (0_u64, 0_u64, 0_u64);
            for e in &r.by_series {
                if !seen.insert(e.geography) {
                    return Err(format!("series {} is listed twice", e.geography));
                }
                let reaches =
                    e.targets >= r.minimum_targets && e.origin_weeks >= r.minimum_origin_weeks;
                if reaches != e.measured.is_some() {
                    return Err(format!(
                        "series {}: a measured skill is present exactly when the floor ({} targets from {} origin weeks) is reached",
                        e.geography, r.minimum_targets, r.minimum_origin_weeks
                    ));
                }
                targets += u64::from(e.targets);
                scored_series += u64::from(e.targets > 0);
                forecasts += u64::from(e.origin_weeks);
            }
            if let Some(p) = &r.pooled {
                if p.scores.targets as u64 != targets {
                    return Err("pooled targets must equal the sum over the series".into());
                }
                if u64::from(p.series) != scored_series {
                    return Err("pooled series must be the series with a scored target".into());
                }
                if u64::from(p.forecasts) != forecasts {
                    return Err(
                        "pooled forecasts must be the sum of the series' scored origin weeks"
                            .into(),
                    );
                }
                if p.scores.targets < r.minimum_targets || p.forecasts < r.minimum_origin_weeks {
                    return Err("pooled scores are below the floor for a measured skill".into());
                }
            }
            Ok(())
        };
        checked().map_err(D::Error::custom)?;
        Ok(Self {
            name: r.name,
            series: r.series,
            case_definition: r.case_definition,
            basis: r.basis,
            protocol: r.protocol,
            seed: r.seed,
            provisional_weeks: r.provisional_weeks,
            minimum_targets: r.minimum_targets,
            minimum_origin_weeks: r.minimum_origin_weeks,
            pooled: r.pooled,
            by_series: r.by_series,
            report_path: r.report_path,
            report_sha256: r.report_sha256,
            input_sha256: r.input_sha256,
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
            series_backtest: Option<SeriesBacktest>,
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
            // The report-vintage backtest speaks only for the series it scored.
            if let Some(b) = &r.backtest {
                for s in r
                    .series
                    .iter()
                    .filter(|s| s.skill == SeriesSkill::Backtested)
                {
                    if (s.geography, s.case_definition) != (b.geography, b.case_definition) {
                        return Err(format!(
                            "series {} is marked backtested but is not the backtested series",
                            s.geography
                        ));
                    }
                }
            } else if r.series.iter().any(|s| s.skill == SeriesSkill::Backtested) {
                return Err("a series is marked backtested but there is no backtest".into());
            }
            // The series backtest speaks for a series only through its own entry: measured exactly
            // when the entry carries scores, insufficient data when it does not.
            let entries: BTreeMap<GeoId, &SeriesBacktestEntry> = r
                .series_backtest
                .iter()
                .flat_map(|b| b.by_series.iter())
                .map(|e| (e.geography, e))
                .collect();
            for s in r.series.iter().filter(|s| {
                matches!(
                    s.skill,
                    SeriesSkill::Measured | SeriesSkill::InsufficientData
                )
            }) {
                let Some(b) = &r.series_backtest else {
                    return Err(format!(
                        "series {} cites a series backtest but there is none",
                        s.geography
                    ));
                };
                if s.case_definition != b.case_definition {
                    return Err(format!(
                        "series {} is not the case definition the series backtest scored",
                        s.geography
                    ));
                }
                let Some(entry) = entries.get(&s.geography) else {
                    return Err(format!(
                        "series {} cites the series backtest but it did not run on it",
                        s.geography
                    ));
                };
                if (s.skill == SeriesSkill::Measured) != entry.measured.is_some() {
                    return Err(format!(
                        "series {}: its skill disagrees with the backtest's entry for it",
                        s.geography
                    ));
                }
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
            series_backtest: r.series_backtest,
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
