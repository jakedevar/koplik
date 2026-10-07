//! Contract v1. Immutable once released: change a shape by adding `v2`, never by editing this.
//!
//! Conventions:
//! - Every struct is `deny_unknown_fields`; no `usize`/`isize` fields (32-bit `wasm32`).
//! - Geography is keyed by FIPS (`GeoId`), never by a free-text name.
//! - Validated newtypes check on construction and on deserialize.
//! - A missing value is represented explicitly (see `CaseCount`), never as zero or a guess.
//! - Contract types hold no model defaults: defaults and their citations live in `koplik-epi`.

mod coverage;
mod fips;
mod forecast;
mod geography;
mod mmwr;
mod population;
mod provenance;
mod rt;
mod scenario;
mod weekly_cases;

pub use coverage::{CoverageValue, KindergartenMmrCoverage, SchoolYear};
pub use fips::{CountyFips, FipsError, GeoId, GeoLevel, StateFips};
pub use forecast::{Forecast, ForecastQuantile};
pub use geography::{Centroid, Geography};
pub use mmwr::{MmwrError, MmwrWeek};
pub use population::Population;
pub use provenance::{Provenance, ProvenanceError, Provenances, Sha256Hex};
pub use rt::{RtEstimate, RtStatus};
pub use scenario::{
    BaselineCoverage, CoverageOverride, GravityParameters, R0, ScenarioInput, ScenarioNode,
    SeirParameters,
};
pub use weekly_cases::{CaseCount, MissingReason, WeeklyCaseCount};

/// Version label of this module, used as the schema directory name.
pub const VERSION: &str = "v1";

/// Reject a non-finite or out-of-range float (shared by row validators).
pub(crate) fn check_range(
    field: &'static str,
    value: f64,
    min: f64,
    max: f64,
) -> Result<(), String> {
    if value.is_finite() && value >= min && value <= max {
        Ok(())
    } else {
        Err(format!(
            "{field} must be a finite number in [{min}, {max}], got {value}"
        ))
    }
}
