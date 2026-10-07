//! Regenerates the JSON Schema for every top-level v6 type and fails when the committed
//! files under `schema/v6/` differ. Regenerate with `make schema`
//! (`KOPLIK_REGEN_SCHEMA=1 cargo test -p koplik-contracts --test schema_v6`), then commit.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use koplik_contracts::v6::*;
use schemars::{JsonSchema, schema_for};

fn render<T: JsonSchema>() -> (String, String) {
    let name = T::schema_name().into_owned();
    let mut text = serde_json::to_string_pretty(&schema_for!(T)).unwrap();
    text.push('\n');
    (format!("{name}.schema.json"), text)
}

fn all() -> BTreeMap<String, String> {
    [
        render::<StateFips>(),
        render::<CountyFips>(),
        render::<GeoId>(),
        render::<Geography>(),
        render::<MmwrWeek>(),
        render::<Provenance>(),
        render::<WeeklyCaseCount>(),
        render::<CaseDefinition>(),
        render::<KindergartenMmrCoverage>(),
        render::<Population>(),
        render::<RtEstimate>(),
        render::<ScenarioInput>(),
        render::<Forecast>(),
        render::<TrajectoryResult>(),
        render::<EnsembleResult>(),
        render::<ScenarioProvenance>(),
        render::<ForecastProvenance>(),
        render::<GeographyArtifact>(),
        render::<WeeklyCaseCountArtifact>(),
        render::<KindergartenMmrCoverageArtifact>(),
        render::<RtEstimateArtifact>(),
        render::<ForecastArtifact>(),
    ]
    .into_iter()
    .collect()
}

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("schema")
        .join(VERSION)
}

#[test]
fn committed_schema_matches_types() {
    let want = all();
    let dir = dir();
    if std::env::var_os("KOPLIK_REGEN_SCHEMA").is_some() {
        fs::create_dir_all(&dir).unwrap();
        for entry in fs::read_dir(&dir).unwrap() {
            fs::remove_file(entry.unwrap().path()).unwrap();
        }
        for (name, text) in &want {
            fs::write(dir.join(name), text).unwrap();
        }
        return;
    }
    let mut have = BTreeMap::new();
    for entry in fs::read_dir(&dir).expect("schema/v6 missing: run `make schema`") {
        let entry = entry.unwrap();
        have.insert(
            entry.file_name().to_string_lossy().into_owned(),
            fs::read_to_string(entry.path()).unwrap(),
        );
    }
    assert_eq!(
        have.keys().collect::<Vec<_>>(),
        want.keys().collect::<Vec<_>>(),
        "schema file set differs: run `make schema` and commit"
    );
    for (name, text) in &want {
        assert_eq!(
            &have[name], text,
            "{name} is stale: run `make schema` and commit"
        );
    }
}
