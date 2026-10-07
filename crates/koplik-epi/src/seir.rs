//! Closed-population stochastic SEIR, simultaneous bounded tau-leaps.
//!
//! q=1-c*VE2, where c is two-dose kindergarten coverage as a fraction. This
//! treats coverage as a proxy for ALL ages, protection as all-or-none, and all
//! nonvaccinated residents as susceptible (no natural immunity, waning, births,
//! deaths or migration). These are demonstration assumptions, not measured
//! population immunity. CDC effectiveness: https://www.cdc.gov/measles/hcp/vaccine-considerations/index.html.
//! Initial susceptible pool = round(N*q); E0+I0 is removed from that pool,
//! and the remainder of N is R (immune/recovered). Inconsistent seeds are errors.
//! One-dose effectiveness is retained in the input but unused: v1 has no
//! one-dose-only coverage. Missing coverage requires an explicit override.
//!
//! beta=R0/infectious_period; p_SE=1-exp(-beta*mix(I/N)*dt),
//! p_EI=1-exp(-dt/latent_period), p_IR=1-exp(-dt/infectious_period).
//! Draw all three transitions from the OLD state; a new exposure cannot become
//! infectious within that leap. Binomial pools prevent negative compartments.
//! Gravity only changes infection pressure: S+E+I+R=N for EACH node at ALL steps.
//! This exponential waiting-time SEIR is a numerical approximation to disease
//! natural history, with step-size bias; smaller steps support sensitivity checks.

use crate::{ensemble::derive_seed, fingerprint, gravity, sampling};
use koplik_contracts::v1::{BaselineCoverage, Centroid, GeoId, R0, ScenarioInput};
use rand_chacha::ChaCha8Rng;
use rand_core::SeedableRng;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EngineError {
    #[error("invalid SEIR scenario: {0}")]
    Invalid(String),
}

/// Exact integer counts internally; populations are limited to 2^53 so the
/// fingerprint's required f64 conversion loses no information.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Compartments {
    pub susceptible: u64,
    pub exposed: u64,
    pub infectious: u64,
    /// Includes vaccine immunity as well as recovery.
    pub recovered: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    pub day: f64,
    /// Same canonical GeoId order as Trajectory::geographies.
    pub nodes: Vec<Compartments>,
}

/// In-memory calculation result; deliberately not a serialized shared contract.
#[derive(Debug, Clone, PartialEq)]
pub struct Trajectory {
    pub member: u32,
    pub seed: [u8; 32],
    pub r0: f64,
    pub geographies: Vec<GeoId>,
    /// Includes the initial state and EVERY leap, including daily boundaries.
    pub steps: Vec<Step>,
    pub fingerprint: String,
}

pub(crate) struct Prepared<'a> {
    pub input: &'a ScenarioInput,
    initial: Vec<Compartments>,
    weights: Vec<Vec<f64>>,
    steps_per_day: u32,
}

impl<'a> Prepared<'a> {
    pub fn new(input: &'a ScenarioInput) -> Result<Self, EngineError> {
        let invalid = |s: &str| EngineError::Invalid(s.into());
        input.parameters.validate().map_err(EngineError::Invalid)?;
        if input.nodes.is_empty() || input.run_count == 0 {
            return Err(invalid("nodes and run_count must be nonempty"));
        }
        if input.nodes.windows(2).any(|w| w[0].id >= w[1].id) {
            return Err(invalid(
                "nodes must be unique and sorted in canonical GeoId order",
            ));
        }
        // Subdivide each calendar day into dt, ..., residual. Clipping at daily
        // boundaries guarantees exact daily summaries for any positive dt, even
        // dt>1 (then one day is the effective maximum). Bound indexing identically
        // on 32/64-bit targets, and reject steps too small to represent safely.
        let steps = libm::ceil(1.0 / input.parameters.time_step_days).max(1.0);
        if steps > f64::from(u32::MAX)
            || steps * f64::from(input.parameters.horizon_days) >= f64::from(u32::MAX)
        {
            return Err(invalid(
                "too many time steps (must fit u32 including the initial state)",
            ));
        }
        for (i, o) in input.coverage_overrides.iter().enumerate() {
            if !o.coverage_pct.is_finite()
                || !(0.0..=100.0).contains(&o.coverage_pct)
                || !input.nodes.iter().any(|n| n.id == o.geography)
                || input.coverage_overrides[..i]
                    .iter()
                    .any(|p| p.geography == o.geography)
            {
                return Err(invalid("invalid, duplicate or unknown coverage override"));
            }
        }
        let mut initial = Vec::with_capacity(input.nodes.len());
        for node in &input.nodes {
            if node.population == 0 || node.population > (1_u64 << 53) {
                return Err(invalid(
                    "population must be in 1..=2^53 for exact f64 fingerprints",
                ));
            }
            Centroid::new(node.centroid.latitude, node.centroid.longitude)
                .map_err(EngineError::Invalid)?;
            if let BaselineCoverage::Reported {
                coverage_pct,
                imputed,
                imputation_method,
                ..
            } = &node.baseline_coverage
            {
                if !coverage_pct.is_finite()
                    || !(0.0..=100.0).contains(coverage_pct)
                    || match (*imputed, imputation_method) {
                        (false, None) => false,
                        (true, Some(method)) => method.trim().is_empty(),
                        _ => true,
                    }
                {
                    return Err(invalid(
                        "invalid baseline coverage or imputation declaration",
                    ));
                }
            }
            let coverage = match input
                .coverage_overrides
                .iter()
                .find(|o| o.geography == node.id)
            {
                Some(o) => o.coverage_pct,
                None => match node.baseline_coverage {
                    BaselineCoverage::Reported { coverage_pct, .. } => coverage_pct,
                    BaselineCoverage::Missing { .. } => {
                        return Err(invalid("missing coverage requires an explicit override"));
                    }
                },
            };
            let pool = libm::round(
                node.population as f64
                    * (1.0 - coverage / 100.0 * input.parameters.mmr_effectiveness_two_doses),
            ) as u64;
            let exposed = u64::from(node.initial_exposed);
            let infectious = u64::from(node.initial_infectious);
            let susceptible = pool.checked_sub(exposed + infectious).ok_or_else(|| {
                invalid("initial E+I exceeds the coverage-derived susceptible pool")
            })?;
            initial.push(Compartments {
                susceptible,
                exposed,
                infectious,
                recovered: node.population - pool,
            });
        }
        Ok(Self {
            input,
            initial,
            weights: gravity::weights(input)?,
            steps_per_day: steps as u32,
        })
    }

    pub fn run(&self, member: u32) -> Trajectory {
        let seed = derive_seed(self.input.seed, member);
        let mut rng = ChaCha8Rng::from_seed(seed);
        let p = &self.input.parameters;
        let r0 = match p.r0 {
            R0::Fixed { value } => value,
            R0::UniformPrior { min, max } => min + (max - min) * sampling::uniform(&mut rng),
        };
        let mut state = self.initial.clone();
        let mut steps = vec![Step {
            day: 0.0,
            nodes: state.clone(),
        }];
        let mut prevalence = vec![0.0; state.len()];
        for day in 0..p.horizon_days {
            let mut previous = 0.0;
            for substep in 0..self.steps_per_day {
                let end = (f64::from(substep + 1) * p.time_step_days).min(1.0);
                let dt = end - previous;
                previous = end;
                for ((value, node), counts) in
                    prevalence.iter_mut().zip(&self.input.nodes).zip(&state)
                {
                    *value = counts.infectious as f64 / node.population as f64;
                }
                let p_ei = -libm::expm1(-dt / p.latent_period_days);
                let p_ir = -libm::expm1(-dt / p.infectious_period_days);
                for (counts, weights) in state.iter_mut().zip(&self.weights) {
                    let pressure: f64 = weights.iter().zip(&prevalence).map(|(w, v)| w * v).sum();
                    // Divide after multiplying pressure: avoid inf*0 at zero
                    // prevalence for tiny user-supplied infectious periods.
                    let hazard = (r0 * pressure) / p.infectious_period_days;
                    let p_se = -libm::expm1(-hazard * dt);
                    let se = sampling::binomial(&mut rng, counts.susceptible, p_se);
                    let ei = sampling::binomial(&mut rng, counts.exposed, p_ei);
                    let ir = sampling::binomial(&mut rng, counts.infectious, p_ir);
                    *counts = Compartments {
                        susceptible: counts.susceptible - se,
                        exposed: counts.exposed - ei + se,
                        infectious: counts.infectious - ir + ei,
                        recovered: counts.recovered + ir,
                    };
                }
                steps.push(Step {
                    day: f64::from(day) + end,
                    nodes: state.clone(),
                });
            }
        }
        let fingerprint = fingerprint::fingerprint_steps(&steps);
        Trajectory {
            member,
            seed,
            r0,
            geographies: self.input.nodes.iter().map(|n| n.id).collect(),
            steps,
            fingerprint,
        }
    }
}

/// Run a specified ensemble member independently. Member index must belong to
/// input.run_count, enabling scheduling in any order without changing results.
pub fn simulate_member(input: &ScenarioInput, member: u32) -> Result<Trajectory, EngineError> {
    if member >= input.run_count {
        return Err(EngineError::Invalid(
            "member index exceeds run_count".into(),
        ));
    }
    Ok(Prepared::new(input)?.run(member))
}
