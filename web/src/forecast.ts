import forecastSchema from '../../crates/koplik-contracts/schema/v1/Forecast.schema.json';
import provenanceSchema from '../../crates/koplik-contracts/schema/v5/ForecastProvenance.schema.json';
import type { Forecast } from './generated/Forecast';
import type { ForecastProvenance } from './generated/v5/ForecastProvenance';
import { ajv } from './scenario';
import { caseDefinitionLabels } from './data';

export type { Forecast, ForecastProvenance };
export type ForecastSeries = ForecastProvenance['series'][number];
export type BacktestSkill = NonNullable<ForecastProvenance['backtest']>;
export interface PublishedForecast { rows: Forecast[]; provenance: ForecastProvenance }
export interface Week { year: number; week: number }

/**
 * The 4-8 week forecast the pipeline publishes in `data/forecasts/` (#1465): v1 `Forecast` rows and
 * their contract v5 provenance companion. Rows are shown only beside a companion that describes
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
  if (!validateProvenance(provenanceRaw)) return fail(`invalid v5 companion: ${ajv.errorsText(validateProvenance.errors)}`);
  const provenance = provenanceRaw as unknown as ForecastProvenance;
  const rows = rowsRaw as Forecast[];
  if (provenance.contract_version !== 5) fail('contract_version must be 5');
  for (const [name, text] of [['statement', provenance.statement], ['method', provenance.method], ['origin_rule', provenance.origin_rule], ['scope_note', provenance.scope_note]] as const) {
    if (blank(text)) fail(`${name} must not be empty`);
  }
  for (const parameter of provenance.parameters) if (blank(parameter.source) || blank(parameter.note)) fail(`${parameter.parameter} needs a source and a note`);
  if (new Set(provenance.series.map((s) => s.geography)).size !== provenance.series.length) fail('a series is listed twice');
  for (const s of provenance.series) {
    if ((s.status === 'forecast') !== (s.reason == null)) fail(`series ${s.geography}: status and reason disagree`);
    if (s.backtested && (!provenance.backtest || s.geography !== provenance.backtest.geography || s.case_definition !== provenance.backtest.case_definition)) fail(`series ${s.geography} is marked backtested but is not the backtested series`);
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

/** A share as a percentage to one decimal, never rounded to a whole number (30 of 48 is 62.5%, not 62% or 63%). */
const percent = (share: number) => `${Math.round(share * 1000) / 10}%`;
/** " (30 of 48 forecasts)" when the share is a whole number of the scored targets, as it is for a measured coverage. */
const outOf = (share: number, targets: number) => (Math.abs(share * targets - Math.round(share * targets)) < 1e-6 ? ` (${Math.round(share * targets)} of ${targets})` : '');
const oneDecimal = (value: number) => (Math.round(value * 100) / 100).toString();

/** The backtest's measured skill in plain words, exactly as measured (nothing is rounded beyond whole percentages and two decimals). */
export function skillWords(skill: BacktestSkill, series: ForecastSeries | undefined) {
  const headline = `In a backtest on ${skill.name}, 90% intervals contained the true count ${percent(skill.coverage_90)} of the time${outOf(skill.coverage_90, skill.targets)}; a well-calibrated 90% interval would, about 90%. 50% intervals contained it ${percent(skill.coverage_50)} of the time${outOf(skill.coverage_50, skill.targets)}; about 50% would be expected.`;
  const scores = `Mean CRPS ${oneDecimal(skill.mean_crps)} cases (lower is better); carrying the latest count forward instead scored ${oneDecimal(skill.mean_persistence_abs_error)}. The test scored ${skill.targets} forecasts of later weeks, made on ${skill.forecast_dates} forecast dates (${skill.origin_weeks} distinct origin weeks), on ${skill.series}.`;
  const narrow = skill.coverage_90 < 0.9 || skill.coverage_50 < 0.5
    ? 'The intervals were too narrow: the true count fell outside them more often than their labels say. Read the bands as optimistic.' : '';
  const scope = !series ? '' : series.backtested
    ? 'This series is the one the backtest scored.'
    : `This series (${caseDefinitionLabels[series.case_definition]}) was NOT backtested: the numbers above are how the same method did on a different series, not a measurement of how this forecast will do.`;
  return { headline, scores, narrow, scope };
}

/** Where the pipeline publishes the exact backtest report the skill was read from. */
export const backtestReportPath = 'data/forecasts/backtest-west-texas-2025.json';
