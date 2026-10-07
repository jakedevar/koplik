//! Contract v6: per-file provenance tables for published row artifacts.
//!
//! The expanded vocabulary remains v5. On the wire an artifact is an envelope with
//! `contract_version: 6`, a `provenance` table and `rows`; each row's `provenance`
//! is a non-empty array of zero-based u32 indices into that file's table. Entries
//! are deduplicated by the complete record, not by snapshot hash alone. Row and
//! provenance order (including repeated references) are preserved losslessly.
//! Empty row sets have an empty table. The scenario and forecast companions and
//! GeoJSON boundaries retain their released shapes.

use std::borrow::Cow;
use std::collections::BTreeMap;

use schemars::{JsonSchema, Schema, SchemaGenerator, schema_for};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::DeserializeOwned};
use serde_json::{Value, json};

pub use super::v5::*;
pub const VERSION: &str = "v6";

mod sealed {
    pub trait Row {}
    impl Row for super::Geography {}
    impl Row for super::WeeklyCaseCount {}
    impl Row for super::KindergartenMmrCoverage {}
    impl Row for super::RtEstimate {}
    impl Row for super::Forecast {}
}

/// A supported released row type with a top-level non-empty `provenance` array.
/// Sealed so a caller cannot accidentally publish a shape without that invariant.
pub trait PublicationRow: sealed::Row + Serialize + DeserializeOwned + JsonSchema {}
impl<T: sealed::Row + Serialize + DeserializeOwned + JsonSchema> PublicationRow for T {}

/// A v6 wire artifact, held as validated, expanded rows in Rust. Serialization
/// interns provenance; deserialization expands indices and runs the released row
/// validator. Thus no scientific or provenance validation is weakened by packing.
#[derive(Debug, Clone, PartialEq)]
pub struct RowArtifact<T: PublicationRow> {
    pub rows: Vec<T>,
}

pub type GeographyArtifact = RowArtifact<Geography>;
pub type WeeklyCaseCountArtifact = RowArtifact<WeeklyCaseCount>;
pub type KindergartenMmrCoverageArtifact = RowArtifact<KindergartenMmrCoverage>;
pub type RtEstimateArtifact = RowArtifact<RtEstimate>;
pub type ForecastArtifact = RowArtifact<Forecast>;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireArtifact {
    contract_version: u8,
    provenance: Vec<Provenance>,
    rows: Vec<Value>,
}

/// Pack existing row JSON while preserving the exact JSON of every non-provenance
/// field. The normal serde_json decimal parser may round a float by one ULP;
/// parsing then reserializing such a value would change what a browser displays.
/// RawValue avoids that second numerical representation, without changing the
/// parser or any upstream science. Released row validators still check the input.
pub fn pack_json<T: PublicationRow>(bytes: &[u8]) -> serde_json::Result<Vec<u8>> {
    use serde_json::value::{RawValue, to_raw_value};
    let _: Vec<T> = serde_json::from_slice(bytes)?;
    let mut rows: Vec<BTreeMap<String, Box<RawValue>>> = serde_json::from_slice(bytes)?;
    let mut table = Vec::<Provenance>::new();
    let mut lookup = BTreeMap::new();
    for row in &mut rows {
        // The typed validation above guarantees this non-empty array exists.
        let records: Vec<Provenance> = serde_json::from_str(row["provenance"].get())?;
        let mut indices = Vec::with_capacity(records.len());
        for record in records {
            let key = serde_json::to_string(&record)?;
            let index = match lookup.get(&key) {
                Some(index) => *index,
                None => {
                    let index = u32::try_from(table.len()).map_err(|_| {
                        <serde_json::Error as serde::ser::Error>::custom(
                            "provenance table exceeds u32",
                        )
                    })?;
                    table.push(record);
                    lookup.insert(key, index);
                    index
                }
            };
            indices.push(index);
        }
        row.insert("provenance".to_owned(), to_raw_value(&indices)?);
    }
    #[derive(Serialize)]
    struct RawArtifact {
        contract_version: u8,
        provenance: Vec<Provenance>,
        rows: Vec<BTreeMap<String, Box<RawValue>>>,
    }
    let mut packed = serde_json::to_vec(&RawArtifact {
        contract_version: 6,
        provenance: table,
        rows,
    })?;
    packed.push(b'\n');
    Ok(packed)
}

impl<T: PublicationRow> Serialize for RowArtifact<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::Error;
        let mut table = Vec::new();
        let mut lookup = BTreeMap::new();
        let mut rows = Vec::with_capacity(self.rows.len());
        for row in &self.rows {
            let mut row = serde_json::to_value(row).map_err(S::Error::custom)?;
            let records = row["provenance"]
                .as_array()
                .ok_or_else(|| S::Error::custom("row needs provenance"))?;
            if records.is_empty() {
                return Err(S::Error::custom("row needs non-empty provenance"));
            }
            let mut indices = Vec::with_capacity(records.len());
            for record in records {
                let key = serde_json::to_string(record).map_err(S::Error::custom)?;
                let index = match lookup.get(&key) {
                    Some(index) => *index,
                    None => {
                        let index = u32::try_from(table.len()).map_err(S::Error::custom)?;
                        table.push(
                            serde_json::from_value::<Provenance>(record.clone())
                                .map_err(S::Error::custom)?,
                        );
                        lookup.insert(key, index);
                        index
                    }
                };
                indices.push(json!(index));
            }
            row["provenance"] = Value::Array(indices);
            rows.push(row);
        }
        WireArtifact {
            contract_version: 6,
            provenance: table,
            rows,
        }
        .serialize(serializer)
    }
}

impl<'de, T: PublicationRow> Deserialize<'de> for RowArtifact<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let wire = WireArtifact::deserialize(deserializer)?;
        if wire.contract_version != 6 {
            return Err(D::Error::custom("row artifact contract_version must be 6"));
        }
        let mut rows = Vec::with_capacity(wire.rows.len());
        for mut row in wire.rows {
            let indices = row
                .get("provenance")
                .and_then(Value::as_array)
                .filter(|indices| !indices.is_empty())
                .ok_or_else(|| D::Error::custom("row needs non-empty provenance indices"))?;
            let mut records = Vec::with_capacity(indices.len());
            for index in indices {
                let index = index
                    .as_u64()
                    .and_then(|i| u32::try_from(i).ok())
                    .ok_or_else(|| D::Error::custom("provenance index must be a u32"))?;
                let record = wire.provenance.get(index as usize).ok_or_else(|| {
                    D::Error::custom("provenance index outside this file's table")
                })?;
                records.push(serde_json::to_value(record).map_err(D::Error::custom)?);
            }
            row["provenance"] = Value::Array(records);
            rows.push(serde_json::from_value(row).map_err(D::Error::custom)?);
        }
        Ok(Self { rows })
    }
}

impl<T: PublicationRow> JsonSchema for RowArtifact<T> {
    fn schema_name() -> Cow<'static, str> {
        Cow::Owned(format!("{}Artifact", T::schema_name()))
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        // Reuse the released row schema, replacing only its provenance field.
        // Lift its definitions to the envelope root so every local $ref resolves.
        let mut row = serde_json::to_value(schema_for!(T)).expect("row schema serializes");
        let definitions = row
            .as_object_mut()
            .unwrap()
            .remove("$defs")
            .unwrap_or(json!({}));
        row.as_object_mut().unwrap().remove("$schema");
        row["properties"]["provenance"] = json!({
            "type": "array", "minItems": 1,
            "items": { "type": "integer", "format": "uint32", "minimum": 0, "maximum": u32::MAX }
        });
        let provenance_schema = serde_json::to_value(schema_for!(Provenance)).unwrap();
        let mut definitions = definitions.as_object().unwrap().clone();
        if let Some(provenance_defs) = provenance_schema.get("$defs").and_then(Value::as_object) {
            definitions.extend(provenance_defs.clone());
        }
        let mut provenance = provenance_schema.as_object().unwrap().clone();
        provenance.remove("$schema");
        provenance.remove("$defs");
        Schema::try_from(json!({
            "type": "object", "additionalProperties": false,
            "required": ["contract_version", "provenance", "rows"],
            "properties": {
                "contract_version": { "const": 6, "type": "integer" },
                "provenance": { "type": "array", "items": provenance },
                "rows": { "type": "array", "items": row }
            },
            "$defs": definitions
        }))
        .expect("artifact schema is an object")
    }
}
