//! Read the ingest crate's vintage manifest (`data/dshs/vintage-manifest.json`, manifest
//! version 1) into [`ReportVintage`]s for the outbreak total. Parsing only: the caller reads
//! the bytes.
//!
//! Only the fields the backtest needs are read; the rest of the manifest (county detail
//! flags, parser issues, every later snapshot of the same version) is left as the ingest
//! crate documents it. The outbreak total is read from **every** version, including the
//! 30 dashboard-era versions without a county table, because the total is the one number
//! those pages still print (Issue #1402); county numbers are never derived from them.

use chrono::{DateTime, NaiveDate, Utc};
use koplik_contracts::v3::{CaseDefinition, Provenance, Sha256Hex};
use serde::Deserialize;

use super::vintages::ReportVintage;

#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("vintage manifest is not valid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported manifest_version {0} (expected 1)")]
    Version(u32),
    #[error("entry {report_date}: {problem}")]
    Entry {
        report_date: NaiveDate,
        problem: String,
    },
}

#[derive(Debug, Deserialize)]
struct Manifest {
    manifest_version: u32,
    entries: Vec<Entry>,
}

#[derive(Debug, Deserialize)]
struct Entry {
    report_date: NaiveDate,
    outbreak_total: u32,
    first_seen_at: DateTime<Utc>,
    first_snapshot: Snapshot,
    /// The manifest's statement of how the version labels its count; the backtest only
    /// accepts versions the ingest crate marked as confirmed cases.
    #[serde(default)]
    confirmed_basis: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Snapshot {
    source_id: String,
    url: String,
    retrieved_at: DateTime<Utc>,
    sha256: String,
    licence_id: String,
}

/// Parse a manifest into outbreak-total vintages in `first_seen_at` order. An entry
/// without a `confirmed_basis` is refused: the backtest counts confirmed cases only.
pub fn outbreak_total_vintages(json: &str) -> Result<Vec<ReportVintage>, ManifestError> {
    let manifest: Manifest = serde_json::from_str(json)?;
    if manifest.manifest_version != 1 {
        return Err(ManifestError::Version(manifest.manifest_version));
    }
    let mut out = Vec::with_capacity(manifest.entries.len());
    for e in manifest.entries {
        if e.confirmed_basis.as_deref().unwrap_or("").is_empty() {
            return Err(ManifestError::Entry {
                report_date: e.report_date,
                problem:
                    "no confirmed_basis: the version does not label its total as confirmed cases"
                        .into(),
            });
        }
        let sha256 =
            Sha256Hex::new(e.first_snapshot.sha256).map_err(|err| ManifestError::Entry {
                report_date: e.report_date,
                problem: err.to_string(),
            })?;
        out.push(ReportVintage {
            report_date: e.report_date,
            first_seen_at: e.first_seen_at,
            cumulative: e.outbreak_total,
            case_definition: CaseDefinition::Confirmed,
            provenance: Provenance {
                source_id: e.first_snapshot.source_id,
                url: e.first_snapshot.url,
                retrieved_at: e.first_snapshot.retrieved_at,
                sha256,
                licence_id: e.first_snapshot.licence_id,
            },
        });
    }
    out.sort_by(|a, b| (a.first_seen_at, a.report_date).cmp(&(b.first_seen_at, b.report_date)));
    Ok(out)
}
