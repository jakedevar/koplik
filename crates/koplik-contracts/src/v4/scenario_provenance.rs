use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};

use super::{GeoId, MmwrWeek, ScenarioInput};

/// Tag every v4 scenario provenance carries.
pub const SCENARIO_PROVENANCE_VERSION: u32 = 4;

/// How the scenario is seeded: a stated assumption, never data. The scenario is a
/// hypothetical introduction, not a reconstruction of an observed outbreak.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SeedingAssumption {
    /// The geography the infectious people are introduced into; every other node starts
    /// with no exposed or infectious people.
    pub geography: GeoId,
    /// Infectious people at the start week.
    pub initial_infectious: u32,
    /// Exposed (latent) people at the start week.
    pub initial_exposed: u32,
    /// The week the simulation starts, stated as an assumption (not derived from any case
    /// report).
    pub start_week: MmwrWeek,
    /// The assumption in words (non-empty).
    pub assumption: String,
    /// Why this start week, and what it does and does not mean (non-empty).
    pub start_week_basis: String,
    /// What the seeding cannot capture, stated to the reader (non-empty).
    pub limitation: String,
}

/// One model parameter exactly as the scenario ran it, with its citation (#1400).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ParameterProvenance {
    /// The `SeirParameters` field name.
    pub parameter: String,
    /// The value the scenario ran with, as it appears in the scenario's `parameters`.
    pub value: serde_json::Value,
    /// The published source (non-empty).
    pub source: String,
    /// Where to read it, when the source is a public page.
    pub url: Option<String>,
    /// What the source supports and what it does not (non-empty).
    pub note: String,
}

/// What a node's inputs are, beyond the source records the scenario already carries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NodeInputs {
    pub geography: GeoId,
    pub name: String,
    #[schemars(range(min = 1))]
    pub population: u64,
    pub population_basis: String,
    pub centroid_basis: String,
    /// School year of the baseline kindergarten MMR coverage, e.g. `2023-24`.
    pub coverage_school_year: String,
    pub coverage_basis: String,
}

/// A county the neighbourhood rule would have simulated but could not, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExcludedNode {
    pub geography: GeoId,
    pub name: String,
    pub reason: String,
}

/// Companion of a what-if [`ScenarioInput`]: where its inputs came from, the seeding stated as
/// an assumption, and every parameter's citation. A scenario is only published beside a
/// companion for which [`ScenarioProvenance::check_against`] holds.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScenarioProvenance {
    /// Always [`SCENARIO_PROVENANCE_VERSION`].
    pub contract_version: u32,
    /// Artifact name, e.g. `gaines-2025`.
    pub scenario: String,
    /// Plain-words statement of what the scenario is and is not (non-empty).
    pub statement: String,
    /// The scenario's RNG seed as decimal text (a JavaScript number would round a large u64).
    #[schemars(regex(pattern = r"^(0|[1-9][0-9]{0,19})$"))]
    pub seed: String,
    #[schemars(range(min = 1))]
    pub run_count: u32,
    pub seeding: SeedingAssumption,
    /// One entry per `SeirParameters` field, in field order (non-empty, unique).
    pub parameters: Vec<ParameterProvenance>,
    /// The simulated nodes, in the scenario's canonical order.
    pub nodes: Vec<NodeInputs>,
    /// Counties the neighbourhood rule would have simulated but could not.
    pub excluded_nodes: Vec<ExcludedNode>,
    /// Why the node set is what it is (non-empty).
    pub neighbourhood_note: String,
}

fn non_empty(name: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!("{name} must not be empty"))
    } else {
        Ok(())
    }
}

impl<'de> Deserialize<'de> for SeedingAssumption {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            geography: GeoId,
            initial_infectious: u32,
            initial_exposed: u32,
            start_week: MmwrWeek,
            assumption: String,
            start_week_basis: String,
            limitation: String,
        }
        use serde::de::Error;
        let r = Raw::deserialize(d)?;
        non_empty("seeding.assumption", &r.assumption).map_err(D::Error::custom)?;
        non_empty("seeding.start_week_basis", &r.start_week_basis).map_err(D::Error::custom)?;
        non_empty("seeding.limitation", &r.limitation).map_err(D::Error::custom)?;
        Ok(Self {
            geography: r.geography,
            initial_infectious: r.initial_infectious,
            initial_exposed: r.initial_exposed,
            start_week: r.start_week,
            assumption: r.assumption,
            start_week_basis: r.start_week_basis,
            limitation: r.limitation,
        })
    }
}

impl<'de> Deserialize<'de> for ScenarioProvenance {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            contract_version: u32,
            scenario: String,
            statement: String,
            seed: String,
            run_count: u32,
            seeding: SeedingAssumption,
            parameters: Vec<ParameterProvenance>,
            nodes: Vec<NodeInputs>,
            excluded_nodes: Vec<ExcludedNode>,
            neighbourhood_note: String,
        }
        use serde::de::Error;
        let r = Raw::deserialize(d)?;
        if r.contract_version != SCENARIO_PROVENANCE_VERSION {
            return Err(D::Error::custom(format!(
                "contract_version must be {SCENARIO_PROVENANCE_VERSION}, got {}",
                r.contract_version
            )));
        }
        non_empty("scenario", &r.scenario).map_err(D::Error::custom)?;
        non_empty("statement", &r.statement).map_err(D::Error::custom)?;
        non_empty("neighbourhood_note", &r.neighbourhood_note).map_err(D::Error::custom)?;
        let canonical = r.seed.parse::<u64>().is_ok_and(|n| n.to_string() == r.seed);
        if !canonical {
            return Err(D::Error::custom(
                "seed must be an unsigned 64-bit integer in decimal text",
            ));
        }
        if r.run_count == 0 {
            return Err(D::Error::custom("run_count must be at least 1"));
        }
        if r.parameters.is_empty() {
            return Err(D::Error::custom("parameters must not be empty"));
        }
        let mut names = BTreeSet::new();
        for p in &r.parameters {
            non_empty("parameter name", &p.parameter).map_err(D::Error::custom)?;
            non_empty(&format!("{} source", p.parameter), &p.source).map_err(D::Error::custom)?;
            non_empty(&format!("{} note", p.parameter), &p.note).map_err(D::Error::custom)?;
            if !names.insert(p.parameter.as_str()) {
                return Err(D::Error::custom(format!(
                    "parameter {} is cited twice",
                    p.parameter
                )));
            }
        }
        if r.nodes.is_empty() {
            return Err(D::Error::custom("nodes must not be empty"));
        }
        for n in &r.nodes {
            if n.population == 0 {
                return Err(D::Error::custom(format!(
                    "population of {} must be > 0",
                    n.geography
                )));
            }
        }
        Ok(Self {
            contract_version: r.contract_version,
            scenario: r.scenario,
            statement: r.statement,
            seed: r.seed,
            run_count: r.run_count,
            seeding: r.seeding,
            parameters: r.parameters,
            nodes: r.nodes,
            excluded_nodes: r.excluded_nodes,
            neighbourhood_note: r.neighbourhood_note,
        })
    }
}

impl ScenarioProvenance {
    /// Whether this companion describes `input`: the same seed, run count, start week, node set
    /// and populations, the same seeding (and no other node seeded), and a citation for every
    /// parameter with the value the scenario runs with. A scenario is only published beside a
    /// companion that agrees with it.
    pub fn check_against(&self, input: &ScenarioInput) -> Result<(), String> {
        let differs = |what: &str| {
            Err(format!(
                "the provenance companion does not describe the scenario beside it ({what})"
            ))
        };
        if self.seed != input.seed.to_string() {
            return differs("seed");
        }
        if self.run_count != input.run_count {
            return differs("run_count");
        }
        if self.seeding.start_week != input.start_week {
            return differs("start week");
        }
        let seeded = input
            .nodes
            .iter()
            .find(|n| n.id == self.seeding.geography)
            .ok_or("the seeded geography is not a node of the scenario")?;
        if (seeded.initial_infectious, seeded.initial_exposed)
            != (
                self.seeding.initial_infectious,
                self.seeding.initial_exposed,
            )
        {
            return differs("seeding");
        }
        if input.nodes.iter().any(|n| {
            n.id != self.seeding.geography && (n.initial_infectious, n.initial_exposed) != (0, 0)
        }) {
            return differs("another node is seeded");
        }
        if self.nodes.len() != input.nodes.len()
            || !self
                .nodes
                .iter()
                .zip(&input.nodes)
                .all(|(a, b)| a.geography == b.id && a.population == b.population)
        {
            return differs("nodes");
        }
        let values = serde_json::to_value(input.parameters).map_err(|e| e.to_string())?;
        let stated = values.as_object().ok_or("parameters are not an object")?;
        if stated.len() != self.parameters.len()
            || !self
                .parameters
                .iter()
                .all(|p| stated.get(&p.parameter) == Some(&p.value))
        {
            return differs("parameters");
        }
        Ok(())
    }
}
