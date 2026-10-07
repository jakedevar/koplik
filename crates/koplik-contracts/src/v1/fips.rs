//! FIPS geography keys. Serialized as zero-padded strings ("04", "48165").
//!
//! Source: FIPS 5-2 (state) and FIPS 6-4 (county); a county code is the 2-digit state code
//! followed by a 3-digit county code, so `CountyFips::state()` is its prefix.

use std::fmt;
use std::str::FromStr;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::borrow::Cow;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FipsError {
    #[error(
        "FIPS code must be ASCII digits only and zero-padded to {expected} digits, got {got:?}"
    )]
    BadFormat { expected: &'static str, got: String },
    #[error("state FIPS code must be in 01..=99, got {0}")]
    StateOutOfRange(u8),
    #[error("county FIPS code must have a non-zero 3-digit county part, got {0:05}")]
    CountyPartZero(u32),
    #[error("county FIPS {county:05} does not belong to state {state:02}")]
    StateMismatch { state: u8, county: u32 },
}

fn parse_digits(s: &str, len: usize, expected: &'static str) -> Result<u32, FipsError> {
    if s.len() != len || !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(FipsError::BadFormat {
            expected,
            got: s.to_owned(),
        });
    }
    // At most 5 ASCII digits: fits u32 and the parse cannot fail.
    s.parse::<u32>().map_err(|_| FipsError::BadFormat {
        expected,
        got: s.to_owned(),
    })
}

/// Level of a geography.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum GeoLevel {
    State,
    County,
}

/// 2-digit state FIPS code, `01..=99`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StateFips(u8);

impl StateFips {
    pub fn new(code: u8) -> Result<Self, FipsError> {
        if (1..=99).contains(&code) {
            Ok(Self(code))
        } else {
            Err(FipsError::StateOutOfRange(code))
        }
    }
    pub fn code(self) -> u8 {
        self.0
    }
}

impl fmt::Display for StateFips {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02}", self.0)
    }
}

impl FromStr for StateFips {
    type Err = FipsError;
    fn from_str(s: &str) -> Result<Self, FipsError> {
        let n = parse_digits(s, 2, "2")?;
        Self::new(n as u8)
    }
}

/// 5-digit county FIPS code: state prefix plus a non-zero 3-digit county part.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CountyFips(u32);

impl CountyFips {
    /// Build from the full 5-digit numeric code (e.g. `48165`).
    pub fn new(code: u32) -> Result<Self, FipsError> {
        let state = code / 1000;
        if state == 0 || state > 99 {
            return Err(FipsError::StateOutOfRange(state.min(255) as u8));
        }
        if code.is_multiple_of(1000) {
            return Err(FipsError::CountyPartZero(code));
        }
        Ok(Self(code))
    }
    /// Build from a state and its 3-digit county part (`1..=999`).
    pub fn from_parts(state: StateFips, county: u16) -> Result<Self, FipsError> {
        if county == 0 || county > 999 {
            return Err(FipsError::CountyPartZero(u32::from(state.code()) * 1000));
        }
        Self::new(u32::from(state.code()) * 1000 + u32::from(county))
    }
    /// Build checking that the code belongs to `state`.
    pub fn new_in_state(state: StateFips, code: u32) -> Result<Self, FipsError> {
        let c = Self::new(code)?;
        if c.state() == state {
            Ok(c)
        } else {
            Err(FipsError::StateMismatch {
                state: state.code(),
                county: code,
            })
        }
    }
    pub fn code(self) -> u32 {
        self.0
    }
    /// The state this county belongs to (the code's 2-digit prefix).
    pub fn state(self) -> StateFips {
        StateFips((self.0 / 1000) as u8)
    }
}

impl fmt::Display for CountyFips {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:05}", self.0)
    }
}

impl FromStr for CountyFips {
    type Err = FipsError;
    fn from_str(s: &str) -> Result<Self, FipsError> {
        Self::new(parse_digits(s, 5, "5")?)
    }
}

/// A geography key: a state or a county. Serialized as its zero-padded FIPS string
/// ("48" or "48165"); the string length tells the level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GeoId {
    State(StateFips),
    County(CountyFips),
}

impl GeoId {
    pub fn level(self) -> GeoLevel {
        match self {
            GeoId::State(_) => GeoLevel::State,
            GeoId::County(_) => GeoLevel::County,
        }
    }
    /// The state itself, or the state a county belongs to.
    pub fn state(self) -> StateFips {
        match self {
            GeoId::State(s) => s,
            GeoId::County(c) => c.state(),
        }
    }
}

impl fmt::Display for GeoId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GeoId::State(s) => s.fmt(f),
            GeoId::County(c) => c.fmt(f),
        }
    }
}

impl FromStr for GeoId {
    type Err = FipsError;
    fn from_str(s: &str) -> Result<Self, FipsError> {
        match s.len() {
            2 => s.parse().map(GeoId::State),
            5 => s.parse().map(GeoId::County),
            _ => Err(FipsError::BadFormat {
                expected: "2 (state) or 5 (county)",
                got: s.to_owned(),
            }),
        }
    }
}

macro_rules! string_serde {
    ($ty:ty, $pattern:expr, $desc:expr, $name:expr) => {
        impl Serialize for $ty {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.collect_str(self)
            }
        }
        impl<'de> Deserialize<'de> for $ty {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let s = String::deserialize(d)?;
                s.parse().map_err(de::Error::custom)
            }
        }
        impl JsonSchema for $ty {
            fn schema_name() -> Cow<'static, str> {
                Cow::Borrowed($name)
            }
            fn json_schema(_: &mut SchemaGenerator) -> Schema {
                json_schema!({ "type": "string", "pattern": $pattern, "description": $desc })
            }
        }
    };
}

string_serde!(
    StateFips,
    "^(0[1-9]|[1-9][0-9])$",
    "2-digit zero-padded state FIPS code",
    "StateFips"
);
string_serde!(
    CountyFips,
    "^(0[1-9]|[1-9][0-9])(00[1-9]|0[1-9][0-9]|[1-9][0-9]{2})$",
    "5-digit county FIPS code: 2-digit state prefix plus a non-zero 3-digit county part",
    "CountyFips"
);
string_serde!(
    GeoId,
    "^((0[1-9]|[1-9][0-9])|(0[1-9]|[1-9][0-9])(00[1-9]|0[1-9][0-9]|[1-9][0-9]{2}))$",
    "Geography key: state FIPS (2 digits) or county FIPS (5 digits), zero-padded",
    "GeoId"
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_padding() {
        let s: StateFips = "04".parse().unwrap();
        assert_eq!(serde_json::to_string(&s).unwrap(), "\"04\"");
        let c: CountyFips = "04013".parse().unwrap();
        assert_eq!(serde_json::to_string(&c).unwrap(), "\"04013\"");
        assert_eq!(c.state(), s);
        let g: GeoId = serde_json::from_str("\"48165\"").unwrap();
        assert_eq!(g.level(), GeoLevel::County);
        assert_eq!(g.state().to_string(), "48");
    }

    #[test]
    fn rejects_invalid() {
        for bad in ["4", "004", "00", "ab", " 4", "+4", "1.", "-1"] {
            assert!(bad.parse::<StateFips>().is_err(), "{bad}");
        }
        for bad in ["4813", "048165", "48000", "00001", "4816a", "48 65"] {
            assert!(bad.parse::<CountyFips>().is_err(), "{bad}");
        }
        assert!(serde_json::from_str::<GeoId>("\"481\"").is_err());
        assert!(serde_json::from_str::<GeoId>("48").is_err());
        assert!(StateFips::new(0).is_err());
        assert!(CountyFips::new_in_state(StateFips::new(4).unwrap(), 48165).is_err());
    }
}
