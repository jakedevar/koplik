//! Effective reproduction number `R_t` by the renewal equation (Cori et al. 2013).
//!
//! Method (Cori A, Ferguson NM, Fraser C, Cauchemez S. *A new framework and software to
//! estimate time-varying reproduction numbers during epidemics.* Am J Epidemiol
//! 2013;178(9):1505-1512, doi:10.1093/aje/kwt133; reference implementation: the EpiEstim R
//! package, `estimate_R.R`, v3.0.0): with incidence `I_s` on a regular grid and a
//! discretised serial interval `w_k` (`w_0 = 0`), the overall infectivity is
//! `Λ_s = Σ_{k≥1} w_k I_{s-k}`. For a window of `τ` steps ending at `t` and a gamma prior on
//! `R` with shape `a` and scale `b`, the posterior is
//! `Gamma(shape = a + Σ_{s∈window} I_s, scale = 1 / (1/b + Σ_{s∈window} Λ_s))`, and the
//! credible intervals are its equal-tailed quantiles.
//!
//! Honesty rules, each enforced in [`estimate_series`]:
//! - A missing count is unknown, never zero: a window or a look-back that touches a missing
//!   step yields `insufficient_data`.
//! - Below the minimum-count threshold the output is `insufficient_data`, never an estimate.
//! - When the window's overall infectivity is zero (no infectors in the data), `R` is
//!   undefined and the output is `insufficient_data`, not a prior-driven number.
//! - Steps before the start of the series are unknown by default
//!   ([`BeforeSeries::Unknown`]); assuming them zero is an explicit, documented choice.
//!
//! Weekly data: [`SerialInterval::discretize_weekly`] maps the continuous serial interval to
//! the MMWR-week grid (method documented there), and [`estimate_weekly`] turns v1
//! `WeeklyCaseCount` rows into v1 `RtEstimate` rows with the most recent weeks flagged
//! provisional. Nothing here is stochastic: the same inputs give bit-identical output.

mod estimate;
mod gamma;
mod serial_interval;
mod weekly;

use koplik_contracts::v1::{GeoId, MmwrError, MmwrWeek, ProvenanceError};

pub use estimate::{
    BeforeSeries, CredibleInterval, InsufficientReason, RenewalConfig, StepEstimate,
    estimate_series,
};
pub use gamma::{GammaDist, regularized_lower_gamma};
pub use serial_interval::{DiscreteSerialInterval, SerialInterval};
pub use weekly::{RtConfig, estimate_weekly};

/// Errors from the R_t module. Data problems are reported, never silently repaired.
#[derive(Debug, thiserror::Error)]
pub enum RtError {
    #[error("invalid R_t configuration: {0}")]
    Config(String),
    #[error("two weekly counts for {geography} in {week}")]
    DuplicateWeek { geography: GeoId, week: MmwrWeek },
    #[error("series too long: {0} steps")]
    SeriesTooLong(u64),
    #[error(transparent)]
    Mmwr(#[from] MmwrError),
    #[error(transparent)]
    Provenance(#[from] ProvenanceError),
}
