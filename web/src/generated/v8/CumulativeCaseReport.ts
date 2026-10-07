/* Generated from koplik-contracts schema/v8. Run npm run generate:types. */

/**
 * Why a report gives no usable cumulative count for a geography. None of these is zero and none
 * is estimated: a missing count is unknown.
 */
export type CumulativeMissingReason =
  "no_county_table" | "not_labelled_confirmed" | "not_listed_in_county_table" | "ambiguous";
/**
 * Geography key: state FIPS (2 digits) or county FIPS (5 digits), zero-padded
 */
export type GeoId = string;
/**
 * The snapshots this row was read from (the report) and any reference data used to key it.
 *
 * @minItems 1
 */
export type Provenances = [Provenance, ...Provenance[]];

/**
 * Cases counted since the outbreak began, as printed in the source's report of `report_date`,
 * for one geography, under an explicit case definition. Not a weekly count: it is never
 * differenced into weeks here, never interpolated between reports, and never carried
 * forward to a date the source did not report.
 */
export interface CumulativeCaseReport {
  /**
   * What the series counts. On a `missing` row it names the series the row belongs to; it does
   * not assert that the report's own wording matched (see `not_labelled_confirmed`).
   */
  case_definition: "confirmed" | "confirmed_or_unknown_status";
  /**
   * The cumulative count as the report prints it, or its explicit absence with the reason.
   */
  cases:
    | {
        count: number;
        status: "reported";
      }
    | {
        reason: CumulativeMissingReason;
        status: "missing";
      };
  geography: GeoId;
  provenance: Provenances;
  /**
   * The date the report printed. At most one row per geography and report date.
   */
  report_date: string;
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
