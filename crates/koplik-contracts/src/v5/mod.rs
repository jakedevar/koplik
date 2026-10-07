//! Contract v5. Immutable once released: change a shape by adding `v6`, never by editing this.
//!
//! v5 = v4 plus one new type group, [`ForecastProvenance`] and the types it is made of: the
//! companion of a published set of v1 [`Forecast`] rows (#1465). It says which series the rows
//! forecast and which could not be forecast (and why), fixes the method, its parameters with
//! their citations, the seed and the origin week, hashes the input series, and carries the
//! measured skill of the backtest the method was scored on, with the scope of that backtest.
//! The forecast rows themselves keep their v1 shape. Every other type is re-exported from v4
//! (and so from v3, v2 and v1) unchanged: same Rust type, same JSON. The conventions listed in
//! `v1` apply to v5 as well.

mod forecast_provenance;

pub use super::v4::*;
pub use forecast_provenance::{
    BacktestSkill, FORECAST_PROVENANCE_VERSION, ForecastInput, ForecastProvenance, ForecastSeries,
    ForecastStatus, InsufficientReason, SkillByHorizon,
};

/// Version label of this module, used as the schema directory name.
pub const VERSION: &str = "v5";
