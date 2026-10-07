import forecastSchema from '../../crates/koplik-contracts/schema/v1/Forecast.schema.json';
import provenanceSchema from '../../crates/koplik-contracts/schema/v6/ForecastProvenance.schema.json';
import type { Forecast } from './generated/Forecast';
import type { ForecastProvenance } from './generated/v6/ForecastProvenance';
import { ajv } from './scenario';
import { caseDefinitionLabels } from './data';

export type { Forecast, ForecastProvenance };
export type ForecastSeries = ForecastProvenance['series'][number];
export type BacktestSkill = NonNullable<ForecastProvenance['backtest']>;
export type SeriesBacktest = NonNullable<ForecastProvenance['series_backtest']>;
export type SeriesBacktestEntry = SeriesBacktest['by_series'][number];
export type MeasuredScores = NonNullable<SeriesBacktestEntry['measured']>;
export interface PublishedForecast { rows: Forecast[]; provenance: ForecastProvenance }
export interface Week { year: number; week: number }

/**
 * The 4-8 week forecast the pipeline publishes in `data/forecasts/` (#1465): v1 `Forecast` rows and
 * their contract v6 provenance companion. Rows are shown only beside a companion that describes
 * them; the cross-checks below mirror `ForecastProvenance::check_against` in koplik-contracts.
 */
const validateRow = ajv.compile(forecastSchema);
const validateProvenance = ajv.compile(provenanceSchema);

const fail = (message: string): never => { throw new Error(`Invalid forecast: ${message}`); };
const blank = (value: string) => !value.trim();

/** MMWR week 1 begins on the Sunday of the first week with at least four days in the year. */
function mmwrStartDay(year: number): number {
  const jan1 = Date.UTC(year, 0, 1) / 86_400_000;
  const dayOfWeek = new Date(Date.UTC(year, 0, 1)).getUTCDay();
  return dayOfWeek <= 3 ? jan1 - dayOfWeek : jan1 + 7 - dayOfWeek;
}
/** Consecutive MMWR weeks have consecutive ordinals, across years of 52 and 53 weeks. */
export function weekOrdinal(week: Week): number {
  return (mmwrStartDay(week.year) - 3) / 7 + week.week - 1;
}
export const weekLabel = (week: Week) => `${week.year} W${week.week}`;

/** The quantile published at `level`, if any. */
export function quantileAt(row: Forecast, level: number): number | undefined {
  return row.quantiles.find((q) => Math.abs(q.level - level) < 1e-9)?.value;
}
export function parameterValue(provenance: ForecastProvenance, name: string): unknown {
  return provenance.parameters.find((p) => p.parameter === name)?.value;
}

/** Why the rows and the companion cannot be shown together, or null when the companion describes them. */
export function mismatch(rows: Forecast[], provenance: ForecastProvenance): string | null {
  const origin = weekOrdinal(provenance.origin_week);
  const byGeography = new Map<string, Forecast[]>();
  for (const row of rows) {
    if (row.seed !== provenance.seed) return 'seed';
    if (row.run_count !== provenance.run_count) return 'run_count';
    if (weekOrdinal(row.origin_week) !== origin) return 'origin week';
    if (row.quantiles.length !== provenance.levels.length || row.quantiles.some((q, i) => q.level !== provenance.levels[i])) return 'quantile levels';
    byGeography.set(row.geography, [...(byGeography.get(row.geography) ?? []), row]);
  }
  const forecast = new Set(provenance.series.filter((s) => s.status === 'forecast').map((s) => s.geography as string));
  if (byGeography.size !== forecast.size || [...byGeography.keys()].some((id) => !forecast.has(id))) return 'the series that were forecast';
  for (const list of byGeography.values()) {
    if (list.length !== provenance.horizon_weeks) return 'horizons';
    if (list.some((row, i) => weekOrdinal(row.target_week) !== origin + i + 1)) return 'target weeks';
  }
  if (forecast.size && ![0.05, 0.25, 0.5, 0.75, 0.95].every((level) => provenance.levels.some((l) => Math.abs(l - level) < 1e-9))) return 'quantile levels (the 50% and 90% bands are missing)';
  return null;
}

export function parseForecast(rowsRaw: unknown, provenanceRaw: unknown, synthetic = false): PublishedForecast {
  if (!Array.isArray(rowsRaw)) return fail('expected an array of forecast rows');
  for (const raw of rowsRaw) {
    if (!validateRow(raw)) return fail(`invalid v1 row: ${ajv.errorsText(validateRow.errors)}`);
    const row = raw as unknown as Forecast;
    if (weekOrdinal(row.target_week) <= weekOrdinal(row.origin_week)) fail('target_week must be after origin_week');
    const quantiles = row.quantiles;
    if (quantiles.some((q, i) => !(q.level > 0 && q.level < 1) || !Number.isFinite(q.value) || q.value < 0 || (i > 0 && !(quantiles[i - 1].level < q.level && quantiles[i - 1].value <= q.value)))) {
      fail('quantile levels must increase strictly and values must not decrease');
    }
  }
  if (!validateProvenance(provenanceRaw)) return fail(`invalid v6 companion: ${ajv.errorsText(validateProvenance.errors)}`);
  const provenance = provenanceRaw as unknown as ForecastProvenance;
  const rows = rowsRaw as Forecast[];
  if (provenance.contract_version !== 6) fail('contract_version must be 6');
  for (const [name, text] of [['statement', provenance.statement], ['method', provenance.method], ['origin_rule', provenance.origin_rule], ['scope_note', provenance.scope_note]] as const) {
    if (blank(text)) fail(`${name} must not be empty`);
  }
  if (provenance.series.some((s) => !skillStatuses.includes(s.skill))) fail('a series has an unknown skill status');
  for (const parameter of provenance.parameters) if (blank(parameter.source) || blank(parameter.note)) fail(`${parameter.parameter} needs a source and a note`);
  if (new Set(provenance.series.map((s) => s.geography)).size !== provenance.series.length) fail('a series is listed twice');
  const backtest = provenance.series_backtest;
  const entries = new Map((backtest?.by_series ?? []).map((e) => [e.geography as string, e]));
  if (backtest) {
    if (entries.size !== backtest.by_series.length) fail('a series is listed twice in the series backtest');
    if (backtest.basis === pseudoRealTime && !backtest.protocol.includes('pseudo-real-time')) fail('a pseudo-real-time backtest must say so in its protocol');
    for (const e of backtest.by_series) {
      const reaches = e.targets >= backtest.minimum_targets && e.origin_weeks >= backtest.minimum_origin_weeks;
      if (reaches !== (e.measured != null)) fail(`series ${e.geography}: a measured skill is present exactly when the floor is reached`);
    }
  }
  for (const s of provenance.series) {
    if ((s.status === 'forecast') !== (s.reason == null)) fail(`series ${s.geography}: status and reason disagree`);
    if (s.skill === 'backtested' && (!provenance.backtest || s.geography !== provenance.backtest.geography || s.case_definition !== provenance.backtest.case_definition)) fail(`series ${s.geography} is marked backtested but is not the backtested series`);
    if (s.skill === 'measured' || s.skill === insufficientSkillStatus) {
      const entry = entries.get(s.geography);
      if (!backtest || !entry || s.case_definition !== backtest.case_definition) fail(`series ${s.geography} cites a series backtest that did not run on it`);
      else if ((s.skill === 'measured') !== (entry.measured != null)) fail(`series ${s.geography}: its skill disagrees with the backtest's entry for it`);
    }
  }
  const problem = mismatch(rows, provenance);
  if (problem) fail(`the companion does not describe the rows beside it (${problem})`);
  if (!synthetic && rows.flatMap((r) => r.provenance).some((p) => p.source_id.startsWith('synthetic') || p.url.includes('example.invalid'))) {
    fail('synthetic forecast requires explicit development fixture mode');
  }
  return { rows, provenance };
}

export async function loadForecast(base: string, synthetic = false, read: typeof fetch = fetch): Promise<PublishedForecast | null> {
  const root = `${base.replace(/\/$/, '')}/data/forecasts/${synthetic ? 'synthetic-weekly-cases' : 'weekly-cases'}`;
  const rows = await read(`${root}.json`);
  if (rows.status === 404) return null;
  if (!rows.ok) throw new Error(`Forecast artifact unavailable (${rows.status})`);
  const provenance = await read(`${root}.provenance.json`);
  if (provenance.status === 404) throw new Error('Forecast provenance not yet available: a forecast is never shown without its sources');
  if (!provenance.ok) throw new Error(`Forecast provenance unavailable (${provenance.status})`);
  return parseForecast(await rows.json(), await provenance.json(), synthetic);
}

/** A forecast series' rows in target-week order. */
export function seriesRows(published: PublishedForecast, geography: string): Forecast[] {
  return published.rows.filter((r) => r.geography === geography).sort((a, b) => weekOrdinal(a.target_week) - weekOrdinal(b.target_week));
}

/** Why a series has no forecast, in words that name the numbers behind it. */
export function insufficientWords(provenance: ForecastProvenance, series: ForecastSeries): string {
  const window = Number(parameterValue(provenance, 'window_weeks'));
  const minimum = Number(parameterValue(provenance, 'min_cases'));
  switch (series.reason) {
    case 'below_threshold':
      return `Insufficient data: ${series.cases_in_window ?? 'fewer than the minimum'} ${series.cases_in_window === 1 ? 'case was' : 'cases were'} reported in the last ${window} complete weeks, and the method needs at least ${minimum} to estimate how fast the outbreak is growing. No forecast is published rather than a guess.`;
    case 'missing_count':
      return `Insufficient data: a weekly count in the last ${window} complete weeks, or in the weeks before them that the method looks back on, is missing or not reported. A missing count is never treated as zero, so no forecast is published.`;
    case 'incomplete_window':
      return `Insufficient data: this series does not reach back far enough before the forecast origin to estimate from. No forecast is published.`;
    case 'no_infectivity':
      return `Insufficient data: there were no cases in the weeks before the last ${window} complete weeks, so the growth rate cannot be estimated. No forecast is published rather than a guess.`;
    default:
      return 'Insufficient data: no forecast is published.';
  }
}

/** What a series no backtest scored says, wherever its forecast is shown. */
export const notBacktestedStatus = 'not backtested; no measured skill';
/** A series a backtest ran on but scored too little for a skill (below the floor fixed before any score). */
export const insufficientSkillStatus = 'insufficient data for a measured skill';
const skillStatuses: string[] = ['backtested', 'measured', insufficientSkillStatus, notBacktestedStatus];
/** The basis a backtest carries when one retrieval of revised counts was truncated at each forecast date: never real-time. */
export const pseudoRealTime = 'pseudo-real-time (revised counts truncated at each forecast date)';
/** Shown above the chart of every forecast whose series was not backtested, before anything else. */
export const noMeasuredSkill = 'No measured skill for this series. This forecast method has not been tested on this data; treat the bands as illustrative, not as calibrated uncertainty.';
const noMeasuredSkillLead = 'No measured skill for this series.';

/** A share as a percentage with exactly one decimal (30 of 48 is 62.5%): the one precision used everywhere, as in the pipeline's companion. */
export const percent = (share: number) => `${(Math.round(share * 1000) / 10).toFixed(1)}%`;
/** " (30 of 48)" when the share is a whole number of the scored targets, as a measured coverage is. */
const outOf = (share: number, targets: number) => (Math.abs(share * targets - Math.round(share * targets)) < 1e-6 ? ` (${Math.round(share * targets)} of ${targets})` : '');
const twoDecimals = (value: number) => value.toFixed(2);

/**
 * The backtest's measured skill in plain words, exactly as measured: percentages to one decimal with counts,
 * scores to two decimals. This is an evaluation of the method on the backtested series; it is shown in its own
 * section and never beside the chart of a series that was not backtested.
 */
export function skillWords(skill: BacktestSkill) {
  const headline = `In a backtest on ${skill.name}, 90% intervals contained the true count ${percent(skill.coverage_90)} of the time${outOf(skill.coverage_90, skill.targets)}; a well-calibrated 90% interval would, about 90%. 50% intervals contained it ${percent(skill.coverage_50)} of the time${outOf(skill.coverage_50, skill.targets)}; about 50% would be expected.`;
  const scores = `Mean CRPS ${twoDecimals(skill.mean_crps)} cases (lower is better); carrying the latest count forward instead scored ${twoDecimals(skill.mean_persistence_abs_error)}. The test scored ${skill.targets} forecasts of later weeks, made on ${skill.forecast_dates} forecast dates (${skill.origin_weeks} distinct origin weeks), on ${skill.series}.`;
  const narrow = skill.coverage_90 < 0.9 || skill.coverage_50 < 0.5
    ? 'In this backtest the intervals were too narrow: the true count fell outside them more often than their labels say.' : '';
  return { headline, scores, narrow };
}

/** Which of the forecast series the report-vintage backtest scored, in words: it measures the method on its own series only. */
export function evaluationScope(provenance: ForecastProvenance): string {
  const forecast = provenance.series.filter((s) => s.status === 'forecast');
  const unmeasured = forecast.filter((s) => s.skill !== 'backtested');
  if (!forecast.length) return 'No series was forecast, so there is nothing for this backtest to speak to.';
  const definitions = [...new Set(unmeasured.map((s) => caseDefinitionLabels[s.case_definition]))].join(' and ');
  const which = unmeasured.length === forecast.length
    ? `None of the ${forecast.length} series forecast above (${definitions}) is the series that was scored by this test.`
    : unmeasured.length
      ? `${unmeasured.length} of the ${forecast.length} series forecast above (${definitions}) are not the series that was scored by this test.`
      : `Every one of the ${forecast.length} series forecast above is the series that was scored.`;
  return `This backtest does not measure how the forecasts above will do. ${which}`;
}

const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;
/** The label a backtest's information basis is shown with: pseudo-real-time is never called real-time. */
export const basisLabel = (basis: SeriesBacktest['basis']) => (basis === pseudoRealTime ? pseudoRealTime : 'real-time by report vintage');
/** "In a ..., " lead of a sentence about a backtest on the series it ran on. */
const leadOf = (backtest: SeriesBacktest, what: string) => `In a ${basisLabel(backtest.basis)} backtest on ${what},`;

/** The shared plain-words sentences for one set of measured scores. */
function scoreSentences(scores: MeasuredScores, lead: string) {
  const headline = `${lead} 90% intervals contained the true count ${percent(scores.coverage_90)} of the time${outOf(scores.coverage_90, scores.targets)}; a well-calibrated 90% interval would, about 90%. 50% intervals contained it ${percent(scores.coverage_50)} of the time${outOf(scores.coverage_50, scores.targets)}; about 50% would be expected.`;
  const narrow = scores.coverage_90 < 0.9 || scores.coverage_50 < 0.5
    ? 'In this backtest the intervals were too narrow: the true count fell outside them more often than their labels say.' : '';
  const against = scores.mean_crps > scores.mean_persistence_abs_error
    ? 'In this backtest the method\'s mean error was larger than that of carrying the latest count forward.'
    : scores.mean_crps < scores.mean_persistence_abs_error ? 'In this backtest the method\'s mean error was smaller than that of carrying the latest count forward.' : '';
  return { headline, narrow, against };
}

/**
 * The measured skill of one series, in plain words, exactly as measured: percentages to one decimal with counts, scores to
 * two decimals, the basis (pseudo-real-time, never real-time) and the number of origin weeks it rests on. Only for a series
 * whose entry carries scores (it reached the floor); anything else is [`noSkillWords`].
 */
export function seriesSkillWords(backtest: SeriesBacktest, entry: SeriesBacktestEntry) {
  const scores = entry.measured;
  if (!scores) throw new Error(`series ${entry.geography} has no measured skill`);
  const words = scoreSentences(scores, leadOf(backtest, 'this series'));
  const detail = `Mean CRPS ${twoDecimals(scores.mean_crps)} cases (lower is better); carrying the latest count forward instead scored ${twoDecimals(scores.mean_persistence_abs_error)}. The test scored ${plural(scores.targets, 'forecast', 'forecasts')} of later weeks for this series, made from ${plural(entry.origin_weeks, 'origin week', 'distinct origin weeks')}. Basis: ${basisLabel(backtest.basis)}${backtest.basis === pseudoRealTime ? ', not real-time' : ''}.`;
  return { ...words, detail };
}

/** Why a series forecast above has no measured skill from the series backtest, in the words shown above its chart. */
export function noSkillWords(backtest: SeriesBacktest, entry: SeriesBacktestEntry): string {
  const scored = entry.targets === 0
    ? 'it scored no forecast of this series'
    : `it scored ${plural(entry.targets, 'forecast', 'forecasts')} of later weeks for this series, from ${plural(entry.origin_weeks, 'origin week', 'origin weeks')}`;
  return `${noMeasuredSkillLead} The ${basisLabel(backtest.basis)} backtest on ${backtest.name} ran on it but ${scored}, too little to state a skill: a measured skill needs at least ${backtest.minimum_targets} from at least ${backtest.minimum_origin_weeks} origin weeks, a floor fixed before any score. Treat the bands as illustrative, not as calibrated uncertainty.`;
}

/**
 * The series backtest pooled over every scored series, in plain words. Never the skill of any one series: the means are in
 * cases, so the series and forecasts with the largest counts and projections dominate them.
 */
export function pooledWords(backtest: SeriesBacktest) {
  const pooled = backtest.pooled;
  if (!pooled) return null;
  const words = scoreSentences(pooled.scores, `${leadOf(backtest, backtest.name)} pooled over ${plural(pooled.series, 'series', 'series')} and ${plural(pooled.forecasts, 'forecast', 'forecasts')},`);
  const scores = `Mean CRPS ${twoDecimals(pooled.scores.mean_crps)} cases (lower is better); carrying the latest count forward instead scored ${twoDecimals(pooled.scores.mean_persistence_abs_error)}. The test scored ${plural(pooled.scores.targets, 'forecast', 'forecasts')} of later weeks. These means are in cases, so the series and forecasts with the largest counts and projections dominate them: they are not the skill of any one series.`;
  return { ...words, scores };
}

/** Which of the forecast series the series backtest measured, in words: each series has its own skill or none. */
export function seriesEvaluationScope(provenance: ForecastProvenance): string {
  const forecast = provenance.series.filter((s) => s.status === 'forecast');
  if (!forecast.length) return 'No series was forecast, so there is nothing for this test to speak to.';
  const count = (skill: string) => forecast.filter((s) => s.skill === skill).length;
  const [measured, insufficient] = [count('measured'), count(insufficientSkillStatus)];
  const rest = forecast.length - measured - insufficient;
  return `Of the ${plural(forecast.length, 'series', 'series')} forecast above, ${measured} ${measured === 1 ? 'has' : 'have'} a measured skill from this test, ${insufficient} ${insufficient === 1 ? 'has' : 'have'} insufficient data for one and ${rest} ${rest === 1 ? 'was' : 'were'} not part of it. A series' skill is its own: no other series' number is evidence about it.`;
}

/** Where the pipeline publishes the exact series backtest report. */
export const seriesBacktestReportPath = 'data/forecasts/backtest-cdc-states.json';
/** Where the pipeline publishes the exact backtest report the skill was read from. */
export const backtestReportPath = 'data/forecasts/backtest-west-texas-2025.json';
