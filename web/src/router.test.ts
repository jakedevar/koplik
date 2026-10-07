import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { mountDashboard } from './app';
import { footerSources } from './attribution';
import { fixtureDataset } from './fixtures.test-utils';
import { mountRouter, pageEvent, pageFromHash, pageHash, pageIds, pageTitles } from './router';

// MapLibre needs a real browser; the map is injected.
vi.mock('maplibre-gl', () => ({ default: {} }));

let scrolled = 0;
beforeEach(() => { scrolled = 0; vi.spyOn(window, 'scrollTo').mockImplementation(() => { scrolled++; }); });
afterEach(() => { vi.restoreAllMocks(); document.body.replaceChildren(); history.replaceState(null, '', '#'); });

function mount() {
  const root = document.createElement('div'); document.body.append(root);
  const cleanup = mountDashboard(root, fixtureDataset(), () => ({ update: () => {}, destroy: () => {}, resize: () => { resizes++; } }));
  return { root, cleanup };
}
let resizes = 0;
const go = (hash: string) => { history.replaceState(null, '', hash); window.dispatchEvent(new HashChangeEvent('hashchange')); };
const visible = (root: HTMLElement) => pageIds.filter((id) => !root.querySelector<HTMLElement>(`[data-page-view="${id}"]`)!.hidden);
const current = (root: HTMLElement) => [...root.querySelectorAll<HTMLAnchorElement>('nav[aria-label="Primary"] a[aria-current="page"]')].map((a) => a.dataset.page);

describe('pageFromHash', () => {
  it('maps each page hash to its page', () => {
    expect(pageFromHash('#/explorer')).toBe('explorer');
    expect(pageFromHash('#/forecast')).toBe('forecast');
    expect(pageFromHash('#/what-if')).toBe('what-if');
    expect(pageFromHash('#/sources')).toBe('sources');
    for (const id of pageIds) expect(pageFromHash(pageHash(id))).toBe(id);
  });
  it('falls back to the Explorer for an empty or unknown hash', () => {
    for (const hash of ['', '#', '#/', '#/nonsense', '#forecast-geography', '#/forecast/extra', '#/Forecast', '#/__proto__']) expect(pageFromHash(hash), hash).toBe('explorer');
  });
});

describe('router', () => {
  it('shows the Explorer for an empty hash and marks it current', () => {
    history.replaceState(null, '', '#');
    const { root } = mount();
    expect(visible(root)).toEqual(['explorer']);
    expect(current(root)).toEqual(['explorer']);
  });
  it('lists the four pages in a primary nav, each a link to its hash', () => {
    const { root } = mount();
    const links = [...root.querySelectorAll<HTMLAnchorElement>('header nav[aria-label="Primary"] a')];
    expect(links.map((a) => a.textContent)).toEqual(['Explorer', 'Forecast', 'What-if', 'Sources']);
    expect(links.map((a) => a.getAttribute('href'))).toEqual(['#/explorer', '#/forecast', '#/what-if', '#/sources']);
  });
  it('opens the page a deep link names, on first paint', () => {
    history.replaceState(null, '', '#/sources');
    const { root } = mount();
    expect(visible(root)).toEqual(['sources']);
    expect(current(root)).toEqual(['sources']);
    expect(document.title).toBe('Sources · Koplik');
  });
  it('follows hash changes (back, forward and clicks), one current link at a time', () => {
    const { root } = mount();
    for (const page of ['forecast', 'what-if', 'sources', 'explorer'] as const) {
      go(pageHash(page));
      expect(visible(root), page).toEqual([page]);
      expect(current(root), page).toEqual([page]);
      expect(document.title).toBe(page === 'explorer' ? 'Koplik · Measles outbreak intelligence' : `${pageTitles[page]} · Koplik`);
    }
  });
  it('falls back to the Explorer for an unknown hash and rewrites it', () => {
    const { root } = mount();
    go('#/forecast'); go('#/nope');
    expect(visible(root)).toEqual(['explorer']);
    expect(current(root)).toEqual(['explorer']);
    expect(location.hash).toBe('#/explorer');
  });
  it('moves focus to the new page heading on navigation, not on first paint', () => {
    history.replaceState(null, '', '#/forecast');
    const { root } = mount();
    expect(document.activeElement).toBe(document.body);
    go('#/what-if');
    expect(scrolled).toBe(1);
    expect(document.activeElement).toBe(root.querySelector('[data-page-view="what-if"] h1'));
    go('#/explorer');
    expect(document.activeElement).toBe(root.querySelector('[data-page-view="explorer"] h1'));
  });
  it('announces each page and resizes the map when the Explorer shows again', () => {
    const { root } = mount();
    const seen: string[] = [];
    root.addEventListener(pageEvent, (e) => seen.push((e as CustomEvent<{ page: string }>).detail.page));
    resizes = 0;
    go('#/forecast'); expect(resizes).toBe(0);
    go('#/explorer'); expect(resizes).toBe(1);
    expect(seen).toEqual(['forecast', 'explorer']);
  });
  it('closes an open provenance drawer when the page changes (Back or Forward) and focuses the new page h1', () => {
    history.replaceState(null, '', '#/explorer');
    const { root } = mount();
    const dialog = root.querySelector<HTMLDialogElement>('dialog.provenance-drawer')!;
    const trigger = root.querySelector<HTMLElement>('[data-page-view="explorer"] .provenance-number')!;
    trigger.focus(); trigger.click();
    expect(dialog.open).toBe(true);
    go('#/forecast');
    expect(dialog.open).toBe(false);
    expect(current(root)).toEqual(['forecast']);
    expect(document.activeElement).toBe(root.querySelector('[data-page-view="forecast"] h1'));
  });
  it('stops listening after cleanup', () => {
    const root = document.createElement('div'); document.body.append(root);
    root.innerHTML = '<nav aria-label="Primary"><a data-page="explorer"></a><a data-page="forecast"></a></nav><div data-page-view="explorer"></div><div data-page-view="forecast" hidden></div>';
    history.replaceState(null, '', '#');
    const cleanup = mountRouter(root);
    cleanup();
    go('#/forecast');
    expect(root.querySelector<HTMLElement>('[data-page-view="forecast"]')!.hidden).toBe(true);
  });
});

describe('every page', () => {
  it('carries the verbatim disclaimer and the source footer with the provider links (DSHS twice) and a Sources link', () => {
    const { root } = mount();
    const footer = root.querySelector('footer')!;
    expect(footer.querySelector('.disclaimer')?.textContent).toBe('Demonstration project; not medical or public-health advice; not affiliated with CDC or WHO.');
    const links = [...footer.querySelectorAll<HTMLAnchorElement>('.source-footer a')];
    expect(footer.querySelector('.source-footer')?.textContent).toBe('Data: CDC, US Census Bureau, Texas DSHS (outbreak data, school coverage) · Sources and attribution');
    expect(links.map((a) => a.textContent)).toEqual(['CDC', 'US Census Bureau', 'outbreak data', 'school coverage', 'Sources and attribution']);
    expect(links[0].href).toBe(footerSources[0].url);
    expect(links[1].href).toBe('https://www.census.gov/');
    expect(links[2].href).toBe('https://www.dshs.texas.gov/news-alerts/measles-outbreak-2025');
    expect(links[3].href).toBe('https://www.dshs.texas.gov/immunizations/data/school/coverage');
    expect(links[2].getAttribute('aria-label')).toBe('Texas DSHS outbreak data');
    expect(links[4].getAttribute('href')).toBe('#/sources');
    // The footer sits outside the page views, so it is visible whichever page is open.
    for (const link of links) expect(root.querySelector('main')!.contains(link)).toBe(false);
  });
  it('gives each page an h1 and keeps the full attribution on the Sources page only', () => {
    const { root } = mount();
    for (const id of pageIds) expect(root.querySelectorAll(`[data-page-view="${id}"] h1`), id).toHaveLength(1);
    expect(root.querySelector('[data-page-view="sources"] .attribution-list')).not.toBeNull();
    expect(root.querySelectorAll('.attribution-list')).toHaveLength(1);
    expect(root.querySelectorAll('.census-files')).toHaveLength(1);
  });
});
