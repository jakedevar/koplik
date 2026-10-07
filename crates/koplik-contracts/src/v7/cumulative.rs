//! Cumulative case counts as a source printed them, one row per geography and report date.
//!
//! A cumulative count answers "how many cases has the source counted since the outbreak began,
//! as of this report", so it is plotted against report dates, never against weeks. Reports are
//! irregular (the Texas DSHS outbreak page was updated twice a week, then replaced by a
//! dashboard, then summarised in three PDFs), and a connector must not turn the series into
//! weekly or per-interval counts, interpolate between reports, or carry a value forward: where
//! a report gives no usable count the row says so with a reason, and nothing is estimated.

use std::borrow::Cow;
use std::collections::BTreeSet;

use chrono::{Datelike, NaiveDate};
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema, schema_for};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Value, json};
use thiserror::Error;

use super::{CaseDefinition, GeoId, Provenance, Provenances};

/// Supported year range (the same bounds as an MMWR year; keeps date arithmetic safe).
const MIN_YEAR: i32 = 1900;
const MAX_YEAR: i32 = 2200;
const DATE_FORMAT: &str = "%Y-%m-%d";

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ReportDateError {
    #[error("report date {0:?} must be a calendar date written YYYY-MM-DD")]
    Malformed(String),
    #[error("report date year {0} outside supported range {MIN_YEAR}..={MAX_YEAR}")]
    YearOutOfRange(i32),
}

/// The date a source printed on a report (for example the "News Updates" date of a web page
/// or the title date of a PDF), a calendar date with no time zone. It is not the retrieval time
/// (see `provenance`) and not necessarily the date the figures are current to.
/// Written as `YYYY-MM-DD` and validated on construction and on deserialize.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReportDate(NaiveDate);

impl ReportDate {
    pub fn new(date: NaiveDate) -> Result<Self, ReportDateError> {
        if (MIN_YEAR..=MAX_YEAR).contains(&date.year()) {
            Ok(Self(date))
        } else {
            Err(ReportDateError::YearOutOfRange(date.year()))
        }
    }

    /// Parse the canonical `YYYY-MM-DD` form only (no unpadded or otherwise lenient spellings).
    pub fn parse(text: &str) -> Result<Self, ReportDateError> {
        let malformed = || ReportDateError::Malformed(text.to_owned());
        let date = NaiveDate::parse_from_str(text, DATE_FORMAT).map_err(|_| malformed())?;
        if date.format(DATE_FORMAT).to_string() != text {
            return Err(malformed());
        }
        Self::new(date)
    }

    pub fn date(self) -> NaiveDate {
        self.0
    }
}

impl std::fmt::Display for ReportDate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.format(DATE_FORMAT))
    }
}

impl Serialize for ReportDate {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for ReportDate {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::parse(&String::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

impl JsonSchema for ReportDate {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("ReportDate")
    }
    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "format": "date",
            "pattern": "^[0-9]{4}-[0-9]{2}-[0-9]{2}$",
            "description": "A calendar date as printed on a report, YYYY-MM-DD, no time zone"
        })
    }
}

/// Why a report gives no usable cumulative count for a geography. None of these is zero and none
/// is estimated: a missing count is unknown.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum CumulativeMissingReason {
    /// The source published a report on this date but no readable county breakdown (for
    /// example the Texas DSHS dashboard period, whose county figures cannot be fetched).
    NoCountyTable,
    /// The report has a county breakdown, but its own labelling does not establish that the
    /// counts are confirmed cases, so none of its counts enter a series whose case definition
    /// is `confirmed`.
    NotLabelledConfirmed,
    /// The county is not listed in this report's table, and the table is not shown to list every
    /// county (its rows do not add up to its printed total), so its absence cannot be read as zero.
    NotListed,
    /// The county is listed but its count cannot be read as one number (an unreadable cell, or
    /// the county listed more than once).
    Ambiguous,
}

/// A cumulative count that can be explicitly missing. `reported` with `count: 0` is a real zero
/// (the report lists the county as zero, or demonstrably lists every county and omits this one);
/// `missing` is unknown. They are never interchangeable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum CumulativeCount {
    Reported { count: u32 },
    Missing { reason: CumulativeMissingReason },
}

impl CumulativeCount {
    /// The count, or `None` when missing (never 0).
    pub fn count(self) -> Option<u32> {
        match self {
            CumulativeCount::Reported { count } => Some(count),
            CumulativeCount::Missing { .. } => None,
        }
    }
}

/// Cases counted since the outbreak began, as printed in the source's report of `report_date`,
/// for one geography, under an explicit case definition. Not a weekly count: it is never
/// differenced into weeks here, never interpolated between reports, and never carried
/// forward to a date the source did not report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CumulativeCaseReport {
    pub geography: GeoId,
    /// The date the report printed. At most one row per geography and report date.
    pub report_date: ReportDate,
    /// The cumulative count as printed, or its explicit absence with the reason.
    pub cases: CumulativeCount,
    /// What the series counts. On a `missing` row it names the series the row belongs to; it does
    /// not assert that the report's own wording matched (see `not_labelled_confirmed`).
    pub case_definition: CaseDefinition,
    /// The snapshots this row was read from (the report) and any reference data used to key it.
    pub provenance: Provenances,
}

// ---------------------------------------------------------------------------------------------
// Published artifact: the v6 envelope (a per-file provenance table that rows index), version 7.

/// The artifact as validated, expanded rows in Rust. Serialization interns provenance by the
/// complete record, preserving row order and the order and repetition of each row's records;
/// deserialization expands the indices and validates every row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CumulativeCaseReportArtifact {
    pub rows: Vec<CumulativeCaseReport>,
}

#[derive(Serialize)]
struct PackedRow<'a> {
    geography: GeoId,
    report_date: ReportDate,
    cases: CumulativeCount,
    case_definition: CaseDefinition,
    provenance: &'a [u32],
}

#[derive(Serialize)]
struct PackedArtifact<'a> {
    contract_version: u8,
    provenance: &'a [Provenance],
    rows: Vec<PackedRow<'a>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireRow {
    geography: GeoId,
    report_date: ReportDate,
    cases: CumulativeCount,
    case_definition: CaseDefinition,
    provenance: Vec<u32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireArtifact {
    contract_version: u8,
    provenance: Vec<Provenance>,
    rows: Vec<WireRow>,
}

impl Serialize for CumulativeCaseReportArtifact {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::Error;
        let mut table: Vec<Provenance> = Vec::new();
        let mut lookup = std::collections::BTreeMap::new();
        let mut indices: Vec<Vec<u32>> = Vec::with_capacity(self.rows.len());
        for row in &self.rows {
            let mut row_indices = Vec::with_capacity(row.provenance.as_slice().len());
            for record in row.provenance.as_slice() {
                let key = serde_json::to_string(record).map_err(S::Error::custom)?;
                let index = match lookup.get(&key) {
                    Some(index) => *index,
                    None => {
                        let index = u32::try_from(table.len()).map_err(S::Error::custom)?;
                        table.push(record.clone());
                        lookup.insert(key, index);
                        index
                    }
                };
                row_indices.push(index);
            }
            indices.push(row_indices);
        }
        PackedArtifact {
            contract_version: 7,
            provenance: &table,
            rows: self
                .rows
                .iter()
                .zip(&indices)
                .map(|(row, provenance)| PackedRow {
                    geography: row.geography,
                    report_date: row.report_date,
                    cases: row.cases,
                    case_definition: row.case_definition,
                    provenance,
                })
                .collect(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for CumulativeCaseReportArtifact {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let wire = WireArtifact::deserialize(deserializer)?;
        if wire.contract_version != 7 {
            return Err(D::Error::custom(
                "cumulative case artifact contract_version must be 7",
            ));
        }
        let mut seen = BTreeSet::new();
        let mut rows = Vec::with_capacity(wire.rows.len());
        for row in wire.rows {
            let mut records = Vec::with_capacity(row.provenance.len());
            for index in row.provenance {
                records.push(
                    wire.provenance
                        .get(index as usize)
                        .ok_or_else(|| {
                            D::Error::custom("provenance index outside this file's table")
                        })?
                        .clone(),
                );
            }
            if !seen.insert((row.geography, row.report_date)) {
                return Err(D::Error::custom(format!(
                    "duplicate row for geography {} and report date {}",
                    row.geography, row.report_date
                )));
            }
            rows.push(CumulativeCaseReport {
                geography: row.geography,
                report_date: row.report_date,
                cases: row.cases,
                case_definition: row.case_definition,
                provenance: Provenances::new(records).map_err(D::Error::custom)?,
            });
        }
        Ok(Self { rows })
    }
}

impl JsonSchema for CumulativeCaseReportArtifact {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("CumulativeCaseReportArtifact")
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        // The row schema with its provenance replaced by indices into the file's table; its
        // definitions are lifted to the envelope root so every local $ref resolves (the same
        // construction as the v6 envelope).
        let mut row =
            serde_json::to_value(schema_for!(CumulativeCaseReport)).expect("row schema serializes");
        let definitions = row
            .as_object_mut()
            .unwrap()
            .remove("$defs")
            .unwrap_or(json!({}));
        row.as_object_mut().unwrap().remove("$schema");
        row["properties"]["provenance"] = json!({
            "type": "array", "minItems": 1,
            "items": { "type": "integer", "format": "uint32", "minimum": 0, "maximum": u32::MAX }
        });
        let provenance_schema = serde_json::to_value(schema_for!(Provenance)).unwrap();
        let mut definitions = definitions.as_object().unwrap().clone();
        if let Some(provenance_defs) = provenance_schema.get("$defs").and_then(Value::as_object) {
            definitions.extend(provenance_defs.clone());
        }
        let mut provenance = provenance_schema.as_object().unwrap().clone();
        provenance.remove("$schema");
        provenance.remove("$defs");
        Schema::try_from(json!({
            "type": "object", "additionalProperties": false,
            "required": ["contract_version", "provenance", "rows"],
            "properties": {
                "contract_version": { "const": 7, "type": "integer" },
                "provenance": { "type": "array", "items": provenance },
                "rows": { "type": "array", "items": row }
            },
            "$defs": definitions
        }))
        .expect("artifact schema is an object")
    }
}
