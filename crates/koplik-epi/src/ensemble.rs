//! Ordered ensembles and daily equal-tail empirical summaries.

use crate::seir::{EngineError, Prepared, Trajectory};
use koplik_contracts::v1::{GeoId, ScenarioInput};
use sha2::{Digest, Sha256};

/// Seed derivation v1: SHA256(b"koplik-seir-member-v1\0" || base_seed LE u64
/// || member LE u32). All 32 digest bytes initialize ChaCha8 via from_seed.
/// No native-width hash inputs, ambient entropy or dependency-specific u64 seeding.
pub fn derive_seed(base_seed: u64, member: u32) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"koplik-seir-member-v1\0");
    hash.update(base_seed.to_le_bytes());
    hash.update(member.to_le_bytes());
    hash.finalize().into()
}

/// Quantiles are linear interpolation at (N-1)*p (Hyndman-Fan type 7).
/// These are ensemble predictive bands, not confidence intervals for the median.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Band {
    pub median: f64,
    pub lower_50: f64,
    pub upper_50: f64,
    pub lower_90: f64,
    pub upper_90: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DailySummary {
    pub day: u32,
    pub geography: GeoId,
    pub susceptible: Band,
    pub exposed: Band,
    pub infectious: Band,
    pub recovered: Band,
    /// S(day-1)-S(day); day 0 has zero new exposures. Not reported cases.
    pub new_exposures: Band,
    /// S(0)-S(day)+E(0)+I(0); excludes vaccine immunity in R.
    pub cumulative_infections: Band,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Ensemble {
    /// Ascending member index, regardless of any future execution scheduling.
    pub members: Vec<Trajectory>,
    /// Ascending day, then canonical GeoId. Includes day zero.
    pub daily: Vec<DailySummary>,
}

fn band(mut values: Vec<u64>) -> Band {
    values.sort_unstable();
    let q = |p: f64| {
        // Length is checked against the contract's u32 run_count, never hashed
        // or used as an RNG draw. Index conversion alone may use usize.
        let rank = (values.len() as u32 - 1) as f64 * p;
        let lo = libm::floor(rank) as usize;
        let hi = libm::ceil(rank) as usize;
        let fraction = rank - libm::floor(rank);
        values[lo] as f64 + (values[hi] - values[lo]) as f64 * fraction
    };
    Band {
        median: q(0.5),
        lower_50: q(0.25),
        upper_50: q(0.75),
        lower_90: q(0.05),
        upper_90: q(0.95),
    }
}

pub fn simulate_ensemble(input: &ScenarioInput) -> Result<Ensemble, EngineError> {
    let prepared = Prepared::new(input)?;
    let members: Vec<_> = (0..input.run_count).map(|k| prepared.run(k)).collect();
    let mut daily = Vec::new();
    let mut previous_step = 0;
    // Exact integer-day endpoints are constructed by Prepared, never interpolated.
    for (index, step) in members[0].steps.iter().enumerate() {
        if step.day != libm::floor(step.day) {
            continue;
        }
        for (node_index, node) in input.nodes.iter().enumerate() {
            let select = |f: fn(crate::seir::Compartments) -> u64| {
                band(
                    members
                        .iter()
                        .map(|m| f(m.steps[index].nodes[node_index]))
                        .collect(),
                )
            };
            daily.push(DailySummary {
                day: step.day as u32,
                geography: node.id,
                susceptible: select(|n| n.susceptible),
                exposed: select(|n| n.exposed),
                infectious: select(|n| n.infectious),
                recovered: select(|n| n.recovered),
                new_exposures: band(
                    members
                        .iter()
                        .map(|m| {
                            m.steps[previous_step].nodes[node_index].susceptible
                                - m.steps[index].nodes[node_index].susceptible
                        })
                        .collect(),
                ),
                cumulative_infections: band(
                    members
                        .iter()
                        .map(|m| {
                            m.steps[0].nodes[node_index].susceptible
                                - m.steps[index].nodes[node_index].susceptible
                                + u64::from(node.initial_exposed)
                                + u64::from(node.initial_infectious)
                        })
                        .collect(),
                ),
            });
        }
        previous_step = index;
    }
    Ok(Ensemble { members, daily })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn type_seven_quantiles() {
        assert_eq!(
            band(vec![4]),
            Band {
                median: 4.0,
                lower_50: 4.0,
                upper_50: 4.0,
                lower_90: 4.0,
                upper_90: 4.0
            }
        );
        let b = band(vec![30, 0, 20, 10]);
        assert_eq!(b.median, 15.0);
        assert_eq!((b.lower_50, b.upper_50), (7.5, 22.5));
        assert!((b.lower_90 - 1.5).abs() < 1e-12);
        assert!((b.upper_90 - 28.5).abs() < 1e-12);
    }
}
