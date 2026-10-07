//! Contract v8. Immutable once released: change a shape by adding `v9`, never by editing this.
//!
//! v8 = v7 plus one new type group, [`CumulativeCaseReport`] and its published artifact
//! [`CumulativeCaseReportArtifact`] (#1439): a *cumulative* case count as one source printed it
//! on one report date, for one geography, under an explicit case definition. It is its own type
//! because it is not a [`WeeklyCaseCount`]: a cumulative count is never converted into, or
//! presented as, a weekly count. The artifact keeps the v6 envelope (a per-file provenance
//! table and rows that index it), carrying `contract_version: 8`. Every other type is
//! re-exported from v7 (and so from v6 to v1) unchanged: same Rust type, same JSON. The
//! conventions listed in `v1` apply to v8 as well.

mod cumulative;

pub use super::v7::*;
pub use cumulative::{
    CumulativeCaseReport, CumulativeCaseReportArtifact, CumulativeCount, CumulativeMissingReason,
    ReportDate, ReportDateError,
};

/// Version label of this module, used as the schema directory name.
pub const VERSION: &str = "v8";
