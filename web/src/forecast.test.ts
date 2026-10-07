import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it, vi } from 'vitest';
import { insufficientWords, loadForecast, mismatch, parseForecast, quantileAt, seriesRows, skillWords, weekOrdinal, type Forecast, type ForecastProvenance } from './forecast';
import { fixtureRoot } from './fixtures.test-utils';

const read = (name: string) => JSON.parse(readFileSync(resolve(fixtureRoot, `synthetic-v1/${name}`), 'utf8'));
const rows = (): Forecast[] => read('synthetic-forecast.json');
const companion = (): ForecastProvenance => read('synthetic-forecast.provenance.json');
const real = JSON.parse(readFileSync(resolve(process.cwd(), '../data/reports/backtest/west-texas-2025.json'), 'utf8'));

describe('MMWR week ordinals', () => {
  it('are consecutive across years of 52 and 53 weeks', () => {
    // 2024 has 52 weeks and 2025 has 53 (MMWR week 53 ends 2026-01-03), as koplik-contracts computes.
    expect(weekOrdinal({ year: 2025, week: 1 }) - weekOrdinal({ year: 2024, week: 52 })).toBe(1);
    expect(weekOrdinal({ year: 2025, week: 53 }) - weekOrdinal({ year: 2025, week: 52 })).toBe(1);
    expect(weekOrdinal({ year: 2026, week: 1 }) - weekOrdinal({ year: 2025, week: 53 })).toBe(1);
    expect(weekOrdinal({ year: 2026, week: 38 }) - weekOrdinal({ year: 2026, week: 36 })).toBe(2);
  });
});

describe('parseForecast', () => {
  it('accepts rows beside a companion that describes them', () => {
    const published = parseForecast(rows(), companion(), true);
    expect(published.rows).toHaveLength(8);
    expect(seriesRows(published, '48').map((r) => r.target_week.week)).toEqual([2, 3, 4, 5, 6, 7, 8, 9]);
    expect(quantileAt(published.rows[0], 0.5)).toBe(3.5);
    expect(quantileAt(published.rows[0], 0.4)).toBeUndefined();
    expect(mismatch(published.rows, published.provenance)).toBeNull();
  });

  it('refuses rows the companion does not describe', () => {
    const refuse = (edit: (r: Forecast[], c: ForecastProvenance) => void, expected: string) => {
      const r = rows(); const c = companion(); edit(r, c);
      expect(() => parseForecast(r, c, true)).toThrow(expected);
    };
    refuse((r) => { r[0].seed = 2; }, 'seed');
    refuse((r) => { r[0].run_count = 10; }, 'run_count');
    refuse((r) => { r[0].origin_week = { year: 2026, week: 2 }; r[0].target_week = { year: 2026, week: 3 }; }, 'origin week');
    refuse((r) => { r.pop(); }, 'horizons');
    refuse((r) => { r[2].target_week = r[3].target_week; }, 'target weeks');
    refuse((r) => { r.length = 0; }, 'the series that were forecast');
    refuse((r) => { r[0].quantiles.pop(); }, 'quantile levels');
    // A series that was not forecast has no rows.
    refuse((r) => { r.push(...r.map((x) => ({ ...x, geography: '40' }))); }, 'the series that were forecast');
  });

  it('refuses malformed rows and companions', () => {
    const refuse = (edit: (r: Forecast[], c: ForecastProvenance) => void, expected: string) => {
      const r = rows(); const c = companion(); edit(r, c);
      expect(() => parseForecast(r, c, true)).toThrow(expected);
    };
    refuse((r) => { r[0].target_week = r[0].origin_week; }, 'target_week must be after origin_week');
    refuse((r) => { r[0].quantiles[2].value = 0; }, 'values must not decrease');
    refuse((r) => { r[0].quantiles[0].level = 0.6; }, 'increase strictly');
    refuse((_, c) => { (c as { contract_version: number }).contract_version = 4; }, 'contract_version must be 5');
    refuse((_, c) => { c.statement = ' '; }, 'statement must not be empty');
    refuse((_, c) => { c.parameters[0].source = ''; }, 'needs a source and a note');
    refuse((_, c) => { c.series[0].reason = null; }, 'status and reason disagree');
    refuse((_, c) => { c.series[1].backtested = true; }, 'marked backtested but is not the backtested series');
    refuse((_, c) => { c.series.push({ ...c.series[0] }); }, 'listed twice');
    expect(() => parseForecast({}, companion(), true)).toThrow('expected an array');
    expect(() => parseForecast(rows(), { ...companion(), unexpected: 1 }, true)).toThrow('invalid v5 companion');
  });

  it('keeps synthetic fixtures out of a production page', () => {
    expect(() => parseForecast(rows(), companion(), false)).toThrow('requires explicit development fixture mode');
  });
});

describe('loadForecast', () => {
  const answer = (status: number, body: unknown = {}) => ({ ok: status < 400, status, json: async () => body }) as Response;
  it('says nothing was published when the rows are absent, and refuses rows without their companion', async () => {
    expect(await loadForecast('/', true, vi.fn().mockResolvedValue(answer(404)))).toBeNull();
    const rowsOnly = vi.fn(async (url: RequestInfo | URL) => (String(url).endsWith('.provenance.json') ? answer(404) : answer(200, rows())));
    await expect(loadForecast('/', true, rowsOnly as unknown as typeof fetch)).rejects.toThrow('never shown without its sources');
    await expect(loadForecast('/', true, vi.fn().mockResolvedValue(answer(500)))).rejects.toThrow('Forecast artifact unavailable (500)');
  });
  it('reads the synthetic or the production artifact names', async () => {
    const calls: string[] = [];
    const both = vi.fn(async (url: RequestInfo | URL) => {
      calls.push(String(url));
      return answer(200, String(url).endsWith('.provenance.json') ? companion() : rows());
    });
    expect((await loadForecast('/koplik/', true, both as unknown as typeof fetch))!.rows).toHaveLength(8);
    expect(calls).toEqual(['/koplik/data/forecasts/synthetic-weekly-cases.json', '/koplik/data/forecasts/synthetic-weekly-cases.provenance.json']);
  });
});

describe('plain words', () => {
  // The numbers of the committed backtest report, exactly as measured.
  const pooled = real.primary.pooled;
  const skill = { ...companion().backtest!, name: 'the 2025 West Texas outbreak', targets: pooled.n, mean_crps: pooled.mean_crps, coverage_50: pooled.coverage_50, coverage_90: pooled.coverage_90, mean_persistence_abs_error: pooled.mean_persistence_abs_error };
  it('states the measured coverage and scores, and that the intervals were too narrow', () => {
    const series = companion().series.find((s) => s.geography === '48')!;
    const words = skillWords(skill, series);
    expect(words.headline).toBe('In a backtest on the 2025 West Texas outbreak, 90% intervals contained the true count 62.5% of the time (30 of 48); a well-calibrated 90% interval would, about 90%. 50% intervals contained it 47.9% of the time (23 of 48); about 50% would be expected.');
    expect(words.scores).toContain('Mean CRPS 3.66 cases');
    expect(words.scores).toContain('scored 5.79');
    expect(words.scores).toContain('48 forecasts');
    expect(words.narrow).toContain('too narrow');
    expect(words.scope).toContain('NOT backtested');
    expect(skillWords({ ...skill, coverage_50: 0.55, coverage_90: 0.92 }, series).narrow).toBe('');
    expect(skillWords(skill, { ...series, backtested: true }).scope).toBe('This series is the one the backtest scored.');
  });
  it('names the numbers behind every reason a series was not forecast', () => {
    const c = companion();
    const series = c.series.find((s) => s.geography === '40')!;
    expect(insufficientWords(c, series)).toContain('2 cases were reported in the last 3 complete weeks');
    expect(insufficientWords(c, series)).toContain('at least 11');
    expect(insufficientWords(c, { ...series, cases_in_window: 1 })).toContain('1 case was reported');
    expect(insufficientWords(c, { ...series, reason: 'missing_count' })).toContain('never treated as zero');
    expect(insufficientWords(c, { ...series, reason: 'incomplete_window' })).toContain('does not reach back far enough');
    expect(insufficientWords(c, { ...series, reason: 'no_infectivity' })).toContain('no cases in the weeks before');
  });
});
