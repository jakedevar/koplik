import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { attributionFor, licenceTerms, licences, sourceEntry, sources } from './attribution';
import { attributionSection } from './attribution-view';
import { mountDashboard } from './app';
import { fixtureDataset } from './fixtures.test-utils';
import { mountProvenanceDrawer, provenanceNumber } from './provenance';

vi.mock('maplibre-gl', () => ({ default: {} }));
afterEach(() => document.body.replaceChildren());

const markdown = readFileSync(resolve(process.cwd(), '../SOURCES.md'), 'utf8');
const rows = (heading: string) => markdown.split(`\n## ${heading}`)[1].split('\n## ')[0].split('\n').filter((l) => l.startsWith('| `'));
const firstId = (row: string) => row.split('|')[1].trim().replace(/`/g, '');

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
  it('shows the same ruling and attribution text as SOURCES.md for each licence', () => {
    for (const row of licenceRows) {
      const [id, ruling, , attribution] = row.split(' | ').map((c) => c.replace(/^\| /, '').trim());
      const terms = licences[id.replace(/`/g, '')];
      expect(terms, id).toBeDefined();
      // The web ruling is the SOURCES.md ruling cell, word for word.
      expect(ruling, id).toBe(terms.ruling);
      if (terms.attribution) expect(attribution, id).toBe(`"${terms.attribution}"`);
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
    // An Archive capture of DSHS content carries both attributions.
    expect(attributionFor('dshs-measles-outbreak-page-wayback', 'internet-archive-terms-of-use').join(' ')).toMatch(/Texas Department of State Health Services.*Internet Archive/);
  });
});

describe('attribution in the UI', () => {
  it('lists every source with its link and attribution text, and labels synthetic builds', () => {
    const section = attributionSection(fixtureDataset());
    document.body.append(section);
    expect(section.querySelectorAll('li')).toHaveLength(sources.length);
    const text = section.textContent!;
    expect(text).toContain('Source: Centers for Disease Control and Prevention (CDC), NNDSS Weekly Data');
    expect(text).toContain('Source: US Census Bureau, 2024 cartographic boundary files');
    expect(text).toContain('Texas Department of State Health Services (DSHS)');
    expect(text).toContain('SYNTHETIC');
    const links = [...section.querySelectorAll<HTMLAnchorElement>('li > a')].map((a) => a.href);
    expect(links).toContain('https://www.dshs.texas.gov/news-alerts/measles-outbreak-2025');
    expect(links).toContain('https://data.cdc.gov/resource/x9gk-5huc');
  });
  it('names a data source without recorded terms instead of inventing attribution', () => {
    const data = fixtureDataset();
    data.geographies[0].provenance = [{ ...data.geographies[0].provenance[0], source_id: 'new-source-without-terms' }];
    expect(attributionSection(data).textContent).toContain('without recorded terms or attribution: new-source-without-terms');
  });
  it('is part of the dashboard', () => {
    const root = document.createElement('div'); document.body.append(root);
    mountDashboard(root, fixtureDataset(), () => ({ update: vi.fn(), destroy: vi.fn() }));
    expect(root.querySelector('.attribution h3')?.textContent).toBe('Data sources and attribution');
  });
  it('shows the ruling, terms and attribution beside the licence id in the provenance drawer', () => {
    const root = document.createElement('div'); document.body.append(root);
    mountProvenanceDrawer(root);
    const base = fixtureDataset().cases[0].provenance[0];
    const records = [
      { ...base, source_id: 'cdc-nndss-weekly-measles', licence_id: 'cdc-open-data-terms-unconfirmed' },
      { ...base, source_id: 'dshs-measles-outbreak-page-wayback', licence_id: 'internet-archive-terms-of-use', sha256: 'cd'.repeat(32) },
      { ...base, source_id: 'unlisted-source', licence_id: 'unlisted-licence', sha256: 'ef'.repeat(32) },
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
  });
});
