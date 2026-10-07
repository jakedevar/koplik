use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};

use super::fips::{GeoId, GeoLevel};

/// A geography: FIPS key, level and a display name. The name is for display only; it is
/// never a key and may change spelling between sources.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Geography {
    pub id: GeoId,
    /// Must agree with the length of `id` (2 digits = state, 5 digits = county).
    pub level: GeoLevel,
    /// Display name, e.g. "Gaines County" (non-empty).
    pub name: String,
}

impl Geography {
    pub fn new(id: GeoId, name: impl Into<String>) -> Result<Self, String> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err("geography display name must not be empty".into());
        }
        Ok(Self {
            level: id.level(),
            id,
            name,
        })
    }
}

impl<'de> Deserialize<'de> for Geography {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            id: GeoId,
            level: GeoLevel,
            name: String,
        }
        let r = Raw::deserialize(d)?;
        if r.level != r.id.level() {
            return Err(serde::de::Error::custom(format!(
                "level {:?} does not match geography id {}",
                r.level, r.id
            )));
        }
        Geography::new(r.id, r.name).map_err(serde::de::Error::custom)
    }
}
