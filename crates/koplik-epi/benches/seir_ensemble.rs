//! Native timing harness; no clock or I/O is used by the library.
use koplik_contracts::v1::ScenarioInput;
use koplik_epi::simulate_ensemble;
use sha2::{Digest, Sha256};
use std::{hint::black_box, time::Instant};

fn main() {
    let input: ScenarioInput = serde_json::from_str(include_str!(
        "../../../data/fixtures/seir/synthetic-scenario.json"
    ))
    .expect("committed synthetic fixture");
    let start = Instant::now();
    let output = simulate_ensemble(black_box(&input)).expect("valid synthetic input");
    let elapsed = start.elapsed();
    // Ensemble digest: SHA-256 over every member's fingerprint hex string in
    // member order. It pins the whole seeded stream, not just member 0.
    let mut digest = Sha256::new();
    for member in &output.members {
        digest.update(member.fingerprint.as_bytes());
    }
    println!(
        "SYNTHETIC benchmark: {} members, {} counties, {} days, dt={} d; ensemble + summaries + fingerprints: {:.6} s; first fingerprint={}; ensemble digest={}",
        output.members.len(),
        input.nodes.len(),
        input.parameters.horizon_days,
        input.parameters.time_step_days,
        elapsed.as_secs_f64(),
        output.members[0].fingerprint,
        hex::encode(digest.finalize())
    );
    black_box(output);
}
