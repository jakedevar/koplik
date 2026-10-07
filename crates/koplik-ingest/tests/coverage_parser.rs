//! Offline checks against unmodified source responses. Mutated JSON cases below are
//! deliberately invalid/edge parser inputs, never published source data.
use koplik_contracts::v1::{CoverageValue, KindergartenMmrCoverage, MissingReason};
use koplik_ingest::{
    coverage,
    store::{Retrieval, RetrievalMeta, SnapshotStore, sha256_of},
};
use std::path::PathBuf;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/fixtures/coverage")
}
fn fixture(name: &str) -> (Vec<u8>, Retrieval) {
    (
        std::fs::read(dir().join(name)).unwrap(),
        serde_json::from_slice(
            &std::fs::read(dir().join(format!("{name}.retrieval.json"))).unwrap(),
        )
        .unwrap(),
    )
}
fn cdc() -> Vec<KindergartenMmrCoverage> {
    let (b, r) = fixture("cdc-2023-25.json");
    coverage::parse_cdc(&b, &r, 2023, 2024).unwrap()
}
fn texas(year: u16) -> Vec<KindergartenMmrCoverage> {
    let (b, r) = fixture(&format!("texas-{year}-{:02}.xlsx", (year + 1) % 100));
    let (cb, cr) = fixture("texas-county-fips.html");
    coverage::parse_texas(&b, &r, &cb, &cr, year).unwrap()
}
fn at(rows: &[KindergartenMmrCoverage], id: &str, year: u16) -> CoverageValue {
    rows.iter()
        .find(|r| r.geography.to_string() == id && r.school_year.start_year() == year)
        .unwrap()
        .coverage
}
fn record(store: &SnapshotStore, b: &[u8], r: &Retrieval) {
    store
        .record(
            RetrievalMeta {
                source_id: r.source_id.clone(),
                url: r.url.clone(),
                retrieved_at: r.retrieved_at,
                licence_id: r.licence_id.clone(),
                http_status: r.http_status,
                content_type: r.content_type.clone(),
            },
            b,
        )
        .unwrap();
}

#[test]
fn fixtures_are_exact_recorded_snapshots() {
    for name in [
        "cdc-2023-25.json",
        "texas-2023-24.xlsx",
        "texas-2024-25.xlsx",
        "texas-county-fips.html",
        "census-api-missing-key.html",
    ] {
        let (b, r) = fixture(name);
        assert_eq!(sha256_of(&b), r.sha256);
        assert_eq!(b.len() as u64, r.bytes);
    }
    let (_, r) = fixture("cdc-2023-25.json");
    assert_eq!(r.url, coverage::cdc_source_spec(2023, 2024).unwrap().url);
}
#[test]
fn cdc_measured_values_and_v1_provenance_for_every_state_year() {
    let rows = cdc();
    assert_eq!(rows.len(), 102); // 50 states + DC x 2 school years.
    assert_eq!(
        at(&rows, "48", 2023),
        CoverageValue::Reported {
            coverage_pct: 94.3,
            exemption_pct: Some(3.9)
        }
    );
    assert_eq!(
        at(&rows, "48", 2024),
        CoverageValue::Reported {
            coverage_pct: 93.2,
            exemption_pct: Some(4.3)
        }
    );
    // Statewide New York is directly published; do not add or average NYC/rest-of-state.
    assert_eq!(at(&rows, "36", 2023).coverage_pct(), Some(97.7));
    assert_eq!(at(&rows, "36", 2024).coverage_pct(), Some(97.8));
    assert_eq!(
        coverage::gaps(&rows)
            .iter()
            .map(|r| (r.geography.to_string(), r.school_year.to_string()))
            .collect::<Vec<_>>(),
        vec![
            ("30".into(), "2023-24".into()),
            ("30".into(), "2024-25".into()),
            ("54".into(), "2024-25".into())
        ]
    );
    let (_, r) = fixture("cdc-2023-25.json");
    for row in &rows {
        assert_eq!(row.provenance.as_slice(), &[r.provenance()]);
        assert!(!row.imputed);
        assert_eq!(row.imputation_method, None);
        let round: KindergartenMmrCoverage =
            serde_json::from_value(serde_json::to_value(row).unwrap()).unwrap();
        assert_eq!(&round, row);
    }
}
#[test]
fn texas_published_county_rates_and_explicit_nr_gaps() {
    let (_, crosswalk) = fixture("texas-county-fips.html");
    for (year, gaines, missing_ids) in [
        (
            2023,
            81.9672131147541,
            vec!["48103", "48301", "48385", "48433"],
        ),
        (2024, 77.25631768953069, vec!["48301"]),
    ] {
        let rows = texas(year);
        assert_eq!(rows.len(), 254);
        let reported = at(&rows, "48165", year);
        assert_eq!(
            reported,
            CoverageValue::Reported {
                coverage_pct: gaines,
                exemption_pct: None
            }
        );
        let (_, r) = fixture(&format!("texas-{year}-{:02}.xlsx", (year + 1) % 100));
        for row in &rows {
            assert_eq!(
                row.provenance.as_slice(),
                &[r.provenance(), crosswalk.provenance()]
            );
            assert!(!row.imputed);
            serde_json::from_value::<KindergartenMmrCoverage>(serde_json::to_value(row).unwrap())
                .unwrap();
        }
        let gaps = coverage::gaps(&rows);
        assert_eq!(
            gaps.iter()
                .map(|r| r.geography.to_string())
                .collect::<Vec<_>>(),
            missing_ids
        );
        assert!(gaps.iter().all(|r| r.coverage
            == CoverageValue::Missing {
                reason: MissingReason::NotReported
            }));
        // Same DSHS agency spells these differently in its two tables; case folding is exact.
        for id in ["48123", "48307", "48309", "48327"] {
            assert!(at(&rows, id, year).coverage_pct().is_some());
        }
    }
}
#[test]
fn retrieval_mismatch_and_wrong_grade_or_year_are_refused() {
    let (b, mut r) = fixture("cdc-2023-25.json");
    r.url.push_str("&changed=true");
    assert!(coverage::parse_cdc(&b, &r, 2023, 2024).is_err());
    let (b, r) = fixture("texas-2023-24.xlsx");
    let (cb, cr) = fixture("texas-county-fips.html");
    assert!(coverage::parse_texas(&b, &r, &cb, &cr, 2024).is_err());
    let mut wrong_year = r.clone();
    let spec = coverage::texas_source_spec(2024).unwrap();
    wrong_year.source_id = spec.source_id;
    wrong_year.url = spec.url;
    assert!(coverage::parse_texas(&b, &wrong_year, &cb, &cr, 2024).is_err());
    let mut tampered = b.clone();
    tampered.push(b' ');
    assert!(coverage::parse_texas(&tampered, &r, &cb, &cr, 2023).is_err());
    // An HTTP-200 HTML credential error is not an identity crosswalk.
    let (html, _) = fixture("census-api-missing-key.html");
    let mut fake = cr;
    fake.sha256 = sha256_of(&html);
    fake.bytes = html.len() as u64;
    assert!(coverage::texas_counties(&html, &fake).is_err());
}

fn changed_cdc(f: impl FnOnce(&mut Vec<serde_json::Value>)) -> (Vec<u8>, Retrieval) {
    let (b, mut r) = fixture("cdc-2023-25.json");
    let mut raw = serde_json::from_slice(&b).unwrap();
    f(&mut raw);
    let b = serde_json::to_vec(&raw).unwrap();
    r.sha256 = sha256_of(&b);
    r.bytes = b.len() as u64;
    (b, r)
}
#[test]
fn absent_suppressed_blank_bounds_and_zero_remain_distinct() {
    let (b, r) = changed_cdc(|raw| {
        raw.retain(|r| !(r["geography"] == "Alaska" && r["vaccine"] == "MMR"));
        for row in raw {
            if row["vaccine"] == "MMR" {
                match row["geography"].as_str().unwrap() {
                    "Texas" => row["coverage_estimate"] = "**".into(),
                    "Alabama" => row["coverage_estimate"] = "".into(),
                    "Arizona" => row["coverage_estimate"] = "0".into(),
                    "Arkansas" => row["coverage_estimate"] = "<0.1".into(),
                    _ => {}
                }
            }
            if row["geography"] == "Arizona" && row["vaccine"] == "Exemption" {
                row["coverage_estimate"] = "<0.1".into();
            }
        }
    });
    let rows = coverage::parse_cdc(&b, &r, 2023, 2024).unwrap();
    for year in [2023, 2024] {
        assert_eq!(
            at(&rows, "48", year),
            CoverageValue::Missing {
                reason: MissingReason::Suppressed
            }
        );
        assert_eq!(
            at(&rows, "01", year),
            CoverageValue::Missing {
                reason: MissingReason::NotReported
            }
        );
        assert_eq!(
            at(&rows, "02", year),
            CoverageValue::Missing {
                reason: MissingReason::NotReported
            }
        );
        assert_eq!(
            at(&rows, "05", year),
            CoverageValue::Missing {
                reason: MissingReason::Ambiguous
            }
        );
        assert_eq!(
            at(&rows, "04", year),
            CoverageValue::Reported {
                coverage_pct: 0.0,
                exemption_pct: None
            }
        );
    }
    assert_eq!(coverage::gaps(&rows).len(), 11);
    let (b, r) = changed_cdc(Vec::clear);
    let rows = coverage::parse_cdc(&b, &r, 2023, 2024).unwrap();
    assert_eq!(coverage::gaps(&rows).len(), 102);
}
#[test]
fn duplicate_unknown_and_truncated_cdc_inputs_are_errors() {
    for mutation in [0, 1, 2, 3] {
        let (b, r) = changed_cdc(|raw| match mutation {
            0 => raw.push(raw[0].clone()),
            1 => raw[0]["geography"] = "Unknown state".into(),
            2 => raw[0]["year_season"] = "2022-23".into(),
            _ => raw.resize(coverage::ROW_LIMIT, raw[0].clone()),
        });
        assert!(coverage::parse_cdc(&b, &r, 2023, 2024).is_err());
    }
}
#[test]
fn offline_cli_writes_v1_rows_and_complete_gaps() {
    let tmp = tempfile::tempdir().unwrap();
    let store = SnapshotStore::open(tmp.path().join("store")).unwrap();
    for name in [
        "cdc-2023-25.json",
        "texas-2023-24.xlsx",
        "texas-2024-25.xlsx",
        "texas-county-fips.html",
    ] {
        let (b, r) = fixture(name);
        record(&store, &b, &r);
    }
    assert_eq!(
        coverage::parse_latest_cdc(&store, 2023, 2024).unwrap(),
        cdc()
    );
    for year in [2023, 2024] {
        assert_eq!(
            coverage::parse_latest_texas(&store, year).unwrap(),
            texas(year)
        );
    }
    for (name, rows) in [
        ("cdc-2023-25-gaps.json", cdc()),
        ("texas-2023-24-gaps.json", texas(2023)),
        ("texas-2024-25-gaps.json", texas(2024)),
    ] {
        let path = dir().join("../../reports/coverage").join(name);
        let committed: Vec<KindergartenMmrCoverage> =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(
            committed,
            coverage::gaps(&rows)
                .into_iter()
                .cloned()
                .collect::<Vec<_>>()
        );
    }
    for (src, n, gap_count) in [("cdc-coverage", 102, 3), ("texas-coverage", 254, 4)] {
        let out = tmp.path().join(format!("{src}.json"));
        let gaps = tmp.path().join(format!("{src}.gaps.json"));
        let result = std::process::Command::new(env!("CARGO_BIN_EXE_koplik-ingest"))
            .args(["parse", src, "--store"])
            .arg(store.root())
            .arg("--out")
            .arg(&out)
            .arg("--gaps")
            .arg(&gaps)
            .env_remove("KOPLIK_CONTACT")
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let rows: Vec<KindergartenMmrCoverage> =
            serde_json::from_slice(&std::fs::read(out).unwrap()).unwrap();
        let report: Vec<KindergartenMmrCoverage> =
            serde_json::from_slice(&std::fs::read(gaps).unwrap()).unwrap();
        assert_eq!(rows.len(), n);
        assert_eq!(report.len(), gap_count);
        assert_eq!(
            report,
            coverage::gaps(&rows)
                .into_iter()
                .cloned()
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn live_coverage_fetch_requires_contact_before_creating_store() {
    for source in ["cdc-coverage", "texas-coverage"] {
        for contact in [Some(""), Some("   "), Some("\t")] {
            let dir = tempfile::tempdir().unwrap();
            let store = dir.path().join("snapshots");
            let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_koplik-ingest"));
            cmd.args(["fetch", source, "--store"])
                .arg(&store)
                .env_remove("KOPLIK_CONTACT");
            if let Some(contact) = contact {
                cmd.env("KOPLIK_CONTACT", contact);
            }
            let result = cmd.output().unwrap();
            assert!(!result.status.success());
            let error = String::from_utf8_lossy(&result.stderr);
            assert!(error.contains("KOPLIK_CONTACT"), "{error}");
            assert!(
                !store.exists(),
                "contact refusal must precede store creation"
            );
        }
    }
}
