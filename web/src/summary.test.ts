import { describe, expect, it } from 'vitest';
import { explorerSummary, formatRetrieved } from './summary';
import { summarySection } from './summary-view';
import type { Dataset } from './data';
import type { WeeklyCaseCount } from './generated/v3/WeeklyCaseCount';
import type { Provenance } from './generated/Provenance';

const prov = (retrieved_at: string, sha = 'a'.repeat(64)): Provenance => ({ source_id: 'cdc-nndss-weekly-measles', url: 'https://example.test/x', retrieved_at, sha256: sha, licence_id: 'cdc-open-data-terms-unconfirmed' });
const reported = (geography: string, year: number, week: number, count: number, p = prov('2026-10-07T09:55:29Z'), definition: WeeklyCaseCount['case_definition'] = 'confirmed_or_unknown_status'): WeeklyCaseCount =>
  ({ geography, week: { year, week }, cases: { status: 'reported', count }, case_definition: definition, provenance: [p] });
const missing = (geography: string, year: number, week: number): WeeklyCaseCount =>
  ({ geography, week: { year, week }, cases: { status: 'missing', reason: 'not_reported' }, case_definition: 'confirmed_or_unknown_status', provenance: [prov('2026-10-07T09:55:29Z')] });
const geo = (id: string, name: string) => ({ id, name, level: 'state' as const, provenance: [prov('2026-10-07T09:55:29Z')] });
function dataset(cases: WeeklyCaseCount[], extra: Partial<Dataset> = {}): Dataset {
  return { geographies: [geo('01', 'Alabama'), geo('02', 'Alaska'), geo('04', 'Arizona'), geo('05', 'Arkansas')], cases, coverage: [], rt: [], cumulative: [],
    states: { type: 'FeatureCollection', features: [] }, counties: { type: 'FeatureCollection', features: [] }, synthetic: false, ...extra } as Dataset;
}

describe('explorer summary', () => {
  const later = prov('2026-10-08T11:00:00Z', 'b'.repeat(64));
  const cases = [
    reported('01', 2025, 1, 5), reported('01', 2025, 2, 7),            // Alabama: complete, 12
    reported('02', 2025, 1, 0), reported('02', 2025, 2, 3),            // Alaska: complete, 3
    reported('04', 2025, 1, 4), missing('04', 2025, 2),                // Arizona: a missing week, so no 2025 figure
    // Arkansas: no rows at all.
    reported('01', 2026, 1, 2), reported('02', 2026, 1, 1, later), reported('04', 2026, 1, 9),
    reported('01', 2026, 2, 6), reported('02', 2026, 2, 0, later),
  ];
  const summary = explorerSummary(dataset(cases));
  const stat = (id: string) => summary.stats.find((s) => s.id === id)!;

  it('sums only jurisdictions with a complete series and counts the rest as missing, never as zero', () => {
    expect(stat('cases-2025').value).toBe(15);
    expect(stat('cases-2025')).toMatchObject({ included: 2, total: 4, notIncluded: ['Arizona', 'Arkansas'] });
    expect(stat('cases-2025').detail).toContain('2 of 4 jurisdictions');
    expect(stat('cases-2025').detail).toContain('not counted as zero');
    expect(stat('cases-2025').detail).toContain('not a full-year total');
    expect(stat('cases-2026').value).toBe(18);
    expect(stat('cases-2026')).toMatchObject({ included: 3, notIncluded: ['Arkansas'] });
  });
  it('sums the latest week over the jurisdictions that reported it, with their coverage', () => {
    expect(summary.asOf?.week).toEqual({ year: 2026, week: 2 });
    expect(stat('latest-week').value).toBe(6);
    expect(stat('latest-week')).toMatchObject({ included: 2, total: 4, notIncluded: ['Arizona', 'Arkansas'] });
    expect(stat('latest-week').label).toBe('New cases · latest week, MMWR 2026 W2');
    expect(stat('latest-week').detail).toContain('2 of 4 jurisdictions reported this week');
  });
  it('takes the as-of time from the records of the latest week, not from the clock', () => {
    expect(summary.asOf?.retrieved).toBe('2026-10-08T11:00:00Z');
    expect(formatRetrieved('2026-10-08T11:00:00Z')).toBe('2026-10-08 11:00 UTC');
    expect(formatRetrieved('not a time')).toBe('not a time');
  });
  it('carries the source records of every summed row', () => {
    expect(stat('cases-2025').records.map((r) => r.sha256)).toEqual(['a'.repeat(64)]);
    expect(stat('cases-2026').records.map((r) => r.sha256).sort()).toEqual(['a'.repeat(64), 'b'.repeat(64)]);
    expect(stat('latest-week').records.map((r) => r.sha256).sort()).toEqual(['a'.repeat(64), 'b'.repeat(64)]);
  });
  it('never mixes case definitions: a jurisdiction on another definition is not included', () => {
    const mixed = explorerSummary(dataset([...cases, reported('05', 2025, 1, 100, prov('2026-10-07T09:55:29Z'), 'confirmed'), reported('05', 2025, 2, 100, prov('2026-10-07T09:55:29Z'), 'confirmed')]));
    expect(mixed.stats[0].value).toBe(15);
    expect(mixed.stats[0].notIncluded).toContain('Arkansas');
  });
  it('shows no data, not zero, when no jurisdiction has a figure', () => {
    const none = explorerSummary(dataset([]));
    expect(none.asOf).toBeUndefined();
    expect(none.stats.map((s) => [s.value, s.included, s.total])).toEqual([[null, 0, 4], [null, 0, 4], [null, 0, 4]]);
    const node = summarySection(none, false);
    expect(node.textContent).toContain('As-of date unavailable');
    expect(node.querySelectorAll('.stat-value')).toHaveLength(3);
    expect([...node.querySelectorAll('.stat-value')].map((v) => v.textContent)).toEqual(['No data', 'No data', 'No data']);
    expect(node.querySelectorAll('button')).toHaveLength(0);
  });
  it('renders each number as a provenance trigger with its coverage and the as-of line', () => {
    const node = summarySection(summary, false);
    expect(node.querySelector('.summary-asof')?.textContent).toBe('Data as of MMWR 2026 W2, the latest week with reports (provisional: still subject to reporting delay) · source snapshot retrieved 2026-10-08 11:00 UTC');
    const values = [...node.querySelectorAll('.stat-value button')];
    expect(values.map((v) => v.textContent)).toEqual(['15', '18', '6']);
    expect(node.querySelector('[data-stat="cases-2025"] .stat-note')?.textContent).toContain('2 of 4 jurisdictions');
    const seen: { label: string; note: string; records: unknown[] }[] = [];
    document.body.append(node);
    node.addEventListener('koplik:provenance', (event) => seen.push((event as CustomEvent).detail));
    (values[0] as HTMLElement).click();
    expect(seen[0].label).toBe('United States · Reported cases · 2025 · 15');
    expect(seen[0].note).toContain('Not included (no figure for it from the source\'s reports): Arizona, Arkansas.');
    expect(seen[0].records).toHaveLength(1);
    node.remove();
  });
  describe('the latest week is provisional, as the pipeline flags it', () => {
    const rtRow = (geography: string, week: number, provisional: boolean) => ({ geography, week: { year: 2026, week }, interval_level: 0.9, status: 'insufficient_data', provisional, provenance: [prov('2026-10-07T09:55:29Z')] }) as unknown as Dataset['rt'][number];
    const flagged = explorerSummary(dataset(cases, { rt: [rtRow('01', 2, true), rtRow('02', 2, true), rtRow('01', 1, false)] }));
    it('carries the provisional flag on the card, the as-of line and both drawers, with the count unchanged', () => {
      expect(flagged.stats.find((s) => s.id === 'latest-week')).toMatchObject({ value: 6, provisional: true });
      expect(flagged.asOf?.provisional).toBe(true);
      const node = summarySection(flagged, false);
      expect(node.querySelector('[data-stat="latest-week"] .stat-caveat')?.textContent).toBe('Provisional: this week is still subject to reporting delay and may change.');
      expect(node.querySelectorAll('.stat-caveat')).toHaveLength(1);
      expect(node.querySelector('.summary-asof')?.textContent).toContain('(provisional: still subject to reporting delay)');
      expect([...node.querySelectorAll('.stat-value button')].map((v) => v.textContent)).toEqual(['15', '18', '6']);
      const seen: { note: string }[] = [];
      document.body.append(node);
      node.addEventListener('koplik:provenance', (event) => seen.push((event as CustomEvent).detail));
      (node.querySelector('[data-stat="latest-week"] .stat-value button') as HTMLElement).click();
      (node.querySelector('.summary-asof button') as HTMLElement).click();
      expect(seen[0].note).toContain('Provisional (reporting delay)');
      expect(seen[1].note).toContain('Provisional (reporting delay)');
      node.remove();
    });
    it('is not flagged when the pipeline marks that week final', () => {
      const final = explorerSummary(dataset(cases, { rt: [rtRow('01', 2, false)] }));
      expect(final.asOf?.provisional).toBe(false);
      expect(summarySection(final, false).querySelectorAll('.stat-caveat')).toHaveLength(0);
      expect(summarySection(final, false).querySelector('.summary-asof')?.textContent).toBe('Data as of MMWR 2026 W2, the latest week with reports · source snapshot retrieved 2026-10-08 11:00 UTC');
    });
  });
  it('explains the latest-week sum for that week alone, and leaves the annual notes as they were', () => {
    const node = summarySection(summary, false);
    const seen: { label: string; note: string }[] = [];
    document.body.append(node);
    node.addEventListener('koplik:provenance', (event) => seen.push((event as CustomEvent).detail));
    (node.querySelector('[data-stat="latest-week"] .stat-value button') as HTMLElement).click();
    (node.querySelector('[data-stat="cases-2026"] .stat-value button') as HTMLElement).click();
    const [latest, annual] = seen.map((e) => e.note);
    expect(latest).toContain('A sum over 2 of 4 state-level jurisdictions');
    expect(latest).toContain('have a reported count for this one week');
    expect(latest).toContain('left out only when its count for this week is absent or missing');
    expect(latest).toContain('Not included (no figure for it from the source\'s reports): Arizona, Arkansas.');
    expect(annual).toContain('A jurisdiction with missing or incomplete weekly reports is left out');
    expect(annual).toContain('this is not a full-year total');
    node.remove();
  });
});
