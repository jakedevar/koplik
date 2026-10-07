/* Generated from koplik-contracts schema/v6. Run npm run generate:types. */

/**
 * Geography key: state FIPS (2 digits) or county FIPS (5 digits), zero-padded
 */
export type GeoId = string;
/**
 * Quality of an R_t estimate.
 */
export type RtStatus = "ok" | "insufficient_data";

export interface RtEstimateArtifact {
  contract_version: 6;
  provenance: Provenance[];
  rows: RtEstimate[];
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
 * Effective reproduction number estimate for a geography and MMWR week.
 */
export interface RtEstimate {
  geography: GeoId;
  /**
   * Credible-interval level in (0, 1), e.g. 0.9 for a 90% interval.
   */
  interval_level: number;
  /**
   * Lower credible bound; `null` iff `status` is `insufficient_data`.
   */
  lower?: number | null;
  /**
   * Posterior mean; `null` iff `status` is `insufficient_data`.
   */
  mean?: number | null;
  /**
   * @minItems 1
   */
  provenance: [number, ...number[]];
  /**
   * Recent week still subject to reporting delay. Independent of `status`: a row can be
   * provisional with an estimate, or provisional and `insufficient_data` at once.
   */
  provisional: boolean;
  status: RtStatus;
  /**
   * Upper credible bound; `null` iff `status` is `insufficient_data`.
   */
  upper?: number | null;
  week: MmwrWeek;
}
/**
 * An MMWR year and week number (`week` is `1..=52` or `1..=53`).
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
