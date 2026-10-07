/* Generated from koplik-contracts schema/v1. Run npm run generate:types. */

/**
 * A kindergarten MMR coverage value that can be explicitly missing. `reported` with
 * `coverage_pct: 0` is a real zero; `missing` is unknown. They are never interchangeable.
 */
export type CoverageValue =
  | {
      /**
       * Percent of kindergartners with the MMR series, 0 to 100.
       */
      coverage_pct: number;
      /**
       * Percent with any exemption, 0 to 100; `null` when the source does not report it.
       */
      exemption_pct?: number | null;
      status: "reported";
    }
  | {
      reason: MissingReason;
      status: "missing";
    };
/**
 * Why a weekly count is missing.
 */
export type MissingReason = "not_reported" | "suppressed" | "ambiguous";
/**
 * Geography key: state FIPS (2 digits) or county FIPS (5 digits), zero-padded
 */
export type GeoId = string;
/**
 * @minItems 1
 */
export type Provenances = [Provenance, ...Provenance[]];
/**
 * School year, e.g. 2023-24 (second part is the next year mod 100)
 */
export type SchoolYear = string;

/**
 * Kindergarten MMR vaccination coverage for a geography and school year.
 */
export interface KindergartenMmrCoverage {
  coverage: CoverageValue;
  geography: GeoId;
  /**
   * Non-empty description of the imputation method when `imputed`; `null` otherwise.
   */
  imputation_method?: string | null;
  /**
   * True when the reported value was filled in by a method rather than measured.
   * Never true for a missing value.
   */
  imputed: boolean;
  provenance: Provenances;
  school_year: SchoolYear;
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
