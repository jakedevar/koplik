use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{CaseCount, GeoId, MmwrWeek, Provenances};
use crate::v1;
#[cfg(test)]
use crate::v2;

/// Which cases a weekly count includes. A count is only comparable with counts that share its
/// definition, so every row states it. Add a variant (in a new contract version) for any other
/// source definition; never reuse `Confirmed` for a count that includes other case statuses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CaseDefinition {
    /// Only cases whose case status is confirmed under the source's case definition, with no
    /// other status mixed in.
    Confirmed,
    /// Confirmed cases plus cases whose case status is unknown, without the two being told
    /// apart. This is what CDC's NNDSS measles publication criteria publish (event code 10140:
    /// case status "confirmed" and "unknown"); the weekly query has no case-status field to
    /// split them, so these counts must not be presented as confirmed cases.
    ConfirmedOrUnknownStatus,
}

/// Newly reported cases in one MMWR week for one geography, under an explicit case definition.
/// (Cumulative-only sources are differenced by the connector; the row is always per week.)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WeeklyCaseCount {
    pub geography: GeoId,
    pub week: MmwrWeek,
    /// The count (or its explicit absence). `reported` with `count: 0` is a real zero;
    /// `missing` is unknown.
    pub cases: CaseCount,
    /// What `cases` counts.
    pub case_definition: CaseDefinition,
    pub provenance: Provenances,
}

/// Lossless upgrade: a v1 row counted confirmed cases, so it becomes `case_definition: confirmed`.
/// v2 re-exports v1's `WeeklyCaseCount` unchanged (it is the same Rust type), so this one impl
/// is also `From<v2::WeeklyCaseCount>`.
impl From<v1::WeeklyCaseCount> for WeeklyCaseCount {
    fn from(v: v1::WeeklyCaseCount) -> Self {
        Self {
            geography: v.geography,
            week: v.week,
            cases: v.confirmed,
            case_definition: CaseDefinition::Confirmed,
            provenance: v.provenance,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upgrades_a_v1_row_without_losing_anything() {
        let json = serde_json::json!({
            "geography": "48",
            "week": {"year": 2025, "week": 9},
            "confirmed": {"status": "reported", "count": 12},
            "provenance": [{
                "source_id": "s", "url": "https://example.test/x",
                "retrieved_at": "2026-09-30T12:00:00Z",
                "sha256": "ab".repeat(32), "licence_id": "l"
            }]
        });
        let old: v1::WeeklyCaseCount = serde_json::from_value(json).unwrap();
        let new = WeeklyCaseCount::from(old.clone());
        // A v2 row is the same type as a v1 row and upgrades identically.
        let via_v2: v2::WeeklyCaseCount = old.clone();
        assert_eq!(WeeklyCaseCount::from(via_v2), new);
        assert_eq!(new.cases, old.confirmed);
        assert_eq!(new.case_definition, CaseDefinition::Confirmed);
        assert_eq!((new.geography, new.week), (old.geography, old.week));
        assert_eq!(new.provenance, old.provenance);
    }
}
