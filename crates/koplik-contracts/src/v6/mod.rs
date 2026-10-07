//! Contract v6. Immutable once released: change a shape by adding `v7`, never by editing this.
//!
//! v6 = v5 with one change to the forecast companion (#1503). The v5 companion carried one
//! backtest, of one series (the Texas DSHS 2025 outbreak total), and could only say of a series
//! that it was `backtested` (that series) or `not backtested; no measured skill`. The forecasts
//! the site publishes are of other series (the CDC NNDSS state counts), which were never scored.
//! v6 adds a second evaluation, [`SeriesBacktest`], which scores a whole family of series and says
//! for each one whether its skill is *measured* (enough scored targets from enough origin weeks, by
//! a floor fixed in advance) or has *insufficient data*, and what information basis the scoring
//! had ([`InformationBasis`]: real-time by report vintage, or pseudo-real-time with revised counts
//! truncated at each forecast date, never presented as real-time). A forecast series' `skill`
//! gains the values `measured` and `insufficient data for a measured skill`; the per-series
//! numbers sit in `series_backtest`, keyed by geography. Everything else in the companion keeps its
//! v5 shape and meaning. The forecast rows keep their v1 shape. Every other type is re-exported
//! from v5 unchanged: same Rust type, same JSON. The conventions listed in `v1` apply to v6 as
//! well.

mod forecast_provenance;

pub use super::v5::*;
pub use forecast_provenance::{
    FORECAST_PROVENANCE_VERSION, ForecastProvenance, ForecastSeries, InformationBasis,
    MeasuredScores, PooledScores, SeriesBacktest, SeriesBacktestEntry, SeriesSkill,
};

/// Version label of this module, used as the schema directory name.
pub const VERSION: &str = "v6";
