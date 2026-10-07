import { afterEach, describe, expect, it } from 'vitest';
import { cumulativeChart, cumulativeGapRuns, cumulativeHeading, cumulativeMissingWords, cumulativeSection, cumulativeSeries, cumulativeTable, cumulativeText } from './cumulative';
import type { CumulativeCaseReport } from './generated/v7/CumulativeCaseReport';
import { fixtureDataset } from './fixtures.test-utils';

afterEach(() => document.body.replaceChildren());

const provenance = (tag: string): CumulativeCaseReport['provenance'] => [{ source_id: 'synthetic-web-test', url: `https://example.invalid/${tag}`,
  retrieved_at: '2026-10-07T00:00:00Z', sha256: 'cd'.repeat(32), licence_id: 'synthetic-test-only' }];
const row = (report_date: string, cases: CumulativeCaseReport['cases'], geography = '48165'): CumulativeCaseReport =>
  ({ geography, report_date, cases, case_definition: 'confirmed', provenance: provenance(report_date) });
const count = (n: number) => ({ status: 'reported', count: n }) as const;
const missing = (reason: CumulativeCaseReport['cases'] extends infer C ? C extends { reason: infer R } ? R : never : never) => ({ status: 'missing', reason }) as const;

const rows: CumulativeCaseReport[] = [
  row('2025-03-04', count(107)), row('2025-03-25', count(226)), row('2025-03-28', missing('no_county_table')),
  row('2025-04-22', missing('no_county_table')), row('2025-11-24', count(414)),
];

describe('cumulative series', () => {
  it('keeps one geography in report-date order', () => {
    const mixed = [rows[4], row('2025-03-04', count(1), '48445'), rows[0], rows[2]];
    expect(cumulativeSeries(mixed, '48165').map((r) => r.report_date)).toEqual(['2025-03-04', '2025-03-28', '2025-11-24']);
    expect(cumulativeSeries(mixed, '48141')).toEqual([]);
  });
  it('says a count or "No data" with the reason in plain words, never zero', () => {
    expect(cumulativeText(rows[0])).toBe('107 cumulative confirmed cases');
    expect(cumulativeText(rows[2])).toBe('No data: DSHS published no county table in this report');
    expect(cumulativeText(row('2025-03-11', count(0)))).toBe('0 cumulative confirmed cases');
    expect(Object.keys(cumulativeMissingWords).sort()).toEqual(['ambiguous', 'no_county_table', 'not_labelled_confirmed', 'not_listed']);
  });
  it('groups consecutive reports with the same reason into one line and keeps different reasons apart', () => {
    const runs = cumulativeGapRuns([
      row('2025-03-04', count(1)), row('2025-03-28', missing('no_county_table')), row('2025-04-22', missing('no_county_table')),
      row('2025-05-30', missing('not_labelled_confirmed')), row('2025-06-30', count(5)), row('2025-08-12', missing('no_county_table')),
    ]);
    expect(runs.map((r) => r.text)).toEqual([
      '2025-03-28 to 2025-04-22 (2 reports): DSHS published no county table in this report',
      `2025-05-30: ${cumulativeMissingWords.not_labelled_confirmed}`,
      '2025-08-12: DSHS published no county table in this report',
    ]);
  });
});

describe('cumulative chart', () => {
  it('draws one point per report that printed a count and one × per report that did not, joined by nothing', () => {
    const svg = cumulativeChart(rows, 'Gaines County');
    const points = [...svg.querySelectorAll('circle.cumulative-point')];
    const gaps = [...svg.querySelectorAll('path.cumulative-missing')];
    expect(points.map((p) => p.getAttribute('data-report-date'))).toEqual(['2025-03-04', '2025-03-25', '2025-11-24']);
    expect(points.map((p) => p.getAttribute('data-count'))).toEqual(['107', '226', '414']);
    expect(gaps.map((p) => [p.getAttribute('data-report-date'), p.getAttribute('data-reason')])).toEqual([['2025-03-28', 'no_county_table'], ['2025-04-22', 'no_county_table']]);
    // No line, step, area or curve between reports: nothing is known between them.
    expect(svg.querySelectorAll('polyline, polygon, rect.cumulative-step').length).toBe(0);
    expect([...svg.querySelectorAll('path')].every((p) => p.classList.contains('cumulative-missing'))).toBe(true);
    expect(svg.querySelector('title')?.textContent).toContain('points are not joined');
  });
  it('places points on a date axis and counts on a zero-based axis; a report with no count is not drawn at any height', () => {
    const svg = cumulativeChart(rows, 'Gaines County');
    const at = (date: string) => svg.querySelector(`circle[data-report-date="${date}"]`)!;
    const cx = (date: string) => Number(at(date).getAttribute('cx'));
    const cy = (date: string) => Number(at(date).getAttribute('cy'));
    expect(cx('2025-03-04')).toBe(50);
    expect(cx('2025-11-24')).toBe(610);
    // 2025-03-25 is 21 of 265 days along.
    expect(cx('2025-03-25')).toBeCloseTo(50 + 21 / 265 * 560, 6);
    expect(cy('2025-11-24')).toBe(28);
    expect(cy('2025-03-04')).toBeCloseTo(170 - 107 / 414 * 142, 6);
    const cross = svg.querySelector('path[data-report-date="2025-03-28"]')!;
    expect(cross.getAttribute('d')).toContain(`M${50 + 24 / 265 * 560 - 4} `);
    // Every × sits on the strip under the plot, below the zero line (y = 170), not on a count.
    expect(Number(/M[\d.]+ ([\d.]+)l/.exec(cross.getAttribute('d')!)![1])).toBeGreaterThan(170);
    // Month starts inside the span, thinned to a readable few; the exact dates are in the table and each mark's title.
    expect([...svg.querySelectorAll('text.axis-label')].map((t) => t.textContent).slice(0, 6)).toEqual(['0', '414', 'Apr 2025', 'Jun 2025', 'Aug 2025', 'Oct 2025']);
  });
  it('opens provenance from every mark, with the report it was read from and what the mark is not', () => {
    const svg = cumulativeChart(rows, 'Gaines County', true);
    document.body.append(svg);
    const seen: { label: string; records: unknown[]; note: string; synthetic: boolean }[] = [];
    svg.addEventListener('koplik:provenance', (event) => seen.push((event as CustomEvent).detail));
    const point = svg.querySelector<SVGElement>('circle[data-report-date="2025-03-25"]')!;
    expect(point.getAttribute('role')).toBe('button');
    expect(point.getAttribute('tabindex')).toBe('0');
    point.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    const gap = svg.querySelector<SVGElement>('path[data-report-date="2025-03-28"]')!;
    gap.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    expect(seen).toHaveLength(2);
    expect(seen[0].records).toEqual(rows[1].provenance);
    expect(seen[0].label).toBe('Gaines County · DSHS report of 2025-03-25 · 226 cumulative confirmed cases');
    expect(seen[0].note).toContain('It is not a weekly count');
    expect(seen[0].synthetic).toBe(true);
    expect(seen[1].records).toEqual(rows[2].provenance);
    expect(seen[1].label).toBe('Gaines County · DSHS report of 2025-03-28 · no data');
    expect(seen[1].note).toContain('DSHS published no county table in this report');
    expect(seen[1].note).toContain('unknown, not zero');
  });
  it('draws a single report and a count of zero without inventing a scale or a line', () => {
    const single = cumulativeChart([row('2025-03-04', count(0))], 'Gaines County');
    expect(single.querySelector('circle')?.getAttribute('cx')).toBe('330');
    expect(single.querySelector('circle')?.getAttribute('cy')).toBe('170');
    expect(single.outerHTML).not.toContain('NaN');
    const onlyGaps = cumulativeChart([row('2025-03-04', missing('ambiguous')), row('2025-03-11', missing('not_listed'))], 'Gaines County');
    expect(onlyGaps.querySelectorAll('circle').length).toBe(0);
    expect(onlyGaps.querySelectorAll('path.cumulative-missing').length).toBe(2);
    expect(onlyGaps.outerHTML).not.toContain('NaN');
  });
});

describe('cumulative section and table', () => {
  it('gives the heading, the chart and the plain-words notes: irregular reports, no line, no weekly counts, the reasons listed', () => {
    const blocks = cumulativeSection(rows, '48165', 'Gaines County');
    expect(blocks[0].textContent).toBe('Cumulative confirmed cases as reported by Texas DSHS');
    expect(cumulativeHeading).toBe('Cumulative confirmed cases as reported by Texas DSHS');
    const text = blocks.map((b) => b.textContent).join('\n');
    expect(text).toContain('DSHS published these reports irregularly');
    expect(text).toContain('they are not joined by a line');
    expect(text).toContain('Weekly counts are not derived from this series.');
    expect(text).toContain('2025-03-28 to 2025-04-22 (2 reports): DSHS published no county table in this report');
    expect(blocks.some((b) => b instanceof SVGSVGElement)).toBe(true);
  });
  it('states that a county no report names has no series, and that this is not zero', () => {
    const blocks = cumulativeSection(rows, '48141', 'El Paso County');
    expect(blocks.map((b) => b.textContent)).toEqual([cumulativeHeading, 'No cumulative series for this county: no Texas DSHS report this site reads names it. That is not a count of zero.']);
  });
  it('charts only confirmed-case rows under the confirmed heading and says how many it left out', () => {
    const other: CumulativeCaseReport = { ...rows[0], report_date: '2025-03-11', case_definition: 'confirmed_or_unknown_status' };
    const blocks = cumulativeSection([...rows, other], '48165', 'Gaines County');
    expect(blocks[1].textContent).toBe('1 report rows use a different case definition and are not charted here.');
    expect(blocks.find((b): b is HTMLElement & SVGSVGElement => b instanceof SVGSVGElement)!.querySelectorAll('circle').length).toBe(3);
  });
  it('lists every report date with its exact count or its reason', () => {
    const table = cumulativeTable(rows, 'Gaines County');
    document.body.append(table);
    expect(table.querySelector('caption')?.textContent).toBe(`Gaines County · ${cumulativeHeading}`);
    expect([...table.querySelectorAll('tbody tr')].map((tr) => [tr.getAttribute('data-report-date'), tr.querySelector('td')?.textContent])).toEqual([
      ['2025-03-04', '107'], ['2025-03-25', '226'], ['2025-03-28', 'No data: DSHS published no county table in this report'],
      ['2025-04-22', 'No data: DSHS published no county table in this report'], ['2025-11-24', '414'],
    ]);
    expect(table.querySelectorAll('td button[data-provenance]').length).toBe(3);
  });
});

describe('with the synthetic dataset', () => {
  it('has a validated cumulative series for the Texas counties only, with a report that has no county table', () => {
    const data = fixtureDataset();
    expect(new Set(data.cumulative.map((r) => r.geography))).toEqual(new Set(['48165', '48115']));
    expect(cumulativeSeries(data.cumulative, '48165').map((r) => r.cases.status)).toEqual(['reported', 'reported', 'missing', 'reported']);
  });
});
