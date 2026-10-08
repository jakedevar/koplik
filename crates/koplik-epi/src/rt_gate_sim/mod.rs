//! Research-only simulator for the #1677 R_t infectiousness-gate study (step 2).
//!
//! Implements `thoughts/shared/research/1677-rt-gate-preregistration.md` (registration commit
//! `b5e499000f8175f3fda90511f5a2a06e951608da`). It lives inside `koplik-epi` only to reach the
//! crate-private portable samplers; nothing in the pipeline, the contracts or the wasm facade
//! uses it, and its output is synthetic simulation data, never published.

pub mod agg;
pub mod design;
pub mod generator;
pub mod report;
pub mod run;

#[cfg(test)]
mod tests;
