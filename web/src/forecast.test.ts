import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it, vi } from 'vitest';
import { admits, basisLabel, evaluationScope, insufficientSkillStatus, insufficientWords, loadForecast, mismatch, noMeasuredSkill, measuredScoresOf, noSkillWords, parseForecast, percent, policyWords, pooledWords, quantileAt, seriesEvaluationScope, seriesRows, seriesSkillWords, skillWords, weekOrdinal, withheldNotice, withheldSeriesWords, type Forecast, type ForecastProvenance } from './forecast';
import { fixtureRoot } from './fixtures.test-utils';

const read = (name: string) => JSON.parse(readFileSync(resolve(fixtureRoot, `synthetic-v1/${name}`), 'utf8'));
const rows = (): Forecast[] => read('synthetic-forecast.json');
const companion = (): ForecastProvenance => read('synthetic-forecast.provenance.json');
const real = JSON.parse(readFileSync(resolve(process.cwd(), '../data/reports/backtest/west-texas-2025.json'), 'utf8'));
const cdc = JSON.parse(readFileSync(resolve(process.cwd(), '../data/reports/backtest/cdc-states.json'), 'utf8')).primary;

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
    expect(quantileAt(seriesRows(published, '48')[0], 0.5)).toBe(3.5);
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
    refuse((_, c) => { (c as { contract_version: number }).contract_version = 4; }, 'contract_version must be 7');
    refuse((_, c) => { c.statement = ' '; }, 'statement must not be empty');
    refuse((_, c) => { c.parameters[0].source = ''; }, 'needs a source and a note');
    refuse((_, c) => { c.series.find((x) => x.geography === '40')!.reason = null; }, 'status and reason disagree');
    refuse((_, c) => { c.series.find((x) => x.geography === '48')!.skill = 'backtested'; }, 'marked backtested but is not the backtested series');
    refuse((_, c) => { c.series.push({ ...c.series[0] }); }, 'listed twice');
    // A series' skill is its own entry's: it cannot claim a measured skill the backtest did not measure, or deny one it did.
    refuse((_, c) => { c.series.find((x) => x.geography === '48')!.skill = insufficientSkillStatus; }, "its skill disagrees with the backtest's entry");
    refuse((_, c) => { c.series.find((x) => x.geography === '40')!.skill = 'measured'; }, "its skill disagrees with the backtest's entry");
    refuse((_, c) => { c.series_backtest!.by_series = c.series_backtest!.by_series.filter((e) => e.geography !== '48'); }, 'did not run on it');
    refuse((_, c) => { c.series_backtest = null; }, 'did not run on it');
    refuse((_, c) => { c.series_backtest!.by_series.find((e) => e.geography === '20')!.measured = null; }, 'present exactly when the floor');
    refuse((_, c) => { c.series_backtest!.protocol = 'each forecast used only what was known'; }, 'must say so in its protocol');
    refuse((_, c) => { c.series_backtest!.by_series.push({ ...c.series_backtest!.by_series[0] }); }, 'listed twice in the series backtest');
    // The publication policy is re-applied: a series is published exactly when its own measured scores meet it.
    const texas = (c: ForecastProvenance) => c.series_backtest!.by_series.find((e) => e.geography === '48')!.measured!;
    const kansas = (c: ForecastProvenance) => c.series_backtest!.by_series.find((e) => e.geography === '20')!.measured!;
    refuse((_, c) => { texas(c).mean_crps = 9; }, 'is published but its measured skill does not meet the publication policy');
    refuse((_, c) => { texas(c).coverage_90 = 0.74; }, 'is published but its measured skill does not meet the publication policy');
    refuse((_, c) => { c.series.find((x) => x.geography === '48')!.skill = 'not backtested; no measured skill'; }, 'is published but its measured skill does not meet the publication policy');
    refuse((_, c) => { c.publication_policy.maximum_crps_over_persistence = 0.5; }, 'is published but its measured skill does not meet the publication policy');
    refuse((_, c) => { kansas(c).mean_crps = 4; kansas(c).coverage_90 = 0.9; }, 'meets the publication policy but is withheld');
    refuse((_, c) => { c.series.find((x) => x.geography === '20')!.withheld = 'not_backtested'; }, 'withheld for the wrong reason');
    refuse((_, c) => { c.series.find((x) => x.geography === '20')!.withheld = null; }, 'reason it is withheld disagree');
    refuse((_, c) => { c.series.find((x) => x.geography === '48')!.withheld = 'skill_below_policy'; }, 'reason it is withheld disagree');
    refuse((_, c) => { c.publication_policy.rule = ' '; }, 'publication policy is malformed');
    expect(() => parseForecast({}, companion(), true)).toThrow('expected an array');
    expect(() => parseForecast(rows(), { ...companion(), unexpected: 1 }, true)).toThrow('invalid v7 companion');
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
  it('states the measured coverage with one precision and counts, the scores, and that the backtest intervals were too narrow', () => {
    const words = skillWords(skill);
    expect(words.headline).toBe('In a backtest on the 2025 West Texas outbreak, 90% intervals contained the true count 62.5% of the time (30 of 48); a well-calibrated 90% interval would, about 90%. 50% intervals contained it 47.9% of the time (23 of 48); about 50% would be expected.');
    expect(words.scores).toContain('Mean CRPS 3.66 cases');
    expect(words.scores).toContain('scored 5.79');
    expect(words.scores).toContain('48 forecasts');
    expect(words.narrow).toBe('In this backtest the intervals were too narrow: the true count fell outside them more often than their labels say.');
    expect(skillWords({ ...skill, coverage_50: 0.5, coverage_90: 0.9 }).narrow).toBe('');
    expect(skillWords({ ...skill, mean_crps: 3.5 }).scores).toContain('Mean CRPS 3.50 cases');
  });
  it('formats every share to one decimal, as the pipeline does', () => {
    expect([0.625, 30 / 48, 23 / 48, 0.5, 1, 0].map(percent)).toEqual(['62.5%', '62.5%', '47.9%', '50.0%', '100.0%', '0.0%']);
  });
  it('says a series that was not backtested has no measured skill, in the words the page shows above its chart', () => {
    expect(noMeasuredSkill).toBe('No measured skill for this series. This forecast method has not been tested on this data; treat the bands as illustrative, not as calibrated uncertainty.');
    const c = companion();
    expect(c.series.find((s) => s.geography === '40')!.skill).toBe('not backtested; no measured skill');
  });
  it('limits what the report-vintage backtest speaks to: it is not a measure of the series that were not scored', () => {
    const c = companion();
    expect(evaluationScope(c)).toBe("This backtest does not measure how any other series' forecast will do. None of the 2 series the method forecast (confirmed or unknown-status cases) is the series that was scored by this test.");
    const scored = companion();
    scored.series.forEach((s) => { if (s.status !== 'insufficient_data') s.skill = 'backtested'; });
    expect(evaluationScope(scored)).toContain('Every one of the 2 series the method forecast is the series that was scored.');
    const none = companion();
    none.series.forEach((s) => { s.status = 'insufficient_data'; s.reason = 'below_threshold'; s.withheld = null; });
    expect(evaluationScope(none)).toBe('The method forecast no series, so there is nothing for this backtest to speak to.');
  });
  it('states the measured skill of a series in plain words, with its basis, exactly as measured', () => {
    const c = companion();
    const backtest = c.series_backtest!;
    const texas = backtest.by_series.find((e) => e.geography === '48')!;
    // The numbers of the committed NNDSS report for Texas, exactly as measured.
    const real = cdc.series.find((s: { geography: string }) => s.geography === '48');
    const measured = { ...texas.measured!, targets: real.pooled.n, mean_crps: real.pooled.mean_crps, coverage_50: real.pooled.coverage_50, coverage_90: real.pooled.coverage_90, mean_persistence_abs_error: real.pooled.mean_persistence_abs_error };
    const words = seriesSkillWords(backtest, { ...texas, targets: real.pooled.n, origin_weeks: real.origin_weeks_scored, measured });
    expect(words.headline).toBe('In a pseudo-real-time (revised counts truncated at each forecast date) backtest on this series, 90% intervals contained the true count 35.1% of the time (101 of 288); a well-calibrated 90% interval would, about 90%. 50% intervals contained it 19.4% of the time (56 of 288); about 50% would be expected.');
    expect(words.detail).toContain('Mean CRPS 3374.24 cases');
    expect(words.detail).toContain('scored 14.35');
    expect(words.detail).toContain('288 forecasts of later weeks for this series, made from 36 distinct origin weeks');
    expect(words.detail).toContain('Basis: pseudo-real-time (revised counts truncated at each forecast date), not real-time.');
    expect(words.narrow).toContain('too narrow');
    expect(words.against).toBe("In this backtest the method's mean error was larger than that of carrying the latest count forward.");
    expect(seriesSkillWords(backtest, { ...texas, measured: { ...measured, mean_crps: 1 } }).against).toBe("In this backtest the method's mean error was smaller than that of carrying the latest count forward.");
    expect(seriesSkillWords(backtest, { ...texas, measured: { ...measured, mean_crps: measured.mean_persistence_abs_error } }).against).toBe('');
    // A series with no scores has no skill to state.
    expect(() => seriesSkillWords(backtest, backtest.by_series.find((e) => e.geography === '40')!)).toThrow('no measured skill');
  });
  it('says a series below the floor has insufficient data, with what was scored for it and the floor', () => {
    const backtest = companion().series_backtest!;
    const below = { ...backtest.by_series.find((e) => e.geography === '20')!, measured: null, targets: 2, origin_weeks: 1 };
    expect(noSkillWords(backtest, below)).toBe('No measured skill for this series. The pseudo-real-time (revised counts truncated at each forecast date) backtest on the synthetic fixture state series ran on it but it scored 2 forecasts of later weeks for this series, from 1 origin week, too little to state a skill: a measured skill needs at least 4 from at least 2 origin weeks, a floor fixed before any score. Treat the bands as illustrative, not as calibrated uncertainty.');
    expect(noSkillWords(backtest, { ...below, targets: 0, origin_weeks: 0, forecasts: 0 })).toContain('it scored no forecast of this series');
    expect(noSkillWords(backtest, { ...below, targets: 1 })).toContain('scored 1 forecast of later weeks');
    expect(insufficientSkillStatus).toBe('insufficient data for a measured skill');
  });
  it('states the pooled result as pooled, never as the skill of one series', () => {
    const backtest = companion().series_backtest!;
    const pooled = { ...backtest.pooled!, series: 22, forecasts: 239, scores: { ...backtest.pooled!.scores, targets: cdc.pooled.n, mean_crps: cdc.pooled.mean_crps, coverage_50: cdc.pooled.coverage_50, coverage_90: cdc.pooled.coverage_90, mean_persistence_abs_error: cdc.pooled.mean_persistence_abs_error } };
    const words = pooledWords({ ...backtest, pooled })!;
    expect(words.headline).toBe('In a pseudo-real-time (revised counts truncated at each forecast date) backtest on the synthetic fixture state series, pooled over 22 series and 239 forecasts, 90% intervals contained the true count 39.0% of the time (682 of 1748); a well-calibrated 90% interval would, about 90%. 50% intervals contained it 20.9% of the time (365 of 1748); about 50% would be expected.');
    expect(words.scores).toContain('Mean CRPS 23049631.33 cases');
    expect(words.scores).toContain('scored 15.42');
    expect(words.scores).toContain('not the skill of any one series');
    expect(pooledWords({ ...backtest, pooled: null })).toBeNull();
    expect(basisLabel('real-time by report vintage')).toBe('real-time by report vintage');
    expect(basisLabel(backtest.basis)).toBe('pseudo-real-time (revised counts truncated at each forecast date)');
  });
  it('says which of the series the method forecast the series backtest measured: each series has its own skill or none', () => {
    const c = companion();
    expect(seriesEvaluationScope(c)).toBe("Of the 2 series the method forecast, 2 have a measured skill from this test, 0 have insufficient data for one and 0 were not part of it; 1 is published. A series' skill is its own: no other series' number is evidence about it.");
    c.series.find((s) => s.geography === '48')!.skill = 'not backtested; no measured skill';
    expect(seriesEvaluationScope(c)).toContain('1 has a measured skill from this test, 0 have insufficient data for one and 1 was not part of it');
    c.series.forEach((s) => { s.status = 'insufficient_data'; s.reason = 'below_threshold'; s.withheld = null; });
    expect(seriesEvaluationScope(c)).toBe('The method forecast no series, so there is nothing for this test to speak to.');
  });
  it('re-applies the publication policy exactly as the contract does', () => {
    const policy = { rule: 'a rule', minimum_coverage_90: 0.75, maximum_crps_over_persistence: 1 };
    const scores = (coverage_90: number, mean_crps: number, mean_persistence_abs_error: number) => ({ coverage_90, mean_crps, mean_persistence_abs_error });
    expect(admits(policy, scores(0.75, 4, 4))).toBe(true);
    expect(admits(policy, scores(0.7499, 4, 4))).toBe(false);
    expect(admits(policy, scores(0.9, 4.0001, 4))).toBe(false);
    expect(admits(policy, scores(1, 0, 0))).toBe(true);
    expect(admits(policy, scores(0.9, 1, 0))).toBe(false);
    expect(admits(policy, null)).toBe(false);
    expect(admits({ ...policy, maximum_crps_over_persistence: 2 }, scores(0.8, 8, 4))).toBe(true);
    const c = companion();
    expect(admits(c.publication_policy, measuredScoresOf(c, c.series.find((s) => s.geography === '48')!))).toBe(true);
    expect(admits(c.publication_policy, measuredScoresOf(c, c.series.find((s) => s.geography === '20')!))).toBe(false);
    expect(measuredScoresOf(c, c.series.find((s) => s.geography === '40')!)).toBeNull();
    expect(policyWords(c.publication_policy)).toBe("Our publication rule: a forecast is shown only if, in our test on that very series, its 90% intervals contained the true count at least 75.0% of the time and its mean error was no worse than simply repeating the latest complete week's count.");
    expect(policyWords({ ...c.publication_policy, maximum_crps_over_persistence: 2 })).toContain('no more than 2 times the error of simply repeating');
  });
  // Every series withheld, with the pooled numbers of the committed NNDSS report exactly as measured.
  const allWithheld = () => {
    const c = companion();
    for (const s of c.series) { if (s.status === 'forecast') { s.status = 'withheld'; s.withheld = 'skill_below_policy'; } }
    const t = c.series_backtest!.by_series.find((e) => e.geography === '48')!.measured!;
    t.mean_crps = 9;
    const pooled = c.series_backtest!.pooled!;
    pooled.series = 22; pooled.forecasts = 239;
    Object.assign(pooled.scores, { targets: cdc.pooled.n, mean_crps: cdc.pooled.mean_crps, coverage_50: cdc.pooled.coverage_50, coverage_90: cdc.pooled.coverage_90, mean_persistence_abs_error: cdc.pooled.mean_persistence_abs_error });
    return c;
  };
  it('says at the top, in plain words with the measured numbers, that forecasts are not published and why', () => {
    expect(withheldNotice(allWithheld())).toBe("We do not publish forecasts for these series. In our pseudo-real-time (revised counts truncated at each forecast date) test on CDC state data, the method's 90% intervals contained the true count only 39.0% of the time (682 of 1748) and it did worse than simply repeating the latest complete week's count (mean error 23049631.33 cases against 15.42). See \"How we evaluate forecasts\" below.");
    // When some series do meet the rule, it says which are published and that the rest are not.
    expect(withheldNotice(companion())).toContain('We publish forecasts only for the 1 series whose own test result meets our rule, and we do not publish forecasts for the other 1.');
    // Nothing withheld: no notice. No pooled result or no test: it says so instead of a number.
    const none = companion(); none.series.forEach((s) => { if (s.status === 'withheld') { s.status = 'insufficient_data'; s.reason = 'below_threshold'; s.withheld = null; } });
    expect(withheldNotice(none)).toBeNull();
    const unpooled = allWithheld(); unpooled.series_backtest!.pooled = null;
    expect(withheldNotice(unpooled)).toContain('scored too few forecasts to state a pooled result');
    const untested = allWithheld(); untested.series_backtest = null; untested.series.forEach((s) => { s.skill = 'not backtested; no measured skill'; s.withheld = s.status === 'withheld' ? 'not_backtested' : null; });
    expect(withheldNotice(untested)).toContain('No test has measured this method on these series.');
    // It never claims the method did worse than persistence, or poorly calibrated, when the numbers say otherwise.
    const good = allWithheld(); Object.assign(good.series_backtest!.pooled!.scores, { coverage_90: 0.92, mean_crps: 3, mean_persistence_abs_error: 5 });
    expect(withheldNotice(good)).toContain('contained the true count 92.0% of the time');
    expect(withheldNotice(good)).toContain('it did better than simply repeating');
    expect(withheldNotice(good)).not.toContain('only 92.0%');
  });
  it('says why one series is withheld, with its own measured numbers when it has them', () => {
    const c = allWithheld();
    const kansas = c.series.find((s) => s.geography === '20')!;
    const words = withheldSeriesWords(c, kansas, 'Kansas');
    expect(words.headline).toBe('We do not publish a forecast for Kansas.');
    expect(words.reason).toBe("Its measured skill does not meet our rule: its 90% intervals contained the true count 50.0% of the time, below the 75.0% our rule asks for, and its mean error (9.00 cases) was larger than that of simply repeating the latest complete week's count (5.00).");
    expect(words.measured).toContain('In a pseudo-real-time (revised counts truncated at each forecast date) backtest on this series, 90% intervals contained the true count 50.0% of the time (2 of 4)');
    expect(words.rule).toBe(policyWords(c.publication_policy));
    // Insufficient data for a skill, or never tested, say that instead and show no numbers.
    const below = { ...kansas, skill: insufficientSkillStatus, withheld: 'insufficient_data_for_skill' as const };
    c.series_backtest!.by_series.find((e) => e.geography === '20')!.measured = null;
    const w2 = withheldSeriesWords(c, below, 'Kansas');
    expect(w2.measured).toBe('');
    expect(w2.reason).toContain('too little to state a skill');
    const w3 = withheldSeriesWords(c, { ...kansas, skill: 'not backtested; no measured skill', withheld: 'not_backtested' }, 'Kansas');
    expect(w3).toMatchObject({ measured: '', reason: 'No test has measured this method on this series, so there is no measured skill to meet our rule.' });
  });
  it('says what a refused projection is', () => {
    const c = companion();
    const s = c.series.find((x) => x.geography === '40')!;
    expect(insufficientWords(c, { ...s, reason: 'projection_overflow' })).toContain('grew past the limit it will publish');
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
