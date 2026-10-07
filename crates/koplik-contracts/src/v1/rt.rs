use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};

use super::check_range;
use super::fips::GeoId;
use super::mmwr::MmwrWeek;
use super::provenance::Provenances;

/// Quality of an R_t estimate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RtStatus {
    /// Final estimate.
    Ok,
    /// Recent weeks still subject to reporting delay.
    Provisional,
    /// Below the minimum-count threshold: no estimate is published (mean and bounds are null).
    InsufficientData,
}

/// Effective reproduction number estimate for a geography and MMWR week.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RtEstimate {
    pub geography: GeoId,
    pub week: MmwrWeek,
    pub status: RtStatus,
    /// Posterior mean; `null` iff `status` is `insufficient_data`.
    pub mean: Option<f64>,
    /// Lower credible bound; `null` iff `status` is `insufficient_data`.
    pub lower: Option<f64>,
    /// Upper credible bound; `null` iff `status` is `insufficient_data`.
    pub upper: Option<f64>,
    /// Credible-interval level in (0, 1), e.g. 0.9 for a 90% interval.
    #[schemars(range(min = 0, max = 1))]
    pub interval_level: f64,
    pub provenance: Provenances,
}

impl<'de> Deserialize<'de> for RtEstimate {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            geography: GeoId,
            week: MmwrWeek,
            status: RtStatus,
            mean: Option<f64>,
            lower: Option<f64>,
            upper: Option<f64>,
            interval_level: f64,
            provenance: Provenances,
        }
        use serde::de::Error;
        let r = Raw::deserialize(d)?;
        if !(r.interval_level > 0.0 && r.interval_level < 1.0) {
            return Err(D::Error::custom("interval_level must be in (0, 1)"));
        }
        match (r.status, r.mean, r.lower, r.upper) {
            (RtStatus::InsufficientData, None, None, None) => {}
            (RtStatus::InsufficientData, ..) => {
                return Err(D::Error::custom(
                    "insufficient_data must not carry an estimate",
                ));
            }
            (_, Some(m), Some(lo), Some(hi)) => {
                check_range("mean", m, 0.0, f64::MAX).map_err(D::Error::custom)?;
                check_range("lower", lo, 0.0, f64::MAX).map_err(D::Error::custom)?;
                check_range("upper", hi, 0.0, f64::MAX).map_err(D::Error::custom)?;
                if lo > hi {
                    return Err(D::Error::custom("lower must not exceed upper"));
                }
            }
            _ => {
                return Err(D::Error::custom(
                    "ok/provisional needs mean, lower and upper",
                ));
            }
        }
        Ok(Self {
            geography: r.geography,
            week: r.week,
            status: r.status,
            mean: r.mean,
            lower: r.lower,
            upper: r.upper,
            interval_level: r.interval_level,
            provenance: r.provenance,
        })
    }
}
