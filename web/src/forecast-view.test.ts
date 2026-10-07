import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { mountForecast } from './forecast-view';
import { parseForecast } from './forecast';
import { fixtureDataset, fixtureRoot } from './fixtures.test-utils';

const read = (name: string) => JSON.parse(readFileSync(resolve(fixtureRoot, `synthetic-v1/${name}`), 'utf8'));
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
    expect(banner.textContent).toContain('Mean CRPS 12.25 cases (lower is better); carrying the latest count forward instead scored 7.00.');
    expect(banner.textContent).toContain('6 forecasts of later weeks for this series, made from 3 distinct origin weeks');
    expect(banner.textContent).toContain('Basis: pseudo-real-time (revised counts truncated at each forecast date), not real-time.');
    expect(banner.textContent).toContain("the method's mean error was larger than that of carrying the latest count forward");
    // The numbers are provenance buttons that open the report they were read from.
    expect(banner.querySelector('button')?.getAttribute('aria-label')).toContain('Measured skill · Texas');
    expect(result.querySelector('svg.forecast-chart')).not.toBeNull();
    expect(main.textContent).toContain('SYNTHETIC FORECAST');
    expect(main.textContent).toContain('Model projection from reported counts, not a prediction of what will happen.');
    // Parameters carry their citations.
    expect(main.querySelectorAll('.parameter-citations tbody tr')).toHaveLength(3);
    cleanup();
  });

  it('says first that a series with insufficient data for a skill has none, and what was scored for it', async () => {
    const { main, cleanup } = mount(undefined, '20');
    await flush();
    const banner = main.querySelector('.forecast-result')!.firstElementChild!;
    expect(banner.className).toContain('forecast-no-skill');
    expect(banner.textContent).toBe('No measured skill for this series. The pseudo-real-time (revised counts truncated at each forecast date) backtest on the synthetic fixture state series ran on it but it scored 2 forecasts of later weeks for this series, from 1 origin week, too little to state a skill: a measured skill needs at least 4 from at least 2 origin weeks, a floor fixed before any score. Treat the bands as illustrative, not as calibrated uncertainty.');
    expect(main.querySelector('.forecast-measured')).toBeNull();
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
    expect(series.textContent).toBe('In a pseudo-real-time (revised counts truncated at each forecast date) backtest on the synthetic fixture state series, pooled over 2 series and 4 forecasts, 90% intervals contained the true count 75.0% of the time (6 of 8); a well-calibrated 90% interval would, about 90%. 50% intervals contained it 37.5% of the time (3 of 8); about 50% would be expected.');
    expect(evaluation.querySelector('.forecast-series-basis')?.textContent).toContain('pseudo-real-time (revised counts truncated at each forecast date)');
    expect(evaluation.textContent).toContain('at least 4 forecasts of it from at least 2 origin weeks, a floor fixed before any score was computed');
    expect(evaluation.querySelector('.forecast-series-scope')?.textContent).toBe("Of the 2 series forecast above, 1 has a measured skill from this test, 1 has insufficient data for one and 0 were not part of it. A series' skill is its own: no other series' number is evidence about it.");
    expect(evaluation.textContent).toContain('1 of the 4 state series have a measured skill; 1 more had forecasts scored but too few for one; the other 2 never had a forecast in the test');
    const measured = [...evaluation.querySelectorAll('.series-skill tbody tr')];
    expect(measured).toHaveLength(1);
    expect([...measured[0].children].map((cell) => cell.textContent)).toEqual(['Texas', '6', '3', '12.25', '50.0% (3 of 6)', '83.3% (5 of 6)', '7.00']);
    const pooledAll = [...evaluation.querySelectorAll('.series-pooled tbody tr')].find((row) => row.querySelector('th')?.textContent === 'All')!;
    expect([...pooledAll.querySelectorAll('td')].map((cell) => cell.textContent)).toEqual(['8', '9.50', '37.5% (3 of 8)', '75.0% (6 of 8)']);
    // The West Texas test is its own block with its own headline and scope, unchanged.
    expect(evaluation.querySelector('.forecast-headline')?.textContent).toBe('In a backtest on the synthetic fixture outbreak, 90% intervals contained the true count 50.0% of the time (2 of 4); a well-calibrated 90% interval would, about 90%. 50% intervals contained it 50.0% of the time (2 of 4); about 50% would be expected.');
    expect(evaluation.textContent).toContain('Mean CRPS 3.50 cases');
    // The table uses the prose's precision: percent with one decimal plus the exact count.
    const all = [...evaluation.querySelectorAll('tbody tr')].find((row) => row.querySelector('th')?.textContent === 'All' && row.parentElement?.parentElement?.querySelector('caption')?.textContent?.startsWith('Backtest on'))!;
    expect([...all.querySelectorAll('td')].map((cell) => cell.textContent)).toEqual(['4', '3.50', '50.0% (2 of 4)', '50.0% (2 of 4)']);
    expect(evaluation.querySelector('.forecast-narrow')?.textContent).toContain('In this backtest the intervals were too narrow');
    expect(evaluation.querySelector('.forecast-evaluation-scope')?.textContent).toContain('This backtest does not measure how the forecasts above will do. None of the 2 series forecast above (confirmed or unknown-status cases) is the series that was scored by this test');
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
    expect([...select.querySelectorAll('optgroup')].map((g) => g.label)).toEqual(['Forecast available', 'Insufficient data: no forecast']);
    select.value = '48'; select.dispatchEvent(new Event('change'));
    expect(main.querySelector('svg.forecast-chart')).not.toBeNull();
    expect(main.querySelector('.forecast-insufficient')).toBeNull();
    cleanup();
  });

  it('follows the geography selected on the dashboard when the series has one', async () => {
    const { root, main, cleanup } = mount();
    await flush();
    const select = main.querySelector<HTMLSelectElement>('#forecast-geography')!;
    expect(select.value).toBe('20');
    root.dispatchEvent(new CustomEvent('koplik:selection', { detail: { geography: '48' } }));
    expect(select.value).toBe('48');
    root.dispatchEvent(new CustomEvent('koplik:selection', { detail: { geography: '35' } }));
    expect(select.value).toBe('35');
    expect(main.querySelector('.forecast-insufficient')?.textContent).toContain('missing or not reported');
    root.dispatchEvent(new CustomEvent('koplik:selection', { detail: { geography: '99' } }));
    expect(select.value).toBe('35');
    cleanup();
    root.dispatchEvent(new CustomEvent('koplik:selection', { detail: { geography: '48' } }));
    expect(document.querySelector('.forecast')).toBeNull();
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
