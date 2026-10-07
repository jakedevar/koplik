//! Offline checks against unchanged Census boundary ZIPs, their retrieval records, and
//! the historical robots diagnostic. No test requests the network.
use koplik_ingest::IngestError;
use koplik_ingest::census_boundaries::{self, BoundaryKind};
use koplik_ingest::store::{Retrieval, SnapshotStore, sha256_of};

const ROBOTS: &[u8] = include_bytes!("../../../data/fixtures/census/robots.txt");
const ROBOTS_RECORD: &str = include_str!("../../../data/fixtures/census/robots.retrieval.json");

#[test]
fn diagnostic_fixture_is_the_exact_recorded_response() {
    let retrieval: Retrieval = serde_json::from_str(ROBOTS_RECORD).unwrap();
    assert_eq!(sha256_of(ROBOTS), retrieval.sha256);
    assert_eq!(ROBOTS.len() as u64, retrieval.bytes);
    assert_eq!(retrieval.url, "https://www2.census.gov/robots.txt");
    assert_eq!(
        retrieval.retrieved_at.to_rfc3339(),
        "2026-10-07T02:30:41+00:00"
    );
    assert_eq!(retrieval.http_status, 200);
}

#[test]
fn rejects_corruption_and_wrong_source_in_direct_conversion() {
    let mut retrieval: Retrieval = serde_json::from_str(ROBOTS_RECORD).unwrap();
    retrieval.sha256 = sha256_of(b"different source bytes");
    assert!(matches!(
        census_boundaries::convert(ROBOTS, &retrieval, BoundaryKind::States),
        Err(IngestError::BlobCorrupt { .. })
    ));
    retrieval.sha256 = sha256_of(ROBOTS);
    let error = census_boundaries::convert(ROBOTS, &retrieval, BoundaryKind::States).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("retrieval does not identify the expected source")
    );
}

#[test]
fn missing_snapshot_does_not_write_output() {
    let temp = tempfile::tempdir().unwrap();
    let store = SnapshotStore::open(temp.path().join("snapshots")).unwrap();
    let output = temp.path().join("geo");
    assert!(matches!(
        census_boundaries::write_latest(&store, &output),
        Err(IngestError::NoSnapshot(_))
    ));
    assert!(!output.exists());
}

fn boundary_fixtures() -> [(
    BoundaryKind,
    &'static [u8],
    Retrieval,
    usize,
    usize,
    &'static str,
); 2] {
    [
        (
            BoundaryKind::States,
            include_bytes!("../../../data/fixtures/census/cb_2024_us_state_20m.zip"),
            serde_json::from_str(include_str!(
                "../../../data/fixtures/census/cb_2024_us_state_20m.retrieval.json"
            ))
            .unwrap(),
            52,
            233074,
            "a0349eb44abd31691527f8f9bf287775c477070ccf430d2cebde1663c6f4e820",
        ),
        (
            BoundaryKind::TexasCounties,
            include_bytes!("../../../data/fixtures/census/cb_2024_us_county_20m.zip"),
            serde_json::from_str(include_str!(
                "../../../data/fixtures/census/cb_2024_us_county_20m.retrieval.json"
            ))
            .unwrap(),
            254,
            169613,
            "1351d270dfe641ea2ee41cd31bc139d303f308aed174dcae500267b9cdb920bd",
        ),
    ]
}

#[test]
fn full_real_zip_fixtures_match_pins_and_retrievals() {
    let manifest = census_boundaries::manifest().unwrap();
    for (kind, bytes, record, _, _, _) in boundary_fixtures() {
        let spec = kind.source_spec();
        assert_eq!(sha256_of(bytes), record.sha256);
        assert_eq!(manifest.pin(&spec.url), Some(&record.sha256));
        assert_eq!(record.bytes, bytes.len() as u64);
        assert_eq!(record.url, spec.url);
        assert_eq!(record.source_id, spec.source_id);
        assert_eq!(record.licence_id, spec.licence_id);
        assert_eq!(record.content_type.as_deref(), Some("application/zip"));
    }
}

#[test]
fn real_conversion_is_byte_identical_small_sorted_and_has_source_names_and_provenance() {
    use serde_json::{Value, json};
    for (kind, bytes, record, count, size, digest) in boundary_fixtures() {
        let output = census_boundaries::convert(bytes, &record, kind).unwrap();
        assert_eq!(
            output,
            census_boundaries::convert(bytes, &record, kind).unwrap()
        );
        assert_eq!(output.len(), size);
        assert!(output.len() <= census_boundaries::MAX_GEOJSON_BYTES);
        assert_eq!(sha256_of(&output).as_str(), digest);
        let collection: Value = serde_json::from_slice(&output).unwrap();
        assert_eq!(collection["type"], "FeatureCollection");
        let features = collection["features"].as_array().unwrap();
        assert_eq!(features.len(), count);
        let ids: Vec<&str> = features
            .iter()
            .map(|f| f["properties"]["GEOID"].as_str().unwrap())
            .collect();
        assert!(ids.windows(2).all(|pair| pair[0] < pair[1]));
        for feature in features {
            assert_eq!(feature["type"], "Feature");
            assert_eq!(feature["id"], feature["properties"]["GEOID"]);
            assert!(
                !feature["properties"]["NAME"]
                    .as_str()
                    .unwrap()
                    .trim()
                    .is_empty()
            );
            assert_eq!(
                feature["properties"]["provenance"],
                json!([record.provenance()])
            );
            assert_eq!(feature["geometry"]["type"], "MultiPolygon");
            let id = feature["id"].as_str().unwrap();
            match kind {
                BoundaryKind::States => assert_eq!(id.len(), 2),
                BoundaryKind::TexasCounties => {
                    assert_eq!(id.len(), 5);
                    assert!(id.starts_with("48"));
                }
            }
            for polygon in feature["geometry"]["coordinates"].as_array().unwrap() {
                for (index, ring) in polygon.as_array().unwrap().iter().enumerate() {
                    let points = ring.as_array().unwrap();
                    assert!(points.len() >= 4);
                    assert_eq!(points.first(), points.last());
                    let signed: f64 = points
                        .windows(2)
                        .map(|p| {
                            p[0][0].as_f64().unwrap() * p[1][1].as_f64().unwrap()
                                - p[1][0].as_f64().unwrap() * p[0][1].as_f64().unwrap()
                        })
                        .sum();
                    assert_eq!(signed > 0.0, index == 0);
                    for point in points {
                        assert_eq!(point.as_array().unwrap().len(), 2);
                        assert!(point[0].as_f64().unwrap().abs() <= 180.0);
                        assert!(point[1].as_f64().unwrap().abs() <= 90.0);
                        for coordinate in point.as_array().unwrap() {
                            let scaled =
                                coordinate.as_f64().unwrap() * census_boundaries::COORDINATE_SCALE;
                            assert!((scaled - scaled.round()).abs() < 1e-7);
                        }
                    }
                }
            }
        }
        let named = |id| features.iter().find(|f| f["id"] == id).unwrap();
        match kind {
            BoundaryKind::States => {
                assert_eq!(named("48")["properties"]["NAME"], "Texas");
                assert_eq!(named("02")["properties"]["NAME"], "Alaska");
                assert!(
                    named("02")["geometry"]["coordinates"]
                        .as_array()
                        .unwrap()
                        .len()
                        > 1
                );
                assert_eq!(named("15")["properties"]["NAME"], "Hawaii");
                assert!(
                    named("15")["geometry"]["coordinates"]
                        .as_array()
                        .unwrap()
                        .len()
                        > 1
                );
                assert_eq!(named("72")["properties"]["NAME"], "Puerto Rico");
            }
            BoundaryKind::TexasCounties => {
                assert_eq!(named("48165")["properties"]["NAME"], "Gaines");
                assert_eq!(named("48109")["properties"]["NAME"], "Culberson");
                assert_eq!(named("48279")["properties"]["NAME"], "Lamb");
            }
        }
    }
}

#[test]
fn pipeline_writer_uses_verified_store_and_preserves_bytes_across_cached_builds() {
    use koplik_ingest::store::RetrievalMeta;
    let temp = tempfile::tempdir().unwrap();
    let store = SnapshotStore::open(temp.path().join("snapshots")).unwrap();
    for (_, bytes, record, _, _, _) in boundary_fixtures() {
        let (stored, _) = store
            .record(
                RetrievalMeta {
                    source_id: record.source_id.clone(),
                    url: record.url.clone(),
                    retrieved_at: record.retrieved_at,
                    licence_id: record.licence_id.clone(),
                    http_status: record.http_status,
                    content_type: record.content_type.clone(),
                },
                bytes,
            )
            .unwrap();
        assert_eq!(stored, record);
    }
    let first = temp.path().join("first");
    let second = temp.path().join("second");
    census_boundaries::write_latest(&store, &first).unwrap();
    census_boundaries::write_latest(&store, &second).unwrap();
    for (kind, _, _, _, size, digest) in boundary_fixtures() {
        let output = std::fs::read(first.join(kind.file_name())).unwrap();
        assert_eq!(
            output,
            std::fs::read(second.join(kind.file_name())).unwrap()
        );
        assert_eq!(output.len(), size);
        assert_eq!(sha256_of(&output).as_str(), digest);
    }
}
