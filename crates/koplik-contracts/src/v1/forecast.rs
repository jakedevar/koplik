use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};

use super::fips::GeoId;
use super::mmwr::MmwrWeek;
use super::provenance::Provenances;

/// One quantile of a forecast predictive distribution.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ForecastQuantile {
    /// Quantile level in (0, 1), e.g. 0.05, 0.5, 0.95.
    #[schemars(extend("exclusiveMinimum" = 0, "exclusiveMaximum" = 1))]
    pub level: f64,
    /// Predicted weekly case count at that quantile (non-negative).
    #[schemars(range(min = 0))]
    pub value: f64,
}

/// Forecast of weekly cases for a geography and target week.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Forecast {
    pub geography: GeoId,
    /// Last week of data the forecast was allowed to use (the forecast date).
    pub origin_week: MmwrWeek,
    /// Week being predicted; strictly after `origin_week`.
    pub target_week: MmwrWeek,
    /// Quantiles with strictly increasing levels and non-decreasing values; never empty.
    #[schemars(length(min = 1))]
    pub quantiles: Vec<ForecastQuantile>,
    /// Seed of the stochastic run that produced the ensemble.
    pub seed: u64,
    /// Number of ensemble members behind the quantiles.
    pub run_count: u32,
    pub provenance: Provenances,
}

impl<'de> Deserialize<'de> for Forecast {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            geography: GeoId,
            origin_week: MmwrWeek,
            target_week: MmwrWeek,
            quantiles: Vec<ForecastQuantile>,
            seed: u64,
            run_count: u32,
            provenance: Provenances,
        }
        use serde::de::Error;
        let r = Raw::deserialize(d)?;
        if r.target_week <= r.origin_week {
            return Err(D::Error::custom("target_week must be after origin_week"));
        }
        if r.quantiles.is_empty() {
            return Err(D::Error::custom("quantiles must not be empty"));
        }
        for q in &r.quantiles {
            if !(q.level > 0.0 && q.level < 1.0) {
                return Err(D::Error::custom("quantile level must be in (0, 1)"));
            }
            if !(q.value.is_finite() && q.value >= 0.0) {
                return Err(D::Error::custom("quantile value must be finite and >= 0"));
            }
        }
        if r.quantiles
            .windows(2)
            .any(|w| !(w[0].level < w[1].level && w[0].value <= w[1].value))
        {
            return Err(D::Error::custom(
                "quantile levels must increase strictly and values must not decrease",
            ));
        }
        Ok(Self {
            geography: r.geography,
            origin_week: r.origin_week,
            target_week: r.target_week,
            quantiles: r.quantiles,
            seed: r.seed,
            run_count: r.run_count,
            provenance: r.provenance,
        })
    }
}
