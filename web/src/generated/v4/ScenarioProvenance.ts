/* Generated from koplik-contracts schema/v4. Run npm run generate:types. */

/**
 * Geography key: state FIPS (2 digits) or county FIPS (5 digits), zero-padded
 */
export type GeoId = string;

/**
 * Companion of a what-if [`ScenarioInput`]: where its inputs came from, the seeding stated as
 * an assumption, and every parameter's citation. A scenario is only published beside a
 * companion for which [`ScenarioProvenance::check_against`] holds.
 */
export interface ScenarioProvenance {
  /**
   * Always [`SCENARIO_PROVENANCE_VERSION`].
   */
  contract_version: number;
  /**
   * Counties the neighbourhood rule would have simulated but could not.
   */
  excluded_nodes: ExcludedNode[];
  /**
   * Why the node set is what it is (non-empty).
   */
  neighbourhood_note: string;
  /**
   * The simulated nodes, in the scenario's canonical order.
   */
  nodes: NodeInputs[];
  /**
   * One entry per `SeirParameters` field, in field order (non-empty, unique).
   */
  parameters: ParameterProvenance[];
  run_count: number;
  /**
   * Artifact name, e.g. `gaines-2025`.
   */
  scenario: string;
  /**
   * The scenario's RNG seed as decimal text (a JavaScript number would round a large u64).
   */
  seed: string;
  seeding: SeedingAssumption;
  /**
   * Plain-words statement of what the scenario is and is not (non-empty).
   */
  statement: string;
}
/**
 * A county the neighbourhood rule would have simulated but could not, and why.
 */
export interface ExcludedNode {
  geography: GeoId;
  name: string;
  reason: string;
}
/**
 * What a node's inputs are, beyond the source records the scenario already carries.
 */
export interface NodeInputs {
  centroid_basis: string;
  coverage_basis: string;
  /**
   * School year of the baseline kindergarten MMR coverage, e.g. `2023-24`.
   */
  coverage_school_year: string;
  geography: GeoId;
  name: string;
  population: number;
  population_basis: string;
}
/**
 * One model parameter exactly as the scenario ran it, with its citation (#1400).
 */
export interface ParameterProvenance {
  /**
   * What the source supports and what it does not (non-empty).
   */
  note: string;
  /**
   * The `SeirParameters` field name.
   */
  parameter: string;
  /**
   * The published source (non-empty).
   */
  source: string;
  /**
   * Where to read it, when the source is a public page.
   */
  url?: string | null;
  /**
   * The value the scenario ran with, as it appears in the scenario's `parameters`.
   */
  value: {
    [k: string]: unknown;
  };
}
/**
 * How the scenario is seeded: a stated assumption, never data. The scenario is a
 * hypothetical introduction, not a reconstruction of an observed outbreak.
 */
export interface SeedingAssumption {
  /**
   * The assumption in words (non-empty).
   */
  assumption: string;
  /**
   * Geography key: state FIPS (2 digits) or county FIPS (5 digits), zero-padded
   */
  geography: string;
  /**
   * Exposed (latent) people at the start week.
   */
  initial_exposed: number;
  /**
   * Infectious people at the start week.
   */
  initial_infectious: number;
  /**
   * What the seeding cannot capture, stated to the reader (non-empty).
   */
  limitation: string;
  start_week: MmwrWeek;
  /**
   * Why this start week, and what it does and does not mean (non-empty).
   */
  start_week_basis: string;
}
/**
 * The week the simulation starts, stated as an assumption (not derived from any case
 * report).
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
