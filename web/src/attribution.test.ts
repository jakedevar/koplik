import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { attributionFor, isArchiveCapture, licenceTerms, licences, sourceEntry, sources } from './attribution';
import { attributionSection, censusFiles } from './attribution-view';
import { mountDashboard } from './app';
import { fixtureDataset } from './fixtures.test-utils';
import { mountProvenanceDrawer, provenanceNumber } from './provenance';

vi.mock('maplibre-gl', () => ({ default: {} }));
afterEach(() => document.body.replaceChildren());

const markdown = readFileSync(resolve(process.cwd(), '../SOURCES.md'), 'utf8');
const rows = (heading: string) => markdown.split(`\n## ${heading}`)[1].split('\n## ')[0].split('\n').filter((l) => l.startsWith('| `'));
const firstId = (row: string) => row.split('|')[1].trim().replace(/`/g, '');
const cells = (row: string) => row.split(' | ').map((c) => c.replace(/^\| /, '').replace(/ \|$/, '').trim());
/** A SOURCES.md terms cell as plain text: code ticks dropped, [text](url) written as "text (url)". */
const plain = (cell: string) => cell.replace(/\[([^\]]+)\]\(([^)]+)\)/g, '$1 ($2)').replace(/`/g, '');

describe('attribution table matches SOURCES.md', () => {
  const licenceRows = rows('Licences and terms');
  it('covers every source_id and licence id SOURCES.md lists', () => {
    const sourceRows = markdown.split('\n').slice(0, 30).filter((l) => l.startsWith('| `'));
    for (const row of sourceRows) {
      for (const id of firstId(row).split(' / ')) expect(sourceEntry(id), id).toBeDefined();
    }
    for (const row of licenceRows) expect(licenceTerms(firstId(row)), firstId(row)).toBeDefined();
    expect(Object.keys(licences).filter((id) => !id.startsWith('synthetic')).sort()).toEqual(licenceRows.map(firstId).sort());
  });
  it('shows the same ruling, terms and attribution text as SOURCES.md for each licence', () => {
    for (const row of licenceRows) {
      const [id, ruling, terms, attribution] = cells(row);
      const entry = licences[id.replace(/`/g, '')];
      expect(entry, id).toBeDefined();
      // The web copy is the SOURCES.md cell, word for word (terms as plain text).
      expect(ruling, id).toBe(entry.ruling);
      expect(plain(terms), id).toBe(entry.terms);
      if (entry.attribution) expect(attribution, id).toBe(`"${entry.attribution}"`);
    }
  });
  it('maps each licence to the same sources as SOURCES.md', () => {
    for (const row of licenceRows) {
      const id = firstId(row);
      const listed = [...cells(row)[4].matchAll(/`([^`]+)`/g)].map((m) => m[1]).sort();
      const mapped = sources.filter((s) => s.licences.includes(id)).flatMap((s) => s.ids).sort();
      expect(mapped, id).toEqual(listed);
    }
  });
  it('maps each ruling: CDC and Census public domain, DSHS none stated with attribution and link', () => {
    expect(licenceTerms('cdc-open-data-terms-unconfirmed')?.ruling).toBe('US federal public domain (17 USC 105)');
    expect(licenceTerms('cdc-schoolvaxview-terms-unconfirmed')?.ruling).toContain('public domain');
    expect(licenceTerms('census-open-data-terms-unconfirmed')?.ruling).toContain('public domain');
    expect(licenceTerms('us-census-public-domain')?.ruling).toContain('public domain');
    for (const id of ['dshs-copyright-noncommercial-no-alteration', 'texas-dshs-terms-unconfirmed']) {
      expect(licenceTerms(id)?.ruling).toContain('none stated');
      expect(licenceTerms(id)?.attribution).toMatch(/https:\/\/www\.dshs\.texas\.gov\//);
    }
    expect(licenceTerms('internet-archive-terms-of-use')?.ruling).toContain("publisher's terms");
  });
  it('gives a live DSHS record the DSHS attribution only, and an Archive capture of it DSHS plus the Archive terms', () => {
    const live = attributionFor('dshs-measles-outbreak-page', 'dshs-copyright-noncommercial-no-alteration', 'https://www.dshs.texas.gov/news-alerts/measles-outbreak-2025');
    expect(live).toHaveLength(1);
    expect(live[0]).toMatch(/^Source: Texas Department of State Health Services \(DSHS\).*https:\/\/www\.dshs\.texas\.gov\//);
    const archived = attributionFor('dshs-measles-outbreak-page-wayback', 'internet-archive-terms-of-use',
      'https://web.archive.org/web/20250305000000id_/https://www.dshs.texas.gov/news-alerts/measles-outbreak-2025');
    expect(archived).toHaveLength(2);
    expect(archived[0]).toMatch(/^Source: Texas Department of State Health Services \(DSHS\)/);
    expect(archived[1]).toMatch(/^Archived copy retrieved through the Internet Archive Wayback Machine/);
    // The URL decides, not the source id: the same source id fetched live or from the Archive.
    expect(attributionFor('dshs-measles-data-report', 'x', 'https://web.archive.org/web/2026id_/https://www.dshs.texas.gov/r.pdf')).toHaveLength(2);
    expect(attributionFor('dshs-measles-data-report-wayback', 'x', 'https://www.dshs.texas.gov/r.pdf')).toHaveLength(1);
    expect(isArchiveCapture('https://web.archive.org/web/1/https://example.invalid/')).toBe(true);
    expect(isArchiveCapture('https://www.dshs.texas.gov/web.archive.org')).toBe(false);
  });
  it('treats unknown licence ids, including inherited object members, as unknown terms', () => {
    for (const id of ['__proto__', 'constructor', 'toString', 'hasOwnProperty']) {
      expect(licenceTerms(id), id).toBeUndefined();
      expect(attributionFor('unlisted-source', id), id).toEqual([]);
    }
  });
});

describe('attribution in the UI', () => {
  it('lists every source with its link and attribution text, and labels synthetic builds', () => {
    const section = attributionSection(fixtureDataset());
    document.body.append(section);
    expect(section.querySelectorAll('.attribution-list > li')).toHaveLength(sources.length);
    const text = section.textContent!;
    expect(text).toContain('Source: Centers for Disease Control and Prevention (CDC), NNDSS Weekly Data');
    expect(text).toContain('Source: US Census Bureau, 2024 cartographic boundary files');
    expect(text).toContain('Texas Department of State Health Services (DSHS)');
    expect(text).toContain('SYNTHETIC');
    const links = [...section.querySelectorAll<HTMLAnchorElement>('.attribution-list > li > a')].map((a) => a.href);
    expect(links).toContain('https://www.dshs.texas.gov/news-alerts/measles-outbreak-2025');
    expect(links).toContain('https://data.cdc.gov/resource/x9gk-5huc');
  });
  it('states that Census files come directly from www2.census.gov from a pinned allowlist, listing each exact URL, and names the Archive county file', () => {
    const section = attributionSection(fixtureDataset());
    const statement = section.querySelector('.census-download-statement')?.textContent;
    expect(statement).toContain('downloaded directly from the US Census Bureau (www2.census.gov)');
    expect(statement).toContain('fixed list of named public-domain files whose SHA-256 hashes are pinned');
    const manifestUrls = ['census-boundaries-2024', 'census-population-2025']
      .flatMap((name) => JSON.parse(readFileSync(resolve(process.cwd(), `../crates/koplik-ingest/manifests/${name}.json`), 'utf8')) as { url: string; sha256: string }[]);
    expect(censusFiles).toEqual(manifestUrls);
    expect(manifestUrls).toHaveLength(6);
    const links = [...section.querySelectorAll<HTMLAnchorElement>('.census-files a')].map((a) => a.href);
    expect(links).toEqual(manifestUrls.map((f) => f.url));
    for (const file of manifestUrls) expect(section.querySelector('.census-files')?.textContent).toContain(file.sha256);
    expect(section.querySelector('.census-county-codes-statement')?.textContent).toContain('Internet Archive (Wayback Machine) capture');
  });
  it('names a data source without recorded terms instead of inventing attribution', () => {
    const data = fixtureDataset();
    data.geographies[0].provenance = [{ ...data.geographies[0].provenance[0], source_id: 'new-source-without-terms' }];
    expect(attributionSection(data).textContent).toContain('without recorded terms or attribution: new-source-without-terms');
  });
  it('is the Sources page, and only there', () => {
    const root = document.createElement('div'); document.body.append(root);
    mountDashboard(root, fixtureDataset(), () => ({ update: vi.fn(), destroy: vi.fn() }));
    expect(root.querySelector('.attribution h2')?.textContent).toBe('Data sources and attribution');
    expect(root.querySelectorAll('.attribution')).toHaveLength(1);
    expect(root.querySelector('[data-page-view="sources"] .attribution')).not.toBeNull();
  });
  it('shows the ruling, terms and attribution beside the licence id in the provenance drawer', () => {
    const root = document.createElement('div'); document.body.append(root);
    mountProvenanceDrawer(root);
    const base = fixtureDataset().cases[0].provenance[0];
    const records = [
      { ...base, source_id: 'cdc-nndss-weekly-measles', licence_id: 'cdc-open-data-terms-unconfirmed', url: 'https://data.cdc.gov/resource/x9gk-5huc' },
      { ...base, source_id: 'dshs-measles-outbreak-page-wayback', licence_id: 'internet-archive-terms-of-use', sha256: 'cd'.repeat(32), url: 'https://web.archive.org/web/20250305000000id_/https://www.dshs.texas.gov/news-alerts/measles-outbreak-2025' },
      { ...base, source_id: 'unlisted-source', licence_id: 'unlisted-licence', sha256: 'ef'.repeat(32) },
      { ...base, source_id: 'dshs-measles-outbreak-page', licence_id: 'dshs-copyright-noncommercial-no-alteration', sha256: '12'.repeat(32), url: 'https://www.dshs.texas.gov/news-alerts/measles-outbreak-2025' },
      { ...base, source_id: 'unlisted-source', licence_id: '__proto__', sha256: '34'.repeat(32) },
    ];
    const number = provenanceNumber('5', { label: 'test number', records });
    root.prepend(number); number.click();
    const sections = [...root.querySelectorAll('.provenance-record')];
    expect(sections[0].textContent).toContain('cdc-open-data-terms-unconfirmed');
    expect(sections[0].querySelector('.provenance-terms')?.textContent).toContain('US federal public domain (17 USC 105)');
    expect(sections[0].querySelector('.provenance-attribution')?.textContent).toContain('Source: Centers for Disease Control and Prevention (CDC)');
    expect(sections[1].querySelector('.provenance-attribution')?.textContent).toContain('Texas Department of State Health Services (DSHS)');
    expect(sections[1].querySelector('.provenance-attribution')?.textContent).toContain('Internet Archive Wayback Machine');
    expect(sections[2].querySelector('.provenance-terms')?.textContent).toBe('Terms not recorded for this licence id.');
    expect(sections[2].querySelector('.provenance-attribution')?.textContent).toBe('Attribution not recorded for this source.');
    expect(sections[3].querySelector('.provenance-attribution')?.textContent).toMatch(/^Source: Texas Department of State Health Services \(DSHS\)[^]*Not affiliated with or endorsed by DSHS\.$/);
    expect(sections[4].querySelector('.provenance-terms')?.textContent).toBe('Terms not recorded for this licence id.');
    expect(sections[4].querySelector('.provenance-attribution')?.textContent).toBe('Attribution not recorded for this source.');
  });
});
