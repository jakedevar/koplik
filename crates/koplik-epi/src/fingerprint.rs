//! SHA-256 over step-major, canonical GeoId-major, S/E/I/R-major values.
//! Include t=0 and every leap. Each integer count is converted exactly to f64
//! and encoded with to_le_bytes(). No lengths, timestamps, IDs, seeds, R0 or
//! platform-sized integers are hashed. Compare fingerprints only alongside
//! their scenario: the digest identifies compartment bytes, not the input.

use crate::seir::Step;
use sha2::{Digest, Sha256};

pub fn fingerprint_steps(steps: &[Step]) -> String {
    let mut hash = Sha256::new();
    for step in steps {
        for node in &step.nodes {
            for count in [
                node.susceptible,
                node.exposed,
                node.infectious,
                node.recovered,
            ] {
                hash.update((count as f64).to_le_bytes());
            }
        }
    }
    hex::encode(hash.finalize())
}
