import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { mountForecast } from './forecast-view';
import { parseForecast } from './forecast';
import { fixtureDataset, fixtureRoot } from './fixtures.test-utils';

const read = (name: string) => JSON.parse(readFileSync(resolve(fixtureRoot, `synthetic-v1/${name}`), 'utf8'));
const cdc = JSON.parse(readFileSync(resolve(process.cwd(), '../data/reports/backtest/cdc-states.json'), 'utf8')).primary;
const published = () => parseForecast(read('synthetic-forecast.json'), read('synthetic-forecast.provenance.json'), true);
afterEach(() => { document.body.replaceChildren(); });
const flush = async () => { for (let i = 0; i < 6; i++) await Promise.resolve(); };

function mount(load = vi.fn().mockResolvedValue(published()), geography?: string) {
  const root = document.createElement('div'); document.body.append(root);
  const main = document.createElement('main'); root.append(main);
  const cleanup = mountForecast(main, { base: '/', synthetic: true, data: fixtureDataset(), geography, load });
  return { root, main, cleanup };
}

describe('forecast panel', () => {
  it('draws the forecast bands over the reported counts, with the exact values and their provenance', async () => {
    const { main, cleanup } = mount(undefined, '48');
    await flush();
    const svg = main.querySelector('svg.forecast-chart')!;
    expect(svg.getAttribute('role')).toBe('img');
    expect(svg.getAttribute('aria-label')).toContain('Forecast of weekly confirmed or unknown-status cases for Texas');
    expect(svg.getAttribute('aria-label')).toContain('Exact values are in the table below');
    expect(svg.querySelector('polygon.forecast-band-90')).not.toBeNull();
    expect(svg.querySelector('polygon.forecast-band-50')).not.toBeNull();
    expect(svg.querySelector('polyline.forecast-median')).not.toBeNull();
    // Texas has reported weeks 1-3 of 2026; the origin is week 1, so weeks 2 and 3 are provisional and not used.
    expect(svg.querySelectorAll('svg > .forecast-provisional-bar')).toHaveLength(2);
    expect(svg.querySelectorAll('svg > .case-bar, svg > .case-zero')).toHaveLength(1);
    const body = [...main.querySelectorAll('.forecast-result table')[0].querySelectorAll('tbody tr')];
    expect(body).toHaveLength(8);
    expect(body[0].textContent).toBe('2026 W2' + '3.5' + '2–5' + '0–7.5');
    const median = body[0].querySelector('button')!;
    expect(median.getAttribute('data-provenance')).toBe('');
    expect(median.getAttribute('aria-label')).toContain('Texas · 2026 W2 · median 3.5');
    cleanup();
  });

  it('says first, above the chart, what is measured about that series: its measured skill in plain words, with the basis', async () => {
    const { main, cleanup } = mount(undefined, '48');
    await flush();
    const result = main.querySelector('.forecast-result')!;
    const banner = result.firstElementChild!;
    expect(banner.className).toContain('forecast-measured');
    expect(banner.firstElementChild?.textContent).toBe('Measured skill for this series.');
    expect(banner.querySelector('.forecast-series-headline')?.textContent).toBe('In a pseudo-real-time (revised counts truncated at each forecast date) backtest on this series, 90% intervals contained the true count 83.3% of the time (5 of 6); a well-calibrated 90% interval would, about 90%. 50% intervals contained it 50.0% of the time (3 of 6); about 50% would be expected.');
    expect(banner.textContent).toContain('Mean CRPS 6.50 cases (lower is better); persistence mean absolute error 7.00 cases');
    expect(banner.textContent).toContain('6 forecasts of later weeks for this series, made from 3 distinct origin weeks');
    expect(banner.textContent).toContain('Basis: pseudo-real-time (revised counts truncated at each forecast date), not real-time.');
    expect(banner.textContent).toContain("the method's mean error was smaller than that of carrying the latest count forward");
    // The numbers are provenance buttons that open the report they were read from.
    expect(banner.querySelector('button')?.getAttribute('aria-label')).toContain('Measured skill · Texas');
    expect(result.querySelector('svg.forecast-chart')).not.toBeNull();
    expect(main.textContent).toContain('SYNTHETIC FORECAST');
    expect(main.textContent).toContain('Model projection from reported counts, not a prediction of what will happen.');
    // Parameters carry their citations.
    expect(main.querySelectorAll('.parameter-citations tbody tr')).toHaveLength(3);
    cleanup();
  });

  it('says for a withheld series that no forecast is published, why, its own measured numbers and the rule, and draws no forecast', async () => {
    const { main, cleanup } = mount(undefined, '20');
    await flush();
    const result = main.querySelector('.forecast-result')!;
    const banner = result.firstElementChild!;
    expect(banner.className).toContain('forecast-withheld');
    expect(banner.querySelector('.forecast-withheld-headline')?.textContent).toBe('We do not publish a forecast for Kansas.');
    expect(banner.textContent).toContain("Its measured skill does not meet our rule: its 90% intervals contained the true count 50.0% of the time, below the 75.0% our rule asks for, and its mean CRPS (9.00 cases) was larger than the persistence mean absolute error (5.00 cases) of simply repeating the latest complete week's count.");
    expect(banner.querySelector('.forecast-series-headline')?.textContent).toContain('In a pseudo-real-time (revised counts truncated at each forecast date) backtest on this series, 90% intervals contained the true count 50.0% of the time (2 of 4)');
    expect(banner.querySelector('.forecast-policy')?.textContent).toContain('Our publication rule: a forecast is shown only if');
    // No chart, no forecast values: only the method and its parameters.
    expect(result.querySelector('svg')).toBeNull();
    expect(result.textContent).not.toContain('Exact forecast values');
    expect(result.textContent).toContain('the forecast the method made for this series is withheld and is not published');
    expect(result.querySelectorAll('.parameter-citations tbody tr')).toHaveLength(3);
    cleanup();
  });

  it('says a series with insufficient data for a skill, or never tested, is withheld for that, with no numbers', async () => {
    const base = published();
    const variant = (skill: 'insufficient data for a measured skill' | 'not backtested; no measured skill', why: 'insufficient_data_for_skill' | 'not_backtested') => ({
      ...base,
      provenance: {
        ...base.provenance,
        series: base.provenance.series.map((s) => (s.geography === '20' ? { ...s, skill, withheld: why } : s)),
        series_backtest: { ...base.provenance.series_backtest!, by_series: base.provenance.series_backtest!.by_series.map((e) => (e.geography === '20' ? { ...e, measured: null, targets: 2, origin_weeks: 1 } : e)),
          pooled: { ...base.provenance.series_backtest!.pooled!, forecasts: 4, scores: { ...base.provenance.series_backtest!.pooled!.scores, targets: 8, by_horizon: base.provenance.series_backtest!.pooled!.scores.by_horizon.map((h, i) => (i === 0 ? { ...h, n: 4 } : { ...h, n: 4 })) } } },
      },
    });
    const insufficient = mount(vi.fn().mockResolvedValue(variant('insufficient data for a measured skill', 'insufficient_data_for_skill')), '20');
    await flush();
    const text = insufficient.main.querySelector('.forecast-result')!.firstElementChild!.textContent!;
    expect(text).toContain('We do not publish a forecast for Kansas.');
    expect(text).toContain('it scored 2 forecasts of later weeks for this series, from 1 origin week, too little to state a skill');
    expect(insufficient.main.querySelector('.forecast-result .forecast-series-headline')).toBeNull();
    insufficient.cleanup();
    const untested = mount(vi.fn().mockResolvedValue(variant('not backtested; no measured skill', 'not_backtested')), '20');
    await flush();
    expect(untested.main.querySelector('.forecast-result')!.firstElementChild!.textContent).toContain('No test has measured this method on this series, so there is no measured skill to meet our rule.');
    untested.cleanup();
  });

  it('says at the very top of the panel, before anything else, that forecasts are withheld, with the measured numbers', async () => {
    // Texas is published and Kansas withheld: it says which, and the evaluation numbers behind it.
    const some = mount();
    await flush();
    const panel = some.main.querySelector('.forecast')!;
    expect([...panel.children].slice(0, 3).map((c) => c.tagName)).toEqual(['H2', 'DIV', 'P']);
    expect(panel.querySelector('.forecast-withheld-top')?.textContent).toContain('We publish forecasts only for the 1 series whose own test result meets our rule, and we do not publish forecasts for the other 1.');
    expect(panel.querySelector('.forecast-status')?.textContent).toContain('1 of 4 series have a forecast that meets our publication rule; 1 more are withheld');
    expect([...panel.querySelectorAll<HTMLOptGroupElement>('#forecast-geography optgroup')].map((g) => g.label)).toEqual(['Forecast available', 'Forecast withheld: not published', 'Insufficient data: no forecast']);
    some.cleanup();

    // Every forecast withheld, with the committed NNDSS report's pooled numbers: no chart anywhere, the exact notice first.
    const base = published();
    const pooled = base.provenance.series_backtest!.pooled!;
    const none = {
      ...base,
      rows: [],
      provenance: {
        ...base.provenance,
        series: base.provenance.series.map((s) => (s.status === 'forecast' ? { ...s, status: 'withheld' as const, withheld: 'skill_below_policy' as const } : s)),
        series_backtest: { ...base.provenance.series_backtest!, by_series: base.provenance.series_backtest!.by_series.map((e) => (e.geography === '48' ? { ...e, measured: { ...e.measured!, mean_crps: 9 } } : e)),
          pooled: { ...pooled, series: 22, forecasts: 239, scores: { ...pooled.scores, targets: cdc.pooled.n, mean_crps: cdc.pooled.mean_crps, coverage_50: cdc.pooled.coverage_50, coverage_90: cdc.pooled.coverage_90, mean_persistence_abs_error: cdc.pooled.mean_persistence_abs_error } } },
      },
    };
    const all = mount(vi.fn().mockResolvedValue(none), '48');
    await flush();
    const top = all.main.querySelector('.forecast-withheld-top')!;
    expect(top.textContent).toBe("We do not publish forecasts for these series. In our pseudo-real-time (revised counts truncated at each forecast date) test on CDC state data, the method's 90% intervals contained the true count only 39.0% of the time (682 of 1748), and it performed far worse than simply repeating the latest complete week's count. See \"How we evaluate forecasts\" below.");
    // The exact scores are in the evaluation block, labelled precisely, not in the notice.
    expect(top.textContent).not.toMatch(/23049631|15\.42|CRPS/);
    expect(all.main.querySelector('.forecast-evaluation')!.textContent).toContain('Mean CRPS 23049631.33 cases (lower is better); persistence mean absolute error 15.42 cases');
    expect(all.main.querySelector('.forecast')!.children[1]).toBe(top.parentElement);
    expect(all.main.querySelector('.forecast-status')?.textContent).toBe('No forecast is published: the method made forecasts for 2 of 4 series and our publication rule withheld every one; the other 2 have insufficient data.');
    expect(all.main.querySelector('svg.forecast-chart')).toBeNull();
    expect(all.main.querySelector('.forecast-result')!.firstElementChild!.textContent).toContain('We do not publish a forecast for Texas.');
    // Both evaluations stay, each with its numbers and scope.
    expect(all.main.querySelector('.forecast-evaluation')!.textContent).toContain('Test on the synthetic fixture state series');
    expect(all.main.querySelector('.forecast-evaluation')!.textContent).toContain('Test on the synthetic fixture outbreak');
    all.cleanup();
  });

  it('says what a report-vintage test measured when it is withheld by the evidence floor alone (48 targets from 5 origin weeks)', async () => {
    const base = published();
    const name = base.provenance.backtest!.name;
    const legacy = {
      ...base,
      rows: [],
      provenance: {
        ...base.provenance,
        series_backtest: null,
        publication_policy: { ...base.provenance.publication_policy, minimum_targets: 40, minimum_origin_weeks: 10 },
        backtest: { ...base.provenance.backtest!, geography: '48', case_definition: 'confirmed' as const, targets: 48, origin_weeks: 5, forecast_dates: 5, coverage_90: 0.9, coverage_50: 0.5, mean_crps: 1, mean_persistence_abs_error: 5, by_horizon: [{ horizon: 1, n: 48, mean_crps: 1, coverage_50: 0.5, coverage_90: 0.9 }] },
        series: base.provenance.series.filter((s) => s.geography === '48').map((s) => ({ ...s, case_definition: 'confirmed' as const, skill: 'backtested' as const, status: 'withheld' as const, withheld: 'skill_below_policy' as const })),
      },
    };
    const { main, cleanup } = mount(vi.fn().mockResolvedValue(legacy), '48');
    await flush();
    const floor = 'its evidence is 48 scored forecasts from 5 origin weeks, below the 40 from 10 our rule asks for';
    expect(main.querySelector('.forecast-withheld-top')!.textContent).toBe(`We do not publish forecasts for these series. This method was tested on ${name}, but that test does not meet our rule: ${floor}. See "How we evaluate forecasts" below.`);
    expect(main.querySelector('.forecast-result')!.textContent).toContain(`Its measured skill does not meet our rule: ${floor}.`);
    cleanup();
  });

  it('says a series no backtest scored has no measured skill, in the one fixed sentence', async () => {
    const base = published();
    const unscored = { ...base, provenance: { ...base.provenance, series_backtest: null, series: base.provenance.series.map((s) => ({ ...s, skill: 'not backtested; no measured skill' as const })) } };
    const { main, cleanup } = mount(vi.fn().mockResolvedValue(unscored), '48');
    await flush();
    const banner = main.querySelector('.forecast-result')!.firstElementChild!;
    expect(banner.className).toContain('forecast-no-skill');
    expect(banner.textContent).toBe('No measured skill for this series. This forecast method has not been tested on this data; treat the bands as illustrative, not as calibrated uncertainty.');
    cleanup();
  });

  it('puts the tests in their own section after the forecast, each as an evaluation with its scope, the series test apart from the West Texas one', async () => {
    const { main, cleanup } = mount();
    await flush();
    const panel = main.querySelector('.forecast')!;
    const evaluation = main.querySelector<HTMLElement>('.forecast-evaluation')!;
    expect(panel.nextElementSibling).toBe(evaluation);
    expect(evaluation.hidden).toBe(false);
    expect(evaluation.querySelector('h2')?.textContent).toBe('How we evaluate forecasts');
    expect(evaluation.textContent).toContain('We have tested this method twice, on two different things.');
    expect([...evaluation.querySelectorAll('h3')].map((h) => h.textContent)).toEqual([
      'Test on the synthetic fixture state series: pseudo-real-time (revised counts truncated at each forecast date)',
      'Test on the synthetic fixture outbreak: real-time by report vintage',
    ]);
    // The series test: labelled pseudo-real-time, pooled, with its floor and its own scope.
    const series = evaluation.querySelector('.forecast-series-headline')!;
    expect(series.textContent).toBe('In a pseudo-real-time (revised counts truncated at each forecast date) backtest on the synthetic fixture state series, pooled over 2 series and 5 forecasts, 90% intervals contained the true count 70.0% of the time (7 of 10); a well-calibrated 90% interval would, about 90%. 50% intervals contained it 40.0% of the time (4 of 10); about 50% would be expected.');
    expect(evaluation.querySelector('.forecast-series-basis')?.textContent).toContain('pseudo-real-time (revised counts truncated at each forecast date)');
    expect(evaluation.textContent).toContain('at least 4 forecasts of it from at least 2 origin weeks, a floor fixed before any score was computed');
    expect(evaluation.querySelector('.forecast-series-scope')?.textContent).toBe("Of the 2 series the method forecast, 2 have a measured skill from this test, 0 have insufficient data for one and 0 were not part of it; 1 is published. A series' skill is its own: no other series' number is evidence about it.");
    expect(evaluation.querySelector('.forecast-policy')?.textContent).toContain('Our publication rule: a forecast is shown only if our test on that very series scored at least 4 of its forecasts from at least 2 origin weeks');
    expect(evaluation.textContent).toContain('2 of the 4 state series have a measured skill; 0 more had forecasts scored but too few for one; the other 2 never had a forecast in the test');
    const measured = [...evaluation.querySelectorAll('.series-skill tbody tr')];
    expect(measured).toHaveLength(2);
    expect([...measured[0].children].map((cell) => cell.textContent)).toEqual(['Kansas', '4', '2', '9.00', '25.0% (1 of 4)', '50.0% (2 of 4)', '5.00']);
    expect([...measured[1].children].map((cell) => cell.textContent)).toEqual(['Texas', '6', '3', '6.50', '50.0% (3 of 6)', '83.3% (5 of 6)', '7.00']);
    const pooledAll = [...evaluation.querySelectorAll('.series-pooled tbody tr')].find((row) => row.querySelector('th')?.textContent === 'All')!;
    expect([...pooledAll.querySelectorAll('td')].map((cell) => cell.textContent)).toEqual(['10', '7.50', '40.0% (4 of 10)', '70.0% (7 of 10)']);
    // The West Texas test is its own block with its own headline and scope, unchanged.
    expect(evaluation.querySelector('.forecast-headline')?.textContent).toBe('In a backtest on the synthetic fixture outbreak, 90% intervals contained the true count 50.0% of the time (2 of 4); a well-calibrated 90% interval would, about 90%. 50% intervals contained it 50.0% of the time (2 of 4); about 50% would be expected.');
    expect(evaluation.textContent).toContain('Mean CRPS 3.50 cases');
    // The table uses the prose's precision: percent with one decimal plus the exact count.
    const all = [...evaluation.querySelectorAll('tbody tr')].find((row) => row.querySelector('th')?.textContent === 'All' && row.parentElement?.parentElement?.querySelector('caption')?.textContent?.startsWith('Backtest on'))!;
    expect([...all.querySelectorAll('td')].map((cell) => cell.textContent)).toEqual(['4', '3.50', '50.0% (2 of 4)', '50.0% (2 of 4)']);
    expect(evaluation.querySelector('.forecast-narrow')?.textContent).toContain('In this backtest the intervals were too narrow');
    expect(evaluation.querySelector('.forecast-evaluation-scope')?.textContent).toContain("This backtest does not measure how any other series' forecast will do. None of the 2 series the method forecast (confirmed or unknown-status cases) is the series that was scored by this test");
    // The reports are named, not linked, for a synthetic fixture.
    expect(evaluation.querySelector('a')).toBeNull();
    expect(evaluation.textContent).toContain('data/reports/synthetic-backtest.json');
    expect(evaluation.textContent).toContain('data/reports/synthetic-series-backtest.json');
    cleanup();
    expect(document.querySelector('.forecast-evaluation')).toBeNull();
  });

  it('links the exact reports on a production page', async () => {
    const root = document.createElement('div'); document.body.append(root);
    const main = document.createElement('main'); root.append(main);
    const cleanup = mountForecast(main, { base: '/koplik/', data: fixtureDataset(), load: vi.fn().mockResolvedValue(published()) });
    await flush();
    const links = [...main.querySelectorAll<HTMLAnchorElement>('.forecast-evaluation a')].map((a) => a.getAttribute('href'));
    expect(links).toEqual(['/koplik/data/forecasts/backtest-cdc-states.json', '/koplik/data/forecasts/backtest-west-texas-2025.json']);
    cleanup();
  });

  it('keeps the evaluation section, with its scope note, when no backtest report is attached', async () => {
    const noSkill = { ...published(), provenance: { ...published().provenance, backtest: null, series_backtest: null, scope_note: 'No backtest report for exactly this configuration is attached, so no skill has been measured for these forecasts (2 series forecast).' } };
    const { main, cleanup } = mount(vi.fn().mockResolvedValue(noSkill));
    await flush();
    const evaluation = main.querySelector('.forecast-evaluation')!;
    expect(evaluation.querySelector('h2')?.textContent).toBe('How we evaluate forecasts');
    expect(evaluation.textContent).toContain('no skill has been measured for these forecasts');
    cleanup();
  });

  it('shows a series without enough data as insufficient data, with the reason, never a forecast', async () => {
    const { main, cleanup } = mount(undefined, '40');
    await flush();
    const select = main.querySelector<HTMLSelectElement>('#forecast-geography')!;
    expect(select.value).toBe('40');
    expect(main.querySelector('svg.forecast-chart')).toBeNull();
    expect(main.querySelector('.forecast-insufficient')?.textContent).toContain('2 cases were reported in the last 3 complete weeks');
    expect([...select.querySelectorAll('optgroup')].map((g) => g.label)).toEqual(['Forecast available', 'Forecast withheld: not published', 'Insufficient data: no forecast']);
    select.value = '48'; select.dispatchEvent(new Event('change'));
    expect(main.querySelector('svg.forecast-chart')).not.toBeNull();
    expect(main.querySelector('.forecast-insufficient')).toBeNull();
    cleanup();
  });

  it('follows the geography selected on the dashboard when the series has one', async () => {
    const { root, main, cleanup } = mount();
    await flush();
    const select = main.querySelector<HTMLSelectElement>('#forecast-geography')!;
    expect(select.value).toBe('48');
    root.dispatchEvent(new CustomEvent('koplik:selection', { detail: { geography: '20' } }));
    expect(select.value).toBe('20');
    expect(main.querySelector('.forecast-withheld')?.textContent).toContain('We do not publish a forecast for Kansas.');
    root.dispatchEvent(new CustomEvent('koplik:selection', { detail: { geography: '35' } }));
    expect(select.value).toBe('35');
    expect(main.querySelector('.forecast-insufficient')?.textContent).toContain('missing or not reported');
    const fallback = main.querySelector<HTMLElement>('.forecast-fallback')!;
    expect(fallback.hidden).toBe(true);
    root.dispatchEvent(new CustomEvent('koplik:selection', { detail: { geography: '99' } }));
    expect(select.value).toBe('35');
    expect(fallback.hidden).toBe(false);
    expect(fallback.textContent).toBe('No forecast series for 99; showing New Mexico.');
    root.dispatchEvent(new CustomEvent('koplik:selection', { detail: { geography: '20' } }));
    expect(select.value).toBe('20');
    expect(fallback.hidden).toBe(true);
    root.dispatchEvent(new CustomEvent('koplik:selection', { detail: { geography: '99' } }));
    expect(fallback.textContent).toBe('No forecast series for 99; showing Kansas.');
    // Choosing a series on this page is deliberate: the Explorer's geography no longer applies.
    select.value = '35'; select.dispatchEvent(new Event('change'));
    expect(fallback.hidden).toBe(true);
    cleanup();
    root.dispatchEvent(new CustomEvent('koplik:selection', { detail: { geography: '48' } }));
    expect(document.querySelector('.forecast')).toBeNull();
  });

  it('names both geographies when the Explorer geography has no series on first load', async () => {
    const { main, cleanup } = mount(undefined, '99');
    await flush();
    const fallback = main.querySelector<HTMLElement>('.forecast-fallback')!;
    expect(fallback.hidden).toBe(false);
    expect(fallback.textContent).toMatch(/^No forecast series for 99; showing .+\.$/);
    cleanup();
  });

  it('follows a selection made on the dashboard before the forecast has loaded, and listens on the given events target', async () => {
    const root = document.createElement('div'); document.body.append(root);
    const page = document.createElement('div'); const main = document.createElement('main'); main.append(page); root.append(main);
    const cleanup = mountForecast(page, { base: '/', synthetic: true, data: fixtureDataset(), geography: '48', load: vi.fn().mockResolvedValue(published()), events: root });
    root.dispatchEvent(new CustomEvent('koplik:selection', { detail: { geography: '20' } }));
    await flush();
    const select = page.querySelector<HTMLSelectElement>('#forecast-geography')!;
    expect(select.value).toBe('20');
    root.dispatchEvent(new CustomEvent('koplik:selection', { detail: { geography: '35' } }));
    expect(select.value).toBe('35');
    cleanup();
  });

  it('says plainly when no forecast has been published or it cannot be shown', async () => {
    const none = mount(vi.fn().mockResolvedValue(null));
    await flush();
    expect(none.main.querySelector('.forecast-status')?.textContent).toContain('Forecast not yet available');
    expect(none.main.querySelector('svg')).toBeNull();
    none.cleanup();
    const broken = mount(vi.fn().mockRejectedValue(new Error('Forecast provenance not yet available: a forecast is never shown without its sources')));
    await flush();
    expect(broken.main.querySelector('.forecast-status')?.textContent).toContain('Forecast unavailable. Forecast provenance not yet available');
    expect(broken.main.querySelector('.forecast-status')?.getAttribute('role')).toBe('alert');
    broken.cleanup();
  });
});
