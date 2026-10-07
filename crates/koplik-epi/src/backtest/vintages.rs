//! Weekly series from cumulative report versions, as known at a cutoff time.
//!
//! A *report vintage* is one version of a source's cumulative count with the time it was
//! first seen (`first_seen_at`: an Internet Archive capture time or a live retrieval time,
//! an upper bound on when it became public) and the date the source printed on it
//! (`report_date`). Versions are immutable snapshots, so the series "as known at time T"
//! is built from exactly the versions with `first_seen_at <= T`; nothing later leaks in,
//! and nothing is back-filled from later versions. This is the revision side of the
//! information cutoff (`crate::forecast::forecast_weekly` handles the week side).
//!
//! Derivation (the same rule as the ingest crate's county series, applied to one total):
//! each MMWR week keeps its last known version (latest `report_date`, then latest
//! `first_seen_at`); a week is *complete* only when a known version is dated in a later
//! week, so a week still being reported never gets a partial count. For a complete week
//! `W` whose previous week `W-1` also has a version, the count is
//! `cumulative(W) - cumulative(W-1)`, `reported` when it does not fall and
//! `missing:ambiguous` when it does (cases removed or reclassified); when `W-1` has no
//! version the count cannot be assigned to one week and is `missing:not_reported`. Weeks
//! with no version get no row (missing). Counts are by **report date**, not symptom
//! onset: they say when the source published the cases.

use std::collections::BTreeMap;

use chrono::{DateTime, NaiveDate, Utc};
use koplik_contracts::v3::{
    CaseCount, CaseDefinition, GeoId, MissingReason, MmwrError, MmwrWeek, Provenance,
    ProvenanceError, Provenances, WeeklyCaseCount,
};

/// One version of a cumulative count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportVintage {
    /// The date the source printed on the version.
    pub report_date: NaiveDate,
    /// When the version was first seen (capture or retrieval time): an upper bound on
    /// when it became public.
    pub first_seen_at: DateTime<Utc>,
    /// The cumulative count the version states.
    pub cumulative: u32,
    /// What the count counts.
    pub case_definition: CaseDefinition,
    /// Provenance of the snapshot the version was read from.
    pub provenance: Provenance,
}

#[derive(Debug, thiserror::Error)]
pub enum VintageError {
    #[error(transparent)]
    Mmwr(#[from] MmwrError),
    #[error(transparent)]
    Provenance(#[from] ProvenanceError),
    #[error("vintages mix case definitions")]
    MixedCaseDefinition,
}

/// The series known at `known_at` (every version when `None`).
#[derive(Debug, Clone, PartialEq)]
pub struct KnownSeries {
    /// How many versions were known.
    pub known_versions: u32,
    /// The last week with a version; its own count is not complete.
    pub last_report_week: Option<MmwrWeek>,
    /// The latest complete week: the week before `last_report_week`. The natural origin
    /// for a forecast made at `known_at`.
    pub last_complete_week: Option<MmwrWeek>,
    /// One row per week with a version up to and including `last_complete_week`.
    pub rows: Vec<WeeklyCaseCount>,
}

/// Build the weekly series for `geography` from the versions first seen by `known_at`.
pub fn weekly_from_vintages(
    geography: GeoId,
    vintages: &[ReportVintage],
    known_at: Option<DateTime<Utc>>,
) -> Result<KnownSeries, VintageError> {
    let known: Vec<&ReportVintage> = vintages
        .iter()
        .filter(|v| known_at.is_none_or(|t| v.first_seen_at <= t))
        .collect();
    if let Some(first) = known.first()
        && known
            .iter()
            .any(|v| v.case_definition != first.case_definition)
    {
        return Err(VintageError::MixedCaseDefinition);
    }
    // Last version per week: latest report date, then latest first-seen time.
    let mut by_week: BTreeMap<MmwrWeek, &ReportVintage> = BTreeMap::new();
    for v in &known {
        let week = MmwrWeek::from_date(v.report_date)?;
        let replace = by_week.get(&week).is_none_or(|cur| {
            (v.report_date, v.first_seen_at) > (cur.report_date, cur.first_seen_at)
        });
        if replace {
            by_week.insert(week, v);
        }
    }
    let last_report_week = by_week.keys().next_back().copied();
    let last_complete_week = match last_report_week {
        Some(w) => Some(w.prev()?),
        None => None,
    };
    let mut rows = Vec::new();
    for (&week, v) in &by_week {
        if Some(week) >= last_report_week {
            break;
        }
        let previous = by_week.get(&week.prev()?);
        let (cases, provenance) = match previous {
            Some(p) => {
                let cases = if v.cumulative >= p.cumulative {
                    CaseCount::Reported {
                        count: v.cumulative - p.cumulative,
                    }
                } else {
                    CaseCount::Missing {
                        reason: MissingReason::Ambiguous,
                    }
                };
                let mut records = vec![p.provenance.clone()];
                if v.provenance != p.provenance {
                    records.push(v.provenance.clone());
                }
                (cases, Provenances::new(records)?)
            }
            None => (
                CaseCount::Missing {
                    reason: MissingReason::NotReported,
                },
                Provenances::one(v.provenance.clone()),
            ),
        };
        rows.push(WeeklyCaseCount {
            geography,
            week,
            cases,
            case_definition: v.case_definition,
            provenance,
        });
    }
    Ok(KnownSeries {
        known_versions: u32::try_from(known.len()).expect("fewer than 2^32 versions"),
        last_report_week,
        last_complete_week,
        rows,
    })
}
