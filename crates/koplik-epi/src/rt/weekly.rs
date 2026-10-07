//! Weekly (MMWR) `WeeklyCaseCount` rows in, `RtEstimate` rows out.
//!
//! Input rows are contracts v3 [`WeeklyCaseCount`] (`cases` plus a `case_definition`), or v1
//! rows, which counted confirmed cases and are read as `case_definition: confirmed`.
//!
//! **The estimate inherits the case definition of its input.** R_t here is computed from the
//! counts as given, so an R_t built from `confirmed_or_unknown_status` rows (CDC NNDSS) describes
//! transmission of confirmed *or unknown-status* cases, and one built from `confirmed` rows
//! (Texas DSHS) describes confirmed cases; the two are not directly comparable and neither
//! estimates transmission of all infections. `RtEstimate` (v1) has no field for the definition,
//! so it is not repeated on each output row: use [`case_definitions`] on the same input rows
//! to label the output per geography. Within one geography the definition must not change
//! between weeks (a series that mixes definitions is a different quantity from week to week),
//! so [`estimate_weekly`] rejects that input with [`RtError::MixedCaseDefinition`].

use std::collections::BTreeMap;

use koplik_contracts::v1::WeeklyCaseCount as WeeklyCaseCountV1;
use koplik_contracts::v1::{GeoId, MmwrWeek, Provenance, Provenances, RtEstimate, RtStatus};
use koplik_contracts::v3::{CaseCount, CaseDefinition, WeeklyCaseCount};

use super::RtError;
use super::estimate::{RenewalConfig, estimate_series};
use super::serial_interval::SerialInterval;

/// A weekly case row as R_t reads it: implemented for the v3 row and for the v1 row (which
/// counted confirmed cases, so its definition is `Confirmed`, matching the lossless
/// `From<v1::WeeklyCaseCount> for v3::WeeklyCaseCount`).
pub trait WeeklyCaseRow {
    fn geography(&self) -> GeoId;
    fn week(&self) -> MmwrWeek;
    fn cases(&self) -> &CaseCount;
    fn case_definition(&self) -> CaseDefinition;
    fn provenance(&self) -> &Provenances;
}

impl WeeklyCaseRow for WeeklyCaseCount {
    fn geography(&self) -> GeoId {
        self.geography
    }
    fn week(&self) -> MmwrWeek {
        self.week
    }
    fn cases(&self) -> &CaseCount {
        &self.cases
    }
    fn case_definition(&self) -> CaseDefinition {
        self.case_definition
    }
    fn provenance(&self) -> &Provenances {
        &self.provenance
    }
}

impl WeeklyCaseRow for WeeklyCaseCountV1 {
    fn geography(&self) -> GeoId {
        self.geography
    }
    fn week(&self) -> MmwrWeek {
        self.week
    }
    fn cases(&self) -> &CaseCount {
        &self.confirmed
    }
    fn case_definition(&self) -> CaseDefinition {
        CaseDefinition::Confirmed
    }
    fn provenance(&self) -> &Provenances {
        &self.provenance
    }
}

/// The case definition of each geography's series: what its R_t estimates describe. Errors if one
/// geography's rows disagree (see [`RtError::MixedCaseDefinition`]).
pub fn case_definitions<R: WeeklyCaseRow>(
    rows: &[R],
) -> Result<BTreeMap<GeoId, CaseDefinition>, RtError> {
    let mut out: BTreeMap<GeoId, CaseDefinition> = BTreeMap::new();
    for row in rows {
        let found = *out.entry(row.geography()).or_insert(row.case_definition());
        if found != row.case_definition() {
            return Err(RtError::MixedCaseDefinition {
                geography: row.geography(),
            });
        }
    }
    Ok(out)
}

/// Configuration for weekly R_t. Every default cites its source.
#[derive(Debug, Clone, PartialEq)]
pub struct RtConfig {
    /// Default [`SerialInterval::MEASLES`].
    pub serial_interval: SerialInterval,
    /// Longest lag kept when discretising the serial interval to weeks. Default 8 weeks
    /// (56 days, more than 14 SDs above the measles mean: the truncated tail is negligible
    /// and is reported by `DiscreteSerialInterval::dropped_mass`).
    pub max_lag_weeks: u32,
    pub renewal: RenewalConfig,
    /// The most recent `provisional_weeks` weeks of a series are flagged provisional because
    /// of reporting delay. Default 2 (spec E5: "The most recent two weeks are marked
    /// provisional (reporting delay)"; CDC's weekly measles counts are updated as reports
    /// arrive). Independent of status.
    pub provisional_weeks: u32,
}

impl Default for RtConfig {
    fn default() -> Self {
        Self {
            serial_interval: SerialInterval::MEASLES,
            max_lag_weeks: 8,
            renewal: RenewalConfig::default(),
            provisional_weeks: 2,
        }
    }
}

/// Estimate weekly R_t for every geography in `rows`.
///
/// Rows are grouped by geography and laid on a contiguous MMWR-week grid from the first to
/// the last week present; a week with no row is missing (unknown), exactly like a row whose
/// count is `missing`. Two rows for the same geography and week are an error, not a choice.
/// The output has one `RtEstimate` per (geography, week, credible level), ordered by
/// geography, week, then level; the provenance of each row is the union of the provenance
/// records of that geography's input rows, in week order, without duplicates.
pub fn estimate_weekly<R: WeeklyCaseRow>(
    rows: &[R],
    cfg: &RtConfig,
) -> Result<Vec<RtEstimate>, RtError> {
    cfg.renewal.validate()?;
    case_definitions(rows)?;
    if cfg.provisional_weeks < 1 {
        return Err(RtError::Config("provisional_weeks must be >= 1".into()));
    }
    let si = cfg.serial_interval.discretize_weekly(cfg.max_lag_weeks)?;

    let mut by_geo: BTreeMap<GeoId, BTreeMap<MmwrWeek, &R>> = BTreeMap::new();
    for row in rows {
        if by_geo
            .entry(row.geography())
            .or_default()
            .insert(row.week(), row)
            .is_some()
        {
            return Err(RtError::DuplicateWeek {
                geography: row.geography(),
                week: row.week(),
            });
        }
    }

    let mut out = Vec::new();
    for (geography, weeks) in &by_geo {
        // Contiguous grid.
        let first = *weeks.keys().next().expect("group is non-empty");
        let last = *weeks.keys().next_back().expect("group is non-empty");
        let mut grid = Vec::new();
        let mut counts = Vec::new();
        let mut week = first;
        loop {
            grid.push(week);
            counts.push(weeks.get(&week).and_then(|r| r.cases().count()));
            if week == last {
                break;
            }
            week = week.next()?;
        }
        // Provenance: union in week order, deduplicated.
        let mut records: Vec<Provenance> = Vec::new();
        for row in weeks.values() {
            for p in row.provenance().as_slice() {
                if !records.contains(p) {
                    records.push(p.clone());
                }
            }
        }
        let provenance = Provenances::new(records)?;

        let estimates = estimate_series(&counts, &si, &cfg.renewal)?;
        let n = grid.len() as u32;
        let provisional_from = n.saturating_sub(cfg.provisional_weeks);
        for est in &estimates {
            let provisional = est.step >= provisional_from;
            let week = grid[est.step as usize];
            for &level in &cfg.renewal.levels {
                let (mean, lower, upper) = match est.status {
                    RtStatus::Ok => {
                        let post = est.posterior.expect("ok carries a posterior");
                        let ci = est
                            .intervals
                            .iter()
                            .find(|ci| ci.level == level)
                            .expect("one interval per level");
                        (Some(post.mean()), Some(ci.lower), Some(ci.upper))
                    }
                    RtStatus::InsufficientData => (None, None, None),
                };
                out.push(RtEstimate {
                    geography: *geography,
                    week,
                    status: est.status,
                    provisional,
                    mean,
                    lower,
                    upper,
                    interval_level: level,
                    provenance: provenance.clone(),
                });
            }
        }
    }
    Ok(out)
}
