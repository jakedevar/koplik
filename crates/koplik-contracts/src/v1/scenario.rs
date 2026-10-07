//! Scenario input for the SEIR engine (`koplik-epi`, `koplik-wasm`) and the web what-if panel.
//!
//! These types carry no defaults. Default parameter values and their citations live in
//! `koplik-epi`; a scenario always states every parameter it ran with, so a result is
//! reproducible from the scenario alone.

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};

use super::check_range;
use super::fips::GeoId;
use super::geography::Centroid;
use super::mmwr::MmwrWeek;
use super::provenance::Provenances;
use super::weekly_cases::MissingReason;

/// Basic reproduction number R0 in a fully susceptible population: a fixed value, or a
/// uniform prior that the engine samples once per ensemble member (uniform on `[min, max]`,
/// drawn from that member's derived seed before its first step). The sampling lives in
/// `koplik-epi`; this type only states the choice, so a scenario is reproducible from its
/// seed. E.g. the spec's R0 prior of 12 to 18 is `uniform_prior { min: 12, max: 18 }`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum R0 {
    /// Every member uses this value (`value > 0`).
    Fixed { value: f64 },
    /// Each member draws its own R0 uniformly from `[min, max]` (`0 < min <= max`).
    UniformPrior { min: f64, max: f64 },
}

impl R0 {
    pub fn validate(&self) -> Result<(), String> {
        match *self {
            R0::Fixed { value } => check_range("r0.value", value, f64::MIN_POSITIVE, f64::MAX),
            R0::UniformPrior { min, max } => {
                check_range("r0.min", min, f64::MIN_POSITIVE, f64::MAX)?;
                check_range("r0.max", max, f64::MIN_POSITIVE, f64::MAX)?;
                if min > max {
                    return Err(format!("r0 prior min {min} exceeds max {max}"));
                }
                Ok(())
            }
        }
    }
}

impl<'de> Deserialize<'de> for R0 {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
        enum Raw {
            Fixed { value: f64 },
            UniformPrior { min: f64, max: f64 },
        }
        let r = match Raw::deserialize(d)? {
            Raw::Fixed { value } => R0::Fixed { value },
            Raw::UniformPrior { min, max } => R0::UniformPrior { min, max },
        };
        r.validate().map_err(serde::de::Error::custom)?;
        Ok(r)
    }
}

/// Gravity-model coupling between geographies: flow from i to j is
/// `scale * pop_i^origin_exponent * pop_j^destination_exponent / distance_km^distance_exponent`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GravityParameters {
    pub scale: f64,
    pub origin_exponent: f64,
    pub destination_exponent: f64,
    pub distance_exponent: f64,
}

/// Full SEIR parameter set. Every value is configurable; none is hard-coded in the engine.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SeirParameters {
    /// Basic reproduction number R0: fixed, or a prior sampled once per ensemble member.
    pub r0: R0,
    /// Mean latent (exposed, not yet infectious) period in days.
    pub latent_period_days: f64,
    /// Mean infectious period in days.
    pub infectious_period_days: f64,
    /// Protection from one MMR dose, in [0, 1].
    pub mmr_effectiveness_one_dose: f64,
    /// Protection from two MMR doses, in [0, 1]; applied to kindergarten MMR coverage.
    pub mmr_effectiveness_two_doses: f64,
    /// Tau-leap step length in days.
    pub time_step_days: f64,
    /// Simulated horizon in days.
    pub horizon_days: u32,
    /// Coupling between geographies; `null` runs each geography in isolation.
    pub gravity: Option<GravityParameters>,
}

impl SeirParameters {
    pub fn validate(&self) -> Result<(), String> {
        self.r0.validate()?;
        check_range(
            "latent_period_days",
            self.latent_period_days,
            f64::MIN_POSITIVE,
            f64::MAX,
        )?;
        check_range(
            "infectious_period_days",
            self.infectious_period_days,
            f64::MIN_POSITIVE,
            f64::MAX,
        )?;
        check_range(
            "mmr_effectiveness_one_dose",
            self.mmr_effectiveness_one_dose,
            0.0,
            1.0,
        )?;
        check_range(
            "mmr_effectiveness_two_doses",
            self.mmr_effectiveness_two_doses,
            0.0,
            1.0,
        )?;
        check_range(
            "time_step_days",
            self.time_step_days,
            f64::MIN_POSITIVE,
            f64::MAX,
        )?;
        if self.horizon_days == 0 {
            return Err("horizon_days must be at least 1".into());
        }
        if let Some(g) = &self.gravity {
            for (n, v) in [
                ("gravity.scale", g.scale),
                ("gravity.origin_exponent", g.origin_exponent),
                ("gravity.destination_exponent", g.destination_exponent),
                ("gravity.distance_exponent", g.distance_exponent),
            ] {
                check_range(n, v, 0.0, f64::MAX)?;
            }
        }
        Ok(())
    }
}

/// Replace a geography's baseline kindergarten MMR coverage in a what-if run.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CoverageOverride {
    pub geography: GeoId,
    /// Coverage percent, 0 to 100.
    #[schemars(range(min = 0, max = 100))]
    pub coverage_pct: f64,
}

/// Baseline kindergarten MMR coverage of a node, or an explicit missing value. A node with
/// missing coverage is only valid when a `coverage_overrides` entry supplies its value.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum BaselineCoverage {
    Reported {
        /// Coverage percent, 0 to 100.
        #[schemars(range(min = 0, max = 100))]
        coverage_pct: f64,
        /// True when the value was imputed rather than measured.
        imputed: bool,
        provenance: Provenances,
    },
    Missing {
        reason: MissingReason,
    },
}

impl<'de> Deserialize<'de> for BaselineCoverage {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
        enum Raw {
            Reported {
                coverage_pct: f64,
                imputed: bool,
                provenance: Provenances,
            },
            Missing {
                reason: MissingReason,
            },
        }
        Ok(match Raw::deserialize(d)? {
            Raw::Reported {
                coverage_pct,
                imputed,
                provenance,
            } => {
                check_range("coverage_pct", coverage_pct, 0.0, 100.0)
                    .map_err(serde::de::Error::custom)?;
                BaselineCoverage::Reported {
                    coverage_pct,
                    imputed,
                    provenance,
                }
            }
            Raw::Missing { reason } => BaselineCoverage::Missing { reason },
        })
    }
}

/// One simulated geography with everything the engine needs about it.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScenarioNode {
    pub id: GeoId,
    /// Resident population (`> 0`).
    pub population: u64,
    pub baseline_coverage: BaselineCoverage,
    /// Location used for gravity coupling.
    pub centroid: Centroid,
    /// Exposed (latent) individuals at the start week.
    pub initial_exposed: u32,
    /// Infectious individuals at the start week. Exposed plus infectious must not exceed
    /// the population.
    pub initial_infectious: u32,
}

impl<'de> Deserialize<'de> for ScenarioNode {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            id: GeoId,
            population: u64,
            baseline_coverage: BaselineCoverage,
            centroid: Centroid,
            initial_exposed: u32,
            initial_infectious: u32,
        }
        use serde::de::Error;
        let r = Raw::deserialize(d)?;
        if r.population == 0 {
            return Err(D::Error::custom(format!(
                "population of {} must be > 0",
                r.id
            )));
        }
        if u64::from(r.initial_exposed) + u64::from(r.initial_infectious) > r.population {
            return Err(D::Error::custom(format!(
                "initial exposed + infectious exceed the population of {}",
                r.id
            )));
        }
        Ok(Self {
            id: r.id,
            population: r.population,
            baseline_coverage: r.baseline_coverage,
            centroid: r.centroid,
            initial_exposed: r.initial_exposed,
            initial_infectious: r.initial_infectious,
        })
    }
}

/// Everything needed to reproduce an ensemble: same input, same seed, same trajectories.
/// It is self-contained: the engine needs nothing else (no data lookups).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScenarioInput {
    /// Simulated geographies: non-empty, unique, in canonical order (`GeoId` order: states
    /// by FIPS code, then counties by FIPS code), so output order never depends on input order.
    pub nodes: Vec<ScenarioNode>,
    /// Week the simulation starts.
    pub start_week: MmwrWeek,
    /// Coverage replacements (what-if slider); each names a node and appears at most once.
    pub coverage_overrides: Vec<CoverageOverride>,
    pub parameters: SeirParameters,
    /// RNG seed. Run `k` derives its own seed from this and `k`.
    pub seed: u64,
    /// Number of ensemble members (at least 1).
    pub run_count: u32,
}

impl<'de> Deserialize<'de> for ScenarioInput {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            nodes: Vec<ScenarioNode>,
            start_week: MmwrWeek,
            coverage_overrides: Vec<CoverageOverride>,
            parameters: SeirParameters,
            seed: u64,
            run_count: u32,
        }
        use serde::de::Error;
        let r = Raw::deserialize(d)?;
        if r.nodes.is_empty() {
            return Err(D::Error::custom("nodes must not be empty"));
        }
        for w in r.nodes.windows(2) {
            if w[0].id == w[1].id {
                return Err(D::Error::custom(format!("duplicate node {}", w[0].id)));
            }
            if w[0].id > w[1].id {
                return Err(D::Error::custom(format!(
                    "nodes must be sorted by FIPS: {} comes before {}",
                    w[0].id, w[1].id
                )));
            }
        }
        if r.run_count == 0 {
            return Err(D::Error::custom("run_count must be at least 1"));
        }
        let mut overridden = Vec::new();
        for o in &r.coverage_overrides {
            check_range("coverage_pct", o.coverage_pct, 0.0, 100.0).map_err(D::Error::custom)?;
            if !r.nodes.iter().any(|n| n.id == o.geography) {
                return Err(D::Error::custom(format!(
                    "override for unlisted node {}",
                    o.geography
                )));
            }
            if overridden.contains(&o.geography) {
                return Err(D::Error::custom(format!(
                    "duplicate override for {}",
                    o.geography
                )));
            }
            overridden.push(o.geography);
        }
        for n in &r.nodes {
            if matches!(n.baseline_coverage, BaselineCoverage::Missing { .. })
                && !overridden.contains(&n.id)
            {
                return Err(D::Error::custom(format!(
                    "node {} has missing baseline coverage and no override",
                    n.id
                )));
            }
        }
        r.parameters.validate().map_err(D::Error::custom)?;
        Ok(Self {
            nodes: r.nodes,
            start_week: r.start_week,
            coverage_overrides: r.coverage_overrides,
            parameters: r.parameters,
            seed: r.seed,
            run_count: r.run_count,
        })
    }
}
