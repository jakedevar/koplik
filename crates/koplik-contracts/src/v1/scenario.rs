//! Scenario input for the SEIR engine (`koplik-epi`, `koplik-wasm`) and the web what-if panel.
//!
//! These types carry no defaults. Default parameter values and their citations live in
//! `koplik-epi`; a scenario always states every parameter it ran with, so a result is
//! reproducible from the scenario alone.

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};

use super::check_range;
use super::fips::GeoId;
use super::mmwr::MmwrWeek;

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
    /// Basic reproduction number R0 in a fully susceptible population.
    pub r0: f64,
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
        check_range("r0", self.r0, f64::MIN_POSITIVE, f64::MAX)?;
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

/// Infectious individuals placed in a geography at the start week.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InitialInfection {
    pub geography: GeoId,
    pub infectious: u32,
}

/// Everything needed to reproduce an ensemble: same input, same seed, same trajectories.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScenarioInput {
    /// Geographies simulated (non-empty, no duplicates).
    pub geographies: Vec<GeoId>,
    /// Week the simulation starts.
    pub start_week: MmwrWeek,
    /// Coverage replacements; geographies not listed use their measured baseline.
    pub coverage_overrides: Vec<CoverageOverride>,
    /// Initial infectious counts per geography.
    pub initial_infections: Vec<InitialInfection>,
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
            geographies: Vec<GeoId>,
            start_week: MmwrWeek,
            coverage_overrides: Vec<CoverageOverride>,
            initial_infections: Vec<InitialInfection>,
            parameters: SeirParameters,
            seed: u64,
            run_count: u32,
        }
        use serde::de::Error;
        let r = Raw::deserialize(d)?;
        if r.geographies.is_empty() {
            return Err(D::Error::custom("geographies must not be empty"));
        }
        let mut sorted = r.geographies.clone();
        sorted.sort();
        if sorted.windows(2).any(|w| w[0] == w[1]) {
            return Err(D::Error::custom("geographies must not repeat"));
        }
        if r.run_count == 0 {
            return Err(D::Error::custom("run_count must be at least 1"));
        }
        for o in &r.coverage_overrides {
            check_range("coverage_pct", o.coverage_pct, 0.0, 100.0).map_err(D::Error::custom)?;
            if !r.geographies.contains(&o.geography) {
                return Err(D::Error::custom(format!(
                    "override for unlisted geography {}",
                    o.geography
                )));
            }
        }
        for i in &r.initial_infections {
            if !r.geographies.contains(&i.geography) {
                return Err(D::Error::custom(format!(
                    "infection in unlisted geography {}",
                    i.geography
                )));
            }
        }
        r.parameters.validate().map_err(D::Error::custom)?;
        Ok(Self {
            geographies: r.geographies,
            start_week: r.start_week,
            coverage_overrides: r.coverage_overrides,
            initial_infections: r.initial_infections,
            parameters: r.parameters,
            seed: r.seed,
            run_count: r.run_count,
        })
    }
}
