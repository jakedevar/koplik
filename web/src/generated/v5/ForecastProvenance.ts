/* Generated from koplik-contracts schema/v5. Run npm run generate:types. */

/**
 * Geography key: state FIPS (2 digits) or county FIPS (5 digits), zero-padded
 */
export type GeoId = string;
/**
 * Why a series could not be forecast (the renewal estimator's reason at the origin week).
 */
export type InsufficientReason = "incomplete_window" | "missing_count" | "below_threshold" | "no_infectivity";
/**
 * Whether a series was forecast.
 */
export type ForecastStatus = "forecast" | "insufficient_data";

/**
 * Companion of a published set of v1 [`Forecast`] rows. Rows are only published beside a
 * companion for which [`ForecastProvenance::check_against`] holds.
 */
export interface ForecastProvenance {
  /**
   * Name of the series artifact that was forecast, e.g. `weekly-cases` (non-empty).
   */
  artifact: string;
  /**
   * The backtest skill, when a committed report exists for exactly this configuration.
   */
  backtest?: BacktestSkill | null;
  /**
   * Always [`FORECAST_PROVENANCE_VERSION`].
   */
  contract_version: number;
  /**
   * Weeks forecast ahead of the origin week.
   */
  horizon_weeks: number;
  input: ForecastInput;
  latest_data_week: MmwrWeek;
  /**
   * Quantile levels of every row, strictly increasing in (0, 1).
   */
  levels: number[];
  /**
   * The method and its citation (non-empty).
   */
  method: string;
  /**
   * How the origin week was chosen from the data (non-empty).
   */
  origin_rule: string;
  origin_week: MmwrWeek1;
  /**
   * One entry per configuration value, with the value the forecast ran with and its
   * citation (non-empty, unique).
   */
  parameters: ParameterProvenance[];
  /**
   * Ensemble members behind every row's quantiles.
   */
  run_count: number;
  /**
   * What the skill does and does not say about the series above (non-empty).
   */
  scope_note: string;
  /**
   * Base seed of every series' ensemble (members derive from it, the geography and the
   * member index).
   */
  seed: number;
  /**
   * Every series considered, once each, in geography order.
   */
  series: ForecastSeries[];
  /**
   * What the forecast is and is not, in plain words (non-empty).
   */
  statement: string;
}
/**
 * The measured skill of the method on its backtest, exactly as reported, with the scope it was
 * measured on. It is a statement about that series and period only.
 */
export interface BacktestSkill {
  /**
   * The same scores per horizon, horizons 1, 2, ... in order.
   */
  by_horizon: SkillByHorizon[];
  /**
   * What that series counts.
   */
  case_definition: "confirmed" | "confirmed_or_unknown_status";
  /**
   * Share of targets inside the central 50% interval (nominal 0.5).
   */
  coverage_50: number;
  /**
   * Share of targets inside the central 90% interval (nominal 0.9).
   */
  coverage_90: number;
  /**
   * Forecast dates that produced a forecast.
   */
  forecast_dates: number;
  /**
   * Geography of that series.
   */
  geography: string;
  /**
   * What the backtest does not show (non-empty list of non-empty statements).
   */
  limitations: string[];
  /**
   * SHA-256 of the report-vintage manifest the backtest ran on.
   */
  manifest_sha256: string;
  /**
   * Mean continuous ranked probability score over `targets`, in cases (lower is better).
   */
  mean_crps: number;
  /**
   * Mean absolute error of carrying the origin week's count forward.
   */
  mean_persistence_abs_error: number;
  /**
   * Short name of what was backtested, for a sentence, e.g. `the 2025 West Texas outbreak`
   * (non-empty).
   */
  name: string;
  /**
   * Distinct origin weeks among them.
   */
  origin_weeks: number;
  /**
   * How the backtest respected the information cutoff, in words (non-empty).
   */
  protocol: string;
  /**
   * Repository path of the committed report these numbers were read from.
   */
  report_path: string;
  /**
   * SHA-256 of that report file exactly as read.
   */
  report_sha256: string;
  /**
   * Base seed of the backtest.
   */
  seed: number;
  /**
   * The series that was forecast and scored, in words (non-empty).
   */
  series: string;
  /**
   * Scored targets (forecast week by horizon), pooled.
   */
  targets: number;
}
/**
 * Measured skill at one horizon.
 */
export interface SkillByHorizon {
  /**
   * Share of targets inside the central 50% interval; absent when `n` is 0.
   */
  coverage_50?: number | null;
  /**
   * Share of targets inside the central 90% interval; absent when `n` is 0.
   */
  coverage_90?: number | null;
  /**
   * Weeks after the origin week (1-based).
   */
  horizon: number;
  /**
   * Mean CRPS in cases; absent when `n` is 0.
   */
  mean_crps?: number | null;
  /**
   * Scored targets at this horizon.
   */
  n: number;
}
/**
 * The series file the forecast was made from, hashed.
 */
export interface ForecastInput {
  /**
   * Artifact name, e.g. `weekly-cases`.
   */
  artifact: string;
  rows: number;
  /**
   * SHA-256 of the input file exactly as read.
   */
  sha256: string;
}
/**
 * Latest week with any row in the input series.
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
 * Last week of data the forecast was allowed to use.
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
 * One series the pipeline considered: forecast, or not and why.
 */
export interface ForecastSeries {
  /**
   * True only when this series is the one the backtest scored (same geography, case
   * definition and source). Every other series is forecast by a method whose skill was not
   * measured on it.
   */
  backtested: boolean;
  /**
   * What the series counts; a forecast describes cases under this definition only.
   */
  case_definition: "confirmed" | "confirmed_or_unknown_status";
  /**
   * Cases in the estimation window ending at the origin week, when every count in it is
   * known.
   */
  cases_in_window?: number | null;
  geography: GeoId;
  /**
   * Present exactly when `status` is `insufficient_data`.
   */
  reason?: InsufficientReason | null;
  status: ForecastStatus;
}
