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

  it('puts the measured skill in plain words beside the forecast, and says this series was not backtested', async () => {
    const { main, cleanup } = mount();
    await flush();
    const skill = main.querySelector('.forecast-skill')!;
    expect(skill.querySelector('.forecast-headline')?.textContent).toBe('In a backtest on the synthetic fixture outbreak, 90% intervals contained the true count 50% of the time (2 of 4); a well-calibrated 90% interval would, about 90%. 50% intervals contained it 50% of the time (2 of 4); about 50% would be expected.');
    expect(skill.textContent).toContain('Mean CRPS 3.5 cases');
    expect(skill.querySelector('.forecast-narrow')?.textContent).toContain('too narrow');
    expect(skill.querySelector('.forecast-not-backtested')?.textContent).toContain('was NOT backtested');
    expect(main.textContent).toContain('SYNTHETIC FORECAST');
    expect(main.textContent).toContain('Model projection from reported counts, not a prediction of what will happen.');
    // Parameters carry their citations; the report is named, not linked, for a synthetic fixture.
    expect(main.querySelectorAll('.parameter-citations tbody tr')).toHaveLength(3);
    expect(skill.querySelector('a')).toBeNull();
    expect(skill.textContent).toContain('data/reports/synthetic-backtest.json');
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
