use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::check_range;
use super::fips::GeoId;
use super::provenance::Provenances;
use super::weekly_cases::MissingReason;

/// A school year such as "2023-24" (start year 2023). Serialized as that string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SchoolYear(u16);

impl SchoolYear {
    pub fn new(start_year: u16) -> Result<Self, String> {
        if (1900..=2199).contains(&start_year) {
            Ok(Self(start_year))
        } else {
            Err(format!(
                "school year start {start_year} outside 1900..=2199"
            ))
        }
    }
    pub fn start_year(self) -> u16 {
        self.0
    }
}

impl fmt::Display for SchoolYear {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{:02}", self.0, (self.0 + 1) % 100)
    }
}

impl FromStr for SchoolYear {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        let bad = || format!("school year must look like 2023-24, got {s:?}");
        let (a, b) = s.split_once('-').ok_or_else(bad)?;
        if a.len() != 4 || b.len() != 2 || !(a.bytes().chain(b.bytes())).all(|c| c.is_ascii_digit())
        {
            return Err(bad());
        }
        let start: u16 = a.parse().map_err(|_| bad())?;
        let end: u16 = b.parse().map_err(|_| bad())?;
        if (start + 1) % 100 != end {
            return Err(bad());
        }
        Self::new(start)
    }
}

impl Serialize for SchoolYear {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}
impl<'de> Deserialize<'de> for SchoolYear {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}
impl JsonSchema for SchoolYear {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("SchoolYear")
    }
    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "pattern": "^[0-9]{4}-[0-9]{2}$",
            "description": "School year, e.g. 2023-24 (second part is the next year mod 100)"
        })
    }
}

/// A kindergarten MMR coverage value that can be explicitly missing. `reported` with
/// `coverage_pct: 0` is a real zero; `missing` is unknown. They are never interchangeable.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum CoverageValue {
    Reported {
        /// Percent of kindergartners with the MMR series, 0 to 100.
        #[schemars(range(min = 0, max = 100))]
        coverage_pct: f64,
        /// Percent with any exemption, 0 to 100; `null` when the source does not report it.
        #[schemars(range(min = 0, max = 100))]
        exemption_pct: Option<f64>,
    },
    Missing {
        reason: MissingReason,
    },
}

impl CoverageValue {
    /// Coverage percent, or `None` when missing (never 0).
    pub fn coverage_pct(self) -> Option<f64> {
        match self {
            CoverageValue::Reported { coverage_pct, .. } => Some(coverage_pct),
            CoverageValue::Missing { .. } => None,
        }
    }
}

impl<'de> Deserialize<'de> for CoverageValue {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
        enum Raw {
            Reported {
                coverage_pct: f64,
                exemption_pct: Option<f64>,
            },
            Missing {
                reason: MissingReason,
            },
        }
        use serde::de::Error;
        Ok(match Raw::deserialize(d)? {
            Raw::Reported {
                coverage_pct,
                exemption_pct,
            } => {
                check_range("coverage_pct", coverage_pct, 0.0, 100.0).map_err(D::Error::custom)?;
                if let Some(e) = exemption_pct {
                    check_range("exemption_pct", e, 0.0, 100.0).map_err(D::Error::custom)?;
                }
                CoverageValue::Reported {
                    coverage_pct,
                    exemption_pct,
                }
            }
            Raw::Missing { reason } => CoverageValue::Missing { reason },
        })
    }
}

/// Kindergarten MMR vaccination coverage for a geography and school year.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct KindergartenMmrCoverage {
    pub geography: GeoId,
    pub school_year: SchoolYear,
    pub coverage: CoverageValue,
    /// True when the reported value was filled in by a method rather than measured.
    /// Never true for a missing value.
    pub imputed: bool,
    /// Non-empty description of the imputation method when `imputed`; `null` otherwise.
    pub imputation_method: Option<String>,
    pub provenance: Provenances,
}

impl<'de> Deserialize<'de> for KindergartenMmrCoverage {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            geography: GeoId,
            school_year: SchoolYear,
            coverage: CoverageValue,
            imputed: bool,
            imputation_method: Option<String>,
            provenance: Provenances,
        }
        use serde::de::Error;
        let r = Raw::deserialize(d)?;
        match (&r.imputed, &r.imputation_method) {
            (true, Some(m)) if !m.trim().is_empty() => {}
            (false, None) => {}
            _ => {
                return Err(D::Error::custom(
                    "imputation_method must be present and non-empty exactly when imputed is true",
                ));
            }
        }
        if r.imputed && matches!(r.coverage, CoverageValue::Missing { .. }) {
            return Err(D::Error::custom(
                "a missing coverage value cannot be flagged imputed",
            ));
        }
        Ok(Self {
            geography: r.geography,
            school_year: r.school_year,
            coverage: r.coverage,
            imputed: r.imputed,
            imputation_method: r.imputation_method,
            provenance: r.provenance,
        })
    }
}
