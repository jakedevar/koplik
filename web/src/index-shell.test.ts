import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';
import { mountDashboard } from './app';
import { fixtureDataset } from './fixtures.test-utils';

// index.html paints the shell before any script runs; the script then draws the same shell. They must not drift apart.
describe('the static shell in index.html', () => {
  const html = new DOMParser().parseFromString(readFileSync(resolve(process.cwd(), 'index.html'), 'utf8'), 'text/html');
  const root = document.createElement('div');
  document.body.append(root);
  const cleanup = mountDashboard(root, fixtureDataset(), () => ({ update: () => {}, destroy: () => {} }));
  const links = (scope: ParentNode) => [...scope.querySelectorAll<HTMLAnchorElement>('.primary-nav a')].map((a) => [a.textContent, a.getAttribute('href'), a.dataset.page]);

  it('has the same brand, tagline and navigation as the app', () => {
    expect(html.querySelector('.brand')?.textContent).toBe(root.querySelector('.brand')?.textContent);
    expect(html.querySelector('.brand')?.getAttribute('href')).toBe(root.querySelector('.brand')?.getAttribute('href'));
    expect(html.querySelector('.tagline')?.textContent).toBe(root.querySelector('.tagline')?.textContent);
    expect(links(html)).toEqual(links(root));
    expect(html.querySelector('.primary-nav')?.getAttribute('aria-label')).toBe('Primary');
  });
  it('has the Explorer heading and introduction, and a loading status', () => {
    const explorer = root.querySelector('[data-page-view="explorer"]')!;
    expect(html.querySelector('.page h1')?.textContent).toBe(explorer.querySelector('h1')?.textContent);
    expect(html.querySelector('.page .eyebrow')?.textContent).toBe(explorer.querySelector('.eyebrow')?.textContent);
    expect(html.querySelector('.page .intro')?.textContent).toBe(explorer.querySelector('.intro')?.textContent);
    expect(html.querySelector('[role="status"]')?.textContent).toBe('Loading pipeline artifacts…');
    cleanup();
  });
});
