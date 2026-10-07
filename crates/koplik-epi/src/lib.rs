//! Pure epidemic calculations. Engine results are in-memory types, not wire contracts.

pub mod defaults;
pub mod ensemble;
pub mod fingerprint;
mod gravity;
mod sampling;
pub mod seir;

pub use defaults::default_parameters;
pub use ensemble::{Band, DailySummary, Ensemble, derive_seed, simulate_ensemble};
pub use seir::{Compartments, EngineError, Step, Trajectory, simulate_member};
