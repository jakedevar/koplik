/* Generated from koplik-contracts schema/v1. Run npm run generate:types. */

/**
 * Geography key: state FIPS (2 digits) or county FIPS (5 digits), zero-padded
 */
export type GeoId = string;
/**
 * Baseline kindergarten MMR coverage of a node, or an explicit missing value. A node with
 * missing coverage is only valid when a `coverage_overrides` entry supplies its value.
 */
export type BaselineCoverage =
  | {
      /**
       * Coverage percent, 0 to 100.
       */
      coverage_pct: number;
      /**
       * Non-empty description of the imputation method exactly when `imputed`; `null`
       * otherwise.
       */
      imputation_method?: string | null;
      /**
       * True when the value was imputed rather than measured.
       */
      imputed: boolean;
      provenance: Provenances;
      status: "reported";
    }
  | {
      reason: MissingReason;
      status: "missing";
    };
/**
 * @minItems 1
 */
export type Provenances = [Provenance, ...Provenance[]];
/**
 * Why a weekly count is missing.
 */
export type MissingReason = "not_reported" | "suppressed" | "ambiguous";
/**
 * Source records for the node's population and centroid (never empty). The baseline
 * coverage carries its own.
 *
 * @minItems 1
 */
export type Provenances1 = [Provenance, ...Provenance[]];

/**
 * Everything needed to reproduce an ensemble: same input, same seed, same trajectories.
 * It is self-contained: the engine needs nothing else (no data lookups).
 */
export interface ScenarioInput {
  /**
   * Coverage replacements (what-if slider); each names a node and appears at most once.
   */
  coverage_overrides: CoverageOverride[];
  /**
   * Simulated geographies: non-empty, unique, in canonical order (`GeoId` order: states
   * by FIPS code, then counties by FIPS code), so output order never depends on input order.
   *
   * @minItems 1
   */
  nodes: [ScenarioNode, ...ScenarioNode[]];
  parameters: SeirParameters;
  /**
   * Number of ensemble members (at least 1).
   */
  run_count: number;
  /**
   * RNG seed. Run `k` derives its own seed from this and `k`.
   */
  seed: number;
  start_week: MmwrWeek;
}
/**
 * Replace a geography's baseline kindergarten MMR coverage in a what-if run.
 */
export interface CoverageOverride {
  /**
   * Coverage percent, 0 to 100.
   */
  coverage_pct: number;
  geography: GeoId;
}
/**
 * One simulated geography with everything the engine needs about it.
 */
export interface ScenarioNode {
  baseline_coverage: BaselineCoverage;
  centroid: Centroid;
  id: GeoId;
  /**
   * Exposed (latent) individuals at the start week.
   */
  initial_exposed: number;
  /**
   * Infectious individuals at the start week. Exposed plus infectious must not exceed
   * the population.
   */
  initial_infectious: number;
  /**
   * Resident population (`> 0`).
   */
  population: number;
  provenance: Provenances1;
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
 * Location used for gravity coupling.
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
 * Full SEIR parameter set. Every value is configurable; none is hard-coded in the engine.
 */
export interface SeirParameters {
  /**
   * Coupling between geographies; `null` runs each geography in isolation.
   */
  gravity?: GravityParameters | null;
  /**
   * Simulated horizon in days.
   */
  horizon_days: number;
  /**
   * Mean infectious period in days.
   */
  infectious_period_days: number;
  /**
   * Mean latent (exposed, not yet infectious) period in days.
   */
  latent_period_days: number;
  /**
   * Protection from one MMR dose, in [0, 1].
   */
  mmr_effectiveness_one_dose: number;
  /**
   * Protection from two MMR doses, in [0, 1]; applied to kindergarten MMR coverage.
   */
  mmr_effectiveness_two_doses: number;
  /**
   * Basic reproduction number R0: fixed, or a prior sampled once per ensemble member.
   */
  r0:
    | {
        kind: "fixed";
        value: number;
      }
    | {
        kind: "uniform_prior";
        max: number;
        min: number;
      };
  /**
   * Tau-leap step length in days.
   */
  time_step_days: number;
}
/**
 * Gravity-model coupling between geographies: flow from i to j is
 * `scale * pop_i^origin_exponent * pop_j^destination_exponent / distance_km^distance_exponent`.
 */
export interface GravityParameters {
  destination_exponent: number;
  distance_exponent: number;
  origin_exponent: number;
  scale: number;
}
/**
 * Week the simulation starts.
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
