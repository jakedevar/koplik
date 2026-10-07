use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};

use super::check_range;
use super::fips::{GeoId, GeoLevel};
use super::provenance::Provenances;

/// A point on the WGS84 ellipsoid in decimal degrees (e.g. a county's population or
/// geometric centroid; the source states which in its provenance).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Centroid {
    /// Degrees north, -90 to 90.
    #[schemars(range(min = -90, max = 90))]
    pub latitude: f64,
    /// Degrees east, -180 to 180.
    #[schemars(range(min = -180, max = 180))]
    pub longitude: f64,
}

impl Centroid {
    pub fn new(latitude: f64, longitude: f64) -> Result<Self, String> {
        check_range("latitude", latitude, -90.0, 90.0)?;
        check_range("longitude", longitude, -180.0, 180.0)?;
        Ok(Self {
            latitude,
            longitude,
        })
    }
}

impl<'de> Deserialize<'de> for Centroid {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            latitude: f64,
            longitude: f64,
        }
        let r = Raw::deserialize(d)?;
        Centroid::new(r.latitude, r.longitude).map_err(serde::de::Error::custom)
    }
}

/// A geography: FIPS key, level and a display name. The name is for display only; it is
/// never a key and may change spelling between sources.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Geography {
    pub id: GeoId,
    /// Must agree with the length of `id` (2 digits = state, 5 digits = county).
    pub level: GeoLevel,
    /// Display name, e.g. "Gaines County" (non-empty).
    pub name: String,
    /// Centroid in WGS84 decimal degrees, when known (`null` otherwise).
    pub centroid: Option<Centroid>,
    /// Source records for the name and centroid (never empty).
    pub provenance: Provenances,
}

impl Geography {
    pub fn new(
        id: GeoId,
        name: impl Into<String>,
        provenance: Provenances,
    ) -> Result<Self, String> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err("geography display name must not be empty".into());
        }
        Ok(Self {
            level: id.level(),
            id,
            name,
            centroid: None,
            provenance,
        })
    }

    pub fn with_centroid(mut self, centroid: Centroid) -> Self {
        self.centroid = Some(centroid);
        self
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
            centroid: Option<Centroid>,
            provenance: Provenances,
        }
        let r = Raw::deserialize(d)?;
        if r.level != r.id.level() {
            return Err(serde::de::Error::custom(format!(
                "level {:?} does not match geography id {}",
                r.level, r.id
            )));
        }
        let mut g = Geography::new(r.id, r.name, r.provenance).map_err(serde::de::Error::custom)?;
        g.centroid = r.centroid;
        Ok(g)
    }
}
