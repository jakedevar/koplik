//! Explicit demonstration defaults, never a fit to outbreak or backtest outcomes.

use koplik_contracts::v1::{R0, SeirParameters};

pub fn default_parameters() -> SeirParameters {
    SeirParameters {
        // Conventional measles range, NOT a locally calibrated prior. Guerra et al.
        // (2017) found estimates outside it: https://pubmed.ncbi.nlm.nih.gov/28757186/.
        // The uniform density is a demonstration modelling assumption.
        r0: R0::UniformPrior {
            min: 12.0,
            max: 18.0,
        },
        // Approximation: CDC's mean exposure-to-rash 14 d minus 4 d infectious
        // before rash = 10 d latent. Not the 11–12 d incubation-to-prodrome.
        // https://www.cdc.gov/pinkbook/hcp/table-of-contents/chapter-13-measles.html
        latent_period_days: 10.0,
        // Same CDC source: infectious 4 d before to 4 d after rash. Treating this
        // 8 d window as an exponential mean is an explicit SEIR approximation.
        infectious_period_days: 8.0,
        // CDC estimates, not sterilizing-immunity measurements. The all-or-none
        // susceptibility approximation is documented in seir.rs.
        // https://www.cdc.gov/measles/hcp/vaccine-considerations/index.html
        mmr_effectiveness_one_dose: 0.93,
        mmr_effectiveness_two_doses: 0.97,
        // Numerical/display choices for this demo, not biological estimates:
        // E4 of thoughts/shared/project/koplik-spec.md (daily tau-leaping).
        time_step_days: 1.0,
        // Six-month demo window; an explicit product choice, not a fitted value.
        horizon_days: 180,
        // No calibrated Texas mobility parameters exist in this engine. Callers
        // opt in with explicit scale and exponents; see gravity.rs and its source.
        gravity: None,
    }
}
