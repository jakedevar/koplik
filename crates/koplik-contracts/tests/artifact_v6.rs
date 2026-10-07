use koplik_contracts::v6::{GeographyArtifact, RowArtifact};
use serde_json::{Value, json};

fn row(provenance: Value) -> Value {
    json!({"id":"48", "level":"state", "name":"Texas", "centroid":null, "provenance":provenance})
}

fn record() -> Value {
    json!({"source_id":"fixture", "url":"https://example.invalid/source",
        "retrieved_at":"2026-10-01T00:00:00Z", "sha256":"a".repeat(64), "licence_id":"fixture-terms"})
}

fn artifact() -> Value {
    json!({"contract_version":6,"provenance":[record()],"rows":[row(json!([0]))]})
}

#[test]
fn packing_preserves_order_duplicates_and_the_entire_record() {
    let a = record();
    let mut b = a.clone();
    b["url"] = json!("https://example.invalid/other");
    b["retrieved_at"] = json!("2026-10-02T00:00:00Z");
    b["licence_id"] = json!("other-terms");
    let rows = serde_json::from_value(json!([row(json!([a, b, a])), row(json!([b, a]))])).unwrap();
    let expanded = GeographyArtifact { rows };
    let wire = serde_json::to_value(&expanded).unwrap();
    assert_eq!(wire["provenance"].as_array().unwrap().len(), 2);
    assert_eq!(wire["rows"][0]["provenance"], json!([0, 1, 0]));
    assert_eq!(wire["rows"][1]["provenance"], json!([1, 0]));
    assert_eq!(
        serde_json::from_value::<GeographyArtifact>(wire).unwrap(),
        expanded
    );
}

#[test]
fn empty_artifact_round_trips_with_an_empty_table() {
    let empty: GeographyArtifact = RowArtifact { rows: vec![] };
    assert_eq!(
        serde_json::to_value(&empty).unwrap(),
        json!({"contract_version":6,"provenance":[],"rows":[]})
    );
    assert_eq!(
        serde_json::from_value::<GeographyArtifact>(serde_json::to_value(empty).unwrap())
            .unwrap()
            .rows
            .len(),
        0
    );
}

#[test]
fn malformed_indices_tables_versions_and_expanded_rows_are_rejected() {
    for indices in [
        json!([]),
        json!([-1]),
        json!([0.5]),
        json!([1]),
        json!([4294967296u64]),
        json!(["0"]),
        json!([record()]),
    ] {
        let mut invalid = artifact();
        invalid["rows"][0]["provenance"] = indices;
        assert!(serde_json::from_value::<GeographyArtifact>(invalid).is_err());
    }
    let mut invalid = artifact();
    invalid["contract_version"] = json!(5);
    assert!(serde_json::from_value::<GeographyArtifact>(invalid).is_err());
    let mut invalid = artifact();
    invalid["provenance"][0]["sha256"] = json!("bad");
    assert!(serde_json::from_value::<GeographyArtifact>(invalid).is_err());
    let mut invalid = artifact();
    invalid["rows"][0]["level"] = json!("county");
    assert!(serde_json::from_value::<GeographyArtifact>(invalid).is_err());
    let mut invalid = artifact();
    invalid["extra"] = json!(true);
    assert!(serde_json::from_value::<GeographyArtifact>(invalid).is_err());
}

#[test]
fn packing_existing_json_preserves_numeric_tokens_without_reparsing_floats() {
    use koplik_contracts::v6::{KindergartenMmrCoverage, pack_json};
    use serde_json::value::RawValue;
    let source = format!(
        r#"[{{"geography":"48","school_year":"2023-24","coverage":{{"status":"reported","coverage_pct":94.54545454545455,"exemption_pct":null}},"imputed":false,"imputation_method":null,"provenance":[{}]}}]"#,
        record()
    );
    let packed = pack_json::<KindergartenMmrCoverage>(source.as_bytes()).unwrap();
    #[derive(serde::Deserialize)]
    struct RawRow {
        coverage: Box<RawValue>,
    }
    #[derive(serde::Deserialize)]
    struct Packed {
        rows: Vec<RawRow>,
    }
    let before: Vec<RawRow> = serde_json::from_str(&source).unwrap();
    let after: Packed = serde_json::from_slice(&packed).unwrap();
    assert_eq!(before[0].coverage.get(), after.rows[0].coverage.get());
}
