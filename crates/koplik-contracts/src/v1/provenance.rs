//! Provenance: every derived row traces back to snapshot hash, source URL and retrieval time.

use std::borrow::Cow;
use std::fmt;

use chrono::{DateTime, Utc};
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ProvenanceError {
    #[error("sha256 must be 64 lowercase hex characters, got {0:?}")]
    BadSha256(String),
    #[error("a row needs at least one provenance record")]
    Empty,
}

/// SHA-256 digest as 64 lowercase hex characters (the snapshot's content address).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct Sha256Hex(String);

impl Sha256Hex {
    pub fn new(hex: impl Into<String>) -> Result<Self, ProvenanceError> {
        let s = hex.into();
        if s.len() == 64
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            Ok(Self(s))
        } else {
            Err(ProvenanceError::BadSha256(s))
        }
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Sha256Hex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Sha256Hex {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::new(String::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

impl JsonSchema for Sha256Hex {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("Sha256Hex")
    }
    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "pattern": "^[0-9a-f]{64}$",
            "description": "SHA-256 digest, 64 lowercase hex characters"
        })
    }
}

/// Where a number came from: the immutable snapshot, its source URL and when it was retrieved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    /// Stable id of the source as listed in `SOURCES.md` (e.g. `cdc-measles-weekly`).
    pub source_id: String,
    /// URL the snapshot was retrieved from.
    pub url: String,
    /// Retrieval time, RFC 3339 in UTC (e.g. `2026-09-30T12:00:00Z`).
    pub retrieved_at: DateTime<Utc>,
    /// Content address of the raw snapshot bytes.
    pub sha256: Sha256Hex,
    /// Id of the source's licence or terms as listed in `SOURCES.md`.
    pub licence_id: String,
}

/// One or more provenance records (never empty).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Provenances(Vec<Provenance>);

impl Provenances {
    pub fn new(records: Vec<Provenance>) -> Result<Self, ProvenanceError> {
        if records.is_empty() {
            Err(ProvenanceError::Empty)
        } else {
            Ok(Self(records))
        }
    }
    pub fn one(record: Provenance) -> Self {
        Self(vec![record])
    }
    pub fn as_slice(&self) -> &[Provenance] {
        &self.0
    }
}

impl<'de> Deserialize<'de> for Provenances {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::new(Vec::<Provenance>::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

impl JsonSchema for Provenances {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("Provenances")
    }
    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let mut s = generator.subschema_for::<Vec<Provenance>>();
        s.insert("minItems".into(), 1.into());
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_validation() {
        assert!(Sha256Hex::new("a".repeat(64)).is_ok());
        for bad in [
            &"a".repeat(63),
            &"a".repeat(65),
            &"A".repeat(64),
            &"g".repeat(64),
            "",
        ] {
            assert!(Sha256Hex::new(bad.to_string()).is_err());
        }
        assert!(serde_json::from_str::<Sha256Hex>(&format!("\"{}\"", "F".repeat(64))).is_err());
    }

    #[test]
    fn provenances_non_empty() {
        assert!(serde_json::from_str::<Provenances>("[]").is_err());
    }
}
