import { afterEach, describe, expect, it, vi } from 'vitest';
import { disclaimer, mountDashboard, showStatus, type MapFactory } from './app';
import { fixtureDataset } from './fixtures.test-utils';
import { mapFeatures } from './map';

// MapLibre needs a real browser/WebGL. Test the UI through its injected map adapter.
vi.mock('maplibre-gl', () => ({ default: {} }));

afterEach(() => document.body.replaceChildren());
function mount() {
  const root = document.createElement('div');
  document.body.append(root);
  const map = { update: vi.fn(), destroy: vi.fn() };
  const factory = vi.fn<MapFactory>(() => map);
  const cleanup = mountDashboard(root, fixtureDataset(), factory);
  return { root, map, factory, cleanup };
}
function select(id: string, value: string) {
  const node = document.querySelector<HTMLSelectElement>(`#${id}`)!;
  node.value = value;
  node.dispatchEvent(new Event('change'));
}

describe('dashboard', () => {
  it('shows synthetic labels, disclaimer, exact reports and distinctly labelled R_t quality', () => {
    const { root } = mount();
    expect(root.querySelector('.synthetic')?.textContent).toContain('SYNTHETIC TEST DATA');
    expect(root.querySelector('footer')?.textContent).toBe(disclaimer);
    expect(root.querySelector('h2')?.textContent).toBe('Texas');
    expect(root.querySelector('.headline-value')?.textContent).toBe('32');
    expect(root.querySelector('.insufficient')?.textContent).toBe('Insufficient data');
    expect(root.querySelector('.provisional')?.textContent).toContain('Provisional');
    expect(root.querySelector('tr[data-week="2025-2"]')?.textContent).toContain('Mean 1.2; 90% interval 0.7–1.8');
    expect(root.querySelector('label[for="geography"]')?.textContent).toContain('keyboard accessible');
  });
  it('switches all map metrics and selections using native accessible controls', () => {
    const { root, map } = mount();
    select('metric', 'cases-2026');
    expect(root.querySelector('.headline-value')?.textContent).toBe('3');
    expect(map.update).toHaveBeenLastCalledWith('state', 'cases-2026', '48');
    select('metric', 'coverage');
    expect(root.querySelector('.headline-value')?.textContent).toBe('91.2%');
    select('geography', '35');
    expect(root.querySelector('h2')?.textContent).toBe('New Mexico');
    expect(root.querySelector('.headline-value')?.textContent).toBe('No data');
    select('geography', '40');
    select('metric', 'cases-2025');
    expect(root.querySelector('.headline-value')?.textContent).toBe('0');
  });
  it('drills into Texas 2025 counties, selects a county, and returns to states with visible focus', () => {
    const { root, map } = mount();
    root.querySelector<HTMLButtonElement>('.drill-button')!.click();
    expect(root.querySelector('h2')?.textContent).toBe('Gaines County');
    expect(document.activeElement?.id).toBe('geography');
    expect(root.querySelector<HTMLSelectElement>('#metric')?.disabled).toBe(true);
    expect(map.update).toHaveBeenLastCalledWith('county', 'cases-2025', '48165');
    select('geography', '48115');
    expect(root.querySelector('h2')?.textContent).toBe('Dawson County');
    expect(root.querySelector('.headline-value')?.textContent).toBe('No data');
    root.querySelector<HTMLButtonElement>('.drill-button')!.click();
    expect(root.querySelector('h2')?.textContent).toBe('Texas');
    expect(root.querySelector<HTMLSelectElement>('#metric')?.disabled).toBe(false);
  });
  it('supports map clicks and keeps the selector usable if WebGL fails', () => {
    const { root, factory, cleanup, map } = mount();
    const onSelect = factory.mock.calls[0][2];
    onSelect('20');
    expect(root.querySelector('h2')?.textContent).toBe('Kansas');
    factory.mock.calls[0][3]();
    expect(root.querySelector('.map-status')?.textContent).toContain('Map unavailable');
    select('geography', '40');
    expect(root.querySelector('h2')?.textContent).toBe('Oklahoma');
    cleanup();
    expect(map.destroy).toHaveBeenCalledOnce();
  });
  it('includes the disclaimer on loading and unavailable-data views', () => {
    const root = document.createElement('div');
    showStatus(root, 'Loading pipeline artifacts…');
    expect(root.querySelector('footer')?.textContent).toBe(disclaimer);
    showStatus(root, 'Data unavailable', true);
    expect(root.querySelector('[role="alert"]')?.textContent).toBe('Data unavailable');
    expect(root.querySelector('footer')?.textContent).toBe(disclaimer);
  });
  it('gives missing map features a separate flag while retaining reported zero', () => {
    const features = mapFeatures(fixtureDataset(), 'state', 'cases-2025', '40').features;
    expect(features.find((f) => f.properties.GEOID === '35')?.properties).toMatchObject({ missing: true, value: null });
    expect(features.find((f) => f.properties.GEOID === '40')?.properties).toMatchObject({ missing: false, value: 0, selected: true });
  });
});
