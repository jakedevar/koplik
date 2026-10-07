//! CDC weekly measles cases by state (contracts v2 rows, case definition
//! `confirmed_or_unknown_status`), from the NNDSS Weekly Data table on data.cdc.gov
//! (Socrata dataset `x9gk-5huc`). Why this source and how weekly counts are derived is
//! documented in `SOURCES.md`; in short:
//!
//! - One row per (reporting jurisdiction, MMWR reporting year, week, label), where label is
//!   `Measles, Indigenous` or `Measles, Imported`. `m3` is the cumulative year-to-date count
//!   as published that week; `m1` (the "current week" count) is NOT used because it omits
//!   cases added to earlier weeks (Texas 2025: sum of `m1` = 288, final cumulative = 803).
//! - Weekly new cases for a state in week w = total cumulative(w) - total cumulative(w-1)
//!   (week 1: total cumulative(1)), where total cumulative = Indigenous + Imported (+ New York
//!   City for New York State). Summing before differencing keeps a case reclassified between
//!   Imported and Indigenous from looking like a decrease.
//! - The week is the MMWR *reporting* week of the CDC table (cases are counted when they reach
//!   CDC), not the onset week.
//! - Missing stays missing: absent rows and unavailable flags are `NotReported`; a cumulative
//!   that decreases (reclassification/removal) or cannot be read is `Ambiguous`. The flag `-`
//!   ("no reported cases") is a reported zero cumulative.

use std::collections::BTreeMap;

use koplik_contracts::v2::{
    CaseCount, CaseDefinition, GeoId, MissingReason, MmwrWeek, Provenances, StateFips,
    WeeklyCaseCount,
};
use serde::{Deserialize, Deserializer};

use crate::error::{IngestError, Result};
use crate::jurisdictions::{self, Jurisdiction};
use crate::source::SourceSpec;
use crate::store::{Retrieval, SnapshotStore};

pub const SOURCE_ID: &str = "cdc-nndss-weekly-measles";
/// Licence id as listed in `SOURCES.md`. The dataset metadata carries no licence, so this id
/// stands for "terms not yet confirmed": an operator decision is required before publishing.
pub const LICENCE_ID: &str = "cdc-open-data-terms-unconfirmed";
pub const DATASET_ID: &str = "x9gk-5huc";
pub const ENDPOINT: &str = "https://data.cdc.gov/resource/x9gk-5huc.json";
pub const LABELS: [&str; 2] = ["Measles, Indigenous", "Measles, Imported"];
/// `$limit` of the query; a response with this many rows may be truncated and is refused.
pub const ROW_LIMIT: usize = 50_000;
pub const FIRST_YEAR: u16 = 2025;

/// Percent-encode a query value (RFC 3986 unreserved characters and `,` stay as they are).
fn enc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~' | b',') {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The exact query URL for `first_year..=last_year` (explicit `$select`/`$where`/`$order`/
/// `$limit`, so a re-fetch is reproducible and the response order is deterministic).
pub fn query_url(first_year: u16, last_year: u16) -> Result<String> {
    if first_year > last_year {
        return Err(IngestError::Invalid(format!(
            "first year {first_year} is after last year {last_year}"
        )));
    }
    let labels = LABELS.map(|l| format!("'{l}'")).join(",");
    let years = (first_year..=last_year)
        .map(|y| format!("'{y}'"))
        .collect::<Vec<_>>()
        .join(",");
    let params = [
        ("$select", "states,year,week,label,m3,m3_flag".to_owned()),
        ("$where", format!("label in({labels}) AND year in({years})")),
        ("$order", "year,week,states,label".to_owned()),
        ("$limit", ROW_LIMIT.to_string()),
    ];
    let query = params
        .iter()
        .map(|(k, v)| format!("{k}={}", enc(v)))
        .collect::<Vec<_>>()
        .join("&");
    Ok(format!("{ENDPOINT}?{query}"))
}

pub fn source_spec(first_year: u16, last_year: u16) -> Result<SourceSpec> {
    Ok(SourceSpec {
        source_id: SOURCE_ID.to_owned(),
        url: query_url(first_year, last_year)?,
        licence_id: LICENCE_ID.to_owned(),
    })
}

/// A JSON string or number, read as a string (Socrata sends numbers as strings).
fn flex<'de, D: Deserializer<'de>>(d: D) -> std::result::Result<String, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Flex {
        S(String),
        N(serde_json::Number),
    }
    Ok(match Flex::deserialize(d)? {
        Flex::S(s) => s,
        Flex::N(n) => n.to_string(),
    })
}

fn flex_opt<'de, D: Deserializer<'de>>(d: D) -> std::result::Result<Option<String>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Flex {
        S(String),
        N(serde_json::Number),
    }
    Ok(Option::<Flex>::deserialize(d)?.map(|f| match f {
        Flex::S(s) => s,
        Flex::N(n) => n.to_string(),
    }))
}

#[derive(Debug, Deserialize)]
struct RawRow {
    states: String,
    #[serde(deserialize_with = "flex")]
    year: String,
    #[serde(deserialize_with = "flex")]
    week: String,
    label: String,
    #[serde(default, deserialize_with = "flex_opt")]
    m3: Option<String>,
    #[serde(default)]
    m3_flag: Option<String>,
}

type Cum = std::result::Result<u32, MissingReason>;

fn severity(r: MissingReason) -> u8 {
    match r {
        MissingReason::NotReported => 0,
        MissingReason::Suppressed => 1,
        MissingReason::Ambiguous => 2,
    }
}

fn worst(a: MissingReason, b: MissingReason) -> MissingReason {
    if severity(b) > severity(a) { b } else { a }
}

/// The cumulative count one source row publishes.
fn cumulative(row: Option<&RawRow>) -> Cum {
    let Some(row) = row else {
        return Err(MissingReason::NotReported);
    };
    match (row.m3.as_deref(), row.m3_flag.as_deref()) {
        (Some(v), None) => {
            let x: f64 = v.trim().parse().map_err(|_| MissingReason::Ambiguous)?;
            if x.is_finite() && x >= 0.0 && x.fract() == 0.0 && x <= f64::from(u32::MAX) {
                Ok(x as u32)
            } else {
                Err(MissingReason::Ambiguous)
            }
        }
        // "-": the jurisdiction reports and has no cases, so the cumulative is a real zero.
        (None, Some("-")) => Ok(0),
        // U unavailable, N not reportable, NN not nationally notifiable, NP not published,
        // NC not calculated: no value was published.
        (None, Some("U" | "N" | "NN" | "NP" | "NC")) => Err(MissingReason::NotReported),
        _ => Err(MissingReason::Ambiguous),
    }
}

/// Total cumulative for one geography: the sum over its NNDSS components and labels, missing
/// if any component is missing.
fn total_cumulative(
    cells: &BTreeMap<(String, u16, u8, String), RawRow>,
    names: &[&str],
    year: u16,
    week: u8,
) -> Cum {
    let mut total: u32 = 0;
    let mut missing: Option<MissingReason> = None;
    for name in names {
        for label in LABELS {
            let key = ((*name).to_owned(), year, week, label.to_owned());
            match cumulative(cells.get(&key)) {
                Ok(v) => match total.checked_add(v) {
                    Some(t) => total = t,
                    None => {
                        missing = Some(missing.map_or(MissingReason::Ambiguous, |m| {
                            worst(m, MissingReason::Ambiguous)
                        }))
                    }
                },
                Err(r) => missing = Some(missing.map_or(r, |m| worst(m, r))),
            }
        }
    }
    missing.map_or(Ok(total), Err)
}

/// Parse a stored response into weekly rows for every published state-level geography and
/// every MMWR week from week 1 to the latest week present in each year. Rows are ordered by
/// FIPS then week and each carries the retrieval's provenance.
pub fn parse_weekly_cases(bytes: &[u8], retrieval: &Retrieval) -> Result<Vec<WeeklyCaseCount>> {
    let raw: Vec<RawRow> = serde_json::from_slice(bytes)
        .map_err(|e| IngestError::Parse(format!("not a JSON array of NNDSS rows: {e}")))?;
    if raw.len() >= ROW_LIMIT {
        return Err(IngestError::Parse(format!(
            "response has {} rows, the query limit: it may be truncated",
            raw.len()
        )));
    }
    let mut cells: BTreeMap<(String, u16, u8, String), RawRow> = BTreeMap::new();
    let mut max_week: BTreeMap<u16, u8> = BTreeMap::new();
    for row in raw {
        if !LABELS.contains(&row.label.as_str()) {
            return Err(IngestError::Parse(format!(
                "unexpected label {:?} (expected one of {LABELS:?})",
                row.label
            )));
        }
        let year: u16 = row
            .year
            .trim()
            .parse()
            .map_err(|_| IngestError::Parse(format!("bad year {:?}", row.year)))?;
        let week: u8 = row
            .week
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|w| w.fract() == 0.0 && (1.0..=53.0).contains(w))
            .map(|w| w as u8)
            .ok_or_else(|| IngestError::Parse(format!("bad week {:?}", row.week)))?;
        MmwrWeek::new(year, week)
            .map_err(|e| IngestError::Parse(format!("{} {year}-W{week}: {e}", row.states)))?;
        match jurisdictions::lookup(&row.states) {
            Jurisdiction::Unknown => {
                return Err(IngestError::Parse(format!(
                    "unrecognised jurisdiction {:?}: add it to jurisdictions.rs",
                    row.states
                )));
            }
            Jurisdiction::Aggregate => continue,
            Jurisdiction::State(_) => {}
        }
        let w = max_week.entry(year).or_insert(0);
        *w = (*w).max(week);
        let key = (row.states.clone(), year, week, row.label.clone());
        if cells.contains_key(&key) {
            return Err(IngestError::Parse(format!(
                "duplicate row for {} {year}-W{week} {}",
                key.0, key.3
            )));
        }
        cells.insert(key, row);
    }

    let provenance = Provenances::one(retrieval.provenance());
    let mut out = Vec::new();
    for (fips, names) in jurisdictions::state_components() {
        for (&year, &last) in &max_week {
            let mut prev: Option<Cum> = None;
            for week in 1..=last {
                let cur = total_cumulative(&cells, &names, year, week);
                let cases = weekly(prev, cur);
                prev = Some(cur);
                out.push(WeeklyCaseCount {
                    geography: GeoId::State(fips),
                    week: MmwrWeek::new(year, week).expect("validated while reading rows"),
                    cases,
                    // NNDSS publishes confirmed AND unknown-status measles cases and the query has
                    // no case-status field to tell them apart (SOURCES.md), so never `Confirmed`.
                    case_definition: CaseDefinition::ConfirmedOrUnknownStatus,
                    provenance: provenance.clone(),
                });
            }
        }
    }
    out.sort_by_key(|r| (state_of(r), r.week));
    Ok(out)
}

fn state_of(r: &WeeklyCaseCount) -> StateFips {
    r.geography.state()
}

/// Week-over-week difference of cumulative counts; `prev` is `None` in week 1 (start of year).
fn weekly(prev: Option<Cum>, cur: Cum) -> CaseCount {
    let missing = |reason| CaseCount::Missing { reason };
    let cur = match cur {
        Ok(c) => c,
        Err(r) => return missing(r),
    };
    let base = match prev {
        None => 0,
        Some(Ok(p)) => p,
        // The previous cumulative is unknown, so this week's share cannot be told apart.
        Some(Err(_)) => return missing(MissingReason::Ambiguous),
    };
    match cur.checked_sub(base) {
        Some(count) => CaseCount::Reported { count },
        // The cumulative fell (reclassification or removal): not a negative case count.
        None => missing(MissingReason::Ambiguous),
    }
}

/// Parse the latest stored CDC snapshot (offline).
pub fn parse_latest(store: &SnapshotStore) -> Result<(Retrieval, Vec<WeeklyCaseCount>)> {
    let retrieval = store
        .latest(SOURCE_ID)?
        .ok_or_else(|| IngestError::NoSnapshot(SOURCE_ID.to_owned()))?;
    let bytes = store.get_verified(&retrieval.sha256)?;
    let rows = parse_weekly_cases(&bytes, &retrieval)?;
    Ok((retrieval, rows))
}
