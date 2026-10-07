/* Generated from koplik-contracts schema/v1. Run npm run generate:types. */

/**
 * Geography key: state FIPS (2 digits) or county FIPS (5 digits), zero-padded
 */
export type GeoId = string;
/**
 * Source records for the name and centroid (never empty).
 *
 * @minItems 1
 */
export type Provenances = [Provenance, ...Provenance[]];

/**
 * A geography: FIPS key, level and a display name. The name is for display only; it is
 * never a key and may change spelling between sources.
 */
export interface Geography {
  /**
   * Centroid in WGS84 decimal degrees, when known (`null` otherwise).
   */
  centroid?: Centroid | null;
  id: GeoId;
  /**
   * Must agree with the length of `id` (2 digits = state, 5 digits = county).
   */
  level: "state" | "county";
  /**
   * Display name, e.g. "Gaines County" (non-empty).
   */
  name: string;
  provenance: Provenances;
}
/**
 * A point on the WGS84 ellipsoid in decimal degrees (e.g. a county's population or
 * geometric centroid; the source states which in its provenance).
 */
export interface Centroid {
  /**
   * Degrees north, -90 to 90.
   */
  latitude: number;
  /**
   * Degrees east, -180 to 180.
   */
  longitude: number;
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
