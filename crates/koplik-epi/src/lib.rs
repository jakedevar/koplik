//! Koplik epidemic engine and inference. Pure: no I/O, no network, no clocks.
//!
//! Every transcendental on a path that affects output goes through `libm`, no `usize` enters
//! a random draw or hashed output, and iteration order is always deterministic (see the
//! portability rules in the spec's E4). Engine results are in-memory types, not wire
//! contracts. Module declarations and re-exports only live here.

pub mod backtest;
pub mod defaults;
pub mod ensemble;
pub mod fingerprint;
pub mod forecast;
mod gravity;
pub mod rt;
mod sampling;
pub mod seir;

pub use defaults::default_parameters;
pub use ensemble::{Band, DailySummary, Ensemble, derive_seed, simulate_ensemble};
pub use seir::{Compartments, EngineError, Step, Trajectory, simulate_member};
