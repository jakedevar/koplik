//! Contract v2. Immutable once released: change a shape by adding `v3`, never by editing this.
//!
//! v2 differs from v1 in one type: [`WeeklyCaseCount`] now says what its counts count
//! (`cases` plus a required [`CaseDefinition`]) instead of calling every count "confirmed".
//! Every other type is re-exported from v1 unchanged (same Rust type, same JSON), so v1 and
//! v2 rows can be mixed in one program. The conventions listed in `v1` apply to v2 as well.

mod weekly_cases;

pub use super::v1::{
    BaselineCoverage, Centroid, CountyFips, CoverageOverride, CoverageValue, FipsError, Forecast,
    ForecastQuantile, GeoId, GeoLevel, Geography, GravityParameters, KindergartenMmrCoverage,
    MmwrError, MmwrWeek, Population, Provenance, ProvenanceError, Provenances, R0, RtEstimate,
    RtStatus, ScenarioInput, ScenarioNode, SchoolYear, SeirParameters, Sha256Hex, StateFips,
};
// The weekly count's missing-value vocabulary is unchanged from v1.
pub use super::v1::{CaseCount, MissingReason};
pub use weekly_cases::{CaseDefinition, WeeklyCaseCount};

/// Version label of this module, used as the schema directory name.
pub const VERSION: &str = "v2";
