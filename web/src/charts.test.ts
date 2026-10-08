import { describe, expect, it } from 'vitest';
import { caseCharts, caseSeries, rtAxisMaximum, rtChart, rtChartView, rtLabel } from './charts';
import { fixtureDataset, pairedRtRows } from './fixtures.test-utils';
import { outlierRtRows } from '../tests/rt-outlier-fixture';
import { plot } from './chart-style';
import type { ProvenanceInfo } from './provenance';

const caseChart = (rows: Parameters<typeof caseCharts>[0], year: number, synthetic = false) => {
  const charts = caseCharts(rows, year, synthetic);
  expect(charts).toHaveLength(1);
  return charts[0];
};

describe('accessible SVG reports', () => {
  it('uses a readable percentile display cap, excluding other years and withheld estimates', () => {
    const rows = outlierRtRows();
    const ignored = [
      { ...rows[0], upper: 1000, provisional: true },
      { ...rows[0], upper: null, mean: null, lower: null, status: 'insufficient_data' as const },
      { ...rows[0], upper: 1000, week: { year: 2026, week: 1 } },
    ];
    expect(rtAxisMaximum([...rows, ...ignored], 2025)).toBe(3);
    expect(rtAxisMaximum([rows[0], rows[39]], 2025)).toBe(3);
    expect(rtAxisMaximum(rows.map((row) => ({ ...row, upper: row.week.week <= 38 ? 6 : 100 })), 2025)).toBe(6);
    expect(rtAxisMaximum(rows, 2025, true)).toBe(66.4);
    expect(rtAxisMaximum([{ ...rows[0], mean: 80 }], 2025, true)).toBe(80);
    expect(rtAxisMaximum([], 2025)).toBe(3);
    expect(rtAxisMaximum(ignored.slice(0, 2), 2025)).toBe(3);
  });
  it('marks off-scale intervals and means at the edge, preserving exact values and source records', () => {
    const rows = outlierRtRows();
    const original = JSON.stringify(rows);
    const view = rtChartView(rows, 2025, true);
    const marker = view.querySelector<SVGElement>('.rt-off-scale')!;
    const button = view.querySelector<HTMLButtonElement>('.rt-off-scale-labels button')!;
    expect(marker.getAttribute('data-week')).toBe('40');
    expect(marker.querySelector('title')?.textContent).toBe('Week 40 · 90%: upper bound 66.4, off scale; mean 12.3, off scale');
    expect(marker.getAttribute('aria-label')).toBe('Week 40 · 90%: upper bound 66.4, off scale; mean 12.3, off scale. Open provenance.');
    expect(button.textContent).toContain('upper bound 66.4, off scale');
    expect(view.querySelector('.rt-interval[data-week="40"]')?.getAttribute('y2')).toBe(String(plot.top));
    expect(view.querySelector('.rt-point[data-week="40"]')?.getAttribute('cy')).toBe(String(plot.top));
    expect(view.querySelector('.rt-reference-label')?.textContent).toBe('R_t = 1');
    const requests: ProvenanceInfo[] = [];
    view.addEventListener('koplik:provenance', (event) => requests.push((event as CustomEvent<ProvenanceInfo>).detail));
    marker.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    button.click();
    expect(requests).toHaveLength(2);
    for (const request of requests) {
      expect(request.label).toContain('Mean 12.345; 90% interval 0.7–66.4');
      expect(request.records).toEqual(rows[39].provenance);
    }
    const toggle = view.querySelector<HTMLButtonElement>('button')!;
    expect(toggle.textContent).toBe('Show full range');
    expect(toggle.getAttribute('aria-pressed')).toBe('false');
    toggle.click();
    expect(toggle.getAttribute('aria-pressed')).toBe('true');
    expect(view.querySelector('svg')?.getAttribute('data-axis-maximum')).toBe('66.4');
    expect(view.querySelector('.rt-point[data-week="40"]')?.getAttribute('cy')).toBe(String(plot.base - 12.345 / 66.4 * (plot.base - plot.top)));
    toggle.click();
    expect(toggle.getAttribute('aria-pressed')).toBe('false');
    expect(view.querySelector('.rt-off-scale-labels')?.textContent).toContain('mean 12.3, off scale');
    expect(JSON.stringify(rows)).toBe(original);
  });
  it('formats off-scale labels like axis ticks while keeping exact values in provenance', () => {
    const rows = outlierRtRows();
    rows[39] = { ...rows[39], upper: 66.38493810820933 };
    const view = rtChartView(rows, 2025);
    const button = view.querySelector<HTMLButtonElement>('.rt-off-scale-labels button')!;
    expect(button.textContent).toBe('▲ Week 40 · 90%: upper bound 66.4, off scale; mean 12.3, off scale');
    expect(button.getAttribute('aria-label')).toBe('▲ Week 40 · 90%: upper bound 66.4, off scale; mean 12.3, off scale. Open provenance.');
    let provenanceLabel = '';
    button.addEventListener('koplik:provenance', (event) => {
      provenanceLabel = (event as CustomEvent<ProvenanceInfo>).detail.label;
    });
    button.click();
    expect(provenanceLabel).toContain('upper bound 66.4, off scale');
    expect(provenanceLabel).toContain('Mean 12.345; 90% interval 0.7–66.38493810820933');
  });
  it('formats range notes like axis ticks and hides the toggle when no value is off scale', () => {
    const rows = outlierRtRows().map((row) => ({ ...row, lower: 0.7, mean: 1.2, upper: 2.4 }));
    const view = rtChartView(rows, 2025);
    expect(view.querySelector('button')?.hasAttribute('hidden')).toBe(true);
    expect(view.querySelector('.rt-range-note')?.textContent).toBe('Readable display range: 0–3.');
    rows[39] = { ...rows[39], lower: 0.7, mean: 4.723918, upper: 4.723918 };
    const rangeView = rtChartView(rows, 2025);
    rangeView.querySelector('button')!.click();
    expect(rangeView.querySelector('.rt-range-note')?.textContent).toContain('Full display range: 0–4.7.');
  });
  it('merges paired off-scale interval markers and labels by week', () => {
    const rows = outlierRtRows();
    rows.push({ ...rows[39], interval_level: 0.5, lower: 0.9, mean: 5.25, upper: 7.125 });
    const view = rtChartView(rows, 2025);
    expect(view.querySelectorAll('.rt-off-scale[data-week="40"]')).toHaveLength(1);
    expect(view.querySelectorAll('.rt-off-scale-labels button')).toHaveLength(1);
    expect(view.querySelector('.rt-off-scale[data-week="40"] title')?.textContent).toContain('90%: upper bound 66.4, off scale; mean 12.3, off scale');
    expect(view.querySelector('.rt-off-scale[data-week="40"] title')?.textContent).toContain('50%: upper bound 7.1, off scale; mean 5.3, off scale');
    expect(view.querySelector('.rt-off-scale-labels button')?.textContent).toContain('50%: upper bound 7.1, off scale; mean 5.3, off scale');
  });
  it('marks a mean beyond its interval and an interval wholly above the display cap', () => {
    const rows = outlierRtRows();
    rows[39] = { ...rows[39], upper: 2, mean: 12.345 };
    expect(rtChartView(rows, 2025).querySelector('.rt-off-scale-labels')?.textContent).toBe('▲ Week 40 · 90%: mean 12.3, off scale');
    rows[39] = { ...rows[39], lower: 5, upper: 66.4 };
    const svg = rtChart(rows, 2025);
    expect(svg.querySelector('.rt-interval[data-week="40"]')?.getAttribute('y1')).toBe(String(plot.top));
    expect(svg.querySelector('.rt-off-scale title')?.textContent).toContain('lower bound 5, off scale');
  });
  it('names the case definition in the title, axis label and provenance label', () => {
    const data = fixtureDataset();
    const state = caseChart(data.cases.filter((r) => r.geography === '48'), 2025, true);
    expect(state.querySelector('title')?.textContent).toContain('Weekly confirmed or unknown-status cases');
    expect([...state.querySelectorAll('text')].map((t) => t.textContent)).toContain('New confirmed or unknown-status cases');
    const county = caseChart(data.cases.filter((r) => r.geography === '48165'), 2025, true);
    expect(county.querySelector('title')?.textContent).toContain('Weekly confirmed cases');
    expect([...county.querySelectorAll('text')].map((t) => t.textContent)).toContain('New confirmed cases');
  });
  it('never mixes case definitions: one labelled series per definition, each on its own axis', () => {
    const data = fixtureDataset();
    const nndss = data.cases.find((r) => r.geography === '48' && r.case_definition === 'confirmed_or_unknown_status' && r.cases.status === 'reported')!;
    const dshs = data.cases.find((r) => r.geography === '48165' && r.case_definition === 'confirmed' && r.cases.status === 'reported')!;
    // Same geography, week 1 confirmed and week 2 confirmed-or-unknown-status: schema-valid, not comparable.
    const rows = [{ ...dshs, geography: '48', week: { year: 2025, week: 1 }, cases: { status: 'reported' as const, count: 7 } },
      { ...nndss, geography: '48', week: { year: 2025, week: 2 }, cases: { status: 'reported' as const, count: 900 } }];
    const series = caseSeries(rows, 2025);
    expect(series.map((one) => [one.definition, one.label, one.rows.map((r) => r.week.week)])).toEqual([
      ['confirmed', 'confirmed cases', [1]],
      ['confirmed_or_unknown_status', 'confirmed or unknown-status cases', [2]],
    ]);
    const svgs = caseCharts(rows, 2025, true);
    expect(svgs).toHaveLength(2);
    expect(svgs.map((svg) => svg.querySelector('title')?.textContent)).toEqual([
      expect.stringContaining('Weekly confirmed cases, MMWR 2025. Charted separately from other case definitions; they are never added together.'),
      expect.stringContaining('Weekly confirmed or unknown-status cases, MMWR 2025. Charted separately from other case definitions; they are never added together.'),
    ]);
    expect(svgs.map((svg) => svg.querySelector('.series-legend')?.textContent)).toEqual(['New confirmed cases', 'New confirmed or unknown-status cases']);
    // Each series has its own y scale: the 7 and the 900 are each the tallest bar of their own chart.
    expect(svgs.map((svg) => svg.querySelector('.case-bar')?.getAttribute('data-week'))).toEqual(['1', '2']);
    expect(svgs.map((svg) => [...svg.querySelectorAll('text')].map((t) => t.textContent).filter((t) => t === '7' || t === '900'))).toEqual([['7'], ['900']]);
    expect(svgs.map((svg) => svg.querySelectorAll('.case-bar').length)).toEqual([1, 1]);
  });
  it('renders reported zero and leaves missing weeks as gaps', () => {
    const rows = fixtureDataset().cases.filter((r) => r.geography === '35');
    const svg = caseChart(rows, 2025);
    expect([...svg.querySelectorAll('.case-bar')].map((bar) => bar.getAttribute('data-week'))).toEqual(['1', '3']);
    expect(svg.getAttribute('role')).toBe('button');
    expect(svg.querySelector('title')?.textContent).toContain('MMWR 2025');
    const zero = caseChart(fixtureDataset().cases.filter((r) => r.geography === '40'), 2025);
    expect(zero.querySelector('.case-zero title')?.textContent).toContain('0 confirmed or unknown-status cases');
    const comparison = caseChart([{ ...rows[0], cases: { status: 'reported', count: 0 } }, rows[1]], 2025);
    expect(comparison.querySelector('.case-zero')?.tagName).toBe('line');
    expect(comparison.querySelector('.case-zero')?.getAttribute('y1')).toBe('170');
    expect(comparison.querySelectorAll('[data-week="1"]')).toHaveLength(1);
    expect(comparison.querySelectorAll('[data-week="2"]')).toHaveLength(0);
    expect(svg.querySelectorAll('[data-week="2"]')).toHaveLength(0);
  });
  it('draws only final estimates and breaks ribbons over insufficient/provisional/omitted weeks', () => {
    const rows = fixtureDataset().rt.filter((r) => r.geography === '48');
    const svg = rtChart(rows, 2025);
    expect([...svg.querySelectorAll('.rt-point')].map((point) => point.getAttribute('data-week'))).toEqual(['2', '3', '5']);
    expect(svg.querySelectorAll('.rt-ribbon')).toHaveLength(2);
    expect(svg.querySelector('.rt-point title')?.textContent).toContain('Mean 1.2; 90% interval 0.7–1.8');
    expect(rtLabel(rows[0])).toBe('Insufficient data');
    expect(rtLabel(rows[5])).toContain('Provisional — estimate withheld');
    expect(rtChart([rows[1], rows[4]], 2025).querySelectorAll('.rt-ribbon')).toHaveLength(2);
  });
  it('connects adjacent weeks independently at both 50% and 95% credible levels', () => {
    const svg = rtChart(pairedRtRows().reverse(), 2025);
    const ribbons = [...svg.querySelectorAll('.rt-ribbon')];
    expect(ribbons.map((ribbon) => ribbon.getAttribute('data-interval-level'))).toEqual(['0.95', '0.5']);
    for (const ribbon of ribbons) {
      expect(ribbon.getAttribute('points')?.split(' ')).toHaveLength(4);
      expect(ribbon.querySelector('title')?.textContent).toContain('weeks 2–3');
    }
    expect([...svg.querySelectorAll('.rt-mean')].map((line) => line.getAttribute('points')?.split(' ').length)).toEqual([2, 2]);
  });
  it('splits paired sequences only at missing, provisional and insufficient weeks, retaining quality markers', () => {
    const paired = pairedRtRows().filter((row) => row.week.week === 2);
    const rows = Array.from({ length: 12 }, (_, i) => i + 1).filter((week) => week !== 4)
      .flatMap((week) => paired.map((row) => ({
        ...row, week: { year: 2025, week }, provisional: week === 10,
        ...(week === 7 ? { status: 'insufficient_data' as const, mean: null, lower: null, upper: null } : {}),
      })));
    const svg = rtChart(rows, 2025);
    for (const level of ['0.5', '0.95']) {
      const meanLines = [...svg.querySelectorAll(`.rt-mean[data-interval-level="${level}"]`)];
      expect(meanLines.map((line) => line.getAttribute('points')?.split(' ').length)).toEqual([3, 2, 2, 2]);
      expect(svg.querySelectorAll(`.rt-ribbon[data-interval-level="${level}"]`)).toHaveLength(4);
    }
    expect(svg.querySelector('[data-week="7"] .rt-insufficient-band')?.tagName).toBe('rect');
    expect(svg.querySelector('[data-week="10"] .rt-provisional-band')?.tagName).toBe('rect');
    expect([...svg.querySelectorAll('.rt-point')].map((point) => point.getAttribute('data-week'))).toEqual([
      '1', '2', '3', '5', '6', '8', '9', '11', '12', '1', '2', '3', '5', '6', '8', '9', '11', '12',
    ]);
  });
  it('marks every withheld week with distinct visible and accessible quality states', () => {
    const rows = fixtureDataset().rt.filter((r) => r.geography === '48');
    const both = { ...rows[0], week: { year: 2025, week: 7 }, provisional: true };
    const svg = rtChart([...rows, both], 2025);
    expect([...svg.querySelectorAll('.rt-withheld')].map((marker) => marker.getAttribute('data-week'))).toEqual(['1', '4', '6', '7']);
    expect(svg.querySelector('[data-week="1"] .rt-insufficient-band')?.getAttribute('fill')).toMatch(/^url\(#rt-insufficient-hatch-/);
    expect(svg.querySelector('[data-week="6"] .rt-provisional-band')?.tagName).toBe('rect');
    expect(svg.querySelector('[data-week="7"] text')?.textContent).toBe('IP');
    expect(svg.querySelector('[data-week="7"]')?.getAttribute('aria-label')).toContain('Insufficient data · Provisional');
    expect(svg.querySelectorAll('[data-week="8"]')).toHaveLength(0);
    expect(svg.querySelector('.rt-status-legend')?.textContent).toContain('Blank: no row');
    expect(svg.querySelector('[data-week="1"] title')?.textContent).toBe('Week 1: Insufficient data');
    const description = svg.querySelector(`[id="${svg.getAttribute('aria-describedby')}"]`);
    expect(description?.textContent).toContain('Week 1: Insufficient data');
    expect(description?.textContent).toContain('Week 6: Estimate available · Provisional — estimate withheld');
    expect(svg.querySelectorAll('[data-week="1"] .rt-point, [data-week="6"] .rt-point, [data-week="7"] .rt-point')).toHaveLength(0);
  });
});
