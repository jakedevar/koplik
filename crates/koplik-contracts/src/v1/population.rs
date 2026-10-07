use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::fips::GeoId;
use super::provenance::Provenances;

/// Resident population of a geography for a year (e.g. a Census estimate, July 1 vintage).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Population {
    pub geography: GeoId,
    pub year: u16,
    pub count: u64,
    pub provenance: Provenances,
}
