//! Offline tests for the CDC NNDSS connector: the recorded real response in
//! `data/fixtures/cdc/` plus small synthetic inputs for the edge cases.

use std::collections::BTreeMap;
use std::path::PathBuf;

use koplik_contracts::v2::{
    CaseCount, CaseDefinition, GeoId, MissingReason, MmwrWeek, StateFips, WeeklyCaseCount,
};
use koplik_ingest::cdc;
use koplik_ingest::store::{PutOutcome, Retrieval, RetrievalMeta, SnapshotStore};

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/fixtures/cdc")
}

fn fixture_bytes() -> Vec<u8> {
    std::fs::read(fixture_dir().join("nndss-measles-weekly.json")).unwrap()
}

/// The retrieval record captured when the fixture was fetched.
fn fixture_retrieval() -> Retrieval {
    let line =
        std::fs::read_to_string(fixture_dir().join("nndss-measles-weekly.retrieval.json")).unwrap();
    serde_json::from_str(line.trim()).unwrap()
}

fn rows_for(rows: &[WeeklyCaseCount], fips: u8) -> Vec<&WeeklyCaseCount> {
    rows.iter()
        .filter(|r| r.geography == GeoId::State(StateFips::new(fips).unwrap()))
        .collect()
}

fn count(r: &WeeklyCaseCount) -> Option<u32> {
    r.cases.count()
}

#[test]
fn fixture_is_the_recorded_snapshot() {
    // The fixture's bytes still hash to the address in its retrieval record.
    let r = fixture_retrieval();
    assert_eq!(koplik_ingest::store::sha256_of(&fixture_bytes()), r.sha256);
    assert_eq!(r.source_id, cdc::SOURCE_ID);
    assert_eq!(r.url, cdc::query_url(2025, 2026).unwrap());
}

#[test]
fn parses_every_geography_and_week_with_provenance() {
    let retrieval = fixture_retrieval();
    let rows = cdc::parse_weekly_cases(&fixture_bytes(), &retrieval).unwrap();
    // 56 geographies (50 states, DC, 5 territories) x (53 weeks of 2025 + 38 weeks of 2026).
    assert_eq!(rows.len(), 56 * (53 + 38));
    for fips in [1u8, 36, 45, 48, 49, 72] {
        let g = rows_for(&rows, fips);
        assert_eq!(g.len(), 53 + 38);
        assert_eq!(g[0].week, MmwrWeek::new(2025, 1).unwrap());
        assert_eq!(g[52].week, MmwrWeek::new(2025, 53).unwrap());
        assert_eq!(g[53].week, MmwrWeek::new(2026, 1).unwrap());
        assert_eq!(g[90].week, MmwrWeek::new(2026, 38).unwrap());
    }
    for r in &rows {
        let p = r.provenance.as_slice();
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].sha256, retrieval.sha256);
        assert_eq!(p[0].url, retrieval.url);
        assert_eq!(p[0].retrieved_at, retrieval.retrieved_at);
        assert_eq!(p[0].source_id, cdc::SOURCE_ID);
    }
}

#[test]
fn every_row_declares_the_unknown_status_inclusive_definition() {
    // NNDSS publishes confirmed AND unknown case status for measles, so the connector must
    // never type its totals as confirmed.
    let rows = cdc::parse_weekly_cases(&fixture_bytes(), &fixture_retrieval()).unwrap();
    assert!(
        rows.iter()
            .all(|r| r.case_definition == CaseDefinition::ConfirmedOrUnknownStatus)
    );
    let json = serde_json::to_value(&rows[0]).unwrap();
    assert_eq!(json["case_definition"], "confirmed_or_unknown_status");
    assert!(json.get("confirmed").is_none());
    assert!(json.get("cases").is_some());
}

#[test]
fn weekly_counts_are_cumulative_differences() {
    let rows = cdc::parse_weekly_cases(&fixture_bytes(), &fixture_retrieval()).unwrap();
    // South Carolina 2026, weeks 1-5. The CDC cumulative reads 5, 195, 416, 488, 609, so the
    // weekly counts are 5, 190, 221, 72, 121 (the table's own "current week" column says
    // 5, 72, 84, 33, ... because it leaves out cases added to earlier weeks).
    let sc: Vec<_> = rows_for(&rows, 45)
        .into_iter()
        .filter(|r| r.week.year == 2026)
        .take(5)
        .map(count)
        .collect();
    assert_eq!(sc, [Some(5), Some(190), Some(221), Some(72), Some(121)]);

    // Where nothing decreased, weekly counts sum to the final cumulative (Texas 2025 final
    // cumulative: 787 Indigenous + 16 Imported = 803).
    let tx25: u32 = rows_for(&rows, 48)
        .into_iter()
        .filter(|r| r.week.year == 2025)
        .map(|r| count(r).expect("no missing weeks for Texas"))
        .sum();
    assert_eq!(tx25, 803);
    // New York State and New York City report separately; FIPS 36 is their sum (11+17 + 11+9).
    let ny25: u32 = rows_for(&rows, 36)
        .into_iter()
        .filter(|r| r.week.year == 2025)
        .filter_map(count)
        .sum();
    assert_eq!(ny25, 48);
}

#[test]
fn a_falling_cumulative_is_missing_not_negative_or_zero() {
    let rows = cdc::parse_weekly_cases(&fixture_bytes(), &fixture_retrieval()).unwrap();
    let missing: Vec<(&WeeklyCaseCount, MissingReason)> = rows
        .iter()
        .filter_map(|r| match r.cases {
            CaseCount::Missing { reason } => Some((r, reason)),
            CaseCount::Reported { .. } => None,
        })
        .collect();
    assert_eq!(missing.len(), 16);
    assert!(
        missing
            .iter()
            .all(|(_, why)| *why == MissingReason::Ambiguous)
    );
    let ca = |week| {
        rows_for(&rows, 6)
            .into_iter()
            .find(|r| r.week == MmwrWeek::new(2025, week).unwrap())
            .unwrap()
            .cases
    };
    // California 2025 week 34: total cumulative 20 -> 19 (Indigenous 17 -> 16).
    assert_eq!(
        ca(34),
        CaseCount::Missing {
            reason: MissingReason::Ambiguous
        }
    );
    // Week 35 moves 11 cases from Indigenous to Imported (16 -> 5 and 3 -> 15) in the source;
    // the combined total only rises by one, which is what is reported.
    assert_eq!(ca(35), CaseCount::Reported { count: 1 });
}

#[test]
fn latest_snapshot_parses_through_the_store() {
    let dir = tempfile::tempdir().unwrap();
    let store = SnapshotStore::open(dir.path()).unwrap();
    let fixture = fixture_retrieval();
    let meta = |at: &str| RetrievalMeta {
        source_id: fixture.source_id.clone(),
        url: fixture.url.clone(),
        retrieved_at: at.parse().unwrap(),
        licence_id: fixture.licence_id.clone(),
        http_status: 200,
        content_type: fixture.content_type.clone(),
    };
    let (_, a) = store
        .record(meta("2026-10-07T02:19:30Z"), &fixture_bytes())
        .unwrap();
    let (_, b) = store
        .record(meta("2026-10-08T02:19:30Z"), &fixture_bytes())
        .unwrap();
    assert_eq!((a, b), (PutOutcome::Created, PutOutcome::AlreadyPresent));
    let (retrieval, rows) = cdc::parse_latest(&store).unwrap();
    assert_eq!(
        retrieval.retrieved_at.to_rfc3339(),
        "2026-10-08T02:19:30+00:00"
    );
    assert_eq!(rows.len(), 56 * (53 + 38));
    // Rows trace to the snapshot hash and the retrieval time of the snapshot that was parsed.
    assert_eq!(rows[0].provenance.as_slice()[0].sha256, fixture.sha256);
    assert_eq!(
        rows[0].provenance.as_slice()[0].retrieved_at,
        retrieval.retrieved_at
    );
}

#[test]
fn the_query_url_is_explicit_and_reproducible() {
    let url = cdc::query_url(2025, 2026).unwrap();
    assert!(url.starts_with("https://data.cdc.gov/resource/x9gk-5huc.json?"));
    for part in ["$select=", "$where=", "$order=", "$limit=50000"] {
        assert!(url.contains(part), "{part} in {url}");
    }
    assert_eq!(url, cdc::query_url(2025, 2026).unwrap());
    assert!(cdc::query_url(2026, 2025).is_err());
}

// ---- synthetic edge cases -------------------------------------------------------------

fn retrieval() -> Retrieval {
    Retrieval {
        source_id: cdc::SOURCE_ID.into(),
        url: "https://example.test/q".into(),
        retrieved_at: "2026-10-07T00:00:00Z".parse().unwrap(),
        sha256: koplik_ingest::store::sha256_of(b"synthetic"),
        licence_id: cdc::LICENCE_ID.into(),
        http_status: 200,
        content_type: None,
        bytes: 9,
    }
}

/// `(state, year, week, label, m3 or flag)` -> Socrata-style JSON bytes.
fn raw(rows: &[(&str, u16, u8, &str, &str)]) -> Vec<u8> {
    let objs: Vec<serde_json::Value> = rows
        .iter()
        .map(|(st, y, w, label, v)| {
            let mut o = serde_json::json!({"states": st, "year": y.to_string(), "week": w.to_string(), "label": label});
            if v.parse::<f64>().is_ok() {
                o["m3"] = (*v).into();
            } else {
                o["m3_flag"] = (*v).into();
            }
            o
        })
        .collect();
    serde_json::to_vec(&objs).unwrap()
}

const IND: &str = "Measles, Indigenous";
const IMP: &str = "Measles, Imported";

fn weekly(rows: &[WeeklyCaseCount], fips: u8, year: u16) -> BTreeMap<u8, CaseCount> {
    rows_for(rows, fips)
        .into_iter()
        .filter(|r| r.week.year == year)
        .map(|r| (r.week.week, r.cases))
        .collect()
}

fn reported(n: u32) -> CaseCount {
    CaseCount::Reported { count: n }
}

fn missing(reason: MissingReason) -> CaseCount {
    CaseCount::Missing { reason }
}

#[test]
fn flags_map_to_zero_or_missing_never_a_guess() {
    // Texas: "-" (no reported cases) is a real zero cumulative; "U" is unavailable.
    let bytes = raw(&[
        ("Texas", 2026, 1, IND, "-"),
        ("Texas", 2026, 1, IMP, "-"),
        ("Texas", 2026, 2, IND, "4.0"),
        ("Texas", 2026, 2, IMP, "-"),
        ("Texas", 2026, 3, IND, "U"),
        ("Texas", 2026, 3, IMP, "-"),
        ("Texas", 2026, 4, IND, "9.0"),
        ("Texas", 2026, 4, IMP, "-"),
    ]);
    let rows = cdc::parse_weekly_cases(&bytes, &retrieval()).unwrap();
    let tx = weekly(&rows, 48, 2026);
    assert_eq!(tx[&1], reported(0));
    assert_eq!(tx[&2], reported(4));
    assert_eq!(tx[&3], missing(MissingReason::NotReported));
    // Week 4 follows an unknown cumulative, so its own share cannot be told apart.
    assert_eq!(tx[&4], missing(MissingReason::Ambiguous));
}

#[test]
fn absent_rows_are_not_reported_and_absent_states_stay_missing() {
    // Only Texas week 1 and week 3 exist; week 2 is absent. Alabama has no rows at all.
    let bytes = raw(&[
        ("Texas", 2026, 1, IND, "2"),
        ("Texas", 2026, 1, IMP, "-"),
        ("Texas", 2026, 3, IND, "7"),
        ("Texas", 2026, 3, IMP, "-"),
    ]);
    let rows = cdc::parse_weekly_cases(&bytes, &retrieval()).unwrap();
    let tx = weekly(&rows, 48, 2026);
    assert_eq!(tx[&1], reported(2));
    assert_eq!(tx[&2], missing(MissingReason::NotReported));
    assert_eq!(tx[&3], missing(MissingReason::Ambiguous));
    let al = weekly(&rows, 1, 2026);
    assert_eq!(al.len(), 3);
    assert!(
        al.values()
            .all(|c| *c == missing(MissingReason::NotReported))
    );
}

#[test]
fn reclassification_between_labels_is_not_a_decrease_and_nyc_adds_to_new_york() {
    // Week 2: one case moves Imported -> Indigenous (4+2 -> 5+1); the sum is unchanged.
    let bytes = raw(&[
        ("New York", 2026, 1, IND, "4"),
        ("New York", 2026, 1, IMP, "2"),
        ("New York City", 2026, 1, IND, "1"),
        ("New York City", 2026, 1, IMP, "-"),
        ("New York", 2026, 2, IND, "5"),
        ("New York", 2026, 2, IMP, "1"),
        ("New York City", 2026, 2, IND, "3"),
        ("New York City", 2026, 2, IMP, "-"),
    ]);
    let rows = cdc::parse_weekly_cases(&bytes, &retrieval()).unwrap();
    let ny = weekly(&rows, 36, 2026);
    assert_eq!(ny[&1], reported(7));
    assert_eq!(ny[&2], reported(2));
}

#[test]
fn rejects_malformed_or_unrecognised_input_instead_of_guessing() {
    let r = retrieval();
    // Unknown jurisdiction name.
    assert!(cdc::parse_weekly_cases(&raw(&[("Atlantis", 2026, 1, IND, "1")]), &r).is_err());
    // Unexpected label.
    assert!(cdc::parse_weekly_cases(&raw(&[("Texas", 2026, 1, "Mumps", "1")]), &r).is_err());
    // Duplicate row.
    let dup = raw(&[("Texas", 2026, 1, IND, "1"), ("Texas", 2026, 1, IND, "2")]);
    assert!(cdc::parse_weekly_cases(&dup, &r).is_err());
    // Week 53 of a 52-week MMWR year.
    assert!(cdc::parse_weekly_cases(&raw(&[("Texas", 2026, 53, IND, "1")]), &r).is_err());
    // Not JSON.
    assert!(cdc::parse_weekly_cases(b"<html>", &r).is_err());
    // Non-integral cumulative reads as missing (ambiguous), not rounded.
    let rows = cdc::parse_weekly_cases(
        &raw(&[("Texas", 2026, 1, IND, "1.5"), ("Texas", 2026, 1, IMP, "-")]),
        &r,
    )
    .unwrap();
    assert_eq!(
        weekly(&rows, 48, 2026)[&1],
        missing(MissingReason::Ambiguous)
    );
}

#[test]
fn aggregate_rows_are_skipped_and_a_full_limit_response_is_refused() {
    let r = retrieval();
    let rows = cdc::parse_weekly_cases(
        &raw(&[("Total", 2026, 1, IND, "9"), ("Pacific", 2026, 1, IND, "3")]),
        &r,
    )
    .unwrap();
    // Aggregates define no geography and no weeks: nothing to publish from them.
    assert!(rows.is_empty());
    let many: Vec<serde_json::Value> = (0..cdc::ROW_LIMIT)
        .map(|_| serde_json::json!({"states": "Total", "year": "2026", "week": "1", "label": IND}))
        .collect();
    assert!(cdc::parse_weekly_cases(&serde_json::to_vec(&many).unwrap(), &r).is_err());
}
