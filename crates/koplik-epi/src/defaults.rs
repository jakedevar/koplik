//! Explicit demonstration defaults, never a fit to outbreak or backtest outcomes.

use koplik_contracts::v1::{R0, SeirParameters};

pub fn default_parameters() -> SeirParameters {
    SeirParameters {
        // Guerra et al. (2017), Lancet Infect Dis, systematic review: "For
        // measles, R0 is often cited to be 12-18" and "R0 estimates vary more
        // than the often cited range of 12-18". A conventional range, NOT a
        // locally calibrated prior; the uniform density is a demonstration
        // modelling assumption. https://pubmed.ncbi.nlm.nih.gov/28757186/
        r0: R0::UniformPrior {
            min: 12.0,
            max: 18.0,
        },
        // CDC Pink Book ch. 13 (Measles): "Incubation period 11 to 12 days"
        // (exposure to prodrome); "exposure to rash onset averages 14 days
        // (range, 7 to 21 days)"; "transmissible from 4 days before through 4
        // days after rash onset". Latent (exposure to infectiousness) is taken
        // as 14 - 4 = 10 d, the low end of the spec's 10-12 d band; the 11-12 d
        // figure is incubation to symptoms, not to infectiousness.
        // https://www.cdc.gov/pinkbook/hcp/table-of-contents/chapter-13-measles.html
        latent_period_days: 10.0,
        // Same source: infectious 4 d before to 4 d after rash = 8 d. Treating
        // this window as an exponential mean is an explicit SEIR approximation.
        infectious_period_days: 8.0,
        // CDC: "One dose is 93% (range: 39% to 100%) effective at preventing
        // measles"; "Two doses ... are 97% (range: 67% to 100%) effective".
        // Point estimates, not sterilizing-immunity measurements. The all-or-none
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
