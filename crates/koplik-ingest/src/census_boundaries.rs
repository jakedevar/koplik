//! Census 2024 cartographic boundaries at 1:20m, for overview maps, not analysis.
//! Only `fetch` uses the existing polite transport/store. Conversion and writing are offline.

use std::collections::BTreeMap;
use std::io::{Cursor, Read};
use std::path::Path;

use geo::{
    Area, Contains, Coord, LineString, MapCoords, MultiPolygon, Polygon, SimplifyVwPreserve,
    Validation,
};
use koplik_contracts::v1::{CountyFips, StateFips};
use serde_json::{Value, json};
use shapefile::dbase::{FieldValue, Record};
use shapefile::{PolygonRing, ShapeReader};

use crate::error::{IngestError, Result};
use crate::http::HttpClient;
use crate::polite::{PoliteFetcher, Timekeeper};
use crate::source::{SourceSpec, fetch_to_store};
use crate::store::{Retrieval, SnapshotStore, sha256_of};

pub const VINTAGE: u16 = 2024;
pub const LICENCE_ID: &str = "us-census-public-domain";
/// Visvalingam-Whyatt triangle-area tolerance, in square degrees. Display policy, not an
/// epidemiological parameter. Fixed for reproducible artifacts; see SOURCES.md.
pub const SIMPLIFY_AREA: f64 = 0.000001;
/// Four decimal places (~11 m latitude); well below the 1:20m source's display scale.
pub const COORDINATE_SCALE: f64 = 10_000.0;
pub const MAX_GEOJSON_BYTES: usize = 1_000_000;
const MAX_MEMBER_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryKind {
    States,
    TexasCounties,
}

impl BoundaryKind {
    pub fn source_id(self) -> &'static str {
        match self {
            Self::States => "census-cb-2024-states-20m",
            Self::TexasCounties => "census-cb-2024-counties-20m",
        }
    }
    fn stem(self) -> &'static str {
        match self {
            Self::States => "cb_2024_us_state_20m",
            Self::TexasCounties => "cb_2024_us_county_20m",
        }
    }
    pub fn file_name(self) -> &'static str {
        match self {
            Self::States => "states.geojson",
            Self::TexasCounties => "tx-counties.geojson",
        }
    }
    pub fn source_spec(self) -> SourceSpec {
        SourceSpec {
            source_id: self.source_id().into(),
            url: format!(
                "https://www2.census.gov/geo/tiger/GENZ2024/shp/{}.zip",
                self.stem()
            ),
            licence_id: LICENCE_ID.into(),
        }
    }
}

/// Fetch both national ZIPs through the same polite fetcher and immutable snapshot store.
/// The county snapshot stays national; filtering happens only in derived GeoJSON.
pub fn fetch<C: HttpClient, T: Timekeeper>(
    fetcher: &mut PoliteFetcher<C, T>,
    store: &SnapshotStore,
) -> Result<Vec<Retrieval>> {
    let mut retrieved = Vec::new();
    for kind in [BoundaryKind::States, BoundaryKind::TexasCounties] {
        retrieved.push(fetch_to_store(fetcher, store, &kind.source_spec())?.0);
    }
    Ok(retrieved)
}

fn parse_error(e: impl std::fmt::Display) -> IngestError {
    IngestError::Parse(format!("Census boundaries: {e}"))
}

fn member(zip: &mut zip::ZipArchive<Cursor<&[u8]>>, name: &str) -> Result<Vec<u8>> {
    // Never extract archive paths to disk. Bound decompression even if the size is false.
    let file = zip.by_name(name).map_err(parse_error)?;
    if file.size() > MAX_MEMBER_BYTES {
        return Err(parse_error(format!("oversized ZIP member {name}")));
    }
    let mut bytes = Vec::new();
    file.take(MAX_MEMBER_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(parse_error)?;
    if bytes.len() as u64 > MAX_MEMBER_BYTES {
        return Err(parse_error(format!("oversized ZIP member {name}")));
    }
    Ok(bytes)
}

fn field<'a>(record: &'a Record, name: &str) -> Result<&'a str> {
    match record.get(name) {
        Some(FieldValue::Character(Some(s))) if !s.trim().is_empty() => Ok(s.as_str()),
        _ => Err(parse_error(format!("missing/invalid DBF {name}"))),
    }
}

/// Associate holes by containment, not by DBF/ring order (ESRI does not guarantee order).
fn polygons(shape: shapefile::Polygon) -> Result<MultiPolygon<f64>> {
    let mut shells = Vec::new();
    let mut holes = Vec::new();
    for ring in shape.rings() {
        let line = LineString::new(
            ring.points()
                .iter()
                .map(|p| Coord { x: p.x, y: p.y })
                .collect(),
        );
        if line
            .0
            .iter()
            .any(|p| !p.x.is_finite() || !p.y.is_finite() || p.x.abs() > 180.0 || p.y.abs() > 90.0)
        {
            return Err(parse_error("coordinate outside longitude/latitude range"));
        }
        if line.0.len() < 4 || line.0.first() != line.0.last() {
            return Err(parse_error("unclosed or short ring"));
        }
        let polygon = Polygon::new(line, vec![]);
        if !polygon.is_valid() || polygon.unsigned_area() == 0.0 {
            return Err(parse_error("invalid source ring"));
        }
        match ring {
            PolygonRing::Outer(_) => shells.push(polygon),
            PolygonRing::Inner(_) => holes.push(polygon),
        }
    }
    if shells.is_empty() {
        return Err(parse_error("polygon without a shell"));
    }
    for hole in holes {
        let owner = shells
            .iter()
            .enumerate()
            .filter(|(_, shell)| Polygon::new(shell.exterior().clone(), vec![]).contains(&hole))
            .min_by(|(_, a), (_, b)| a.unsigned_area().total_cmp(&b.unsigned_area()))
            .map(|(i, _)| i)
            .ok_or_else(|| parse_error("orphaned hole"))?;
        shells[owner].interiors_push(hole.exterior().clone());
    }
    let geometry = MultiPolygon(shells);
    if !geometry.is_valid() {
        return Err(parse_error("invalid source multipolygon"));
    }
    Ok(geometry)
}

fn rounded(geometry: &MultiPolygon<f64>) -> MultiPolygon<f64> {
    geometry.map_coords(|p| Coord {
        x: (p.x * COORDINATE_SCALE).round() / COORDINATE_SCALE,
        y: (p.y * COORDINATE_SCALE).round() / COORDINATE_SCALE,
    })
}

fn geometry_json(source: MultiPolygon<f64>) -> Result<Value> {
    // VW may move holes outside shells. Validate after simplification AND rounding; if it
    // changes topology, retain the rounded source rather than silently lose an island/hole.
    let candidate = rounded(&source.simplify_vw_preserve(SIMPLIFY_AREA));
    let geometry = if candidate.is_valid() && candidate.unsigned_area() > 0.0 {
        candidate
    } else {
        let original = rounded(&source);
        if !original.is_valid() || original.unsigned_area() == 0.0 {
            return Err(parse_error("coordinate rounding invalidates geometry"));
        }
        original
    };
    let ring_coords = |ring: &LineString<f64>, exterior: bool| {
        let mut points: Vec<[f64; 2]> = ring.0.iter().map(|p| [p.x, p.y]).collect();
        // RFC 7946: exterior counterclockwise, holes clockwise. ESRI is the reverse.
        let area: f64 = points
            .windows(2)
            .map(|p| p[0][0] * p[1][1] - p[1][0] * p[0][1])
            .sum();
        if (area > 0.0) != exterior {
            points.reverse();
        }
        points
    };
    let coordinates: Vec<Vec<Vec<[f64; 2]>>> = geometry
        .0
        .iter()
        .map(|p| {
            std::iter::once(ring_coords(p.exterior(), true))
                .chain(p.interiors().iter().map(|r| ring_coords(r, false)))
                .collect()
        })
        .collect();
    Ok(json!({"type": "MultiPolygon", "coordinates": coordinates}))
}

/// Convert an authenticated raw Census ZIP into compact standard GeoJSON. Every feature
/// carries contracts v1 provenance. Bytes must match the retrieval digest, including in
/// direct callers; use `write_latest` for verified store reads. Features/id use string FIPS.
pub fn convert(bytes: &[u8], retrieval: &Retrieval, kind: BoundaryKind) -> Result<Vec<u8>> {
    if sha256_of(bytes) != retrieval.sha256 {
        return Err(IngestError::BlobCorrupt {
            sha256: retrieval.sha256.to_string(),
            actual: sha256_of(bytes).to_string(),
        });
    }
    let spec = kind.source_spec();
    if retrieval.source_id != spec.source_id
        || retrieval.url != spec.url
        || retrieval.licence_id != spec.licence_id
    {
        return Err(parse_error(
            "retrieval does not identify the expected source",
        ));
    }
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(parse_error)?;
    let stem = kind.stem();
    let prj = String::from_utf8(member(&mut zip, &format!("{stem}.prj"))?).map_err(parse_error)?;
    if !prj.starts_with("GEOGCS[")
        || !prj.contains("DATUM[\"D_North_American_1983\"")
        || !prj.contains("UNIT[\"Degree\"")
    {
        return Err(parse_error("expected unprojected NAD83 degrees"));
    }
    let shp = member(&mut zip, &format!("{stem}.shp"))?;
    let dbf = member(&mut zip, &format!("{stem}.dbf"))?;
    let shapes = ShapeReader::new(Cursor::new(shp))
        .map_err(parse_error)?
        .read_as::<shapefile::Polygon>()
        .map_err(parse_error)?;
    let records = shapefile::dbase::Reader::new(Cursor::new(dbf))
        .map_err(parse_error)?
        .read()
        .map_err(parse_error)?;
    if shapes.len() != records.len() {
        return Err(parse_error("SHP/DBF record counts disagree"));
    }
    let mut features = BTreeMap::new();
    for (shape, record) in shapes.into_iter().zip(records) {
        let geoid = field(&record, "GEOID")?;
        let state = field(&record, "STATEFP")?;
        state.parse::<StateFips>().map_err(parse_error)?;
        match kind {
            BoundaryKind::States => {
                geoid.parse::<StateFips>().map_err(parse_error)?;
                if geoid != state {
                    return Err(parse_error("GEOID/STATEFP disagree"));
                }
            }
            BoundaryKind::TexasCounties => {
                geoid.parse::<CountyFips>().map_err(parse_error)?;
                if !geoid.starts_with(state) || &geoid[2..] != field(&record, "COUNTYFP")? {
                    return Err(parse_error("GEOID/STATEFP/COUNTYFP disagree"));
                }
                if state != "48" {
                    continue;
                }
            }
        }
        let name = field(&record, "NAME")?;
        let geometry = geometry_json(polygons(shape)?)
            .map_err(|e| parse_error(format!("GEOID {geoid}: {e}")))?;
        let feature = json!({"type": "Feature", "id": geoid, "properties": {"GEOID": geoid, "NAME": name, "provenance": [retrieval.provenance()]}, "geometry": geometry});
        if features.insert(geoid.to_owned(), feature).is_some() {
            return Err(parse_error(format!("duplicate GEOID {geoid}")));
        }
    }
    if features.is_empty() {
        return Err(parse_error("no matching features"));
    }
    let output = serde_json::to_vec(&json!({"type": "FeatureCollection", "features": features.into_values().collect::<Vec<_>>()})).map_err(parse_error)?;
    if output.len() > MAX_GEOJSON_BYTES {
        return Err(parse_error(format!(
            "GeoJSON exceeds {MAX_GEOJSON_BYTES} bytes: {}",
            output.len()
        )));
    }
    Ok(output)
}

/// Write `states.geojson` and `tx-counties.geojson` into the supplied directory (pipeline:
/// `web/public/data/geo`). Converts both before writing so a parse error publishes neither.
/// The caller selects/commits the snapshot store for reproducible historical builds.
pub fn write_latest(store: &SnapshotStore, output_dir: impl AsRef<Path>) -> Result<Vec<Retrieval>> {
    let mut outputs = Vec::new();
    for kind in [BoundaryKind::States, BoundaryKind::TexasCounties] {
        let retrieval = store
            .latest(kind.source_id())?
            .ok_or_else(|| IngestError::NoSnapshot(kind.source_id().into()))?;
        let output = convert(&store.get_verified(&retrieval.sha256)?, &retrieval, kind)?;
        outputs.push((kind, retrieval, output));
    }
    let dir = output_dir.as_ref();
    std::fs::create_dir_all(dir).map_err(|e| IngestError::io(dir, e))?;
    let mut retrieved = Vec::new();
    for (kind, retrieval, output) in outputs {
        let path = dir.join(kind.file_name());
        std::fs::write(&path, output).map_err(|e| IngestError::io(path, e))?;
        retrieved.push(retrieval);
    }
    Ok(retrieved)
}
