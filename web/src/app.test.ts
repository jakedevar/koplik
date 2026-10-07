import { afterEach, describe, expect, it, vi } from 'vitest';
import { countyWeeklyNote, disclaimer, mountDashboard, showStatus, type MapFactory } from './app';
import { cumulativeNone } from './cumulative';
import { fixtureDataset, pairedRtRows } from './fixtures.test-utils';
import { caseScales, fillColor, mapFeatures } from './map';

// MapLibre needs a real browser/WebGL. Test the UI through its injected map adapter.
vi.mock('maplibre-gl', () => ({ default: {} }));

afterEach(() => document.body.replaceChildren());
function mount(data = fixtureDataset()) {
  const root = document.createElement('div');
  document.body.append(root);
  const map = { update: vi.fn(), destroy: vi.fn() };
  const factory = vi.fn<MapFactory>(() => map);
  const cleanup = mountDashboard(root, data, factory);
  return { root, map, factory, cleanup };
}
function select(id: string, value: string) {
  const node = document.querySelector<HTMLSelectElement>(`#${id}`)!;
  node.value = value;
  node.dispatchEvent(new Event('change'));
}

/** Texas reports 7 confirmed cases; New Mexico reports 900 confirmed-or-unknown-status cases (schema-valid, not comparable). */
function twoDefinitionDataset() {
  const data = fixtureDataset();
  const template = data.cases.find((r) => r.geography === '48' && r.cases.status === 'reported')!;
  const row = (geography: string, week: number, count: number, case_definition: typeof template.case_definition) =>
    ({ ...template, geography, week: { year: 2025, week }, cases: { status: 'reported' as const, count }, case_definition });
  data.cases = [row('48', 1, 7, 'confirmed'), row('35', 1, 900, 'confirmed_or_unknown_status')];
  return data;
}

describe('map colour scales never mix case definitions', () => {
  it('colours Texas (7 confirmed) and New Mexico (900 confirmed-or-unknown) each on its own definition only', () => {
    const data = twoDefinitionDataset();
    const values = (definition: 'confirmed' | 'confirmed_or_unknown_status') => Object.fromEntries(
      mapFeatures(data, 'state', 'cases-2025', '48', definition).features
        .filter((f) => ['48', '35'].includes(f.properties.GEOID)).map((f) => [f.properties.GEOID, f.properties.value]));
    expect(values('confirmed')).toEqual({ '48': 7, '35': null });
    expect(values('confirmed_or_unknown_status')).toEqual({ '48': null, '35': 900 });
    expect(fillColor('cases-2025', 'state', 'confirmed')).toEqual(['interpolate', ['linear'], ['get', 'value'], ...caseScales.confirmed.flat()]);
    expect(fillColor('cases-2025', 'state', 'confirmed_or_unknown_status')).toEqual(['interpolate', ['linear'], ['get', 'value'], ...caseScales.confirmed_or_unknown_status.flat()]);
    expect(caseScales.confirmed).not.toEqual(caseScales.confirmed_or_unknown_status);
  });
  it('gives each definition its own legend block naming it, a selector defaulting to the CDC definition, and a note about the other', () => {
    const { root, map } = mount(twoDefinitionDataset());
    const selector = root.querySelector<HTMLSelectElement>('#case-definition')!;
    expect([...selector.options].map((o) => o.textContent)).toEqual(['confirmed cases', 'confirmed or unknown-status cases']);
    expect(selector.value).toBe('confirmed_or_unknown_status');
    expect(root.querySelector('.legend-scale')?.textContent).toBe('Reported confirmed or unknown-status cases · 0 → 500+');
    expect(root.querySelector('.legend-scale')?.getAttribute('data-case-definition')).toBe('confirmed_or_unknown_status');
    expect(root.querySelector('.legend-note')?.textContent).toContain('1 geography reports cases under a different case definition and is shown as No data on this confirmed or unknown-status cases scale');
    expect(map.update).toHaveBeenLastCalledWith('state', 'cases-2025', '48', 'confirmed_or_unknown_status');
    select('case-definition', 'confirmed');
    expect(root.querySelector('.legend-scale')?.textContent).toBe('Reported confirmed cases · 0 → 500+');
    expect(root.querySelector('.legend-scale')?.getAttribute('data-case-definition')).toBe('confirmed');
    expect(root.querySelector('.legend-note')?.textContent).toContain('on this confirmed cases scale');
    expect(map.update).toHaveBeenLastCalledWith('state', 'cases-2025', '48', 'confirmed');
  });
});

describe('dashboard', () => {
  it('shows every supplied R_t interval level in the exact report table', () => {
    const data = fixtureDataset();
    data.rt = pairedRtRows();
    const { root } = mount(data);
    const reports = [...root.querySelectorAll('tr[data-week="2025-2"] [data-interval-level]')];
    expect(reports.map((report) => report.getAttribute('data-interval-level'))).toEqual(['0.5', '0.95']);
    expect(reports[0].textContent).toContain('50% interval');
    expect(reports[1].textContent).toContain('95% interval');
    expect(reports.every((report) => report.textContent?.includes('Mean 1.2'))).toBe(true);
  });
  it('shows synthetic labels, disclaimer, exact reports and distinctly labelled R_t quality', () => {
    const { root } = mount();
    expect(root.querySelector('.synthetic')?.textContent).toContain('SYNTHETIC TEST DATA');
    expect(root.querySelector('footer .disclaimer')?.textContent).toBe(disclaimer);
    expect(root.querySelector('h2')?.textContent).toBe('Texas');
    expect(root.querySelector('.headline-value')?.textContent).toBe('32');
    expect(root.querySelector('.insufficient')?.textContent).toContain('Insufficient data');
    expect(root.querySelector('.provisional')?.textContent).toContain('Provisional');
    expect(root.querySelector('tr[data-week="2025-2"]')?.textContent).toContain('Mean 1.2; 90% interval 0.7–1.8');
    expect(root.querySelector('label[for="geography"]')?.textContent).toContain('keyboard accessible');
  });
  it('names the case definition in the measure, legend, chart heading, table header and number labels', () => {
    const { root } = mount();
    const words = 'confirmed or unknown-status cases';
    expect([...root.querySelectorAll('.eyebrow')].map((n) => n.textContent).join('|')).toContain(`Cases · 2025 · ${words}`);
    expect(root.querySelector('.value-detail')?.textContent).toContain(words);
    expect(root.querySelector('.legend-scale')?.textContent).toContain(`Reported ${words}`);
    expect(root.querySelector('.charts h3')?.textContent).toBe(`Weekly ${words}`);
    expect(root.querySelector('thead')?.textContent).toContain(`New ${words}`);
    expect(root.querySelector('tr[data-week="2025-2"] td [data-provenance]')?.getAttribute('aria-label')).toContain(words);
    root.querySelector<HTMLButtonElement>('.drill-button')!.click();
    expect(root.querySelector('.legend-scale')?.textContent).toContain('Reported confirmed cases');
    expect(root.querySelector('.charts h3')?.textContent).toBe('Weekly confirmed cases');
    expect(root.querySelector('thead')?.textContent).toContain('New confirmed cases');
  });
  it('switches all map metrics and selections using native accessible controls', () => {
    const { root, map } = mount();
    select('metric', 'cases-2026');
    expect(root.querySelector('.headline-value')?.textContent).toBe('3');
    expect(map.update).toHaveBeenLastCalledWith('state', 'cases-2026', '48', 'confirmed_or_unknown_status');
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
    expect(map.update).toHaveBeenLastCalledWith('county', 'cases-2025', '48165', 'confirmed');
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
    expect(root.querySelector('footer .disclaimer')?.textContent).toBe(disclaimer);
    showStatus(root, 'Data unavailable', true);
    expect(root.querySelector('[role="alert"]')?.textContent).toBe('Data unavailable');
    expect(root.querySelector('footer .disclaimer')?.textContent).toBe(disclaimer);
  });
  it('gives missing map features a separate flag while retaining reported zero', () => {
    const features = mapFeatures(fixtureDataset(), 'state', 'cases-2025', '40').features;
    expect(features.find((f) => f.properties.GEOID === '35')?.properties).toMatchObject({ missing: true, value: null });
    expect(features.find((f) => f.properties.GEOID === '40')?.properties).toMatchObject({ missing: false, value: 0, selected: true });
  });
});

describe('Texas county drill-down: cumulative confirmed cases as reported by Texas DSHS', () => {
  const drill = (root: HTMLElement) => root.querySelector<HTMLButtonElement>('.drill-button')!.click();
  it('charts one point per DSHS report above the weekly series, which keeps its own labels', () => {
    const { root } = mount();
    drill(root);
    expect(root.querySelector('h2')?.textContent).toBe('Gaines County');
    expect(root.querySelector('.cumulative-reports h3')?.textContent).toBe('Cumulative confirmed cases as reported by Texas DSHS');
    const svg = root.querySelector('.cumulative-reports svg.cumulative-chart')!;
    expect([...svg.querySelectorAll('circle.cumulative-point')].map((p) => p.getAttribute('data-report-date'))).toEqual(['2025-03-04', '2025-03-25', '2025-11-24']);
    expect([...svg.querySelectorAll('path.cumulative-missing')].map((p) => p.getAttribute('data-report-date'))).toEqual(['2025-03-28']);
    expect(svg.querySelectorAll('polyline, polygon').length).toBe(0);
    // The plain-words notes: irregular reports, no joining line, no weekly counts, the reason for the report with no count.
    const note = root.querySelector('.cumulative-note')?.textContent;
    expect(note).toContain('published these reports irregularly');
    expect(note).toContain('not joined by a line');
    expect(note).toContain('Weekly counts are not derived from this series.');
    expect(root.querySelector('.cumulative-gaps')?.textContent).toBe('2025-03-28: DSHS published no county table in this report');
    // The weekly series is unchanged, with its reason in words and the explanation of what is derived.
    expect(root.querySelector('.charts h3')?.textContent).toBe('Weekly confirmed cases');
    expect(root.querySelector('.county-weekly-note')?.textContent).toBe(countyWeeklyNote);
    expect(countyWeeklyNote).toContain('Nothing is estimated or interpolated.');
    select('geography', '48115');
    expect(root.querySelector('tr[data-week="2025-2"] td')?.textContent).toBe('No data · not reported');
  });
  it('lists the exact reports in the report table, each count opening its provenance', () => {
    const { root } = mount();
    drill(root);
    expect(root.querySelector('.reports summary')?.textContent).toBe('Read exact weekly and cumulative reports and R_t status');
    const rows = [...root.querySelectorAll('.cumulative-table tbody tr')];
    expect(rows.map((tr) => [tr.getAttribute('data-report-date'), tr.querySelector('td')?.textContent])).toEqual([
      ['2025-03-04', '4'], ['2025-03-25', '10'], ['2025-03-28', 'No data: DSHS published no county table in this report'], ['2025-11-24', '20'],
    ]);
    const button = rows[1].querySelector<HTMLButtonElement>('td button')!;
    expect(button.getAttribute('aria-label')).toBe('Gaines County · DSHS report of 2025-03-25 · 10 cumulative confirmed cases. Open provenance.');
  });
  it('opens the provenance drawer from a point, with the report snapshot and what the number is not', () => {
    const { root } = mount();
    drill(root);
    const point = root.querySelector<SVGElement>('circle.cumulative-point[data-report-date="2025-03-25"]')!;
    point.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    const drawer = root.querySelector('dialog')!;
    expect(drawer.textContent).toContain('Gaines County · DSHS report of 2025-03-25 · 10 cumulative confirmed cases');
    expect(drawer.textContent).toContain('https://example.invalid/synthetic-web-test');
    expect(drawer.textContent).toContain('It is not a weekly count');
  });
  it('shows no cumulative block in the state view and says so for a county no report names', () => {
    const data = fixtureDataset();
    const { root } = mount(data);
    expect(root.querySelector<HTMLElement>('.cumulative-reports')!.hidden).toBe(true);
    expect(root.querySelector('.cumulative-reports')?.children.length).toBe(0);
    drill(root);
    expect(root.querySelector<HTMLElement>('.cumulative-reports')!.hidden).toBe(false);
    data.cumulative = data.cumulative.filter((r) => r.geography !== '48165');
    select('geography', '48115');
    select('geography', '48165');
    expect(root.querySelector('.cumulative-none')?.textContent).toBe(cumulativeNone);
    expect(root.querySelector('.cumulative-table')).toBeNull();
    drill(root);
    expect(root.querySelector<HTMLElement>('.cumulative-reports')!.hidden).toBe(true);
  });
});
