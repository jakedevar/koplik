//! U.S. Census Bureau 2025 vintage population estimates and Gazetteer internal points.
//!
//! Population is July 1, 2025 (`POPESTIMATE2025`). County Gazetteer internal points are
//! representative points, not population-weighted or geometric centroids.

use std::io::{Cursor, Read};
use std::str::FromStr;

use csv::{ReaderBuilder, StringRecord};
use koplik_contracts::v1::{
    Centroid, CountyFips, GeoId, Geography, Population, Provenances, StateFips,
};
use zip::ZipArchive;

use crate::error::{IngestError, Result};
use crate::source::SourceSpec;
use crate::store::{Retrieval, SnapshotStore};

pub const STATE_SOURCE_ID: &str = "census-state-population-2025";
pub const COUNTY_SOURCE_ID: &str = "census-county-population-2025";
pub const STATE_GEO_SOURCE_ID: &str = "census-state-gazetteer-2025";
pub const COUNTY_GEO_SOURCE_ID: &str = "census-county-gazetteer-2025";
pub const LICENCE_ID: &str = "us-census-public-domain";
pub const STATE_URL: &str = "https://www2.census.gov/programs-surveys/popest/datasets/2020-2025/state/totals/NST-EST2025-ALLDATA.csv";
pub const COUNTY_URL: &str = "https://www2.census.gov/programs-surveys/popest/datasets/2020-2025/counties/totals/co-est2025-alldata.csv";
pub const GAZETTEER_URL: &str = "https://www2.census.gov/geo/docs/maps-data/data/gazetteer/2025_Gazetteer/2025_Gaz_counties_national.zip";
pub const STATE_GAZETTEER_URL: &str = "https://www2.census.gov/geo/docs/maps-data/data/gazetteer/2025_Gazetteer/2025_Gaz_state_national.zip";
pub const YEAR: u16 = 2025;

pub fn source_spec(source_id: &str) -> Result<SourceSpec> {
    let (url, id) = match source_id {
        "census-state-population" => (STATE_URL, STATE_SOURCE_ID),
        "census-county-population" => (COUNTY_URL, COUNTY_SOURCE_ID),
        "census-texas-counties" => (GAZETTEER_URL, COUNTY_GEO_SOURCE_ID),
        "census-states" => (STATE_GAZETTEER_URL, STATE_GEO_SOURCE_ID),
        _ => {
            return Err(IngestError::Invalid(format!(
                "unknown Census source {source_id:?}"
            )));
        }
    };
    Ok(SourceSpec {
        source_id: id.to_owned(),
        url: url.to_owned(),
        licence_id: LICENCE_ID.to_owned(),
    })
}

/// The fixed four-file run, sequential through one governed fetcher.
pub const SOURCES: [&str; 4] = [
    "census-state-population",
    "census-county-population",
    "census-texas-counties",
    "census-states",
];
pub fn manifest() -> Result<crate::census_files::NamedFileAllowlist> {
    crate::census_files::NamedFileAllowlist::from_json(include_bytes!(
        "../manifests/census-population-2025.json"
    ))
}
pub fn fetch_source<C: crate::http::HttpClient, T: crate::polite::Timekeeper>(
    fetcher: &mut crate::polite::PoliteFetcher<C, T>,
    store: &SnapshotStore,
    source: &str,
) -> Result<(Retrieval, crate::store::PutOutcome)> {
    crate::census_files::fetch_to_store(fetcher, store, &source_spec(source)?, &manifest()?)
}

pub fn parse_populations(
    retrieval: &Retrieval,
    bytes: &[u8],
    states: bool,
) -> Result<Vec<Population>> {
    let mut rdr = ReaderBuilder::new().flexible(false).from_reader(bytes);
    let headers = rdr.headers().map_err(csv_error)?.clone();
    let idx = |name: &str| {
        headers
            .iter()
            .position(|h| h.trim() == name)
            .ok_or_else(|| IngestError::Invalid(format!("Census CSV missing column {name}")))
    };
    let (state_i, county_i, sumlev_i, name_i, pop_i) = if states {
        (
            idx("STATE")?,
            None,
            Some(idx("SUMLEV")?),
            idx("NAME")?,
            idx("POPESTIMATE2025")?,
        )
    } else {
        (
            idx("STATE")?,
            Some(idx("COUNTY")?),
            None,
            idx("CTYNAME")?,
            idx("POPESTIMATE2025")?,
        )
    };
    let mut rows = Vec::new();
    for row in rdr.records() {
        let row = row.map_err(csv_error)?;
        let state_code = value(&row, state_i)?
            .parse::<u8>()
            .map_err(|_| bad_row("STATE", &row))?;
        let geo = if states {
            if row.get(sumlev_i.unwrap()) != Some("040") || state_code == 0 {
                continue;
            }
            GeoId::State(
                StateFips::new(state_code).map_err(|e| IngestError::Invalid(e.to_string()))?,
            )
        } else {
            if state_code != 48 {
                continue;
            }
            let county_code = value(&row, county_i.unwrap())?
                .parse::<u16>()
                .map_err(|_| bad_row("COUNTY", &row))?;
            if county_code == 0 {
                continue;
            }
            GeoId::County(
                CountyFips::from_parts(StateFips::new(48).unwrap(), county_code)
                    .map_err(|e| IngestError::Invalid(e.to_string()))?,
            )
        };
        let count = value(&row, pop_i)?
            .parse::<u64>()
            .map_err(|_| bad_row("POPESTIMATE2025", &row))?;
        let _source_name = value(&row, name_i)?;
        rows.push(Population {
            geography: geo,
            year: YEAR,
            count,
            provenance: Provenances::one(retrieval.provenance()),
        });
    }
    rows.sort_by_key(|r| r.geography);
    if states && rows.len() < 50 {
        return Err(IngestError::Invalid(format!(
            "Census state population snapshot yielded only {} states",
            rows.len()
        )));
    }
    if !states && rows.len() != 254 {
        return Err(IngestError::Invalid(format!(
            "Census county population snapshot yielded {} Texas counties; expected 254",
            rows.len()
        )));
    }
    Ok(rows)
}

pub fn parse_geographies(
    retrieval: &Retrieval,
    bytes: &[u8],
    states: bool,
) -> Result<Vec<Geography>> {
    let mut archive = ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| IngestError::Invalid(format!("invalid Gazetteer ZIP: {e}")))?;
    let wanted = if states { "state" } else { "count" };
    let entry_index = (0..archive.len())
        .find(|&i| {
            archive
                .by_index(i)
                .ok()
                .is_some_and(|f| f.name().to_ascii_lowercase().contains(wanted))
        })
        .ok_or_else(|| IngestError::Invalid("Gazetteer ZIP has no expected data file".into()))?;
    let mut file = archive
        .by_index(entry_index)
        .map_err(|e| IngestError::Invalid(format!("Gazetteer ZIP entry: {e}")))?;
    let mut raw = Vec::new();
    file.read_to_end(&mut raw)
        .map_err(|e| IngestError::Invalid(format!("Gazetteer read: {e}")))?;
    let mut rdr = ReaderBuilder::new()
        .delimiter(b'|')
        .from_reader(raw.as_slice());
    let headers = rdr.headers().map_err(csv_error)?.clone();
    let idx = |name: &str| {
        headers
            .iter()
            .position(|h| h.trim() == name)
            .ok_or_else(|| IngestError::Invalid(format!("Gazetteer missing column {name}")))
    };
    let geoid_i = idx("GEOID")?;
    let name_i = idx("NAME")?;
    let lat_i = idx("INTPTLAT")?;
    let lon_i = idx("INTPTLONG")?;
    let mut rows = Vec::new();
    for row in rdr.records() {
        let row = row.map_err(csv_error)?;
        let geoid = value(&row, geoid_i)?;
        let id = if states {
            let state = match StateFips::from_str(geoid) {
                Ok(s) => s,
                Err(_) => continue,
            };
            GeoId::State(state)
        } else {
            if !geoid.starts_with("48") {
                continue;
            }
            GeoId::County(
                CountyFips::from_str(geoid).map_err(|e| IngestError::Invalid(e.to_string()))?,
            )
        };
        let lat = value(&row, lat_i)?
            .parse::<f64>()
            .map_err(|_| bad_row("INTPTLAT", &row))?;
        let lon = value(&row, lon_i)?
            .parse::<f64>()
            .map_err(|_| bad_row("INTPTLONG", &row))?;
        let centroid = Centroid::new(lat, lon).map_err(IngestError::Invalid)?;
        let name = value(&row, name_i)?.to_owned();
        rows.push(
            Geography::new(id, name, Provenances::one(retrieval.provenance()))
                .map_err(IngestError::Invalid)?
                .with_centroid(centroid),
        );
    }
    rows.sort_by_key(|r| r.id);
    if states && rows.len() < 50 {
        return Err(IngestError::Invalid(format!(
            "Gazetteer yielded only {} states",
            rows.len()
        )));
    }
    if !states && rows.len() != 254 {
        return Err(IngestError::Invalid(format!(
            "Gazetteer yielded {} Texas counties; expected 254",
            rows.len()
        )));
    }
    Ok(rows)
}

pub fn parse_latest_populations(
    store: &SnapshotStore,
    states: bool,
) -> Result<(Retrieval, Vec<Population>)> {
    let id = if states {
        STATE_SOURCE_ID
    } else {
        COUNTY_SOURCE_ID
    };
    let retrieval = store
        .latest(id)?
        .ok_or_else(|| IngestError::Invalid(format!("no snapshot for {id}")))?;
    let bytes = store.get_verified(&retrieval.sha256)?;
    let rows = parse_populations(&retrieval, &bytes, states)?;
    Ok((retrieval, rows))
}

pub fn parse_latest_geographies(
    store: &SnapshotStore,
    states: bool,
) -> Result<(Retrieval, Vec<Geography>)> {
    let id = if states {
        STATE_GEO_SOURCE_ID
    } else {
        COUNTY_GEO_SOURCE_ID
    };
    let retrieval = store
        .latest(id)?
        .ok_or_else(|| IngestError::Invalid(format!("no Census Gazetteer snapshot for {id}")))?;
    let is_state = states;
    let bytes = store.get_verified(&retrieval.sha256)?;
    let rows = parse_geographies(&retrieval, &bytes, is_state)?;
    Ok((retrieval, rows))
}

fn value<'a>(row: &'a StringRecord, i: usize) -> Result<&'a str> {
    row.get(i)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| IngestError::Invalid("Census row has an empty required value".into()))
}
fn bad_row(field: &str, row: &StringRecord) -> IngestError {
    IngestError::Invalid(format!("invalid Census {field} value in row {:?}", row))
}
fn csv_error(error: csv::Error) -> IngestError {
    IngestError::Invalid(format!("invalid Census delimited data: {error}"))
}
