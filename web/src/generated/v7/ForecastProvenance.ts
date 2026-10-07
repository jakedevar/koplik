/* Generated from koplik-contracts schema/v7. Run npm run generate:types. */

/**
 * Geography key: state FIPS (2 digits) or county FIPS (5 digits), zero-padded
 */
export type GeoId = string;
/**
 * Why the method made no forecast for a series (the renewal estimator's reason at the origin week,
 * or the method's refusal of a projection that grew past its limit).
 */
export type InsufficientReason =
  "incomplete_window" | "missing_count" | "below_threshold" | "no_infectivity" | "projection_overflow";
/**
 * Whether a series was forecast, and if the method made one, whether it is published.
 */
export type ForecastStatus = "forecast" | "withheld" | "insufficient_data";
/**
 * Why a forecast the method made is not published.
 */
export type WithheldReason = "not_backtested" | "insufficient_data_for_skill" | "skill_below_policy";

/**
 * Companion of a published set of v1 [`Forecast`] rows. Rows are only published beside a
 * companion for which [`ForecastProvenance::check_against`] holds, and only for the series whose
 * status is `forecast`: the series the publication policy admits.
 */
export interface ForecastProvenance {
  /**
   * Name of the series artifact that was forecast, e.g. `weekly-cases` (non-empty).
   */
  artifact: string;
  /**
   * The report-vintage backtest (the Texas DSHS 2025 outbreak total), when a committed report
   * exists for exactly this configuration.
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
  publication_policy: PublicationPolicy;
  /**
   * Ensemble members behind every row's quantiles.
   */
  run_count: number;
  /**
   * What the skills do and do not say about the series above (non-empty).
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
   * The backtest over the CDC NNDSS state series, when a committed report exists for exactly
   * this configuration.
   */
  series_backtest?: SeriesBacktest | null;
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
 * The rule that decides which forecasts are published, with its thresholds.
 */
export interface PublicationPolicy {
  /**
   * The measured mean CRPS must be at most this multiple of the persistence baseline's mean
   * absolute error (1 is "no worse than carrying the latest count forward").
   */
  maximum_crps_over_persistence: number;
  /**
   * The measured 90% interval coverage must be at least this.
   */
  minimum_coverage_90: number;
  /**
   * ...from at least this many distinct origin weeks (the 8 horizons of one origin are not
   * independent evidence). Applies to the report-vintage backtest and to the series backtest
   * alike: a series with less evidence has no measured skill, whatever its scores.
   */
  minimum_origin_weeks: number;
  /**
   * The evidence floor: at least this many scored targets of the series itself...
   */
  minimum_targets: number;
  /**
   * The rule in plain words, with the reasons for its thresholds (non-empty).
   */
  rule: string;
}
/**
 * One series the pipeline considered: forecast, or not and why.
 */
export interface ForecastSeries {
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
  /**
   * Which backtest, if any, measured this series' skill.
   */
  skill: "backtested" | "measured" | "insufficient data for a measured skill" | "not backtested; no measured skill";
  status: ForecastStatus;
  /**
   * Present exactly when `status` is `withheld`: why the forecast the method made is not
   * published.
   */
  withheld?: WithheldReason | null;
}
/**
 * A backtest of the forecast over a family of series (#1503), with the scope and information
 * basis it was measured on. It is a statement about those series and that period only.
 */
export interface SeriesBacktest {
  /**
   * Whether the information cutoff covered revisions of the counts.
   */
  basis: "real-time by report vintage" | "pseudo-real-time (revised counts truncated at each forecast date)";
  /**
   * Every series the backtest ran on, once each, in geography order.
   */
  by_series: SeriesBacktestEntry[];
  /**
   * What every series scored counts.
   */
  case_definition: "confirmed" | "confirmed_or_unknown_status";
  /**
   * SHA-256 of the source snapshot the backtest ran on.
   */
  input_sha256: string;
  /**
   * What this evaluation does not show (non-empty list of non-empty statements).
   */
  limitations: string[];
  /**
   * ...from at least this many distinct origin weeks. Fixed before any score was computed.
   */
  minimum_origin_weeks: number;
  /**
   * The floor for a measured skill: at least this many scored targets...
   */
  minimum_targets: number;
  /**
   * Short name of what was backtested, for a sentence, e.g. `the CDC NNDSS state series`
   * (non-empty).
   */
  name: string;
  /**
   * Every series pooled; absent when even the pooled result is below the floor.
   */
  pooled?: PooledScores | null;
  /**
   * How the information cutoff was respected, and what that may hide, in words (non-empty).
   */
  protocol: string;
  /**
   * Recent weeks treated as provisional: the origin is the latest week less these, and the same
   * newest weeks are never used as truth.
   */
  provisional_weeks: number;
  /**
   * Repository path of the committed report these numbers were read from.
   */
  report_path: string;
  /**
   * SHA-256 of that report file exactly as read.
   */
  report_sha256: string;
  /**
   * Base seed of the backtest (and of the published forecast).
   */
  seed: number;
  /**
   * What was forecast and scored, in words, including what was not scored (non-empty).
   */
  series: string;
}
/**
 * What the evaluation scored for one series. The scores are present exactly when the series
 * reached the evaluation's floor: below it the series has insufficient data for a skill, however
 * its scores would have looked.
 */
export interface SeriesBacktestEntry {
  /**
   * Origins at which the method forecast this series (its minimum-count rule held).
   */
  forecasts: number;
  geography: GeoId;
  /**
   * The measured skill; present exactly when `targets` and `origin_weeks` reach the floor.
   */
  measured?: MeasuredScores | null;
  /**
   * Distinct origin weeks with at least one scored target.
   */
  origin_weeks: number;
  /**
   * Scored targets.
   */
  targets: number;
}
/**
 * The numbers of a measured evaluation, exactly as measured.
 */
export interface MeasuredScores {
  /**
   * The same scores per horizon, horizons 1, 2, ... in order; the `n` sum to `targets`.
   */
  by_horizon: SkillByHorizon[];
  /**
   * Share of targets inside the central 50% interval (nominal 0.5).
   */
  coverage_50: number;
  /**
   * Share of targets inside the central 90% interval (nominal 0.9).
   */
  coverage_90: number;
  /**
   * Mean continuous ranked probability score over `targets`, in cases (lower is better).
   */
  mean_crps: number;
  /**
   * Mean absolute error of carrying the origin week's count forward.
   */
  mean_persistence_abs_error: number;
  /**
   * Scored targets (forecast week by horizon), pooled.
   */
  targets: number;
}
/**
 * All series pooled: every scored target of every series, weighted equally, so the series with
 * the largest counts dominate a score in cases. A statement about the forecasts made where the
 * method's minimum-count rule held, never about any one series.
 */
export interface PooledScores {
  /**
   * (series, origin) forecasts with at least one scored target.
   */
  forecasts: number;
  scores: MeasuredScores;
  /**
   * Series with at least one scored target.
   */
  series: number;
}
