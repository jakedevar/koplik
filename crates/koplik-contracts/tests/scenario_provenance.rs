//! Contract v4 `ScenarioProvenance`: round trip, rejection of malformed companions, and the
//! check that a companion describes the scenario it sits beside.

use koplik_contracts::v4::*;
use serde_json::{Value, json};

fn scenario() -> ScenarioInput {
    // The committed synthetic scenario: a valid v1 input, used here only as a shape.
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/fixtures/seir/synthetic-scenario.json");
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

/// A companion for [`scenario`], built from the scenario's own values.
fn companion() -> Value {
    let input = scenario();
    let values = serde_json::to_value(input.parameters).unwrap();
    let parameters: Vec<Value> = values
        .as_object()
        .unwrap()
        .iter()
        .map(|(k, v)| {
            json!({
                "parameter": k, "value": v, "source": "a published source",
                "url": null, "note": "what it supports"
            })
        })
        .collect();
    let nodes: Vec<Value> = input
        .nodes
        .iter()
        .map(|n| {
            json!({
                "geography": n.id, "name": "County", "population": n.population,
                "population_basis": "estimate", "centroid_basis": "internal point",
                "coverage_school_year": "2023-24", "coverage_basis": "published"
            })
        })
        .collect();
    json!({
        "contract_version": 4,
        "scenario": "gaines-2025",
        "statement": "Hypothetical: one infectious person arrives.",
        "seed": input.seed.to_string(),
        "run_count": input.run_count,
        "seeding": {
            "geography": "48165",
            "initial_infectious": 5, "initial_exposed": 5,
            "start_week": input.start_week,
            "assumption": "a stated assumption", "start_week_basis": "a reference week",
            "limitation": "what it cannot capture"
        },
        "parameters": parameters,
        "nodes": nodes,
        "excluded_nodes": [],
        "neighbourhood_note": "one county"
    })
}

fn parse(v: &Value) -> Result<ScenarioProvenance, String> {
    serde_json::from_value(v.clone()).map_err(|e| e.to_string())
}

fn edit(change: impl FnOnce(&mut Value)) -> Value {
    let mut v = companion();
    change(&mut v);
    v
}

#[test]
fn round_trips_and_describes_its_scenario() {
    let p = parse(&companion()).unwrap();
    assert_eq!(p.contract_version, SCENARIO_PROVENANCE_VERSION);
    assert_eq!(serde_json::to_value(&p).unwrap(), companion());
    assert_eq!(
        serde_json::from_value::<ScenarioProvenance>(serde_json::to_value(&p).unwrap()).unwrap(),
        p
    );
    p.check_against(&scenario()).unwrap();
}

#[test]
fn rejects_malformed_companions() {
    for (what, bad, message) in [
        (
            "version",
            edit(|v| v["contract_version"] = json!(3)),
            "contract_version must be 4",
        ),
        (
            "statement",
            edit(|v| v["statement"] = json!("  ")),
            "statement must not be empty",
        ),
        (
            "seed with a leading zero",
            edit(|v| v["seed"] = json!("01353")),
            "seed must be",
        ),
        (
            "seed above u64",
            edit(|v| v["seed"] = json!("18446744073709551616")),
            "seed must be",
        ),
        (
            "run_count",
            edit(|v| v["run_count"] = json!(0)),
            "run_count must be at least 1",
        ),
        (
            "no parameters",
            edit(|v| v["parameters"] = json!([])),
            "parameters must not be empty",
        ),
        (
            "empty source",
            edit(|v| v["parameters"][0]["source"] = json!("")),
            "source must not be empty",
        ),
        (
            "duplicate parameter",
            edit(|v| {
                let first = v["parameters"][0].clone();
                v["parameters"].as_array_mut().unwrap().push(first);
            }),
            "cited twice",
        ),
        (
            "empty assumption",
            edit(|v| v["seeding"]["assumption"] = json!("")),
            "seeding.assumption must not be empty",
        ),
        (
            "zero population",
            edit(|v| v["nodes"][0]["population"] = json!(0)),
            "population of",
        ),
        (
            "unknown field",
            edit(|v| v["extra"] = json!(1)),
            "unknown field",
        ),
    ] {
        let err = parse(&bad).expect_err(what);
        assert!(err.contains(message), "{what}: {err}");
    }
}

#[test]
fn a_companion_must_agree_with_the_scenario_beside_it() {
    let input = scenario();
    for (what, change) in [
        ("seed", edit(|v| v["seed"] = json!("1"))),
        ("run_count", edit(|v| v["run_count"] = json!(7))),
        (
            "start week",
            edit(|v| v["seeding"]["start_week"]["week"] = json!(2)),
        ),
        (
            "seeding",
            edit(|v| v["seeding"]["initial_infectious"] = json!(6)),
        ),
        (
            "parameters",
            edit(|v| v["parameters"][1]["value"] = json!(999.0)),
        ),
        (
            "parameter set",
            edit(|v| {
                v["parameters"].as_array_mut().unwrap().pop();
            }),
        ),
        (
            "nodes",
            edit(|v| {
                v["nodes"].as_array_mut().unwrap().pop();
            }),
        ),
        (
            "population",
            edit(|v| v["nodes"][0]["population"] = json!(1)),
        ),
    ] {
        let p = parse(&change).unwrap();
        let err = p.check_against(&input).expect_err(what);
        assert!(err.contains("does not describe"), "{what}: {err}");
    }
    // A seeded geography that is not a node, and another seeded node, are both refused.
    let p = parse(&edit(|v| v["seeding"]["geography"] = json!("48999"))).unwrap();
    assert!(p.check_against(&input).is_err());
    let mut other = input.clone();
    other.nodes[0].initial_infectious = 1;
    let err = parse(&companion())
        .unwrap()
        .check_against(&other)
        .expect_err("another node seeded");
    assert!(err.contains("does not describe"), "{err}");
}
