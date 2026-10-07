//! The Gaines County what-if scenario (#1455): the rule that builds its v1 `ScenarioInput`
//! from snapshot-store inputs, and the configuration that rule runs with.
//!
//! # PRE-REGISTERED RULE v2 (committed before the scenario was rebuilt or run under it)
//!
//! **What the scenario is.** A *hypothetical introduction*: what could happen if one
//! infectious person arrived in Gaines County, given its population and kindergarten MMR
//! coverage. It is not a reconstruction or forecast of the 2025 outbreak, and it is not a
//! fitted model. Its output is never compared with, tuned to, or scored against reported cases.
//!
//! **Why not a replay (correction of rule v1, 2026-10-07).** Rule v1 seeded the simulation with
//! the confirmed count of the earliest retained DSHS report (107 on 2025-03-04) as the
//! *currently infectious* people. That was wrong from first principles: DSHS counts are
//! cumulative since the outbreak began, and a cumulative count is not the number of people
//! infectious now (the infectious period is about 8 days). The retained report vintages cannot
//! repair it: the closest pair with county tables, 2025-03-04 and 2025-03-25, is 21 days apart,
//! so no pair spans one infectious period, and interpolating between them would be imputation.
//! A data-derived seeding of the historical outbreak is therefore not possible from what is
//! held ("insufficient data", AGENTS.md rule 5). Rule v1 was dropped, not tuned; the original
//! rule and its measured replay stay in `thoughts/shared/notes/gaines-2025-scenario-replay.md`.
//!
//! ## Seeding (a stated assumption, not data; configurable)
//! * *Initial infectious* = [`ScenarioConfig::initial_infectious`], default **1**: a single
//!   introduced case, in the focus county. This is an assumption chosen for the hypothetical;
//!   no source supports "1" and none is claimed.
//! * *Initial exposed* = [`ScenarioConfig::initial_exposed`], default **0**.
//! * Every other node starts with zero exposed and infectious people.
//! * *Start week*: a neutral reference week, stated as an assumption. Default: the MMWR week
//!   containing July 1 of the Census population estimate's year (2025-W27), the estimate's
//!   own reference date. The engine has no seasonality, so the start week only labels day 0 of
//!   the calendar and does not change any result. It is not a claim about when the 2025
//!   outbreak began. Configurable through [`ScenarioConfig::start_week`].
//! * The DSHS report vintages are **not** inputs of the scenario.
//!
//! ## Nodes
//! * Default: **Gaines County alone** (FIPS 48165). The engine's defaults carry no gravity
//!   coupling (`koplik_epi::default_parameters().gravity` is `None`: "no calibrated Texas
//!   mobility parameters exist in this engine", `koplik-epi/src/defaults.rs`; the Xia,
//!   Bjørnstad & Grenfell 2004 and Bharti et al. 2008 fits come from other settings and
//!   `gravity.rs` declines to transplant them to Texas). With coupling off, any other county would
//!   be an inert isolated node, so none is published: "the counties the gravity coupling
//!   needs" is the empty set. This is a documented consequence of the cited defaults, not an
//!   omission.
//! * Optional ([`Neighbourhood`], default `None`): every Texas county whose Gazetteer internal
//!   point lies within `radius_km` of Gaines' (haversine, GRS80 mean radius as in
//!   `koplik-epi/src/gravity.rs`), coupled by the *explicit* gravity parameters the caller
//!   supplies. A neighbourhood cannot be set without gravity (the type enforces it), so an
//!   inert neighbourhood is never published. A neighbour is a node only if it has a population, a Gazetteer
//!   centroid **and a reported baseline coverage row**; a county missing any of them is
//!   **excluded and listed in the provenance artifact**, never imputed. (The v1 contract can
//!   only carry a missing coverage together with an override, which would be an invented
//!   baseline, so exclusion is the honest choice.)
//!
//! ## Node inputs (every one from a snapshot in the store)
//! * *Population*: U.S. Census Bureau Vintage 2025 county population estimates,
//!   `POPESTIMATE2025` (July 1, 2025), `co-est2025-alldata.csv`.
//! * *Centroid*: Census 2025 Gazetteer county **internal point** (`INTPTLAT`, `INTPTLONG`), a
//!   representative point, not a population-weighted or geometric centroid.
//! * *Baseline coverage*: Texas DSHS kindergarten MMR coverage for the school year
//!   [`koplik_ingest::coverage::TEXAS_BASELINE_YEAR`] (2023-24, "last completed school year
//!   before the 2025 outbreak"), as published, `imputed: false`.
//!
//! ## Parameters, seed, runs
//! * `parameters` = `koplik_epi::default_parameters()` exactly (R0 uniform prior 12 to 18,
//!   latent 10 d, infectious 8 d, MMR effectiveness 93% / 97%, daily step, 180-day horizon,
//!   no gravity), each with the citation in `koplik-epi/src/defaults.rs`. Their citations are
//!   published beside the scenario (`scenarios/gaines-2025.provenance.json`, #1400).
//! * `seed` = [`SEED`], an arbitrary fixed constant chosen once before any run; changing it
//!   changes only the Monte Carlo draw, never the model. `run_count` = 1000 (spec E6).
//!
//! ## Changing this rule
//! A change to a default is a change to the published method: it needs a reason that does not
//! depend on what the output looks like, a review, and a dated note. It is never made to make
//! the output resemble reported counts.

use std::collections::BTreeMap;

use chrono::NaiveDate;
use koplik_contracts::v1::{
    BaselineCoverage, Centroid, CountyFips, CoverageValue, GeoId, Geography, GravityParameters,
    KindergartenMmrCoverage, MmwrWeek, Population, Provenances, ScenarioInput, ScenarioNode,
};
use koplik_contracts::v4::{
    ExcludedNode, NodeInputs, ParameterProvenance, SCENARIO_PROVENANCE_VERSION, ScenarioProvenance,
    SeedingAssumption,
};
use koplik_ingest::coverage;

/// Gaines County, Texas (state FIPS 48, county 165).
pub const GAINES_FIPS: u32 = 48165;

/// Fixed RNG seed of the published scenario. Arbitrary: chosen once, before any run, and never
/// adjusted to a result (it changes only the Monte Carlo draw, never the model).
pub const SEED: u64 = 20_250_304;

/// Ensemble size the web panel runs and the scenario states (spec E6).
pub const RUN_COUNT: u32 = 1000;

/// Texas counties near the focus county that the scenario also simulates, coupled by gravity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Neighbourhood {
    /// Gazetteer internal-point distance from the focus county, in km.
    pub radius_km: f64,
    /// Explicit gravity coupling; there is no cited Texas value, so there is no default.
    pub gravity: GravityParameters,
}

/// Every choice the scenario builder makes that is not read from a source.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScenarioConfig {
    /// County FIPS (`48xxx`) of the county the infectious people are introduced into.
    pub focus: u32,
    /// RNG seed written into the scenario.
    pub seed: u64,
    /// Ensemble size written into the scenario.
    pub run_count: u32,
    /// Texas school year start of the baseline kindergarten MMR coverage.
    pub coverage_year: u16,
    /// Infectious people introduced into the focus county: a stated assumption, default 1.
    pub initial_infectious: u32,
    /// Exposed people introduced into the focus county: a stated assumption, default 0.
    pub initial_exposed: u32,
    /// The reference week day 0 is labelled with; `None` = the MMWR week containing July 1 of
    /// the population estimate's year (the estimate's own reference date).
    pub start_week: Option<MmwrWeek>,
    /// Neighbouring counties to simulate and couple; `None` = the focus county alone.
    pub neighbourhood: Option<Neighbourhood>,
}

impl Default for ScenarioConfig {
    fn default() -> Self {
        Self {
            focus: GAINES_FIPS,
            seed: SEED,
            run_count: RUN_COUNT,
            coverage_year: coverage::TEXAS_BASELINE_YEAR,
            initial_infectious: 1,
            initial_exposed: 0,
            start_week: None,
            neighbourhood: None,
        }
    }
}

/// Why the scenario cannot be built: an input is absent (shown as missing, never guessed) or
/// something is inconsistent (a bug or a corrupt input).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScenarioError {
    Missing(String),
    Invalid(String),
}

fn missing(reason: impl Into<String>) -> ScenarioError {
    ScenarioError::Missing(reason.into())
}
fn invalid(reason: impl Into<String>) -> ScenarioError {
    ScenarioError::Invalid(reason.into())
}

/// Everything the builder reads, already parsed from store snapshots.
pub struct Sources<'a> {
    /// Census population estimates of the Texas counties.
    pub populations: &'a [Population],
    /// Census Gazetteer rows (name, internal point) of the Texas counties.
    pub gazetteer: &'a [Geography],
    /// Kindergarten MMR coverage rows (every geography and school year; filtered here).
    pub coverage: &'a [KindergartenMmrCoverage],
}

/// The built scenario and its companion.
#[derive(Debug, Clone, PartialEq)]
pub struct Built {
    pub input: ScenarioInput,
    pub provenance: ScenarioProvenance,
}

/// A node's sources, or why this county cannot be a node.
struct NodeParts {
    node: ScenarioNode,
    inputs: NodeInputs,
    /// Year of the population estimate (its reference date is July 1 of it).
    population_year: u16,
}

fn node_parts(
    fips: CountyFips,
    config: &ScenarioConfig,
    sources: &Sources<'_>,
    initial_exposed: u32,
    initial_infectious: u32,
) -> std::result::Result<NodeParts, String> {
    let id = GeoId::County(fips);
    let population = sources
        .populations
        .iter()
        .find(|p| p.geography == id)
        .ok_or("no Census population estimate")?;
    let gazetteer = sources
        .gazetteer
        .iter()
        .find(|g| g.id == id)
        .ok_or("no Census Gazetteer row")?;
    let centroid = gazetteer
        .centroid
        .ok_or("the Gazetteer has no internal point")?;
    let row = sources
        .coverage
        .iter()
        .find(|r| r.geography == id && r.school_year.start_year() == config.coverage_year)
        .ok_or("no kindergarten MMR coverage row for the baseline school year")?;
    let (coverage_pct, coverage_provenance) = match row.coverage {
        CoverageValue::Reported { coverage_pct, .. } => (coverage_pct, row.provenance.clone()),
        CoverageValue::Missing { reason } => {
            return Err(format!(
                "baseline kindergarten MMR coverage is missing ({reason:?})"
            ));
        }
    };
    let mut records = population.provenance.as_slice().to_vec();
    for p in gazetteer.provenance.as_slice() {
        if !records.contains(p) {
            records.push(p.clone());
        }
    }
    Ok(NodeParts {
        node: ScenarioNode {
            id,
            population: population.count,
            baseline_coverage: BaselineCoverage::Reported {
                coverage_pct,
                imputed: false,
                imputation_method: None,
                provenance: coverage_provenance,
            },
            centroid,
            initial_exposed,
            initial_infectious,
            provenance: Provenances::new(records).map_err(|e| e.to_string())?,
        },
        inputs: NodeInputs {
            geography: id,
            name: gazetteer.name.clone(),
            population: population.count,
            population_basis: format!(
                "U.S. Census Bureau Vintage {} county population estimate, July 1, {} (POPESTIMATE{}); not back-cast to any other date",
                population.year, population.year, population.year
            ),
            centroid_basis: "Census 2025 Gazetteer county internal point (INTPTLAT, INTPTLONG): a representative point, not a population-weighted or geometric centroid".into(),
            coverage_school_year: row.school_year.to_string(),
            coverage_basis: "Texas DSHS kindergarten MMR coverage for the last completed school year before the 2025 outbreak, as published (not imputed)".into(),
        },
        population_year: population.year,
    })
}

/// "one infectious person", "3 infectious people and 2 exposed people", ...
fn introduced(infectious: u32, exposed: u32) -> String {
    let people = |n: u32, kind: &str| {
        if n == 1 {
            format!("one {kind} person")
        } else {
            format!("{n} {kind} people")
        }
    };
    match (infectious, exposed) {
        (i, 0) => people(i, "infectious"),
        (0, e) => people(e, "exposed"),
        (i, e) => format!("{} and {}", people(i, "infectious"), people(e, "exposed")),
    }
}

/// Build the scenario by the pre-registered rule (module docs), validate it through the
/// contract and the engine, and describe where everything came from.
pub fn build(
    config: &ScenarioConfig,
    sources: &Sources<'_>,
) -> std::result::Result<Built, ScenarioError> {
    let focus = CountyFips::new(config.focus).map_err(|e| invalid(e.to_string()))?;
    if config.initial_infectious == 0 && config.initial_exposed == 0 {
        return Err(invalid(
            "the scenario must introduce at least one infectious or exposed person",
        ));
    }

    let focus_parts = node_parts(
        focus,
        config,
        sources,
        config.initial_exposed,
        config.initial_infectious,
    )
    .map_err(|reason| missing(format!("focus county {focus}: {reason}")))?;
    let focus_centroid: Centroid = focus_parts.node.centroid;
    let focus_name = focus_parts.inputs.name.clone();
    let population_year = focus_parts.population_year;
    let start_week = match config.start_week {
        Some(w) => w,
        None => {
            let reference = NaiveDate::from_ymd_opt(i32::from(population_year), 7, 1)
                .ok_or_else(|| invalid("population year has no July 1"))?;
            MmwrWeek::from_date(reference).map_err(|e| invalid(e.to_string()))?
        }
    };
    let mut parts: BTreeMap<GeoId, NodeParts> = BTreeMap::new();
    parts.insert(GeoId::County(focus), focus_parts);
    let mut excluded = Vec::new();
    let mut parameters = koplik_epi::default_parameters();
    if let Some(n) = &config.neighbourhood {
        if !n.radius_km.is_finite() || n.radius_km <= 0.0 {
            return Err(invalid(
                "neighbourhood radius must be a positive number of km",
            ));
        }
        parameters.gravity = Some(n.gravity);
        for g in sources.gazetteer {
            let GeoId::County(fips) = g.id else { continue };
            if fips == focus {
                continue;
            }
            // A county the Gazetteer gives no point for cannot be placed, so it cannot be
            // measured against the radius either; it is listed, never assumed near or far.
            match g.centroid {
                None => excluded.push(ExcludedNode {
                    geography: g.id,
                    name: g.name.clone(),
                    reason: "the Gazetteer has no internal point, so its distance is unknown"
                        .into(),
                }),
                Some(c) if koplik_epi::distance_km(focus_centroid, c) <= n.radius_km => {
                    match node_parts(fips, config, sources, 0, 0) {
                        Ok(p) => {
                            parts.insert(g.id, p);
                        }
                        Err(reason) => excluded.push(ExcludedNode {
                            geography: g.id,
                            name: g.name.clone(),
                            reason: format!("within {} km but {reason}", n.radius_km),
                        }),
                    }
                }
                Some(_) => {}
            }
        }
        excluded.sort_by_key(|e| e.geography);
    }

    let mut nodes = Vec::new();
    let mut node_inputs = Vec::new();
    for p in parts.into_values() {
        nodes.push(p.node);
        node_inputs.push(p.inputs);
    }
    let input = ScenarioInput {
        nodes,
        start_week,
        coverage_overrides: Vec::new(),
        parameters,
        seed: config.seed,
        run_count: config.run_count,
    };
    // The published bytes must satisfy the contract's own validation and the engine.
    let bytes = serde_json::to_vec(&input).map_err(|e| invalid(e.to_string()))?;
    let input: ScenarioInput =
        serde_json::from_slice(&bytes).map_err(|e| invalid(format!("contract validation: {e}")))?;
    koplik_epi::simulate_member(&input, 0).map_err(|e| invalid(format!("engine: {e}")))?;

    let parameter_values =
        serde_json::to_value(input.parameters).map_err(|e| invalid(e.to_string()))?;
    let provenance_parameters = koplik_epi::parameter_citations()
        .into_iter()
        .map(|c| ParameterProvenance {
            parameter: c.parameter.to_owned(),
            value: parameter_values[c.parameter].clone(),
            source: c.source.to_owned(),
            url: c.url.map(str::to_owned),
            note: c.note.to_owned(),
        })
        .collect();
    let who = introduced(config.initial_infectious, config.initial_exposed);
    let default_start = config.start_week.is_none();
    let provenance = ScenarioProvenance {
        contract_version: SCENARIO_PROVENANCE_VERSION,
        scenario: crate::SCENARIO_ARTIFACT.to_owned(),
        statement: format!(
            "Hypothetical: what could happen if {who} arrived in {focus_name}, given its population and kindergarten MMR coverage. This is not a reconstruction or forecast of the 2025 outbreak."
        ),
        seed: input.seed.to_string(),
        run_count: input.run_count,
        seeding: SeedingAssumption {
            geography: GeoId::County(focus),
            initial_infectious: config.initial_infectious,
            initial_exposed: config.initial_exposed,
            start_week,
            assumption: format!(
                "{who} are introduced into {focus_name} at the start week, and every other simulated county starts with none. This is a stated assumption for the hypothetical, not data: no source supports these counts and none is claimed, and it is not derived from any case report."
            ),
            start_week_basis: if default_start {
                format!(
                    "A neutral reference week: the MMWR week containing July 1, {population_year}, the reference date of the Census population estimate. The engine has no seasonality, so the start week only labels day 0 of the calendar and changes no result. It is not a claim about when the 2025 outbreak began."
                )
            } else {
                "A reference week set by the scenario's configuration. The engine has no seasonality, so the start week only labels day 0 of the calendar and changes no result. It is not a claim about when the 2025 outbreak began.".into()
            },
            limitation: "The county is modelled as one well-mixed population whose immunity is its kindergarten MMR coverage applied to every resident (see the engine notes in seir.rs). Real outbreaks cluster in close communities and are shaped by reporting and interventions that this scenario does not model, so its totals are not comparable with reported cases.".into(),
        },
        parameters: provenance_parameters,
        nodes: node_inputs,
        excluded_nodes: excluded,
        neighbourhood_note: match &config.neighbourhood {
            None => "Gaines County is simulated alone. The engine's cited defaults carry no gravity coupling (no calibrated Texas mobility parameters exist), so another county would be an inert node and none is published.".into(),
            Some(n) => format!(
                "Counties whose Gazetteer internal point is within {} km of the focus county are simulated and coupled by the gravity parameters listed above; a county missing any input is excluded and listed.",
                n.radius_km
            ),
        },
    };
    // The companion round-trips through the contract's validation and describes the scenario.
    let text = serde_json::to_vec(&provenance).map_err(|e| invalid(e.to_string()))?;
    let provenance: ScenarioProvenance = serde_json::from_slice(&text)
        .map_err(|e| invalid(format!("provenance contract validation: {e}")))?;
    provenance.check_against(&input).map_err(invalid)?;
    Ok(Built { input, provenance })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pre-registered defaults, pinned: changing one is a method change, not a tweak.
    #[test]
    fn preregistered_defaults_are_pinned() {
        let c = ScenarioConfig::default();
        assert_eq!(c.focus, 48165);
        assert_eq!(c.seed, 20_250_304);
        assert_eq!(c.run_count, 1000);
        assert_eq!(c.coverage_year, 2023);
        assert_eq!(c.initial_infectious, 1);
        assert_eq!(c.initial_exposed, 0);
        assert_eq!(c.start_week, None);
        assert_eq!(c.neighbourhood, None);
        assert!(koplik_epi::default_parameters().gravity.is_none());
    }

    #[test]
    fn people_are_described_in_words() {
        assert_eq!(introduced(1, 0), "one infectious person");
        assert_eq!(introduced(3, 0), "3 infectious people");
        assert_eq!(
            introduced(1, 2),
            "one infectious person and 2 exposed people"
        );
        assert_eq!(introduced(0, 1), "one exposed person");
    }
}
