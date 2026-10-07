//! All data tests read the complete unmodified Census snapshots. No test uses the network.
use koplik_contracts::v1::{Centroid, Geography, Population};
use koplik_ingest::{
    census_population as census,
    polite::{
        PoliteConfig, PoliteFetcher,
        fakes::{FakeClient, FakeTime},
    },
    store::{PutOutcome, Retrieval, RetrievalMeta, SnapshotStore, sha256_of},
};
use std::{collections::BTreeSet, path::PathBuf, process::Command};
fn fixture(source: &str) -> (Vec<u8>, Retrieval) {
    let spec = census::source_spec(source).unwrap();
    let name = spec.url.rsplit('/').next().unwrap();
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/fixtures/census-population");
    (
        std::fs::read(root.join(name)).unwrap(),
        serde_json::from_slice(
            &std::fs::read(root.join(format!("{name}.retrieval.json"))).unwrap(),
        )
        .unwrap(),
    )
}
fn record(store: &SnapshotStore, source: &str) -> Retrieval {
    let (bytes, r) = fixture(source);
    let meta = RetrievalMeta {
        source_id: r.source_id.clone(),
        url: r.url.clone(),
        retrieved_at: r.retrieved_at,
        licence_id: r.licence_id.clone(),
        http_status: r.http_status,
        content_type: r.content_type.clone(),
    };
    store.record(meta, &bytes).unwrap().0
}
#[test]
fn complete_source_bytes_match_the_committed_pins_and_receipts() {
    let manifest = census::manifest().unwrap();
    for source in census::SOURCES {
        let (b, r) = fixture(source);
        let spec = census::source_spec(source).unwrap();
        assert_eq!(sha256_of(&b), r.sha256);
        assert_eq!(r.bytes, b.len() as u64);
        assert_eq!(manifest.pin(&r.url), Some(&r.sha256));
        assert_eq!(r.source_id, spec.source_id);
        assert_eq!(r.url, spec.url);
        assert_eq!(r.licence_id, spec.licence_id);
        assert_eq!(r.http_status, 200);
    }
}
#[test]
fn populations_cover_every_state_and_texas_county_with_measured_2025_values() {
    let mut tx_counties = Vec::new();
    for (states, source, len) in [
        (true, "census-state-population", 52),
        (false, "census-county-population", 254),
    ] {
        let (b, r) = fixture(source);
        let rows = census::parse_populations(&r, &b, states).unwrap();
        assert_eq!(rows.len(), len);
        assert_eq!(
            rows.iter()
                .map(|r| r.geography)
                .collect::<BTreeSet<_>>()
                .len(),
            len
        );
        assert!(rows.windows(2).all(|w| w[0].geography < w[1].geography));
        for row in &rows {
            assert_eq!(row.year, 2025);
            assert_eq!(row.provenance.as_slice(), &[r.provenance()]);
            let decoded: Population =
                serde_json::from_value(serde_json::to_value(row).unwrap()).unwrap();
            assert_eq!(&decoded, row);
        }
        if states {
            assert_eq!(
                rows.iter()
                    .find(|r| r.geography.to_string() == "48")
                    .unwrap()
                    .count,
                31_709_821
            );
            assert!(rows.iter().any(|r| r.geography.to_string() == "11"));
            assert!(rows.iter().any(|r| r.geography.to_string() == "72"));
        } else {
            assert_eq!(
                rows.iter()
                    .find(|r| r.geography.to_string() == "48165")
                    .unwrap()
                    .count,
                23_956
            );
            assert!(rows.iter().all(|r| r.geography.state().code() == 48));
            tx_counties = rows;
        }
    }
    let (b, r) = fixture("census-texas-counties");
    let geos = census::parse_geographies(&r, &b, false).unwrap();
    assert_eq!(
        tx_counties.iter().map(|r| r.geography).collect::<Vec<_>>(),
        geos.iter().map(|r| r.id).collect::<Vec<_>>()
    );
}
#[test]
fn gazetteer_internal_points_keep_geography_and_scenario_shape_and_provenance() {
    for (states, source, len) in [
        (true, "census-states", 52),
        (false, "census-texas-counties", 254),
    ] {
        let (b, r) = fixture(source);
        let rows = census::parse_geographies(&r, &b, states).unwrap();
        assert_eq!(rows.len(), len);
        assert!(rows.windows(2).all(|w| w[0].id < w[1].id));
        for row in &rows {
            assert_eq!(row.provenance.as_slice(), &[r.provenance()]);
            assert_eq!(row.level, row.id.level());
            assert!(!row.name.trim().is_empty());
            let point: koplik_contracts::v3::Centroid =
                row.centroid.expect("all published points present");
            assert!(point.latitude.is_finite() && point.longitude.is_finite());
            let decoded: Geography =
                serde_json::from_value(serde_json::to_value(row).unwrap()).unwrap();
            assert_eq!(&decoded, row);
        }
        if !states {
            let gaines = rows.iter().find(|r| r.id.to_string() == "48165").unwrap();
            assert_eq!(gaines.name, "Gaines County");
            assert_eq!(
                gaines.centroid,
                Some(Centroid {
                    latitude: 32.743942,
                    longitude: -102.631561
                })
            );
        }
    }
}
#[test]
fn source_pin_and_byte_mismatches_cannot_publish_rows() {
    let (mut b, mut r) = fixture("census-state-population");
    b.push(b' ');
    assert!(census::parse_populations(&r, &b, true).is_err());
    r.sha256 = sha256_of(&b);
    r.bytes = b.len() as u64;
    assert!(census::parse_populations(&r, &b, true).is_err()); // hash matches bytes but not manifest.
    let (b, r) = fixture("census-state-population");
    assert!(census::parse_populations(&r, &b, false).is_err());
    let (b, r) = fixture("census-states");
    assert!(census::parse_geographies(&r, &b, false).is_err());
}
#[test]
fn cached_named_files_and_offline_cli_need_no_network() {
    let tmp = tempfile::tempdir().unwrap();
    let store = SnapshotStore::open(tmp.path().join("store")).unwrap();
    let client = FakeClient::default();
    let mut fetcher = PoliteFetcher::new(
        client.clone(),
        FakeTime::default(),
        PoliteConfig::live(Some("https://github.com/jakedevar")).unwrap(),
    );
    for source in census::SOURCES {
        let r = record(&store, source);
        let (cached, outcome) = census::fetch_source(&mut fetcher, &store, source).unwrap();
        assert_eq!(cached, r);
        assert_eq!(outcome, PutOutcome::AlreadyPresent);
        let out = tmp.path().join(format!("{source}.json"));
        let result = Command::new(env!("CARGO_BIN_EXE_koplik-ingest"))
            .args(["parse", source, "--store"])
            .arg(store.root())
            .arg("--out")
            .arg(&out)
            .env_remove("KOPLIK_CONTACT")
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let output = std::fs::read(out).unwrap();
        let (b, _) = fixture(source);
        if source.ends_with("population") {
            let rows: Vec<Population> = serde_json::from_slice(&output).unwrap();
            assert_eq!(
                rows,
                census::parse_populations(&r, &b, source == "census-state-population").unwrap()
            );
        } else {
            let rows: Vec<Geography> = serde_json::from_slice(&output).unwrap();
            assert_eq!(
                rows,
                census::parse_geographies(&r, &b, source == "census-states").unwrap()
            );
        }
    }
    assert!(client.calls.borrow().is_empty());
}
#[test]
fn live_population_fetch_requires_contact_before_store_or_request() {
    for source in census::SOURCES.into_iter().chain(["census-population"]) {
        let tmp = tempfile::tempdir().unwrap();
        let store = tmp.path().join("store");
        let result = Command::new(env!("CARGO_BIN_EXE_koplik-ingest"))
            .args(["fetch", source, "--store"])
            .arg(&store)
            .env_remove("KOPLIK_CONTACT")
            .output()
            .unwrap();
        assert!(!result.status.success());
        let err = String::from_utf8_lossy(&result.stderr);
        assert!(err.contains("KOPLIK_CONTACT"), "{err}");
        assert!(!store.exists());
    }
}
