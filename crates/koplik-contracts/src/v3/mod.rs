//! Contract v3. Immutable once released: change a shape by adding `v4`, never by editing this.
//!
//! v3 = v2 plus one changed type: [`WeeklyCaseCount`] now says what its counts count
//! (`cases` plus a required [`CaseDefinition`]) instead of calling every count "confirmed".
//! Every other type is re-exported from v2 (and so from v1) unchanged: same Rust type, same
//! JSON, so rows of different versions can be mixed in one program. The conventions listed in
//! `v1` apply to v3 as well.

mod weekly_cases;

pub use super::v2::*;
pub use weekly_cases::{CaseDefinition, WeeklyCaseCount};

/// Version label of this module, used as the schema directory name.
pub const VERSION: &str = "v3";
