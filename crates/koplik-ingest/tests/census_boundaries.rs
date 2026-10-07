//! Offline checks against the actual Census robots response. Real boundary ZIP conversion
//! acceptance tests remain blocked by the captured source policy (Issue #1373).
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
