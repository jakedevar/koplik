import { test, expect } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const source = JSON.parse(readFileSync(resolve('../data/fixtures/web/synthetic-v1/synthetic-weekly-cases.json'), 'utf8'))[0].provenance[0];
const scenarioSource = JSON.parse(readFileSync(resolve('../data/fixtures/seir/synthetic-scenario.json'), 'utf8')).nodes[0].provenance[0];

test('fixture dashboard renders the map, recomputes the ensemble and opens accessible provenance', async ({ page, context }) => {
  const external: string[] = [];
  const pageErrors: string[] = [];
  page.on('pageerror', (error) => pageErrors.push(String(error)));
  await context.route('**/*', (route) => {
    const url = route.request().url();
    if (url.startsWith('http://127.0.0.1:5158/') || /^(blob|data):/.test(url)) return route.continue();
    external.push(url); return route.abort();
  });
  await page.goto('./');
  await expect(page.getByRole('heading', { name: 'Measles across the United States' })).toBeVisible();
  const attribution = page.getByRole('region', { name: 'Data sources and attribution' });
  await expect(attribution).toBeVisible();
  await expect(attribution).toContainText('Source: Centers for Disease Control and Prevention (CDC), NNDSS Weekly Data');
  await expect(attribution).toContainText('Source: Texas Department of State Health Services (DSHS)');
  await expect(attribution.getByRole('link', { name: /Texas DSHS 2025 measles outbreak page/ })).toHaveAttribute('href', 'https://www.dshs.texas.gov/news-alerts/measles-outbreak-2025');
  await expect(page.locator('.synthetic').first()).toContainText('SYNTHETIC TEST DATA');
  await expect(page.locator('.maplibregl-canvas')).toBeVisible();
  await expect(page.locator('.map')).toHaveAttribute('data-map-state', 'ready');
  await expect(page.locator('.map-status')).toBeEmpty();
  const initial = page.locator('.engine-fingerprint');
  await expect(initial).toHaveText('7a7471b1ed6d648d9a376d591ed21be513b90128d5f5e7c759c184689d5c25fb');
  const points = await page.locator('.ensemble-median').getAttribute('points');

  const total = page.locator('.headline-value button');
  await total.focus(); await total.press('Enter');
  const drawer = page.getByRole('dialog', { name: 'Number provenance' });
  await expect(drawer).toBeVisible();
  const close = drawer.getByRole('button', { name: 'Close provenance' });
  await expect(close).toBeFocused();
  await expect(drawer).toContainText(source.sha256);
  await expect(drawer.getByRole('link')).toHaveAttribute('href', source.url);
  await expect(drawer).toContainText(source.retrieved_at);
  await expect(drawer).toContainText(source.licence_id);
  await expect(drawer).toContainText('not a published source');
  await expect(drawer.locator('.provenance-terms')).toBeVisible();
  await expect(drawer.locator('.provenance-attribution')).toBeVisible();
  await close.press('Shift+Tab'); await expect(drawer.getByRole('link')).toBeFocused();
  await drawer.getByRole('link').press('Tab'); await expect(close).toBeFocused();
  await close.press('Escape'); await expect(drawer).toBeHidden(); await expect(total).toBeFocused();

  const plottedNumber = page.locator('.case-bar[data-week="2"]');
  await plottedNumber.focus(); await plottedNumber.press('Space');
  await expect(drawer).toBeVisible();
  await expect(drawer.getByRole('heading')).toContainText('2 confirmed or unknown-status cases');
  await close.press('Escape'); await expect(plottedNumber).toBeFocused();

  const slider = page.getByRole('slider', { name: 'Gaines County kindergarten MMR coverage' });
  await slider.focus(); await slider.press('ArrowRight');
  await expect(slider).toHaveValue('71');
  await expect(page.locator('.what-if')).toHaveAttribute('aria-busy', 'false');
  await expect(initial).toHaveText(/^[0-9a-f]{64}$/);
  await expect(initial).not.toHaveText('7a7471b1ed6d648d9a376d591ed21be513b90128d5f5e7c759c184689d5c25fb');
  await expect(page.locator('.ensemble-median')).not.toHaveAttribute('points', points!);
  await expect(page.locator('.ensemble-band-50')).toBeVisible();
  await expect(page.locator('.ensemble-band-90')).toBeVisible();

  await page.getByText('Exact daily ensemble values', { exact: true }).click();
  const median = page.locator('.what-if-result tbody tr').last().locator('td').first().getByRole('button');
  await median.click(); await expect(drawer).toBeVisible();
  await expect(drawer).toContainText(scenarioSource.sha256);
  await expect(drawer).toContainText(scenarioSource.url);
  await expect(drawer).toContainText('Derived from the exact replay scenario, across all its counties');
  await expect(drawer).toContainText('SYNTHETIC');
  await close.press('Escape'); await expect(median).toBeFocused();
  expect(external).toEqual([]); expect(pageErrors).toEqual([]);
});
