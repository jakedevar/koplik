//! Native timing harness; no clock or I/O is used by the library.
use koplik_contracts::v1::ScenarioInput;
use koplik_epi::simulate_ensemble;
use std::{hint::black_box, time::Instant};

fn main() {
    let input: ScenarioInput = serde_json::from_str(include_str!(
        "../../../data/fixtures/seir/synthetic-scenario.json"
    ))
    .expect("committed synthetic fixture");
    let start = Instant::now();
    let output = simulate_ensemble(black_box(&input)).expect("valid synthetic input");
    let elapsed = start.elapsed();
    println!(
        "SYNTHETIC benchmark: {} members, {} counties, {} days, dt={} d; ensemble + summaries + fingerprints: {:.6} s; first fingerprint={}",
        output.members.len(),
        input.nodes.len(),
        input.parameters.horizon_days,
        input.parameters.time_step_days,
        elapsed.as_secs_f64(),
        output.members[0].fingerprint
    );
    black_box(output);
}
