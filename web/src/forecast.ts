import { expandRowArtifact } from './row-artifact';
import forecastSchema from '../../crates/koplik-contracts/schema/v1/Forecast.schema.json';
import provenanceSchema from '../../crates/koplik-contracts/schema/v7/ForecastProvenance.schema.json';
import type { Forecast } from './generated/Forecast';
import type { ForecastProvenance } from './generated/v7/ForecastProvenance';
import { ajv } from './scenario';
import { caseDefinitionLabels } from './data';

export type { Forecast, ForecastProvenance };
export type ForecastSeries = ForecastProvenance['series'][number];
export type BacktestSkill = NonNullable<ForecastProvenance['backtest']>;
export type SeriesBacktest = NonNullable<ForecastProvenance['series_backtest']>;
export type SeriesBacktestEntry = SeriesBacktest['by_series'][number];
export type MeasuredScores = NonNullable<SeriesBacktestEntry['measured']>;
export type PublicationPolicy = ForecastProvenance['publication_policy'];
export interface PublishedForecast { rows: Forecast[]; provenance: ForecastProvenance }
export interface Week { year: number; week: number }

/**
 * The 4-8 week forecast the pipeline publishes in `data/forecasts/` (#1465): v1 `Forecast` rows and
 * their contract v7 provenance companion. Rows are shown only beside a companion that describes
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

/** The measured evidence a series' skill rests on: scored targets, distinct origin weeks, and the scores. */
export interface Evidence { targets: number; origin_weeks: number; coverage_90: number; mean_crps: number; mean_persistence_abs_error: number }
/** The evidence a series' skill rests on: the series backtest's entry for a measured series, the report-vintage backtest for the series it scored; none otherwise. */
export function measuredScoresOf(provenance: ForecastProvenance, series: ForecastSeries): Evidence | null {
  if (series.skill === 'backtested') {
    const b = provenance.backtest;
    return b ? { targets: b.targets, origin_weeks: b.origin_weeks, coverage_90: b.coverage_90, mean_crps: b.mean_crps, mean_persistence_abs_error: b.mean_persistence_abs_error } : null;
  }
  if (series.skill === 'measured') {
    const entry = provenance.series_backtest?.by_series.find((e) => e.geography === series.geography);
    const m = entry?.measured;
    return entry && m ? { targets: entry.targets, origin_weeks: entry.origin_weeks, coverage_90: m.coverage_90, mean_crps: m.mean_crps, mean_persistence_abs_error: m.mean_persistence_abs_error } : null;
  }
  return null;
}
/** Whether measured evidence meets the publication policy, evidence floor included, for either kind of evaluation: the same rule as `PublicationPolicy::admits` in koplik-contracts. */
export function admits(policy: PublicationPolicy, evidence: Evidence | null): boolean {
  return evidence != null && evidence.targets >= policy.minimum_targets && evidence.origin_weeks >= policy.minimum_origin_weeks
    && evidence.coverage_90 >= policy.minimum_coverage_90 && evidence.mean_crps <= policy.maximum_crps_over_persistence * evidence.mean_persistence_abs_error;
}

export function parseForecast(rowsRaw: unknown, provenanceRaw: unknown, synthetic = false): PublishedForecast {
  if (rowsRaw && typeof rowsRaw === 'object' && 'contract_version' in rowsRaw) rowsRaw = expandRowArtifact('forecast', rowsRaw);
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
  if (!validateProvenance(provenanceRaw)) return fail(`invalid v7 companion: ${ajv.errorsText(validateProvenance.errors)}`);
  const provenance = provenanceRaw as unknown as ForecastProvenance;
  const rows = rowsRaw as Forecast[];
  if (provenance.contract_version !== 7) fail('contract_version must be 7');
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
  const policy = provenance.publication_policy;
  if (blank(policy.rule) || !(policy.minimum_targets >= 1) || !(policy.minimum_origin_weeks >= 1) || !(policy.minimum_coverage_90 >= 0 && policy.minimum_coverage_90 <= 1) || !(policy.maximum_crps_over_persistence >= 0)) fail('the publication policy is malformed');
  for (const s of provenance.series) {
    if ((s.status === 'insufficient_data') !== (s.reason != null)) fail(`series ${s.geography}: status and reason disagree`);
    if ((s.status === 'withheld') !== (s.withheld != null)) fail(`series ${s.geography}: status and the reason it is withheld disagree`);
    if (s.skill === 'backtested' && (!provenance.backtest || s.geography !== provenance.backtest.geography || s.case_definition !== provenance.backtest.case_definition)) fail(`series ${s.geography} is marked backtested but is not the backtested series`);
    if (s.skill === 'measured' || s.skill === insufficientSkillStatus) {
      const entry = entries.get(s.geography);
      if (!backtest || !entry || s.case_definition !== backtest.case_definition) fail(`series ${s.geography} cites a series backtest that did not run on it`);
      else if ((s.skill === 'measured') !== (entry.measured != null)) fail(`series ${s.geography}: its skill disagrees with the backtest's entry for it`);
    }
    // The publication policy, re-applied: a series is published exactly when its own measured scores meet it.
    const admitted = admits(policy, measuredScoresOf(provenance, s));
    if (s.status === 'forecast' && !admitted) fail(`series ${s.geography} is published but its measured skill does not meet the publication policy`);
    if (s.status === 'withheld') {
      const expected = s.skill === 'not backtested; no measured skill' ? 'not_backtested' : s.skill === insufficientSkillStatus ? 'insufficient_data_for_skill' : 'skill_below_policy';
      if (s.withheld !== expected) fail(`series ${s.geography} is withheld for the wrong reason`);
      if (admitted) fail(`series ${s.geography} meets the publication policy but is withheld`);
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
    case 'projection_overflow':
      return `No forecast: the method's projection for this series grew past the limit it will publish (2^40 cases a week), which happens after a burst of cases that follows weeks of almost none. No number is published rather than a clipped one.`;
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
/** The two scores, each labelled precisely: the persistence baseline's mean absolute error is the CRPS of carrying the origin week's count forward. */
const scoreLine = (meanCrps: number, persistence: number) => `Mean CRPS ${twoDecimals(meanCrps)} cases (lower is better); persistence mean absolute error ${twoDecimals(persistence)} cases (the error of simply repeating the origin week's count, which is its CRPS).`;

/**
 * The backtest's measured skill in plain words, exactly as measured: percentages to one decimal with counts,
 * scores to two decimals. This is an evaluation of the method on the backtested series; it is shown in its own
 * section and never beside the chart of a series that was not backtested.
 */
export function skillWords(skill: BacktestSkill) {
  const headline = `In a backtest on ${skill.name}, 90% intervals contained the true count ${percent(skill.coverage_90)} of the time${outOf(skill.coverage_90, skill.targets)}; a well-calibrated 90% interval would, about 90%. 50% intervals contained it ${percent(skill.coverage_50)} of the time${outOf(skill.coverage_50, skill.targets)}; about 50% would be expected.`;
  const scores = `${scoreLine(skill.mean_crps, skill.mean_persistence_abs_error)} The test scored ${skill.targets} forecasts of later weeks, made on ${skill.forecast_dates} forecast dates (${skill.origin_weeks} distinct origin weeks), on ${skill.series}.`;
  const narrow = skill.coverage_90 < 0.9 || skill.coverage_50 < 0.5
    ? 'In this backtest the intervals were too narrow: the true count fell outside them more often than their labels say.' : '';
  return { headline, scores, narrow };
}

/** Which of the series the method forecast the report-vintage backtest scored, in words: it measures the method on its own series only. */
export function evaluationScope(provenance: ForecastProvenance): string {
  const made = provenance.series.filter((s) => s.status !== 'insufficient_data');
  const unmeasured = made.filter((s) => s.skill !== 'backtested');
  if (!made.length) return 'The method forecast no series, so there is nothing for this backtest to speak to.';
  const definitions = [...new Set(unmeasured.map((s) => caseDefinitionLabels[s.case_definition]))].join(' and ');
  const which = unmeasured.length === made.length
    ? `None of the ${made.length} series the method forecast (${definitions}) is the series that was scored by this test.`
    : unmeasured.length
      ? `${unmeasured.length} of the ${made.length} series the method forecast (${definitions}) are not the series that was scored by this test.`
      : `Every one of the ${made.length} series the method forecast is the series that was scored.`;
  return `This backtest does not measure how any other series' forecast will do. ${which}`;
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
  const detail = `${scoreLine(scores.mean_crps, scores.mean_persistence_abs_error)} The test scored ${plural(scores.targets, 'forecast', 'forecasts')} of later weeks for this series, made from ${plural(entry.origin_weeks, 'origin week', 'distinct origin weeks')}. Basis: ${basisLabel(backtest.basis)}${backtest.basis === pseudoRealTime ? ', not real-time' : ''}.`;
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
  const scores = `${scoreLine(pooled.scores.mean_crps, pooled.scores.mean_persistence_abs_error)} The test scored ${plural(pooled.scores.targets, 'forecast', 'forecasts')} of later weeks. These means are in cases, so the series and forecasts with the largest counts and projections dominate them: they are not the skill of any one series.`;
  return { ...words, scores };
}

/** Which of the series the method forecast the series backtest measured, in words: each series has its own skill or none. */
export function seriesEvaluationScope(provenance: ForecastProvenance): string {
  const made = provenance.series.filter((s) => s.status !== 'insufficient_data');
  if (!made.length) return 'The method forecast no series, so there is nothing for this test to speak to.';
  const count = (skill: string) => made.filter((s) => s.skill === skill).length;
  const [measured, insufficient] = [count('measured'), count(insufficientSkillStatus)];
  const rest = made.length - measured - insufficient;
  const published = made.filter((s) => s.status === 'forecast').length;
  return `Of the ${plural(made.length, 'series', 'series')} the method forecast, ${measured} ${measured === 1 ? 'has' : 'have'} a measured skill from this test, ${insufficient} ${insufficient === 1 ? 'has' : 'have'} insufficient data for one and ${rest} ${rest === 1 ? 'was' : 'were'} not part of it; ${published} ${published === 1 ? 'is' : 'are'} published. A series' skill is its own: no other series' number is evidence about it.`;
}

/** Where the pipeline publishes the exact series backtest report. */
export const seriesBacktestReportPath = 'data/forecasts/backtest-cdc-states.json';
/** Where the pipeline publishes the exact backtest report the skill was read from. */
export const backtestReportPath = 'data/forecasts/backtest-west-texas-2025.json';

/** The publication rule in plain words, with its thresholds. */
export function policyWords(policy: PublicationPolicy): string {
  const crps = policy.maximum_crps_over_persistence === 1 ? 'no worse than' : `no more than ${policy.maximum_crps_over_persistence} times the error of`;
  return `Our publication rule: a forecast is shown only if our test on that very series scored at least ${policy.minimum_targets} of its forecasts from at least ${policy.minimum_origin_weeks} origin weeks, its 90% intervals contained the true count at least ${percent(policy.minimum_coverage_90)} of the time, and its mean error was ${crps} simply repeating the latest complete week's count.`;
}

/**
 * The notice at the top of the forecast panel when any forecast is withheld, in plain words with the pooled numbers exactly as
 * measured. It says the pseudo-real-time basis wherever the numbers come from a pseudo-real-time test. Null when nothing is withheld.
 */
export function withheldNotice(provenance: ForecastProvenance): string | null {
  const withheld = provenance.series.filter((s) => s.status === 'withheld').length;
  if (!withheld) return null;
  const published = provenance.series.filter((s) => s.status === 'forecast').length;
  const first = published === 0
    ? 'We do not publish forecasts for these series.'
    : `We publish forecasts only for the ${plural(published, 'series', 'series')} whose own test result meets our rule, and we do not publish forecasts for the other ${withheld}.`;
  const backtest = provenance.series_backtest;
  const pooled = backtest?.pooled?.scores;
  let second: string;
  if (backtest && pooled) {
    // Plain words only: the exact scores are in the evaluation block below. "Far worse" is more than double the persistence error.
    const ratio = pooled.mean_persistence_abs_error > 0 ? pooled.mean_crps / pooled.mean_persistence_abs_error : (pooled.mean_crps > 0 ? Infinity : 1);
    const against = ratio > 2 ? 'it performed far worse than' : ratio > 1 ? 'it performed worse than' : ratio === 1 ? 'it performed no better than' : 'it performed better than';
    const calibrated = pooled.coverage_90 < 0.9 ? 'only ' : '';
    second = `In our ${basisLabel(backtest.basis)} test on CDC state data, the method's 90% intervals contained the true count ${calibrated}${percent(pooled.coverage_90)} of the time${outOf(pooled.coverage_90, pooled.targets)}, and ${against} simply repeating the latest complete week's count.`;
  } else if (backtest) {
    second = `Our ${basisLabel(backtest.basis)} test on CDC state data scored too few forecasts to state a pooled result.`;
  } else if (provenance.backtest && provenance.series.some((s) => s.status === 'withheld' && s.skill === 'backtested')) {
    // A single report-vintage evaluation (e.g. the West Texas outbreak): it did measure the method, so say what it measured and
    // which part of the rule it does not meet, never that no test exists.
    const b = provenance.backtest;
    const failures = policyFailures(provenance.publication_policy, { targets: b.targets, origin_weeks: b.origin_weeks, coverage_90: b.coverage_90, mean_crps: b.mean_crps, mean_persistence_abs_error: b.mean_persistence_abs_error });
    second = failures.length
      ? `This method was tested on ${b.name}, but that test does not meet our rule: ${failures.join(', and ')}.`
      : `This method was tested on ${b.name}.`;
  } else {
    second = 'No test has measured this method on these series.';
  }
  return `${first} ${second} See "How we evaluate forecasts" below.`;
}

/** Which parts of the publication rule the evidence fails, in plain words (the evidence floor included), for either kind of evaluation. */
export function policyFailures(policy: PublicationPolicy, scores: Evidence): string[] {
  return [
    scores.targets < policy.minimum_targets || scores.origin_weeks < policy.minimum_origin_weeks ? `its evidence is ${plural(scores.targets, 'scored forecast', 'scored forecasts')} from ${plural(scores.origin_weeks, 'origin week', 'origin weeks')}, below the ${policy.minimum_targets} from ${policy.minimum_origin_weeks} our rule asks for` : '',
    scores.coverage_90 < policy.minimum_coverage_90 ? `its 90% intervals contained the true count ${percent(scores.coverage_90)} of the time, below the ${percent(policy.minimum_coverage_90)} our rule asks for` : '',
    scores.mean_crps > policy.maximum_crps_over_persistence * scores.mean_persistence_abs_error ? `its mean CRPS (${twoDecimals(scores.mean_crps)} cases) was larger than the persistence mean absolute error (${twoDecimals(scores.mean_persistence_abs_error)} cases) of simply repeating the latest complete week's count` : '',
  ].filter(Boolean);
}

/** Why one series' forecast is withheld, in plain words: the reason, its own measured numbers when it has them, and the rule it did not meet. */
export function withheldSeriesWords(provenance: ForecastProvenance, series: ForecastSeries, name: string): { headline: string; reason: string; measured: string; rule: string } {
  const backtest = provenance.series_backtest;
  const entry = backtest?.by_series.find((e) => e.geography === series.geography);
  const scores = measuredScoresOf(provenance, series);
  let reason = '';
  let measured = '';
  switch (series.withheld) {
    case 'not_backtested':
      reason = 'No test has measured this method on this series, so there is no measured skill to meet our rule.';
      break;
    case 'insufficient_data_for_skill':
      reason = backtest && entry ? noSkillWords(backtest, entry).replace('No measured skill for this series. ', '').replace(' Treat the bands as illustrative, not as calibrated uncertainty.', '') : 'Our test scored too little of this series to state a skill.';
      break;
    case 'skill_below_policy':
      if (scores && backtest && entry?.measured) {
        const words = seriesSkillWords(backtest, entry);
        measured = `${words.headline} ${words.detail}`;
        reason = `Its measured skill does not meet our rule: ${policyFailures(provenance.publication_policy, scores).join(', and ')}.`;
      } else if (scores && provenance.backtest) {
        const words = skillWords(provenance.backtest);
        measured = `${words.headline} ${words.scores}`;
        reason = `Its measured skill does not meet our rule: ${policyFailures(provenance.publication_policy, scores).join(', and ')}.`;
      }
      break;
    default:
      reason = 'The method made a forecast for this series, but it is not published.';
  }
  return { headline: `We do not publish a forecast for ${name}.`, reason, measured, rule: policyWords(provenance.publication_policy) };
}
