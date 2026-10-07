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
//!   Bjørnstad & Grenfell 2004 and Bharti et al. 2008 fits are England and Wales / Niger
//!   fits that `gravity.rs` declines to transplant). With coupling off, any other county would
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

use koplik_contracts::v1::GravityParameters;
use koplik_ingest::coverage;

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
