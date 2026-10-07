/* Generated from koplik-contracts schema/v1. Run npm run generate:types. */

/**
 * Geography key: state FIPS (2 digits) or county FIPS (5 digits), zero-padded
 */
export type GeoId = string;
/**
 * @minItems 1
 */
export type Provenances = [Provenance, ...Provenance[]];

/**
 * Forecast of weekly cases for a geography and target week.
 */
export interface Forecast {
  geography: GeoId;
  origin_week: MmwrWeek;
  provenance: Provenances;
  /**
   * Quantiles with strictly increasing levels and non-decreasing values; never empty.
   *
   * @minItems 1
   */
  quantiles: [ForecastQuantile, ...ForecastQuantile[]];
  /**
   * Number of ensemble members behind the quantiles.
   */
  run_count: number;
  /**
   * Seed of the stochastic run that produced the ensemble.
   */
  seed: number;
  target_week: MmwrWeek1;
}
/**
 * Last week of data the forecast was allowed to use (the forecast date).
 */
export interface MmwrWeek {
  /**
   * Week of the MMWR year; 1 to 52, or 53 in a 53-week year.
   */
  week: number;
  /**
   * MMWR year (not always the calendar year of every day in the week).
   */
  year: number;
}
/**
 * Where a number came from: the immutable snapshot, its source URL and when it was retrieved.
 */
export interface Provenance {
  /**
   * Id of the source's licence or terms as listed in `SOURCES.md`.
   */
  licence_id: string;
  /**
   * Retrieval time, RFC 3339 in UTC (e.g. `2026-09-30T12:00:00Z`).
   */
  retrieved_at: string;
  /**
   * Content address of the raw snapshot bytes.
   */
  sha256: string;
  /**
   * Stable id of the source as listed in `SOURCES.md` (e.g. `cdc-measles-weekly`).
   */
  source_id: string;
  /**
   * URL the snapshot was retrieved from.
   */
  url: string;
}
/**
 * One quantile of a forecast predictive distribution.
 */
export interface ForecastQuantile {
  /**
   * Quantile level in (0, 1), e.g. 0.05, 0.5, 0.95.
   */
  level: number;
  /**
   * Predicted weekly case count at that quantile (non-negative).
   */
  value: number;
}
/**
 * Week being predicted; strictly after `origin_week`.
 */
export interface MmwrWeek1 {
  /**
   * Week of the MMWR year; 1 to 52, or 53 in a 53-week year.
   */
  week: number;
  /**
   * MMWR year (not always the calendar year of every day in the week).
   */
  year: number;
}
