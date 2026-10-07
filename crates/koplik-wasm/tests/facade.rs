use koplik_contracts::{
    v1::ScenarioInput,
    v2::{EnsembleResult, TrajectoryResult},
};
use koplik_epi::{simulate_ensemble, simulate_member};
use koplik_wasm::{ensemble_json, trajectory_json};
use sha2::{Digest, Sha256};

const FIXTURE: &str = include_str!("../../../data/fixtures/seir/synthetic-scenario.json");

#[test]
fn full_trajectory_matches_engine_and_independent_byte_hash() {
    let input: ScenarioInput = serde_json::from_str(FIXTURE).unwrap();
    let engine = simulate_member(&input, 0).unwrap();
    let result: TrajectoryResult =
        serde_json::from_str(&trajectory_json(FIXTURE, 0).unwrap()).unwrap();
    assert_eq!(result.contract_version, 2);
    assert_eq!(result.scenario_json, FIXTURE);
    assert_eq!(result.seed, "1353");
    assert_eq!(result.parameters, input.parameters);
    assert_eq!(result.geographies, engine.geographies);
    assert_eq!(result.member.derived_seed, engine.seed);
    assert_eq!(result.member.r0, engine.r0);
    assert_eq!(result.steps.len(), engine.steps.len());
    let mut hash = Sha256::new();
    for (step, native) in result.steps.iter().zip(&engine.steps) {
        assert_eq!(step.day, native.day);
        for (node, counts) in step.nodes.iter().zip(&native.nodes) {
            let values = [
                node.susceptible,
                node.exposed,
                node.infectious,
                node.recovered,
            ];
            assert_eq!(
                values,
                [
                    counts.susceptible,
                    counts.exposed,
                    counts.infectious,
                    counts.recovered
                ]
            );
            for value in values {
                hash.update((value as f64).to_le_bytes());
            }
        }
    }
    assert_eq!(
        result.member.fingerprint.as_str(),
        hex::encode(hash.finalize())
    );
    assert_eq!(
        result.member.fingerprint.as_str(),
        "7a7471b1ed6d648d9a376d591ed21be513b90128d5f5e7c759c184689d5c25fb"
    );
}

#[test]
fn ensemble_preserves_bands_seeds_parameters_and_provenance() {
    let mut input: ScenarioInput = serde_json::from_str(FIXTURE).unwrap();
    input.run_count = 3;
    input.parameters.horizon_days = 3;
    input.seed = u64::MAX;
    let json = serde_json::to_string(&input).unwrap();
    let engine = simulate_ensemble(&input).unwrap();
    let result: EnsembleResult = serde_json::from_str(&ensemble_json(&json).unwrap()).unwrap();
    assert_eq!(result.seed, u64::MAX.to_string());
    assert_eq!(result.scenario_json, json);
    assert_eq!(result.parameters, input.parameters);
    assert_eq!(result.fingerprint.as_str(), engine.members[0].fingerprint);
    for (member, native) in result.members.iter().zip(&engine.members) {
        assert_eq!(member.member, native.member);
        assert_eq!(member.derived_seed, native.seed);
        assert_eq!(member.r0, native.r0);
        assert_eq!(member.fingerprint.as_str(), native.fingerprint);
    }
    assert_eq!(result.members.len(), 3);
    assert_eq!(result.daily.len(), engine.daily.len());
    for (row, native) in result.daily.iter().zip(&engine.daily) {
        assert_eq!(row.day, native.day);
        assert_eq!(row.geography, native.geography);
        for (band, expected) in [
            (row.susceptible, native.susceptible),
            (row.exposed, native.exposed),
            (row.infectious, native.infectious),
            (row.recovered, native.recovered),
            (row.new_exposures, native.new_exposures),
            (row.cumulative_infections, native.cumulative_infections),
        ] {
            assert_eq!(
                [
                    band.median,
                    band.lower_50,
                    band.upper_50,
                    band.lower_90,
                    band.upper_90
                ],
                [
                    expected.median,
                    expected.lower_50,
                    expected.upper_50,
                    expected.lower_90,
                    expected.upper_90
                ]
            );
        }
    }
}

#[test]
fn invalid_json_missing_coverage_and_invalid_member_are_errors() {
    assert!(ensemble_json("{}").is_err());
    assert!(trajectory_json("invalid JSON", 0).is_err());
    let mut input: ScenarioInput = serde_json::from_str(FIXTURE).unwrap();
    assert!(trajectory_json(FIXTURE, input.run_count).is_err());
    input.coverage_overrides.clear();
    let missing = serde_json::to_string(&input).unwrap();
    assert!(
        ensemble_json(&missing)
            .unwrap_err()
            .contains("missing baseline coverage")
    );
    assert!(
        trajectory_json(&missing, 0)
            .unwrap_err()
            .contains("missing baseline coverage")
    );
}
