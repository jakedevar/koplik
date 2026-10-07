//! No I/O, ambient entropy or engine changes: v1 scenario JSON in, v2 result JSON out.
use koplik_contracts::{v1::ScenarioInput, v2::*};
use koplik_epi::{Band, Trajectory, simulate_ensemble, simulate_member};
use wasm_bindgen::prelude::*;

fn metadata(t: &Trajectory) -> EnsembleMember {
    EnsembleMember {
        member: t.member,
        derived_seed: t.seed,
        r0: t.r0,
        fingerprint: Sha256Hex::new(t.fingerprint.clone()).expect("engine SHA256"),
    }
}

fn band(b: Band) -> SimulationBand {
    SimulationBand {
        median: b.median,
        lower_50: b.lower_50,
        upper_50: b.upper_50,
        lower_90: b.lower_90,
        upper_90: b.upper_90,
    }
}

/// Native entrypoint shared by the WASM binding and tests. Errors are explicit.
pub fn trajectory_json(input_json: &str, member: u32) -> Result<String, String> {
    let input: ScenarioInput = serde_json::from_str(input_json).map_err(|e| e.to_string())?;
    let output = simulate_member(&input, member).map_err(|e| e.to_string())?;
    let result = TrajectoryResult {
        contract_version: SIMULATION_CONTRACT_VERSION,
        scenario_json: input_json.to_owned(),
        seed: input.seed.to_string(),
        parameters: input.parameters,
        member: metadata(&output),
        geographies: output.geographies,
        steps: output
            .steps
            .into_iter()
            .map(|s| SimulationStep {
                day: s.day,
                nodes: s
                    .nodes
                    .into_iter()
                    .map(|n| SimulationCompartments {
                        susceptible: n.susceptible,
                        exposed: n.exposed,
                        infectious: n.infectious,
                        recovered: n.recovered,
                    })
                    .collect(),
            })
            .collect(),
    };
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

pub fn ensemble_json(input_json: &str) -> Result<String, String> {
    let input: ScenarioInput = serde_json::from_str(input_json).map_err(|e| e.to_string())?;
    let output = simulate_ensemble(&input).map_err(|e| e.to_string())?;
    let result = EnsembleResult {
        contract_version: SIMULATION_CONTRACT_VERSION,
        scenario_json: input_json.to_owned(),
        seed: input.seed.to_string(),
        parameters: input.parameters,
        fingerprint: metadata(&output.members[0]).fingerprint,
        members: output.members.iter().map(metadata).collect(),
        daily: output
            .daily
            .into_iter()
            .map(|d| SimulationDay {
                day: d.day,
                geography: d.geography,
                susceptible: band(d.susceptible),
                exposed: band(d.exposed),
                infectious: band(d.infectious),
                recovered: band(d.recovered),
                new_exposures: band(d.new_exposures),
                cumulative_infections: band(d.cumulative_infections),
            })
            .collect(),
    };
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

/// Throws a JS Error on invalid input. Pass raw JSON to preserve the full u64 seed.
#[wasm_bindgen(js_name = runTrajectory)]
pub fn run_trajectory(input_json: &str, member: u32) -> Result<String, JsError> {
    trajectory_json(input_json, member).map_err(|e| JsError::new(&e))
}

#[wasm_bindgen(js_name = runEnsemble)]
pub fn run_ensemble(input_json: &str) -> Result<String, JsError> {
    ensemble_json(input_json).map_err(|e| JsError::new(&e))
}
