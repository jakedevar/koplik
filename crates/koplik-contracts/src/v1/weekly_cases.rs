use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::fips::GeoId;
use super::mmwr::MmwrWeek;
use super::provenance::Provenances;

/// Why a weekly count is missing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MissingReason {
    /// The source did not report this geography and week.
    NotReported,
    /// The source withheld the value (small-number suppression).
    Suppressed,
    /// The source has the week but the value could not be read unambiguously.
    Ambiguous,
}

/// A confirmed-case count that can be explicitly missing. `reported` with `count: 0` is a
/// real zero; `missing` is unknown. They are never interchangeable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum CaseCount {
    Reported { count: u32 },
    Missing { reason: MissingReason },
}

impl CaseCount {
    /// The count, or `None` when missing (never 0).
    pub fn count(self) -> Option<u32> {
        match self {
            CaseCount::Reported { count } => Some(count),
            CaseCount::Missing { .. } => None,
        }
    }
}

/// Newly confirmed measles cases in one MMWR week for one geography.
/// (Cumulative-only sources are differenced by the connector; the row is always per week.)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WeeklyCaseCount {
    pub geography: GeoId,
    pub week: MmwrWeek,
    pub confirmed: CaseCount,
    pub provenance: Provenances,
}
