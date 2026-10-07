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
    const { main, cleanup } = mount();
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

  it('says first, above the chart, that a series that was not backtested has no measured skill', async () => {
    const { main, cleanup } = mount();
    await flush();
    const result = main.querySelector('.forecast-result')!;
    const banner = result.firstElementChild!;
    expect(banner.className).toContain('forecast-no-skill');
    expect(banner.textContent).toBe('No measured skill for this series. This forecast method has not been tested on this data; treat the bands as illustrative, not as calibrated uncertainty.');
    expect(result.querySelector('svg.forecast-chart')).not.toBeNull();
    expect(main.textContent).toContain('SYNTHETIC FORECAST');
    expect(main.textContent).toContain('Model projection from reported counts, not a prediction of what will happen.');
    // Parameters carry their citations.
    expect(main.querySelectorAll('.parameter-citations tbody tr')).toHaveLength(3);
    cleanup();
  });

  it('puts the backtest in its own section after the forecast, as an evaluation of the method with its scope', async () => {
    const { main, cleanup } = mount();
    await flush();
    const panel = main.querySelector('.forecast')!;
    const evaluation = main.querySelector<HTMLElement>('.forecast-evaluation')!;
    expect(panel.nextElementSibling).toBe(evaluation);
    expect(evaluation.hidden).toBe(false);
    expect(evaluation.querySelector('h2')?.textContent).toBe('How we evaluate forecasts');
    expect(evaluation.querySelector('.forecast-headline')?.textContent).toBe('In a backtest on the synthetic fixture outbreak, 90% intervals contained the true count 50.0% of the time (2 of 4); a well-calibrated 90% interval would, about 90%. 50% intervals contained it 50.0% of the time (2 of 4); about 50% would be expected.');
    expect(evaluation.textContent).toContain('Mean CRPS 3.50 cases');
    // The table uses the prose's precision: percent with one decimal plus the exact count.
    const all = [...evaluation.querySelectorAll('tbody tr')].find((row) => row.querySelector('th')?.textContent === 'All')!;
    expect([...all.querySelectorAll('td')].map((cell) => cell.textContent)).toEqual(['4', '3.50', '50.0% (2 of 4)', '50.0% (2 of 4)']);
    expect(evaluation.querySelector('.forecast-narrow')?.textContent).toContain('In this backtest the intervals were too narrow');
    expect(evaluation.querySelector('.forecast-evaluation-scope')?.textContent).toContain('This backtest does not measure how the forecasts above will do. None of the 1 series forecast above (confirmed or unknown-status cases) is the series that was scored');
    // The report is named, not linked, for a synthetic fixture.
    expect(evaluation.querySelector('a')).toBeNull();
    expect(evaluation.textContent).toContain('data/reports/synthetic-backtest.json');
    cleanup();
    expect(document.querySelector('.forecast-evaluation')).toBeNull();
  });

  it('keeps the evaluation section, with its scope note, when no backtest report is attached', async () => {
    const noSkill = { ...published(), provenance: { ...published().provenance, backtest: null, scope_note: 'No backtest report for exactly this configuration is attached, so no skill has been measured for these forecasts (1 series forecast).' } };
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
