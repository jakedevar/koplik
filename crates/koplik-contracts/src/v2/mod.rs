//! Contract v2 adds simulation results; all v1 inputs and rows keep their shapes.
pub use crate::v1::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const VERSION: &str = "v2";
/// Stable output tag; adding a newer contract must not retag v2 results.
pub const SIMULATION_CONTRACT_VERSION: u32 = 2;

/// Equal-tail predictive bands, Hyndman-Fan type 7; not median confidence intervals.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SimulationBand {
    pub median: f64,
    pub lower_50: f64,
    pub upper_50: f64,
    pub lower_90: f64,
    pub upper_90: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SimulationDay {
    pub day: u32,
    pub geography: GeoId,
    pub susceptible: SimulationBand,
    pub exposed: SimulationBand,
    pub infectious: SimulationBand,
    /// Includes vaccine immunity as well as recovery.
    pub recovered: SimulationBand,
    /// S(day-1)-S(day), zero at day 0. Exposures, not reported cases.
    pub new_exposures: SimulationBand,
    /// S(0)-S(day)+E(0)+I(0); excludes vaccine immunity.
    pub cumulative_infections: SimulationBand,
}

/// Counts are exactly representable as JS numbers (engine caps population at 2^53).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SimulationCompartments {
    pub susceptible: u64,
    pub exposed: u64,
    pub infectious: u64,
    pub recovered: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SimulationStep {
    pub day: f64,
    /// Canonical GeoId order, matching the result's geographies.
    pub nodes: Vec<SimulationCompartments>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EnsembleMember {
    pub member: u32,
    /// Portable ChaCha8 initialization bytes from derive_seed(base_seed, member).
    pub derived_seed: [u8; 32],
    pub r0: f64,
    /// SHA256 of all S/E/I/R counts, including day 0 and every leap, as LE f64.
    pub fingerprint: Sha256Hex,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TrajectoryResult {
    pub contract_version: u32,
    /// Exact v1 input JSON, including source provenance; replay this string directly.
    pub scenario_json: String,
    /// Base u64 seed as decimal text to avoid JavaScript number rounding.
    pub seed: String,
    pub parameters: SeirParameters,
    pub member: EnsembleMember,
    pub geographies: Vec<GeoId>,
    /// Initial state and every leap, not just daily summaries.
    pub steps: Vec<SimulationStep>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EnsembleResult {
    pub contract_version: u32,
    pub scenario_json: String,
    pub seed: String,
    pub parameters: SeirParameters,
    /// Member 0 full-trajectory fingerprint, NOT a hash of the median or ensemble.
    /// Individual member fingerprints are also retained below.
    pub fingerprint: Sha256Hex,
    /// Ascending member index; carries sampled R0 and derived seed for every run.
    pub members: Vec<EnsembleMember>,
    /// Ascending day, then canonical GeoId. Includes day 0.
    pub daily: Vec<SimulationDay>,
}
