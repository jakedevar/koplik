/* Generated from koplik-contracts schema/v2. Run npm run generate:types. */

/**
 * Geography key: state FIPS (2 digits) or county FIPS (5 digits), zero-padded
 */
export type GeoId = string;

export interface EnsembleResult {
  contract_version: number;
  /**
   * Ascending day, then canonical GeoId. Includes day 0.
   */
  daily: SimulationDay[];
  /**
   * Member 0 full-trajectory fingerprint, NOT a hash of the median or ensemble.
   * Individual member fingerprints are also retained below.
   */
  fingerprint: string;
  /**
   * Ascending member index; carries sampled R0 and derived seed for every run.
   */
  members: EnsembleMember[];
  parameters: SeirParameters;
  scenario_json: string;
  seed: string;
}
export interface SimulationDay {
  cumulative_infections: SimulationBand;
  day: number;
  exposed: SimulationBand1;
  geography: GeoId;
  infectious: SimulationBand1;
  new_exposures: SimulationBand2;
  recovered: SimulationBand3;
  susceptible: SimulationBand1;
}
/**
 * S(0)-S(day)+E(0)+I(0); excludes vaccine immunity.
 */
export interface SimulationBand {
  lower_50: number;
  lower_90: number;
  median: number;
  upper_50: number;
  upper_90: number;
}
/**
 * Equal-tail predictive bands, Hyndman-Fan type 7; not median confidence intervals.
 */
export interface SimulationBand1 {
  lower_50: number;
  lower_90: number;
  median: number;
  upper_50: number;
  upper_90: number;
}
/**
 * Equal-tail predictive bands, Hyndman-Fan type 7; not median confidence intervals.
 */
export interface SimulationBand2 {
  lower_50: number;
  lower_90: number;
  median: number;
  upper_50: number;
  upper_90: number;
}
/**
 * Equal-tail predictive bands, Hyndman-Fan type 7; not median confidence intervals.
 */
export interface SimulationBand3 {
  lower_50: number;
  lower_90: number;
  median: number;
  upper_50: number;
  upper_90: number;
}
export interface EnsembleMember {
  /**
   * Portable ChaCha8 initialization bytes from derive_seed(base_seed, member).
   *
   * @minItems 32
   * @maxItems 32
   */
  derived_seed: [
    number,
    number,
    number,
    number,
    number,
    number,
    number,
    number,
    number,
    number,
    number,
    number,
    number,
    number,
    number,
    number,
    number,
    number,
    number,
    number,
    number,
    number,
    number,
    number,
    number,
    number,
    number,
    number,
    number,
    number,
    number,
    number
  ];
  /**
   * SHA256 of all S/E/I/R counts, including day 0 and every leap, as LE f64.
   */
  fingerprint: string;
  member: number;
  r0: number;
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
