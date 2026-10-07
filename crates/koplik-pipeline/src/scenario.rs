//! The Gaines County 2025 what-if scenario (#1455): the rule that builds its v1
//! `ScenarioInput` from snapshot-store inputs, and the configuration that rule runs with.
//!
//! # PRE-REGISTERED RULE (committed before any run of the scenario against the outbreak data)
//!
//! The scenario is a **what-if tool, not a fitted model**. Nothing below is tuned, and nothing
//! may later be adjusted, to make a simulated outbreak resemble the observed one (AGENTS.md
//! rule 5). Every default is stated here with its citation; a change to a default is a change
//! to the published method and goes through review, not through the replay.
//!
//! ## Seeding (documented, configurable)
//! * *Which report.* The earliest retained Texas DSHS report vintage (ordered by report date,
//!   then by the time the vintage was first seen) that (a) carries a readable outbreak county
//!   table and (b) is labelled as confirmed cases by DSHS's own wording (`confirmed_basis`),
//!   and (c) lists a Gaines County count. A vintage without a county table, or whose labelling
//!   does not establish "confirmed", is skipped (the earliest *labelled* vintage is used) and
//!   the skipped vintages are recorded in the companion provenance artifact with the reason.
//! * *Start week.* The MMWR week containing that report's date (`MmwrWeek::from_date`). Day 0
//!   of the simulation is the first day of that week; the report date's offset inside the week
//!   is recorded, not corrected for.
//! * *Initial infectious* of the Gaines node = that report's Gaines **confirmed count as
//!   recorded** (cumulative since the outbreak began) multiplied by
//!   [`ScenarioConfig::reporting_multiplier`], rounded to the nearest integer. The default
//!   multiplier is `1.0`: **no inflation for under-reporting**. DSHS itself states that cases
//!   exist that it could not confirm (the data report's footnote about 182 potential Gaines
//!   County cases "due to insufficient information"), but no cited, quantitative
//!   under-reporting factor for this outbreak is adopted here; a user who sets one must also
//!   cite it, and the provenance artifact records the value used.
//! * *Initial exposed* = `round(initial_infectious * exposed_per_infectious)`; the default
//!   ratio is `0.0`, so initial exposed = 0. No cited rule gives a latent pool at the first
//!   report, and inventing one would be tuning.
//! * Limitation, stated to the reader: DSHS's count is cumulative, so seeding it all as
//!   *currently infectious* treats cases that had already recovered as still transmitting, and
//!   the engine has no initial-recovered input. The model is not corrected for this.
//! * Every other node starts with zero exposed and infectious people.
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
//!   supplies. Setting a neighbourhood without gravity is refused (an inert neighbourhood is
//!   never published). A neighbour is a node only if it has a population, a Gazetteer
//!   centroid **and a reported baseline coverage row**; a county missing any of them is
//!   **excluded and listed in the provenance artifact**, never imputed. (The v1 contract can
//!   only carry a missing coverage together with an override, which would be an invented
//!   baseline, so exclusion is the honest choice.)
//!
//! ## Node inputs (every one from a snapshot in the store)
//! * *Population*: U.S. Census Bureau Vintage 2025 county population estimates,
//!   `POPESTIMATE2025` (July 1, 2025), `co-est2025-alldata.csv`. The simulation starts in
//!   March 2025, so the estimate is a few months later than day 0; no back-casting is done.
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

use std::collections::BTreeMap;

use chrono::{DateTime, NaiveDate, Utc};
use koplik_contracts::v1::{
    BaselineCoverage, Centroid, CountyFips, CoverageValue, GeoId, Geography, GravityParameters,
    KindergartenMmrCoverage, MmwrWeek, Population, Provenance, Provenances, ScenarioInput,
    ScenarioNode,
};
use koplik_ingest::census_counties::CountyLookup;
use koplik_ingest::coverage;
use koplik_ingest::dshs_series::Vintage;
use serde::{Deserialize, Serialize};

/// Gaines County, Texas (state FIPS 48, county 165).
pub const GAINES_FIPS: u32 = 48165;

/// Fixed RNG seed of the published scenario. Arbitrary: the digits are the `YYYYMMDD` of the
/// earliest DSHS report that carries a Gaines County table, as a mnemonic only. Chosen once,
/// before any run; never adjusted to a result.
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
    /// County FIPS (`48xxx`) of the seeded county.
    pub focus: u32,
    /// RNG seed written into the scenario.
    pub seed: u64,
    /// Ensemble size written into the scenario.
    pub run_count: u32,
    /// Texas school year start of the baseline kindergarten MMR coverage.
    pub coverage_year: u16,
    /// Multiplier on the recorded confirmed count to seed initial infectious. `1.0` = the count
    /// as recorded; any other value needs its own citation (see the module docs).
    pub reporting_multiplier: f64,
    /// Initial exposed per initial infectious person; `0.0` = none.
    pub exposed_per_infectious: f64,
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
            reporting_multiplier: 1.0,
            exposed_per_infectious: 0.0,
            neighbourhood: None,
        }
    }
}

/// Version of the companion provenance artifact's shape.
pub const PROVENANCE_VERSION: u32 = 1;

/// Plain-words statement the web panel shows beside the scenario (and the artifact carries).
pub const STATEMENT: &str = "This is a what-if tool, not a fitted model. The simulation starts from the case count a Texas DSHS report gave for Gaines County, takes its parameters from published sources, and was never adjusted to match how the outbreak actually unfolded. Compare its output with the DSHS counts only as an illustration of what a simple model does, not as a forecast.";

/// What the seeding cannot capture, stated to the reader.
pub const SEEDING_LIMITATION: &str = "DSHS counts are cumulative since the outbreak began. Seeding all of them as currently infectious treats cases that had already recovered as still transmitting, and the engine has no initial-recovered input. The model is not corrected for this, and is not corrected for under-reporting unless the multiplier says otherwise.";

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
    /// Every DSHS report vintage held, with its snapshots.
    pub vintages: &'a [Vintage],
    /// The Census county-name lookup that turns a printed DSHS name into a FIPS code.
    pub lookup: &'a CountyLookup,
}

/// A DSHS vintage that was passed over on the way to the seeding report, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkippedVintage {
    pub report_date: NaiveDate,
    pub reason: String,
}

/// The seeding, with the report it came from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SeedingProvenance {
    /// The rule, in words (see the module docs).
    pub rule: String,
    pub report_date: NaiveDate,
    /// When the first stored snapshot of this exact report version was captured or fetched.
    pub report_first_seen_at: DateTime<Utc>,
    /// The county name exactly as DSHS printed it, and the cell text.
    pub county_name_as_printed: String,
    pub cell_as_printed: String,
    /// DSHS's own wording that establishes these counts as confirmed cases.
    pub confirmed_basis: String,
    /// The confirmed count as recorded.
    pub recorded_confirmed_count: u32,
    pub reporting_multiplier: f64,
    pub exposed_per_infectious: f64,
    pub initial_infectious: u32,
    pub initial_exposed: u32,
    pub start_week: MmwrWeek,
    /// Where the numbers came from: the DSHS snapshot, and the Census file that maps its
    /// county names to FIPS codes.
    pub provenance: Vec<Provenance>,
    /// Earlier vintages that were passed over, with the reason.
    pub skipped_vintages: Vec<SkippedVintage>,
    pub limitation: String,
}

/// One model parameter exactly as the scenario ran it, with its citation (#1400).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParameterProvenance {
    pub parameter: String,
    pub value: serde_json::Value,
    pub source: String,
    pub url: Option<String>,
    pub note: String,
}

/// What a node's inputs are, beyond the records the scenario already carries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeInputs {
    pub geography: GeoId,
    pub name: String,
    pub population: u64,
    pub population_basis: String,
    pub centroid_basis: String,
    pub coverage_school_year: String,
    pub coverage_basis: String,
}

/// A county that the neighbourhood rule would have simulated but could not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExcludedNode {
    pub geography: GeoId,
    pub name: String,
    pub reason: String,
}

/// The companion artifact `scenarios/gaines-2025.provenance.json`: where every number the
/// scenario does not carry a source record for comes from. Its shape is not a shared data
/// contract (no other crate reads it); the web panel reads it as documented in `web/README.md`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScenarioProvenance {
    pub artifact_version: u32,
    pub scenario: String,
    pub statement: String,
    /// The RNG seed as decimal text (a JavaScript number would round a large u64).
    pub seed: String,
    pub run_count: u32,
    pub seeding: SeedingProvenance,
    pub parameters: Vec<ParameterProvenance>,
    pub nodes: Vec<NodeInputs>,
    pub excluded_nodes: Vec<ExcludedNode>,
    pub neighbourhood_note: String,
}

impl ScenarioProvenance {
    /// Whether this companion describes `input`: the same seed, run count, start week, seeding
    /// and parameter values. A scenario is only published beside a companion that agrees.
    pub fn check_against(&self, input: &ScenarioInput) -> std::result::Result<(), String> {
        let focus = input
            .nodes
            .iter()
            .find(|n| n.initial_infectious > 0 || n.initial_exposed > 0)
            .ok_or("the scenario seeds no node")?;
        let values = serde_json::to_value(input.parameters).map_err(|e| e.to_string())?;
        let same = self.artifact_version == PROVENANCE_VERSION
            && self.seed == input.seed.to_string()
            && self.run_count == input.run_count
            && self.seeding.start_week == input.start_week
            && self.seeding.initial_infectious == focus.initial_infectious
            && self.seeding.initial_exposed == focus.initial_exposed
            && self
                .parameters
                .iter()
                .all(|p| values[p.parameter.as_str()] == p.value)
            && values
                .as_object()
                .is_some_and(|o| o.len() == self.parameters.len())
            && self.nodes.len() == input.nodes.len()
            && self
                .nodes
                .iter()
                .zip(&input.nodes)
                .all(|(a, b)| a.geography == b.id && a.population == b.population);
        if same {
            Ok(())
        } else {
            Err("the provenance companion does not describe the scenario beside it".into())
        }
    }
}

/// The built scenario and its companion.
#[derive(Debug, Clone, PartialEq)]
pub struct Built {
    pub input: ScenarioInput,
    pub provenance: ScenarioProvenance,
}

fn push_unique(records: &mut Vec<Provenance>, more: &[Provenance]) {
    for p in more {
        if !records.contains(p) {
            records.push(p.clone());
        }
    }
}

/// A node's sources, or why this county cannot be a node.
struct NodeParts {
    node: ScenarioNode,
    inputs: NodeInputs,
}

#[allow(clippy::result_large_err)]
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
    push_unique(&mut records, gazetteer.provenance.as_slice());
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
                "U.S. Census Bureau Vintage {} county population estimate, July 1, {} (POPESTIMATE{}); not back-cast to the simulation start",
                population.year, population.year, population.year
            ),
            centroid_basis: "Census 2025 Gazetteer county internal point (INTPTLAT, INTPTLONG): a representative point, not a population-weighted or geometric centroid".into(),
            coverage_school_year: row.school_year.to_string(),
            coverage_basis: "Texas DSHS kindergarten MMR coverage for the last completed school year before the 2025 outbreak, as published (not imputed)".into(),
        },
    })
}

/// Find the seeding report by the pre-registered rule (module docs).
fn find_seed<'a>(
    focus: CountyFips,
    sources: &Sources<'a>,
) -> std::result::Result<
    (
        &'a Vintage,
        koplik_ingest::dshs::CountyEntry,
        Vec<SkippedVintage>,
    ),
    ScenarioError,
> {
    let mut ordered: Vec<&Vintage> = sources.vintages.iter().collect();
    ordered.sort_by_key(|v| (v.report.report_date, v.first_seen_at()));
    let mut skipped = Vec::new();
    for v in ordered {
        let date = v.report.report_date;
        let mut skip = |reason: String| {
            skipped.push(SkippedVintage {
                report_date: date,
                reason,
            })
        };
        let Some(table) = &v.report.outbreak_counties else {
            skip("no readable outbreak county table".into());
            continue;
        };
        if !v.is_confirmed() {
            skip("DSHS's own labelling does not establish these counts as confirmed cases".into());
            continue;
        }
        let rows: Vec<_> = table
            .entries
            .iter()
            .filter(|e| sources.lookup.lookup(&e.name) == Ok(focus))
            .collect();
        match rows.as_slice() {
            [] => skip("the county table has no row for the county".into()),
            [row] if row.cases.is_some() => return Ok((v, (*row).clone(), skipped)),
            [row] => skip(format!(
                "the county's cell is not a plain count ({:?})",
                row.raw
            )),
            _ => skip("the county is listed more than once".into()),
        }
    }
    Err(missing(
        "no DSHS report vintage with a confirmed-case county table gives a count for the county",
    ))
}

/// Build the scenario by the pre-registered rule, validate it through the contract and the
/// engine, and describe where everything came from.
pub fn build(
    config: &ScenarioConfig,
    sources: &Sources<'_>,
) -> std::result::Result<Built, ScenarioError> {
    let focus = CountyFips::new(config.focus).map_err(|e| invalid(e.to_string()))?;
    if !config.reporting_multiplier.is_finite() || config.reporting_multiplier < 0.0 {
        return Err(invalid(
            "reporting multiplier must be finite and not negative",
        ));
    }
    if !config.exposed_per_infectious.is_finite() || config.exposed_per_infectious < 0.0 {
        return Err(invalid(
            "exposed per infectious must be finite and not negative",
        ));
    }

    let (vintage, entry, skipped_vintages) = find_seed(focus, sources)?;
    let recorded = entry.cases.expect("find_seed returns a readable count");
    let seeded = |value: f64| -> std::result::Result<u32, ScenarioError> {
        let r = value.round();
        if r.is_finite() && (0.0..=f64::from(u32::MAX)).contains(&r) {
            Ok(r as u32)
        } else {
            Err(invalid("initial seeding does not fit a u32"))
        }
    };
    let initial_infectious = seeded(f64::from(recorded) * config.reporting_multiplier)?;
    let initial_exposed = seeded(f64::from(initial_infectious) * config.exposed_per_infectious)?;
    let report_date = vintage.report.report_date;
    let start_week = MmwrWeek::from_date(report_date).map_err(|e| invalid(e.to_string()))?;

    let focus_parts = node_parts(focus, config, sources, initial_exposed, initial_infectious)
        .map_err(|reason| missing(format!("focus county {focus}: {reason}")))?;
    let focus_centroid: Centroid = focus_parts.node.centroid;
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
    let mut seed_records = vec![vintage.first().provenance()];
    seed_records.push(sources.lookup.retrieval.provenance());
    let provenance = ScenarioProvenance {
        artifact_version: PROVENANCE_VERSION,
        scenario: crate::SCENARIO_ARTIFACT.to_owned(),
        statement: STATEMENT.to_owned(),
        seed: input.seed.to_string(),
        run_count: input.run_count,
        seeding: SeedingProvenance {
            rule: "Start at the MMWR week of the earliest retained DSHS report vintage that has a confirmed-case county table with a count for the county; initial infectious is that confirmed count as recorded times the reporting multiplier (1.0 unless a cited factor is set); initial exposed is zero unless a ratio is set; every other node starts with none.".into(),
            report_date,
            report_first_seen_at: vintage.first_seen_at(),
            county_name_as_printed: entry.name.clone(),
            cell_as_printed: entry.raw.clone(),
            confirmed_basis: vintage
                .report
                .confirmed_basis
                .clone()
                .expect("find_seed requires a confirmed basis"),
            recorded_confirmed_count: recorded,
            reporting_multiplier: config.reporting_multiplier,
            exposed_per_infectious: config.exposed_per_infectious,
            initial_infectious,
            initial_exposed,
            start_week,
            provenance: seed_records,
            skipped_vintages,
            limitation: SEEDING_LIMITATION.to_owned(),
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
        assert_eq!(c.reporting_multiplier, 1.0);
        assert_eq!(c.exposed_per_infectious, 0.0);
        assert_eq!(c.neighbourhood, None);
        assert!(koplik_epi::default_parameters().gravity.is_none());
    }
}
