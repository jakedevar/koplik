//! Kindergarten MMR coverage: SchoolVaxView states and DSHS's published county worksheet.
//! No estimates are imputed; see SOURCES.md for population limitations and baseline choice.
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Cursor,
};

use calamine::{Data, Reader, Xlsx};
use koplik_contracts::v1::{
    CountyFips, CoverageValue, GeoId, KindergartenMmrCoverage, MissingReason, Provenances,
    SchoolYear,
};
use serde::Deserialize;

use crate::{
    error::{IngestError, Result},
    jurisdictions::{self, Jurisdiction},
    source::SourceSpec,
    store::{Retrieval, SnapshotStore, sha256_of},
};

pub const CDC_SOURCE_ID: &str = "cdc-schoolvaxview-kindergarten";
pub const CDC_LICENCE_ID: &str = "cdc-schoolvaxview-terms-unconfirmed";
pub const ROW_LIMIT: usize = 5000;
pub const FIRST_YEAR: u16 = 2023;
pub const LAST_YEAR: u16 = 2024;
/// Last completed school year before the 2025 outbreak; not an as-of publication guarantee.
pub const TEXAS_BASELINE_YEAR: u16 = 2023;
pub const TEXAS_LICENCE_ID: &str = "texas-dshs-terms-unconfirmed";
pub const COUNTY_SOURCE_ID: &str = "texas-dshs-county-fips";
pub const COUNTY_URL: &str = "https://www.dshs.texas.gov/center-health-statistics/texas-county-numbers-public-health-regions";
pub const COUNTY_LICENCE_ID: &str = TEXAS_LICENCE_ID;

fn parse_error(s: impl Into<String>) -> IngestError {
    IngestError::Parse(s.into())
}
fn enc(s: &str) -> String {
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~' | b',') {
                char::from(b).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}
fn years(first: u16, last: u16) -> Result<Vec<SchoolYear>> {
    if first > last {
        return Err(IngestError::Invalid(
            "first school year is after last".into(),
        ));
    }
    (first..=last)
        .map(|y| SchoolYear::new(y).map_err(IngestError::Invalid))
        .collect()
}
pub fn cdc_source_spec(first: u16, last: u16) -> Result<SourceSpec> {
    let ys = years(first, last)?
        .iter()
        .map(|y| format!("'{y}'"))
        .collect::<Vec<_>>()
        .join(",");
    let params = [
        ("$select", "vaccine,dose,geography_type,geography,year_season,coverage_estimate,foot_notes,survey_type".to_owned()),
        ("$where", format!("year_season in({ys}) AND (vaccine='MMR' OR (vaccine='Exemption' AND dose='Any Exemption'))")),
        ("$order", "year_season,geography,vaccine".to_owned()),
        ("$limit", ROW_LIMIT.to_string()),
    ];
    Ok(SourceSpec {
        source_id: CDC_SOURCE_ID.into(),
        licence_id: CDC_LICENCE_ID.into(),
        url: format!(
            "https://data.cdc.gov/resource/ijqb-a7ye.json?{}",
            params
                .iter()
                .map(|(k, v)| format!("{k}={}", enc(v)))
                .collect::<Vec<_>>()
                .join("&")
        ),
    })
}
pub fn texas_source_spec(year: u16) -> Result<SourceSpec> {
    // These two URLs were verified on DSHS's school coverage landing page, 2026-10-07.
    if !matches!(year, 2023 | 2024) {
        return Err(IngestError::Invalid(
            "DSHS coverage supports verified school years 2023-24 and 2024-25".into(),
        ));
    }
    Ok(SourceSpec {
        source_id: format!("texas-dshs-kindergarten-{year}"),
        licence_id: TEXAS_LICENCE_ID.into(),
        url: format!(
            "https://www.dshs.texas.gov/sites/default/files/LIDS-Immunizations/xls/{year}-{}_School_Vaccination_Coverage_Levels_Kindergarten.xlsx",
            year + 1
        ),
    })
}
pub fn county_source_spec() -> SourceSpec {
    SourceSpec {
        source_id: COUNTY_SOURCE_ID.into(),
        url: COUNTY_URL.into(),
        licence_id: COUNTY_LICENCE_ID.into(),
    }
}

fn verify(bytes: &[u8], r: &Retrieval, spec: &SourceSpec) -> Result<()> {
    if r.source_id != spec.source_id
        || r.url != spec.url
        || r.licence_id != spec.licence_id
        || r.sha256 != sha256_of(bytes)
        || r.bytes != bytes.len() as u64
        || !(200..300).contains(&r.http_status)
    {
        return Err(parse_error(
            "snapshot bytes or retrieval do not match the requested coverage source",
        ));
    }
    Ok(())
}

/// Bounded estimates are unknown, not the bound or a midpoint. A literal 0 is a real zero.
fn percent(value: Option<&str>, scale: f64) -> std::result::Result<f64, MissingReason> {
    let s = value.unwrap_or("").trim();
    match s {
        "" | "NR" | "NA" | "N/A" | "NReq" | "NReq." | "-" => {
            return Err(MissingReason::NotReported);
        }
        "*" | "**" | "***" | "S" | "Suppressed" => return Err(MissingReason::Suppressed),
        _ => {}
    }
    let n = s.parse::<f64>().map_err(|_| MissingReason::Ambiguous)?;
    if !n.is_finite() || !(0.0..=100.0 / scale).contains(&n) {
        return Err(MissingReason::Ambiguous);
    }
    Ok(n * scale)
}
fn coverage(pct: std::result::Result<f64, MissingReason>, exemption: Option<f64>) -> CoverageValue {
    match pct {
        Ok(coverage_pct) => CoverageValue::Reported {
            coverage_pct,
            exemption_pct: exemption,
        },
        Err(reason) => CoverageValue::Missing { reason },
    }
}
fn row(
    geography: GeoId,
    school_year: SchoolYear,
    coverage: CoverageValue,
    provenance: Provenances,
) -> KindergartenMmrCoverage {
    KindergartenMmrCoverage {
        geography,
        school_year,
        coverage,
        imputed: false,
        imputation_method: None,
        provenance,
    }
}

#[derive(Deserialize)]
struct CdcRow {
    vaccine: String,
    #[serde(default)]
    dose: String,
    geography_type: String,
    geography: String,
    year_season: SchoolYear,
    #[serde(default)]
    coverage_estimate: Option<String>,
}
/// Emit 50 states + DC for every requested year, even when the source has no row.
/// City/rest-of-state rows are not state estimates and are never averaged or added.
pub fn parse_cdc(
    bytes: &[u8],
    retrieval: &Retrieval,
    first: u16,
    last: u16,
) -> Result<Vec<KindergartenMmrCoverage>> {
    verify(bytes, retrieval, &cdc_source_spec(first, last)?)?;
    let requested = years(first, last)?;
    let raw: Vec<CdcRow> = serde_json::from_slice(bytes)
        .map_err(|e| parse_error(format!("SchoolVaxView JSON: {e}")))?;
    if raw.len() >= ROW_LIMIT {
        return Err(parse_error(
            "SchoolVaxView query limit reached; response may be truncated",
        ));
    }
    let mut cells = BTreeMap::new();
    for r in raw {
        if !requested.contains(&r.year_season) {
            return Err(parse_error("unexpected SchoolVaxView school year"));
        }
        let is_mmr = r.vaccine == "MMR" && r.dose.is_empty();
        let is_exemption = r.vaccine == "Exemption" && r.dose == "Any Exemption";
        if !is_mmr && !is_exemption {
            return Err(parse_error("unexpected SchoolVaxView vaccine/dose"));
        }
        match r.geography_type.as_str() {
            "National" if matches!(r.geography.as_str(), "United States" | "U.S. Median") => {
                continue;
            }
            "States" => {}
            _ => {
                return Err(parse_error(format!(
                    "unexpected geography type {}",
                    r.geography_type
                )));
            }
        }
        if matches!(
            r.geography.as_str(),
            "NY-City of New York" | "NY-Rest of state" | "TX-City of Houston"
        ) {
            continue;
        }
        let fips = match jurisdictions::lookup(&r.geography) {
            Jurisdiction::State(f) if f.code() <= 56 && r.geography != "New York City" => f,
            _ => {
                return Err(parse_error(format!(
                    "unknown SchoolVaxView state {}",
                    r.geography
                )));
            }
        };
        let key = (fips, r.year_season, is_exemption);
        if cells
            .insert(key, percent(r.coverage_estimate.as_deref(), 1.0))
            .is_some()
        {
            return Err(parse_error(format!(
                "duplicate SchoolVaxView cell for {}",
                r.geography
            )));
        }
    }
    let mut rows = Vec::new();
    for (fips, _) in jurisdictions::state_components()
        .into_iter()
        .filter(|(f, _)| f.code() <= 56)
    {
        for &year in &requested {
            let mmr = cells
                .get(&(fips, year, false))
                .copied()
                .unwrap_or(Err(MissingReason::NotReported));
            let exemption = cells
                .get(&(fips, year, true))
                .and_then(|p| p.as_ref().ok())
                .copied();
            rows.push(row(
                GeoId::State(fips),
                year,
                coverage(mmr, exemption),
                Provenances::one(retrieval.provenance()),
            ));
        }
    }
    Ok(rows)
}

/// DSHS's explicit county name/FIPS crosswalk. Never derive FIPS from spreadsheet order.
pub fn texas_counties(bytes: &[u8], retrieval: &Retrieval) -> Result<BTreeMap<String, CountyFips>> {
    Ok(texas_county_names(bytes, retrieval)?
        .into_iter()
        .map(|(fips, name)| (name.to_ascii_lowercase(), fips))
        .collect())
}

/// The DSHS county/FIPS crosswalk as published: every Texas county FIPS with the county name
/// exactly as DSHS spells it (e.g. `Gaines`), for display. Same checks as [`texas_counties`].
pub fn texas_county_names(
    bytes: &[u8],
    retrieval: &Retrieval,
) -> Result<BTreeMap<CountyFips, String>> {
    verify(bytes, retrieval, &county_source_spec())?;
    let html =
        std::str::from_utf8(bytes).map_err(|e| parse_error(format!("DSHS county HTML: {e}")))?;
    let doc = scraper::Html::parse_document(html);
    let selector = |s| scraper::Selector::parse(s).expect("fixed selector");
    let tables = selector("table");
    let headers = selector("thead th");
    let body_rows = selector("tbody tr");
    let cells = selector("td");
    let text = |e: scraper::ElementRef<'_>| {
        e.text()
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    };
    let matched: Vec<_> = doc
        .select(&tables)
        .filter(|t| {
            let h: Vec<_> = t.select(&headers).map(text).collect();
            h == [
                "CountyNumber",
                "CountyName",
                "FIPSCode",
                "Public HealthRegion",
                "Health ServiceRegion",
            ] || h
                == [
                    "County Number",
                    "County Name",
                    "FIPS Code",
                    "Public Health Region",
                    "Health Service Region",
                ]
        })
        .collect();
    if matched.len() != 1 {
        return Err(parse_error(
            "expected one DSHS county/FIPS table with known columns",
        ));
    }
    let mut out = BTreeMap::new();
    let mut names = BTreeSet::new();
    for r in matched[0].select(&body_rows) {
        let r: Vec<_> = r.select(&cells).map(text).collect();
        if r.len() != 5 {
            return Err(parse_error("unexpected DSHS county/FIPS row"));
        }
        let name = &r[1];
        let fips: CountyFips = r[2]
            .parse()
            .map_err(|e| parse_error(format!("county FIPS: {e}")))?;
        if fips.state().code() != 48
            || name.is_empty()
            || !names.insert(name.to_ascii_lowercase())
            || out.insert(fips, name.clone()).is_some()
        {
            return Err(parse_error("invalid or duplicate DSHS county identity"));
        }
    }
    if out.len() != 254 {
        return Err(parse_error(format!(
            "expected 254 Texas counties, got {}",
            out.len()
        )));
    }
    Ok(out)
}

fn cell_percent(cell: Option<&Data>) -> std::result::Result<f64, MissingReason> {
    match cell {
        None | Some(Data::Empty) => Err(MissingReason::NotReported),
        Some(Data::String(s)) => percent(Some(s), 100.0),
        Some(Data::Float(v)) => percent(Some(&v.to_string()), 100.0),
        Some(Data::Int(v)) => percent(Some(&v.to_string()), 100.0),
        _ => Err(MissingReason::Ambiguous),
    }
}
/// Read DSHS's county MMR column (Excel fractions), without averaging district rates.
/// County identity carries the DSHS crosswalk snapshot as an additional provenance record.
pub fn parse_texas(
    bytes: &[u8],
    retrieval: &Retrieval,
    county_bytes: &[u8],
    county_retrieval: &Retrieval,
    year: u16,
) -> Result<Vec<KindergartenMmrCoverage>> {
    verify(bytes, retrieval, &texas_source_spec(year)?)?;
    let counties = texas_counties(county_bytes, county_retrieval)?;
    let mut book =
        Xlsx::new(Cursor::new(bytes)).map_err(|e| parse_error(format!("DSHS XLSX: {e}")))?;
    let district = book
        .worksheet_range("Coverage by District")
        .map_err(|e| parse_error(format!("DSHS title worksheet: {e}")))?;
    let title = district
        .get((0, 0))
        .map(ToString::to_string)
        .unwrap_or_default();
    if !title.contains("Kindergarten") || !title.contains(&format!("{year}-{}", year + 1)) {
        return Err(parse_error(
            "DSHS workbook title does not match requested grade/year",
        ));
    }
    let sheet = book
        .worksheet_range("Coverage by County")
        .map_err(|e| parse_error(format!("DSHS county worksheet: {e}")))?;
    let mut sheet_rows = sheet.rows();
    // 2023-24 starts with the header; 2024-25 adds a title and an NR explanation.
    let header = sheet_rows
        .by_ref()
        .take(3)
        .find(|r| {
            r.iter().any(|v| v.to_string() == "County") && r.iter().any(|v| v.to_string() == "MMR")
        })
        .ok_or_else(|| parse_error("DSHS county header not in the first three rows"))?;
    let find = |label: &str| -> Result<usize> {
        let indices: Vec<_> = header
            .iter()
            .enumerate()
            .filter(|(_, v)| v.to_string() == label)
            .map(|(i, _)| i)
            .collect();
        if indices.len() != 1 {
            return Err(parse_error(format!("expected one DSHS {label} column")));
        }
        Ok(indices[0])
    };
    let name_col = find("County")?;
    let mmr_col = find("MMR")?;
    let mut cells = BTreeMap::new();
    for cells_row in sheet_rows {
        let name = cells_row
            .get(name_col)
            .map(ToString::to_string)
            .unwrap_or_default();
        let name = name.trim();
        if name.is_empty() && cells_row.iter().all(|c| matches!(c, Data::Empty)) {
            continue;
        }
        if name == "Texas" {
            continue;
        }
        let fips = counties
            .get(&name.to_ascii_lowercase())
            .ok_or_else(|| parse_error(format!("unknown DSHS county {name:?}")))?;
        if cells
            .insert(*fips, cell_percent(cells_row.get(mmr_col)))
            .is_some()
        {
            return Err(parse_error(format!("duplicate DSHS county {name}")));
        }
    }
    let year = SchoolYear::new(year).map_err(IngestError::Invalid)?;
    let provenance = Provenances::new(vec![retrieval.provenance(), county_retrieval.provenance()])
        .expect("two sources");
    let mut out: Vec<_> = counties
        .values()
        .map(|&fips| {
            row(
                GeoId::County(fips),
                year,
                coverage(
                    cells
                        .get(&fips)
                        .copied()
                        .unwrap_or(Err(MissingReason::NotReported)),
                    None,
                ),
                provenance.clone(),
            )
        })
        .collect();
    out.sort_by_key(|r| r.geography);
    Ok(out)
}

fn stored(store: &SnapshotStore, id: &str) -> Result<(Retrieval, Vec<u8>)> {
    let r = store
        .latest(id)?
        .ok_or_else(|| IngestError::NoSnapshot(id.into()))?;
    let bytes = store.get_verified(&r.sha256)?;
    Ok((r, bytes))
}
pub fn parse_latest_cdc(
    store: &SnapshotStore,
    first: u16,
    last: u16,
) -> Result<Vec<KindergartenMmrCoverage>> {
    let (r, b) = stored(store, CDC_SOURCE_ID)?;
    parse_cdc(&b, &r, first, last)
}
pub fn parse_latest_texas(
    store: &SnapshotStore,
    year: u16,
) -> Result<Vec<KindergartenMmrCoverage>> {
    let (r, b) = stored(store, &texas_source_spec(year)?.source_id)?;
    let (cr, cb) = stored(store, COUNTY_SOURCE_ID)?;
    parse_texas(&b, &r, &cb, &cr, year)
}
/// The gaps artifact is a subset of v1 rows, retaining the missing reason and provenance.
pub fn gaps(rows: &[KindergartenMmrCoverage]) -> Vec<&KindergartenMmrCoverage> {
    rows.iter()
        .filter(|r| matches!(r.coverage, CoverageValue::Missing { .. }))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_bounds_and_real_zero() {
        assert_eq!(percent(None, 1.0), Err(MissingReason::NotReported));
        assert_eq!(percent(Some("NR"), 100.0), Err(MissingReason::NotReported));
        assert_eq!(percent(Some("**"), 1.0), Err(MissingReason::Suppressed));
        for s in ["<0.1", ">99", "NaN", "101", "-1", "invalid"] {
            assert_eq!(percent(Some(s), 1.0), Err(MissingReason::Ambiguous));
        }
        assert_eq!(percent(Some("0"), 1.0), Ok(0.0));
        assert_eq!(
            cell_percent(Some(&Data::Error(calamine::CellErrorType::Div0))),
            Err(MissingReason::Ambiguous)
        );
        assert_eq!(
            cell_percent(Some(&Data::Float(1.01))),
            Err(MissingReason::Ambiguous)
        );
    }
}
