//! Koplik shared data contracts.
//!
//! Every crate and the web app build against these shapes. Versioning rule (see the
//! crate README): a released version is immutable; a shape change is a new version
//! module (`v9`, ...) with regenerated JSON Schema under `schema/`.

pub mod v1;
pub mod v2;
pub mod v3;
pub mod v4;
pub mod v5;
pub mod v6;
pub mod v7;
pub mod v8;

/// The newest contract version published by this crate.
pub const CONTRACT_VERSION: u32 = 8;
