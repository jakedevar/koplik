//! U.S. Census Bureau 2025 vintage population estimates and Gazetteer internal points.
//!
//! Population is July 1, 2025 (`POPESTIMATE2025`). County Gazetteer internal points are
//! representative points, not population-weighted or geometric centroids.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Cursor, Read};

use csv::{ByteRecord, ReaderBuilder, StringRecord};
use koplik_contracts::v1::{
    Centroid, CountyFips, GeoId, Geography, Population, Provenances, StateFips,
};
use zip::ZipArchive;

use crate::error::{IngestError, Result};
use crate::source::SourceSpec;
use crate::store::{Retrieval, SnapshotStore, sha256_of};

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

const MAX_MEMBER_BYTES: u64 = 64 * 1024 * 1024;

fn parse_error(message: impl Into<String>) -> IngestError {
    IngestError::Parse(message.into())
}

fn verify(retrieval: &Retrieval, bytes: &[u8], source: &str) -> Result<()> {
    let spec = source_spec(source)?;
    let pins = manifest()?;
    if retrieval.source_id != spec.source_id
        || retrieval.url != spec.url
        || retrieval.licence_id != spec.licence_id
        || !(200..300).contains(&retrieval.http_status)
        || retrieval.bytes != bytes.len() as u64
        || retrieval.sha256 != sha256_of(bytes)
        || pins.pin(&spec.url) != Some(&retrieval.sha256)
    {
        return Err(parse_error(
            "Census snapshot bytes, retrieval or manifest pin do not match the requested source",
        ));
    }
    Ok(())
}

fn expected_geographies(states: bool) -> BTreeSet<GeoId> {
    if states {
        // These published national files cover 50 states, DC and Puerto Rico.
        crate::jurisdictions::state_components()
            .into_iter()
            .filter(|(f, _)| f.code() <= 56 || f.code() == 72)
            .map(|(f, _)| GeoId::State(f))
            .collect()
    } else {
        // Texas FIPS county codes are odd 001..507; this is identifier validation, never
        // population imputation. DSHS's explicit county/FIPS table and allocation formula:
        // https://www.dshs.texas.gov/center-health-statistics/texas-county-numbers-public-health-regions
        (1..=507)
            .step_by(2)
            .map(|part| GeoId::County(CountyFips::new(48000 + part).expect("Texas FIPS")))
            .collect()
    }
}

fn ensure_complete(actual: impl Iterator<Item = GeoId>, states: bool) -> Result<()> {
    let actual: BTreeSet<_> = actual.collect();
    let expected = expected_geographies(states);
    if actual != expected {
        return Err(parse_error(format!(
            "incomplete Census geography set: missing {:?}, unexpected {:?}; no values imputed",
            expected
                .difference(&actual)
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            actual
                .difference(&expected)
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        )));
    }
    Ok(())
}

fn column(headers: &StringRecord, name: &str) -> Result<usize> {
    let matching: Vec<_> = headers
        .iter()
        .enumerate()
        .filter(|(_, h)| h.trim() == name)
        .map(|(i, _)| i)
        .collect();
    if matching.len() != 1 {
        return Err(parse_error(format!(
            "Census delimited data needs exactly one {name} column"
        )));
    }
    Ok(matching[0])
}
fn byte_value(row: &ByteRecord, i: usize) -> Result<&str> {
    std::str::from_utf8(row.get(i).unwrap_or_default())
        .map(str::trim)
        .map_err(|_| parse_error("non-ASCII Census identifier/numeric field"))
}

pub fn parse_populations(
    retrieval: &Retrieval,
    bytes: &[u8],
    states: bool,
) -> Result<Vec<Population>> {
    verify(
        retrieval,
        bytes,
        if states {
            "census-state-population"
        } else {
            "census-county-population"
        },
    )?;
    populations(retrieval, bytes, states)
}
fn populations(retrieval: &Retrieval, bytes: &[u8], states: bool) -> Result<Vec<Population>> {
    // The national county file has Latin-1 names outside Texas (e.g. Doña Ana). Keep raw
    // bytes immutable and read ByteRecord, decoding only numeric/identifier fields needed
    // here. Never use lossy UTF-8 substitution or rewrite the downloaded CSV.
    let mut rdr = ReaderBuilder::new().flexible(false).from_reader(bytes);
    let headers = rdr.headers().map_err(csv_error)?.clone();
    let state_i = column(&headers, "STATE")?;
    let sumlev_i = column(&headers, "SUMLEV")?;
    let county_i = if states {
        None
    } else {
        Some(column(&headers, "COUNTY")?)
    };
    let pop_i = column(&headers, "POPESTIMATE2025")?;
    let mut rows = BTreeMap::new();
    for record in rdr.byte_records() {
        let r = record.map_err(csv_error)?;
        let sumlev = byte_value(&r, sumlev_i)?;
        let state = byte_value(&r, state_i)?;
        let geo = if states {
            if sumlev != "040" {
                continue;
            }
            GeoId::State(
                state
                    .parse::<StateFips>()
                    .map_err(|e| parse_error(e.to_string()))?,
            )
        } else {
            // Explicit summary-level filtering prevents state totals becoming county rows.
            if state != "48" || sumlev != "050" {
                continue;
            }
            let county = byte_value(&r, county_i.expect("county header"))?;
            GeoId::County(
                format!("{state}{county}")
                    .parse::<CountyFips>()
                    .map_err(|e| parse_error(e.to_string()))?,
            )
        };
        let count = byte_value(&r, pop_i)?.parse::<u64>().map_err(|_| {
            parse_error(format!(
                "missing or invalid POPESTIMATE2025 for {geo}; no population published"
            ))
        })?;
        let row = Population {
            geography: geo,
            year: YEAR,
            count,
            provenance: Provenances::one(retrieval.provenance()),
        };
        if rows.insert(geo, row).is_some() {
            return Err(parse_error(format!(
                "duplicate Census population geography {geo}"
            )));
        }
    }
    ensure_complete(rows.keys().copied(), states)?;
    Ok(rows.into_values().collect())
}

pub fn parse_geographies(
    retrieval: &Retrieval,
    bytes: &[u8],
    states: bool,
) -> Result<Vec<Geography>> {
    verify(
        retrieval,
        bytes,
        if states {
            "census-states"
        } else {
            "census-texas-counties"
        },
    )?;
    geographies(retrieval, bytes, states)
}
fn geographies(retrieval: &Retrieval, bytes: &[u8], states: bool) -> Result<Vec<Geography>> {
    let mut archive = ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| parse_error(format!("Gazetteer ZIP: {e}")))?;
    let expected = if states {
        "2025_Gaz_state_national.txt"
    } else {
        "2025_Gaz_counties_national.txt"
    };
    if archive
        .file_names()
        .filter(|name| *name == expected)
        .count()
        != 1
    {
        return Err(parse_error(format!(
            "Gazetteer ZIP requires exactly one {expected}"
        )));
    }
    // Select the exact published filename, never a similarly named metadata/directory entry.
    let file = archive
        .by_name(expected)
        .map_err(|e| parse_error(format!("Gazetteer ZIP member: {e}")))?;
    if file.size() > MAX_MEMBER_BYTES {
        return Err(parse_error("Gazetteer member exceeds size limit"));
    }
    let mut raw = Vec::new();
    file.take(MAX_MEMBER_BYTES + 1)
        .read_to_end(&mut raw)
        .map_err(|e| parse_error(format!("Gazetteer read: {e}")))?;
    if raw.len() as u64 > MAX_MEMBER_BYTES {
        return Err(parse_error("Gazetteer member exceeds size limit"));
    }
    let mut rdr = ReaderBuilder::new()
        .delimiter(b'|')
        .flexible(false)
        .from_reader(raw.as_slice());
    let headers = rdr.headers().map_err(csv_error)?.clone();
    let geoid_i = column(&headers, "GEOID")?;
    let name_i = column(&headers, "NAME")?;
    let lat_i = column(&headers, "INTPTLAT")?;
    let lon_i = column(&headers, "INTPTLONG")?;
    let mut rows = BTreeMap::new();
    for record in rdr.records() {
        let r = record.map_err(csv_error)?;
        let geoid = value(&r, geoid_i)?;
        let geo = if states {
            GeoId::State(
                geoid
                    .parse::<StateFips>()
                    .map_err(|e| parse_error(e.to_string()))?,
            )
        } else {
            if !geoid.starts_with("48") {
                continue;
            }
            GeoId::County(
                geoid
                    .parse::<CountyFips>()
                    .map_err(|e| parse_error(e.to_string()))?,
            )
        };
        let latitude = r.get(lat_i).unwrap_or_default().trim();
        let longitude = r.get(lon_i).unwrap_or_default().trim();
        // Geography v1 represents missing coordinates as null. An incomplete/nonfinite/
        // out-of-range pair stays missing, never (0,0), averaged, or a county-name guess.
        let centroid = latitude
            .parse::<f64>()
            .ok()
            .zip(longitude.parse::<f64>().ok())
            .and_then(|(lat, lon)| Centroid::new(lat, lon).ok());
        let mut row = Geography::new(
            geo,
            value(&r, name_i)?,
            Provenances::one(retrieval.provenance()),
        )
        .map_err(parse_error)?;
        row.centroid = centroid;
        if rows.insert(geo, row).is_some() {
            return Err(parse_error(format!("duplicate Gazetteer geography {geo}")));
        }
    }
    ensure_complete(rows.keys().copied(), states)?;
    Ok(rows.into_values().collect())
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
    let bytes = store.get_verified(&retrieval.sha256)?;
    let rows = parse_geographies(&retrieval, &bytes, states)?;
    Ok((retrieval, rows))
}

fn value<'a>(row: &'a StringRecord, i: usize) -> Result<&'a str> {
    row.get(i)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| IngestError::Invalid("Census row has an empty required value".into()))
}
fn csv_error(error: csv::Error) -> IngestError {
    IngestError::Invalid(format!("invalid Census delimited data: {error}"))
}

#[cfg(test)]
mod tests {
    // Deliberate in-memory mutations test parser failures and missing semantics. These
    // bytes are never fixtures, snapshots or published outputs. Public parsers verify pins.
    use super::*;
    use std::io::Write;
    fn state_fixture() -> (Vec<u8>, Retrieval) {
        let b = include_bytes!("../../../data/fixtures/census-population/NST-EST2025-ALLDATA.csv")
            .to_vec();
        let r = serde_json::from_str(include_str!(
            "../../../data/fixtures/census-population/NST-EST2025-ALLDATA.csv.retrieval.json"
        ))
        .unwrap();
        (b, r)
    }
    fn state_csv(change: impl FnOnce(&StringRecord, &mut Vec<StringRecord>)) -> Vec<u8> {
        let (b, _) = state_fixture();
        let mut reader = ReaderBuilder::new().from_reader(b.as_slice());
        let header = reader.headers().unwrap().clone();
        let mut rows = reader
            .records()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        change(&header, &mut rows);
        let mut writer = csv::Writer::from_writer(Vec::new());
        writer.write_record(&header).unwrap();
        for row in rows {
            writer.write_record(&row).unwrap();
        }
        writer.into_inner().unwrap()
    }
    fn replace(row: &mut StringRecord, index: usize, value: &str) {
        *row = row
            .iter()
            .enumerate()
            .map(|(i, v)| if i == index { value } else { v })
            .collect();
    }
    #[test]
    fn missing_invalid_and_zero_population_are_distinct() {
        let (_, r) = state_fixture();
        for value in ["", "NA", "-1", "1.5", "18446744073709551616", "0"] {
            let b = state_csv(|header, rows| {
                let si = column(header, "STATE").unwrap();
                let pi = column(header, "POPESTIMATE2025").unwrap();
                for row in rows {
                    if row.get(si) == Some("48") {
                        replace(row, pi, value);
                    }
                }
            });
            let result = populations(&r, &b, true);
            if value == "0" {
                assert_eq!(
                    result
                        .unwrap()
                        .iter()
                        .find(|r| r.geography.to_string() == "48")
                        .unwrap()
                        .count,
                    0
                );
            } else {
                let err = result.unwrap_err().to_string();
                assert!(
                    err.contains("48") && err.contains("no population published"),
                    "{err}"
                );
            }
        }
    }
    #[test]
    fn duplicate_and_absent_population_keys_refuse_incomplete_exports() {
        let (_, r) = state_fixture();
        for duplicate in [true, false] {
            let b = state_csv(|header, rows| {
                let si = column(header, "STATE").unwrap();
                if duplicate {
                    let row = rows
                        .iter()
                        .find(|r| r.get(si) == Some("48"))
                        .unwrap()
                        .clone();
                    rows.push(row);
                } else {
                    rows.retain(|r| r.get(si) != Some("48"));
                }
            });
            let err = populations(&r, &b, true).unwrap_err().to_string();
            assert!(err.contains("48"), "{err}");
        }
        assert!(column(&StringRecord::from(vec!["STATE", "STATE"]), "STATE").is_err());
    }
    fn county_member() -> (Vec<u8>, Retrieval) {
        let b = include_bytes!(
            "../../../data/fixtures/census-population/2025_Gaz_counties_national.zip"
        );
        let mut z = ZipArchive::new(Cursor::new(b)).unwrap();
        let mut raw = Vec::new();
        z.by_name("2025_Gaz_counties_national.txt")
            .unwrap()
            .read_to_end(&mut raw)
            .unwrap();
        let r = serde_json::from_str(include_str!(
            "../../../data/fixtures/census-population/2025_Gaz_counties_national.zip.retrieval.json"
        ))
        .unwrap();
        (raw, r)
    }
    fn zip_members(members: &[(&str, &[u8])]) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, bytes) in members {
            writer
                .start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }
    #[test]
    fn zip_requires_exact_member_and_ignores_decoy_names() {
        let (raw, r) = county_member();
        let decoy = "metadata_2025_Gaz_counties_national.txt";
        let bytes = zip_members(&[
            (decoy, b"metadata,not,data"),
            ("2025_Gaz_counties_national.txt", &raw),
        ]);
        assert_eq!(geographies(&r, &bytes, false).unwrap().len(), 254);
        let wrong = zip_members(&[(decoy, &raw)]);
        assert!(geographies(&r, &wrong, false).is_err());
    }
    #[test]
    fn missing_or_ambiguous_internal_points_stay_null_and_zero_is_a_point() {
        let (raw, r) = county_member();
        for (lat, lon) in [
            ("", "-102.631561"),
            ("NaN", "-102.631561"),
            ("91", "-102.631561"),
            ("32.743942", "-181"),
            ("0", "0"),
        ] {
            let mut reader = ReaderBuilder::new()
                .delimiter(b'|')
                .from_reader(raw.as_slice());
            let h = reader.headers().unwrap().clone();
            let mut rows = reader
                .records()
                .collect::<std::result::Result<Vec<_>, _>>()
                .unwrap();
            let gi = column(&h, "GEOID").unwrap();
            let li = column(&h, "INTPTLAT").unwrap();
            let oi = column(&h, "INTPTLONG").unwrap();
            for row in &mut rows {
                if row.get(gi) == Some("48165") {
                    replace(row, li, lat);
                    replace(row, oi, lon);
                }
            }
            let mut writer = csv::WriterBuilder::new()
                .delimiter(b'|')
                .from_writer(Vec::new());
            writer.write_record(&h).unwrap();
            for row in rows {
                writer.write_record(&row).unwrap();
            }
            let changed = writer.into_inner().unwrap();
            let bytes = zip_members(&[("2025_Gaz_counties_national.txt", &changed)]);
            let rows = geographies(&r, &bytes, false).unwrap();
            let g = rows.iter().find(|r| r.id.to_string() == "48165").unwrap();
            let expected = if lat == "0" {
                Some(Centroid {
                    latitude: 0.0,
                    longitude: 0.0,
                })
            } else {
                None
            };
            assert_eq!(g.centroid, expected);
        }
    }
}
