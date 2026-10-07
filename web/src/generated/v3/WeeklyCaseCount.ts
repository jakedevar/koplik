/* Generated from koplik-contracts schema/v3. Run npm run generate:types. */

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
 * Newly reported cases in one MMWR week for one geography, under an explicit case definition.
 * (Cumulative-only sources are differenced by the connector; the row is always per week.)
 */
export interface WeeklyCaseCount {
  /**
   * What `cases` counts.
   */
  case_definition: "confirmed" | "confirmed_or_unknown_status";
  /**
   * The count (or its explicit absence). `reported` with `count: 0` is a real zero;
   * `missing` is unknown.
   */
  cases:
    | {
        count: number;
        status: "reported";
      }
    | {
        reason: MissingReason;
        status: "missing";
      };
  geography: GeoId;
  provenance: Provenances;
  week: MmwrWeek;
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
