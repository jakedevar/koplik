//! Contract v4. Immutable once released: change a shape by adding `v5`, never by editing this.
//!
//! v4 = v3 plus one new type, [`ScenarioProvenance`]: the companion of a what-if
//! [`ScenarioInput`] (#1400, #1455). It says where the scenario's inputs came from, states the
//! seeding as the assumption it is, and cites every model parameter to its published source.
//! The scenario input itself keeps its v1 shape. Every other type is re-exported from v3 (and
//! so from v2 and v1) unchanged: same Rust type, same JSON. The conventions listed in `v1`
//! apply to v4 as well.

mod scenario_provenance;

pub use super::v3::*;
pub use scenario_provenance::{
    ExcludedNode, NodeInputs, ParameterProvenance, SCENARIO_PROVENANCE_VERSION, ScenarioProvenance,
    SeedingAssumption,
};

/// Version label of this module, used as the schema directory name.
pub const VERSION: &str = "v4";
