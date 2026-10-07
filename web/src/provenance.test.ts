import { afterEach, describe, expect, it, vi } from 'vitest';
import { bindProvenance, mountProvenanceDrawer, provenanceNumber, uniqueProvenance } from './provenance';
import { fixtureDataset, pairedRtRows } from './fixtures.test-utils';
import { mountDashboard } from './app';
import { caseChart, rtChart } from './charts';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

vi.mock('maplibre-gl', () => ({ default: {} }));

const record = fixtureDataset().cases[0].provenance[0];
const scenario = JSON.parse(readFileSync(resolve(process.cwd(), '../data/fixtures/seir/synthetic-scenario.json'), 'utf8'));
const second = scenario.nodes[0].provenance[0];
afterEach(() => document.body.replaceChildren());
function drawer() {
  const root = document.createElement('div'); document.body.append(root);
  const cleanup = mountProvenanceDrawer(root);
  const dialog = root.querySelector<HTMLDialogElement>('dialog')!;
  const number = provenanceNumber('32', { label: 'Texas confirmed cases · 32', records: [record] });
  root.prepend(number);
  return { root, cleanup, dialog, number };
}

describe('provenance drawer', () => {
  it('opens on the number, displays only the linked fields, and moves focus into the drawer', () => {
    const { dialog, number, cleanup } = drawer();
    number.focus(); number.click();
    expect(dialog.open).toBe(true);
    expect(document.activeElement).toBe(dialog.querySelector('button'));
    expect(dialog.textContent).toContain(record.sha256);
    expect(dialog.textContent).toContain(record.retrieved_at);
    expect(dialog.textContent).toContain(record.licence_id);
    expect(dialog.querySelector('a')?.getAttribute('href')).toBe(record.url);
    expect(dialog.textContent).toContain('SYNTHETIC');
    expect(dialog.textContent).toContain('not a published source');
    dialog.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    expect(dialog.open).toBe(false); expect(document.activeElement).toBe(number); cleanup();
  });
  it('traps Tab and Shift-Tab and returns focus on close and native cancellation', () => {
    const { dialog, number, cleanup } = drawer(); number.click();
    const close = dialog.querySelector<HTMLButtonElement>('button')!;
    const last = dialog.querySelector<HTMLAnchorElement>('a')!;
    close.dispatchEvent(new KeyboardEvent('keydown', { key: 'Tab', shiftKey: true, bubbles: true, cancelable: true }));
    expect(document.activeElement).toBe(last);
    last.dispatchEvent(new KeyboardEvent('keydown', { key: 'Tab', bubbles: true, cancelable: true }));
    expect(document.activeElement).toBe(close);
    close.click(); expect(document.activeElement).toBe(number);
    number.click(); dialog.dispatchEvent(new Event('cancel', { cancelable: true }));
    expect(dialog.open).toBe(false); expect(document.activeElement).toBe(number); cleanup();
  });
  it('keeps partial and absent source records missing and renders executable URLs and HTML as text', () => {
    const { root, dialog, cleanup } = drawer();
    const unsafe = provenanceNumber('1', { label: '<img src=x onerror=alert(1)>', records: [{ source_id: '<script>bad</script>', url: 'javascript:alert(1)' }] });
    root.append(unsafe); unsafe.click();
    expect([...dialog.querySelectorAll('dd')].map((node) => node.textContent)).toEqual(['<script>bad</script>', 'Missing', 'javascript:alert(1)', 'Missing', 'Missing']);
    expect(dialog.querySelectorAll('a[href], img, script')).toHaveLength(0);
    expect(dialog.querySelector('h2')?.textContent).toBe('<img src=x onerror=alert(1)>');
    const absent = provenanceNumber('95%', { label: 'User-selected coverage', records: [], note: 'Configuration, not an observation.' });
    root.append(absent); absent.click();
    expect(dialog.textContent).toContain('Source provenance missing');
    expect(dialog.textContent).toContain('Configuration, not an observation.'); cleanup();
  });
  it('deduplicates exact records only, preserving differing licences, URLs, source ids and retrieval times', () => {
    const records = [record, second, record, { ...record, licence_id: 'synthetic-other-terms' }, { ...record, retrieved_at: second.retrieved_at }, { ...record, url: second.url }, { ...record, source_id: second.source_id }];
    expect(uniqueProvenance(records)).toEqual([records[0], records[1], ...records.slice(3)]);
  });
  it('supports SVG keyboard activation and updated provenance without accumulating listeners', () => {
    const { root, dialog, cleanup } = drawer();
    const chart = caseChart(fixtureDataset().cases.filter((r) => r.geography === '48'), 2025, true);
    root.append(chart);
    const mark = chart.querySelector<SVGElement>('.case-bar')!;
    bindProvenance(mark, { label: 'Updated source', records: [second], synthetic: true });
    mark.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    expect(dialog.querySelector('h2')?.textContent).toBe('Updated source');
    expect(dialog.textContent).toContain(second.sha256);
    expect(dialog.querySelectorAll('.provenance-record')).toHaveLength(1); cleanup();
  });
  it('replaces shell listeners and the drawer when remounted', () => {
    const { root, number, cleanup } = drawer();
    const secondCleanup = mountProvenanceDrawer(root); number.click();
    expect(root.querySelectorAll('dialog')).toHaveLength(1);
    expect(root.querySelectorAll('.provenance-record')).toHaveLength(1);
    cleanup(); expect(root.querySelectorAll('dialog')).toHaveLength(1); secondCleanup();
  });
});

describe('published values use their own input provenance', () => {
  it('lists all weekly inputs for a derived total, and updates with coverage, geography and year', () => {
    const data = fixtureDataset();
    data.cases.find((r) => r.geography === '48' && r.week.year === 2025 && r.week.week === 3)!.provenance = [second];
    const root = document.createElement('div'); document.body.append(root);
    const cleanup = mountDashboard(root, data, () => ({ update: () => {}, destroy: () => {} }));
    const openHeadline = () => root.querySelector<HTMLButtonElement>('.headline-value button')!.click();
    openHeadline();
    expect(root.querySelectorAll('.provenance-record')).toHaveLength(2);
    const change = (id: string, value: string) => {
      root.querySelector<HTMLButtonElement>('dialog button')!.click();
      const select = root.querySelector<HTMLSelectElement>(`#${id}`)!; select.value = value; select.dispatchEvent(new Event('change')); openHeadline();
    };
    change('metric', 'coverage');
    expect(root.querySelectorAll('.provenance-record')).toHaveLength(1);
    expect(root.querySelector('dialog h2')?.textContent).toContain('91.2%');
    change('metric', 'cases-2026');
    expect(root.querySelectorAll('.provenance-record')).toHaveLength(1);
    expect(root.querySelector('dialog h2')?.textContent).toContain('3'); cleanup();
  });
  it('opens exact case rows and every attached R_t dependency, without substituting neighbouring case sources', () => {
    const data = fixtureDataset();
    const target = data.rt.find((r) => r.geography === '48' && r.week.week === 2)!;
    target.provenance = [record, second];
    const root = document.createElement('div'); document.body.append(root);
    const cleanup = mountDashboard(root, data, () => ({ update: () => {}, destroy: () => {} }));
    root.querySelector<HTMLButtonElement>('tr[data-week="2025-2"] [data-interval-level] button')!.click();
    expect(root.querySelectorAll('.provenance-record')).toHaveLength(2);
    expect(root.querySelector('dialog')?.textContent).toContain('Derived R_t');
    root.querySelector<HTMLButtonElement>('dialog button')!.click();
    root.querySelector<HTMLButtonElement>('tr[data-week="2025-2"] td button')!.click();
    expect(root.querySelectorAll('.provenance-record')).toHaveLength(1);
    expect(root.querySelector('dialog')?.textContent).toContain(record.sha256);
    const ribbon = rtChart(pairedRtRows(), 2025, true).querySelector<SVGElement>('.rt-ribbon')!;
    root.append(ribbon); ribbon.dispatchEvent(new KeyboardEvent('keydown', { key: ' ', bubbles: true }));
    expect(root.querySelector('dialog h2')?.textContent).toContain('credible interval'); cleanup();
  });
});
