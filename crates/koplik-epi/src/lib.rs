//! Koplik epidemic engine and inference. Pure: no I/O, no network, no clocks.
//!
//! Every transcendental on a path that affects output goes through `libm`, nothing uses
//! `usize` in a computed value, and iteration order is always deterministic (see the
//! portability rules in the spec's E4). Module declarations and re-exports only live here.

pub mod rt;
