//! The v7 cumulative case report row and its artifact envelope (#1439).

use koplik_contracts::v7::{
    CaseDefinition, CumulativeCaseReport, CumulativeCaseReportArtifact, CumulativeCount,
    CumulativeMissingReason, ReportDate,
};
use serde_json::{Value, json};

fn record(tag: &str) -> Value {
    json!({"source_id":"fixture", "url":format!("https://example.invalid/{tag}"),
        "retrieved_at":"2026-10-01T00:00:00Z", "sha256":"a".repeat(64), "licence_id":"fixture-terms"})
}

fn row(geography: &str, date: &str, cases: Value, provenance: Value) -> Value {
    json!({"geography":geography, "report_date":date, "cases":cases,
        "case_definition":"confirmed", "provenance":provenance})
}

fn reported(count: u32) -> Value {
    json!({"status":"reported", "count":count})
}

fn artifact() -> Value {
    json!({"contract_version":7, "provenance":[record("a")],
        "rows":[row("48165", "2025-03-04", reported(107), json!([0]))]})
}

fn parse(value: Value) -> Result<CumulativeCaseReportArtifact, serde_json::Error> {
    serde_json::from_value(value)
}

#[test]
fn rows_round_trip_through_a_shared_provenance_table() {
    let a = record("a");
    let b = record("b");
    let rows: Vec<CumulativeCaseReport> = serde_json::from_value(json!([
        row("48165", "2025-03-04", reported(107), json!([a, b])),
        row("48165", "2025-03-25", reported(226), json!([b, a, b])),
        row(
            "48445",
            "2025-03-28",
            json!({"status":"missing", "reason":"no_county_table"}),
            json!([b])
        ),
    ]))
    .unwrap();
    let artifact = CumulativeCaseReportArtifact { rows };
    let wire = serde_json::to_value(&artifact).unwrap();
    assert_eq!(wire["contract_version"], json!(7));
    assert_eq!(wire["provenance"], json!([a, b]));
    assert_eq!(wire["rows"][0]["provenance"], json!([0, 1]));
    assert_eq!(wire["rows"][1]["provenance"], json!([1, 0, 1]));
    assert_eq!(wire["rows"][2]["provenance"], json!([1]));
    assert_eq!(
        serde_json::from_value::<CumulativeCaseReportArtifact>(wire).unwrap(),
        artifact
    );
}

#[test]
fn a_real_zero_and_a_missing_count_stay_different() {
    let wire = json!({"contract_version":7, "provenance":[record("a")], "rows":[
        row("48165", "2025-03-04", reported(0), json!([0])),
        row("48445", "2025-03-04",
            json!({"status":"missing", "reason":"not_listed"}), json!([0])),
    ]});
    let artifact = parse(wire.clone()).unwrap();
    assert_eq!(artifact.rows[0].cases.count(), Some(0));
    assert_eq!(artifact.rows[1].cases.count(), None);
    assert_eq!(
        artifact.rows[1].cases,
        CumulativeCount::Missing {
            reason: CumulativeMissingReason::NotListed
        }
    );
    assert_eq!(artifact.rows[0].case_definition, CaseDefinition::Confirmed);
    assert_eq!(serde_json::to_value(&artifact).unwrap(), wire);
}

#[test]
fn every_missing_reason_is_accepted_and_unknown_ones_are_not() {
    for reason in [
        "no_county_table",
        "not_labelled_confirmed",
        "not_listed",
        "ambiguous",
    ] {
        let mut wire = artifact();
        wire["rows"][0]["cases"] = json!({"status":"missing", "reason":reason});
        assert!(parse(wire).is_ok(), "{reason}");
    }
    for cases in [
        json!({"status":"missing", "reason":"not_reported"}),
        json!({"status":"missing"}),
        json!({"status":"missing", "reason":"ambiguous", "count":1}),
        json!({"status":"reported"}),
        json!({"status":"reported", "count":-1}),
        json!({"status":"reported", "count":1.5}),
        json!({"status":"reported", "count":4294967296u64}),
        json!({"status":"imputed", "count":3}),
        json!({"status":"reported", "count":3, "reason":"ambiguous"}),
    ] {
        let mut wire = artifact();
        wire["rows"][0]["cases"] = cases;
        assert!(parse(wire).is_err());
    }
}

#[test]
fn report_dates_are_canonical_calendar_dates() {
    for good in ["2025-03-04", "2026-01-12", "2024-02-29", "1900-01-01"] {
        assert_eq!(ReportDate::parse(good).unwrap().to_string(), good);
    }
    for bad in [
        "2025-3-4",
        "2025-03-4",
        "2025-02-30",
        "2025-13-01",
        "2025-03-04T00:00:00Z",
        " 2025-03-04",
        "03/04/2025",
        "1899-12-31",
        "2201-01-01",
        "",
    ] {
        assert!(ReportDate::parse(bad).is_err(), "{bad:?}");
        let mut wire = artifact();
        wire["rows"][0]["report_date"] = json!(bad);
        assert!(parse(wire).is_err(), "{bad:?}");
    }
    let mut wire = artifact();
    wire["rows"][0]["report_date"] = json!(20250304);
    assert!(parse(wire).is_err());
}

#[test]
fn malformed_envelopes_indices_and_rows_are_rejected() {
    assert!(parse(artifact()).is_ok());
    for indices in [
        json!([]),
        json!([-1]),
        json!([0.5]),
        json!([1]),
        json!([4294967296u64]),
        json!(["0"]),
        json!([record("a")]),
    ] {
        let mut wire = artifact();
        wire["rows"][0]["provenance"] = indices;
        assert!(parse(wire).is_err());
    }
    let mut wire = artifact();
    wire["contract_version"] = json!(6);
    assert!(parse(wire).is_err());
    let mut wire = artifact();
    wire["provenance"][0]["sha256"] = json!("bad");
    assert!(parse(wire).is_err());
    let mut wire = artifact();
    wire["rows"][0]["geography"] = json!("48x");
    assert!(parse(wire).is_err());
    let mut wire = artifact();
    wire["rows"][0]["case_definition"] = json!("everything");
    assert!(parse(wire).is_err());
    let mut wire = artifact();
    wire["rows"][0]["extra"] = json!(true);
    assert!(parse(wire).is_err());
    let mut wire = artifact();
    wire["extra"] = json!(true);
    assert!(parse(wire).is_err());
    let mut wire = artifact();
    wire.as_object_mut().unwrap().remove("provenance");
    assert!(parse(wire).is_err());
    let mut wire = artifact();
    wire["rows"][0]
        .as_object_mut()
        .unwrap()
        .remove("case_definition");
    assert!(parse(wire).is_err());
}

#[test]
fn one_row_per_geography_and_report_date() {
    let wire = json!({"contract_version":7, "provenance":[record("a")], "rows":[
        row("48165", "2025-03-04", reported(107), json!([0])),
        row("48165", "2025-03-04", reported(108), json!([0])),
    ]});
    assert!(parse(wire).is_err());
    // The same date for another county, or another date for the same county, is fine.
    let wire = json!({"contract_version":7, "provenance":[record("a")], "rows":[
        row("48165", "2025-03-04", reported(107), json!([0])),
        row("48445", "2025-03-04", reported(22), json!([0])),
        row("48165", "2025-03-25", reported(226), json!([0])),
    ]});
    assert_eq!(parse(wire).unwrap().rows.len(), 3);
}

#[test]
fn an_empty_artifact_has_an_empty_table() {
    let empty = CumulativeCaseReportArtifact { rows: vec![] };
    let wire = serde_json::to_value(&empty).unwrap();
    assert_eq!(
        wire,
        json!({"contract_version":7, "provenance":[], "rows":[]})
    );
    assert_eq!(parse(wire).unwrap(), empty);
}

#[test]
fn a_cumulative_row_is_not_a_weekly_count() {
    // The v3 weekly shape needs `week` and `cases` with a different `status` payload; a
    // cumulative row must not parse as one (and the reverse).
    use koplik_contracts::v7::WeeklyCaseCount;
    let cumulative = row("48165", "2025-03-04", reported(107), json!([record("a")]));
    assert!(serde_json::from_value::<WeeklyCaseCount>(cumulative).is_err());
    let weekly = json!({"geography":"48165", "week":{"year":2025,"week":10},
        "cases":reported(1), "case_definition":"confirmed", "provenance":[record("a")]});
    assert!(serde_json::from_value::<CumulativeCaseReport>(weekly).is_err());
}
