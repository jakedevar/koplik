//! Koplik ingest: source connectors and the content-addressed snapshot store.
//!
//! This is the only crate that touches the network, and only through [`http::HttpClient`]
//! (injected, so tests run offline against fixtures). Flow: [`polite::PoliteFetcher`] ->
//! [`store::SnapshotStore`] (raw bytes under their SHA-256 plus an append-only retrieval log)
//! -> a connector parser (e.g. [`cdc`]) that turns stored bytes into contracts v2 rows (CDC
//! weekly cases carry an explicit case definition), each carrying a `Provenance` (the unchanged
//! v1 type: snapshot hash, source URL, retrieval time, licence id).

pub mod cdc;
pub mod error;
pub mod http;
pub mod jurisdictions;
pub mod polite;
pub mod robots;
pub mod source;
pub mod store;

pub use error::{IngestError, Result};
