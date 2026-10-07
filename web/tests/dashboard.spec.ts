import { test, expect } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const source = JSON.parse(readFileSync(resolve('../data/fixtures/web/synthetic-v1/synthetic-weekly-cases.json'), 'utf8'))[0].provenance[0];
const scenarioSource = JSON.parse(readFileSync(resolve('../data/fixtures/seir/synthetic-scenario.json'), 'utf8')).nodes[0].provenance[0];

test('fixture dashboard renders the map, recomputes the ensemble and opens accessible provenance', async ({ page, context }) => {
  const appOrigin = new URL(test.info().project.use.baseURL!).origin;
  const external: string[] = [];
  const pageErrors: string[] = [];
  page.on('pageerror', (error) => pageErrors.push(String(error)));
  await context.route('**/*', (route) => {
    const url = route.request().url();
    if (url.startsWith(`${appOrigin}/`) || /^(blob|data):/.test(url)) return route.continue();
    external.push(url); return route.abort();
  });
  await page.goto('./');
  await expect(page.getByRole('heading', { name: 'Measles across the United States' })).toBeVisible();
  const attribution = page.getByRole('region', { name: 'Data sources and attribution' });
  await expect(attribution).toBeVisible();
  await expect(attribution).toContainText('Source: Centers for Disease Control and Prevention (CDC), NNDSS Weekly Data');
  await expect(attribution).toContainText('Source: Texas Department of State Health Services (DSHS)');
  await expect(attribution.getByRole('link', { name: /Texas DSHS 2025 measles outbreak page/ })).toHaveAttribute('href', 'https://www.dshs.texas.gov/news-alerts/measles-outbreak-2025');
  await expect(attribution).toContainText('downloaded directly from the US Census Bureau (www2.census.gov), limited to a fixed list of named public-domain files whose SHA-256 hashes are pinned');
  await expect(attribution.getByRole('link', { name: 'https://www2.census.gov/geo/tiger/GENZ2024/shp/cb_2024_us_state_20m.zip' })).toBeVisible();
  await expect(attribution).toContainText('national_county2020.txt) is the one Census file not downloaded directly from the Census Bureau: Koplik uses an Internet Archive (Wayback Machine) capture');
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

  // The panel says what it is, shows how it was seeded, and cites every parameter.
  const panel = page.locator('.what-if');
  await expect(panel.locator('.hypothetical')).toContainText('This is not a reconstruction or forecast of the 2025 outbreak');
  await expect(panel.locator('.what-if-metadata')).toContainText('Introduced at the start: 5 infectious and 5 exposed in Gaines County (a stated assumption, not data)');
  await panel.getByText('Model parameters and their sources', { exact: true }).click();
  await expect(panel.locator('.parameter-citations tbody tr')).toHaveCount(8);
  await panel.locator('.parameter-citations tbody tr').first().getByRole('button').click();
  await expect(drawer).toContainText('Published source: SYNTHETIC fixture');
  await close.press('Escape');

  // The forecast sits beside its measured backtest skill in plain words, with the exact values and their provenance.
  const forecast = page.locator('.forecast');
  await expect(forecast.getByRole('heading', { name: /Where next/ })).toBeVisible();
  await expect(forecast.locator('svg.forecast-chart')).toBeVisible();
  // First thing above the chart: what is measured about this very series, in plain words with the basis (here Texas has a
  // measured skill); the tests are their own section below.
  // At the very top of the panel: which forecasts are published and which are not, and why.
  await expect(forecast.locator('.forecast-withheld-top')).toContainText('We publish forecasts only for the 1 series whose own test result meets our rule');
  const banner = forecast.locator('.forecast-result').locator('> :first-child');
  await expect(banner).toContainText('Measured skill for this series.');
  await expect(banner).toContainText('In a pseudo-real-time (revised counts truncated at each forecast date) backtest on this series, 90% intervals contained the true count 83.3% of the time (5 of 6)');
  await expect(banner).toContainText('not real-time');
  const evaluation = page.locator('.forecast-evaluation');
  await expect(evaluation.getByRole('heading', { name: 'How we evaluate forecasts' })).toBeVisible();
  // The state-series test is its own block, pooled and labelled pseudo-real-time, apart from the West Texas one.
  await expect(evaluation.locator('.forecast-series-headline')).toContainText('pooled over 2 series and 5 forecasts');
  await expect(evaluation.locator('.forecast-series-scope')).toContainText('2 have a measured skill from this test, 0 have insufficient data for one');
  await expect(evaluation.locator('h3').first()).toContainText('pseudo-real-time (revised counts truncated at each forecast date)');
  await expect(evaluation.locator('.forecast-headline')).toContainText('In a backtest on the synthetic fixture outbreak, 90% intervals contained the true count 50.0% of the time (2 of 4)');
  await expect(evaluation.locator('.forecast-narrow')).toContainText('In this backtest the intervals were too narrow');
  await expect(evaluation.locator('.forecast-evaluation-scope')).toContainText("does not measure how any other series' forecast will do");
  await forecast.getByText('Exact forecast values', { exact: true }).click();
  const forecastMedian = forecast.locator('.forecast-result tbody tr').first().getByRole('button').first();
  await forecastMedian.click(); await expect(drawer).toBeVisible();
  await expect(drawer).toContainText('SYNTHETIC');
  await expect(drawer).toContainText('not a source observation');
  await close.press('Escape'); await expect(forecastMedian).toBeFocused();
  // A series the test ran on but scored too little for has no measured skill, and says what was scored.
  await forecast.getByLabel('Forecast for').selectOption('20');
  await expect(forecast.locator('.forecast-result').locator('> :first-child')).toContainText('We do not publish a forecast for Kansas.');
  await expect(forecast.locator('.forecast-result').locator('> :first-child')).toContainText('does not meet our rule');
  await expect(forecast.locator('svg.forecast-chart')).toHaveCount(0);
  await forecast.getByLabel('Forecast for').selectOption('40');
  await expect(forecast.locator('.forecast-insufficient')).toContainText('Insufficient data');
  await expect(forecast.locator('svg.forecast-chart')).toHaveCount(0);
  expect(external).toEqual([]); expect(pageErrors).toEqual([]);
});
