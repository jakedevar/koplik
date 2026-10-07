import { test, expect, type Page } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const source = JSON.parse(readFileSync(resolve('../data/fixtures/web/synthetic-v1/synthetic-weekly-cases.json'), 'utf8'))[0].provenance[0];
const scenarioSource = JSON.parse(readFileSync(resolve('../data/fixtures/seir/synthetic-scenario.json'), 'utf8')).nodes[0].provenance[0];

const disclaimerText = 'Demonstration project; not medical or public-health advice; not affiliated with CDC or WHO.';
const pages = [
  { name: 'Explorer', hash: '#/explorer', heading: 'Measles across the United States' },
  { name: 'Forecast', hash: '#/forecast', heading: 'Where is measles going next?' },
  { name: 'What-if', hash: '#/what-if', heading: 'What if vaccination coverage were different?' },
  { name: 'Sources', hash: '#/sources', heading: 'Data sources and method' },
];
/** Every page: the verbatim disclaimer and the compact attribution footer with its three source links and the Sources link. */
async function expectPageChrome(page: Page, name: string, emptyHash = false) {
  const entry = pages.find((p) => p.name === name)!;
  // The Explorer is also what an empty hash shows (the address then has no page hash).
  if (!emptyHash) await expect(page).toHaveURL(new RegExp(`${entry.hash}$`));
  await expect(page.getByRole('heading', { level: 1, name: entry.heading })).toBeVisible();
  await expect(page.getByRole('navigation', { name: 'Primary' }).getByRole('link', { name })).toHaveAttribute('aria-current', 'page');
  await expect(page.getByRole('navigation', { name: 'Primary' }).locator('[aria-current="page"]')).toHaveCount(1);
  await expect(page.locator('footer .disclaimer')).toHaveText(disclaimerText);
  const footer = page.locator('footer .source-footer');
  await expect(footer.getByRole('link', { name: 'CDC' })).toHaveAttribute('href', 'https://data.cdc.gov/resource/x9gk-5huc');
  await expect(footer.getByRole('link', { name: 'US Census Bureau' })).toHaveAttribute('href', 'https://www.census.gov/');
  await expect(footer).toContainText('Texas DSHS (outbreak data, school coverage)');
  await expect(footer.getByRole('link', { name: 'Texas DSHS outbreak data' })).toHaveAttribute('href', 'https://www.dshs.texas.gov/news-alerts/measles-outbreak-2025');
  await expect(footer.getByRole('link', { name: 'Texas DSHS school coverage' })).toHaveAttribute('href', 'https://www.dshs.texas.gov/immunizations/data/school/coverage');
  await expect(footer.getByRole('link', { name: 'Sources and attribution' })).toHaveAttribute('href', '#/sources');
}
const nav = (page: Page) => page.getByRole('navigation', { name: 'Primary' });

test('fixture dashboard renders the map, recomputes the ensemble and opens accessible provenance', async ({ page, context }) => {
  const appOrigin = new URL(test.info().project.use.baseURL!).origin;
  const external: string[] = [];
  const pageErrors: string[] = [];
  const wasmRequests: string[] = [];
  page.on('pageerror', (error) => pageErrors.push(String(error)));
  page.on('request', (request) => { if (/koplik_wasm|simulation\.worker/.test(request.url())) wasmRequests.push(request.url()); });
  await context.route('**/*', (route) => {
    const url = route.request().url();
    if (url.startsWith(`${appOrigin}/`) || /^(blob|data):/.test(url)) return route.continue();
    external.push(url); return route.abort();
  });
  await page.goto('./');
  await expectPageChrome(page, 'Explorer', true);
  await expect(page.locator('.synthetic').first()).toContainText('SYNTHETIC TEST DATA');
  await expect(page.locator('.maplibregl-canvas')).toBeVisible();
  await page.locator('.maplibregl-canvas').evaluate((node) => { node.dataset.mounted = 'once'; });
  await expect(page.locator('.map')).toHaveAttribute('data-map-state', 'ready');
  await expect(page.locator('.map-status')).toBeEmpty();
  // The Explorer's summary: the as-of date and the headline numbers, each with its coverage and a provenance trigger.
  const summary = page.locator('.summary');
  await expect(summary.locator('.summary-asof')).toContainText('Data as of MMWR');
  await expect(summary.locator('.summary-asof')).toContainText('source snapshot retrieved');
  await expect(summary.locator('.stat-value button')).toHaveCount(3);
  await expect(summary.locator('.stat-note').first()).toContainText(/\d+ of \d+ jurisdictions/);
  await expect(summary.locator('.stat-note').first()).toContainText('not a full-year total');
  // The what-if engine (WASM and worker) is not fetched until its page is first shown.
  expect(wasmRequests).toEqual([]);

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

  await nav(page).getByRole('link', { name: 'What-if' }).click();
  await expectPageChrome(page, 'What-if');
  const initial = page.locator('.engine-fingerprint');
  await expect(initial).toHaveText('7a7471b1ed6d648d9a376d591ed21be513b90128d5f5e7c759c184689d5c25fb');
  const points = await page.locator('.ensemble-median').getAttribute('points');
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

  await nav(page).getByRole('link', { name: 'Forecast' }).click();
  await expectPageChrome(page, 'Forecast');
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

  // The Texas county drill-down charts the cumulative count each DSHS report printed, one point per report, never joined
  // by a line, each point opening its provenance, and says in words that weekly counts are not derived from it.
  await nav(page).getByRole('link', { name: 'Explorer' }).click();
  await expectPageChrome(page, 'Explorer');
  await page.getByRole('button', { name: 'Explore Texas counties →' }).click();
  const cumulative = page.locator('.cumulative-reports');
  await expect(cumulative.getByRole('heading', { name: 'Cumulative confirmed cases as reported by Texas DSHS' })).toBeVisible();
  await expect(cumulative.locator('svg.cumulative-chart circle.cumulative-point')).toHaveCount(3);
  await expect(cumulative.locator('svg.cumulative-chart polyline, svg.cumulative-chart polygon')).toHaveCount(0);
  await expect(cumulative.locator('.cumulative-note')).toContainText('Weekly counts are not derived from this series.');
  await expect(cumulative.locator('.cumulative-gaps')).toHaveText('2025-03-28: DSHS published no county table in this report');
  const cumulativePoint = cumulative.locator('circle.cumulative-point[data-report-date="2025-03-25"]');
  await cumulativePoint.focus(); await cumulativePoint.press('Enter');
  await expect(drawer).toBeVisible();
  await expect(drawer.getByRole('heading')).toContainText('DSHS report of 2025-03-25 · 10 cumulative confirmed cases');
  await expect(drawer).toContainText(source.sha256);
  await expect(drawer).toContainText('It is not a weekly count');
  await close.press('Escape'); await expect(cumulativePoint).toBeFocused();
  // The weekly series keeps its own heading and says why weeks can be No data.
  await expect(page.locator('.charts h3').first()).toHaveText('Weekly confirmed cases');
  await expect(page.locator('.county-weekly-note')).toContainText('Nothing is estimated or interpolated.');

  // The Forecast page follows the geography chosen on the Explorer, while hidden.
  await page.getByRole('button', { name: '← United States' }).click();
  await page.getByLabel('Select a state').selectOption('20');
  await nav(page).getByRole('link', { name: 'Forecast' }).click();
  await expectPageChrome(page, 'Forecast');
  await expect(page.locator('#forecast-geography')).toHaveValue('20');
  await expect(forecast.locator('.forecast-result').locator('> :first-child')).toContainText('We do not publish a forecast for Kansas.');

  // Sources: the full attribution lives here and nowhere else.
  await nav(page).getByRole('link', { name: 'Sources' }).click();
  await expectPageChrome(page, 'Sources');
  const attribution = page.getByRole('region', { name: 'Data sources and attribution' });
  await expect(attribution).toBeVisible();
  await expect(attribution).toContainText('Source: Centers for Disease Control and Prevention (CDC), NNDSS Weekly Data');
  await expect(attribution).toContainText('Source: Texas Department of State Health Services (DSHS)');
  await expect(attribution.getByRole('link', { name: /Texas DSHS 2025 measles outbreak page/ })).toHaveAttribute('href', 'https://www.dshs.texas.gov/news-alerts/measles-outbreak-2025');
  await expect(attribution).toContainText('downloaded directly from the US Census Bureau (www2.census.gov), limited to a fixed list of named public-domain files whose SHA-256 hashes are pinned');
  await expect(attribution.getByRole('link', { name: 'https://www2.census.gov/geo/tiger/GENZ2024/shp/cb_2024_us_state_20m.zip' })).toBeVisible();
  await expect(attribution).toContainText('national_county2020.txt) is the one Census file not downloaded directly from the Census Bureau: Koplik uses an Internet Archive (Wayback Machine) capture');
  await expect(page.locator('.attribution')).toHaveCount(1);
  await expect(page.locator('[data-page-view="sources"] .attribution')).toBeVisible();

  // Navigation never rebuilt the map or the what-if: the same canvas and the recomputed ensemble are still there.
  await nav(page).getByRole('link', { name: 'Explorer' }).click();
  await expectPageChrome(page, 'Explorer');
  await expect(page.locator('.maplibregl-canvas[data-mounted="once"]')).toBeVisible();
  await expect(page.locator('.map')).toHaveAttribute('data-map-state', 'ready');
  await expect(initial).not.toHaveText('7a7471b1ed6d648d9a376d591ed21be513b90128d5f5e7c759c184689d5c25fb');
  // Hidden pages are out of the accessibility tree, so the slider is found by id here.
  await expect(page.locator('#what-if-coverage')).toHaveValue('71');
  expect(external).toEqual([]); expect(pageErrors).toEqual([]);
});

test('pages are reachable by keyboard and direct hash, survive a reload, and follow back and forward', async ({ page }) => {
  const pageErrors: string[] = [];
  page.on('pageerror', (error) => pageErrors.push(String(error)));
  // Direct hash: each page opens on load, and a reload keeps it.
  for (const entry of pages) {
    await page.goto(`./${entry.hash}`);
    await expectPageChrome(page, entry.name);
    await page.reload();
    await expectPageChrome(page, entry.name);
  }
  // An empty hash is the Explorer; an unknown one falls back to it.
  await page.goto('./');
  await expectPageChrome(page, 'Explorer', true);
  await page.goto('./#/no-such-page');
  await expectPageChrome(page, 'Explorer');
  // Keyboard: Tab to a nav link, Enter, and focus lands on the new page's heading.
  await page.goto('./#/explorer');
  await expectPageChrome(page, 'Explorer');
  const forecastLink = nav(page).getByRole('link', { name: 'Forecast' });
  await forecastLink.focus();
  await expect(forecastLink).toBeFocused();
  await forecastLink.press('Enter');
  await expectPageChrome(page, 'Forecast');
  await expect(page.getByRole('heading', { level: 1, name: 'Where is measles going next?' })).toBeFocused();
  await nav(page).getByRole('link', { name: 'Sources' }).click();
  await expectPageChrome(page, 'Sources');
  // Back and forward walk the page history.
  await page.goBack();
  await expectPageChrome(page, 'Forecast');
  await page.goBack();
  await expectPageChrome(page, 'Explorer');
  await page.goForward();
  await expectPageChrome(page, 'Forecast');
  // The Explorer's map is laid out again when it comes back into view.
  await nav(page).getByRole('link', { name: 'Explorer' }).click();
  await expect(page.locator('.maplibregl-canvas')).toBeVisible();
  const [map, canvas] = await Promise.all([page.locator('.map').boundingBox(), page.locator('.maplibregl-canvas').boundingBox()]);
  expect(canvas!.width).toBeGreaterThan(map!.width - 2);
  // Back with the provenance drawer open closes the drawer and lands focus on the page that shows.
  await page.locator('[data-page-view="explorer"] .provenance-number').first().click();
  const drawer = page.getByRole('dialog', { name: 'Number provenance' });
  await expect(drawer).toBeVisible();
  await page.goBack();
  await expectPageChrome(page, 'Forecast');
  await expect(drawer).toBeHidden();
  await expect(page.getByRole('heading', { level: 1, name: 'Where is measles going next?' })).toBeFocused();
  expect(pageErrors).toEqual([]);
});

test('a lazy page whose code fails to load says so, and loads on retry', async ({ page }) => {
  const pageErrors: string[] = [];
  page.on('pageerror', (error) => pageErrors.push(error.message));
  await page.route('**/assets/forecast-view*.js', (route) => route.abort());
  await page.goto('./#/forecast');
  const alert = page.getByRole('alert').filter({ hasText: 'The forecast is unavailable' });
  await expect(alert).toBeVisible();
  expect(pageErrors).toEqual([]);
  await page.unroute('**/assets/forecast-view*.js');
  await alert.getByRole('button', { name: 'Retry loading The forecast' }).click();
  await expect(page.locator('.parameter-citations')).toBeAttached();
  await expect(alert).toBeHidden();
});

test('the navbar fits a narrow screen', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 800 });
  await page.goto('./#/explorer');
  await expectPageChrome(page, 'Explorer');
  for (const entry of pages) {
    const link = nav(page).getByRole('link', { name: entry.name });
    await expect(link).toBeVisible();
    const box = (await link.boundingBox())!;
    expect(box.x).toBeGreaterThanOrEqual(0);
    expect(box.x + box.width).toBeLessThanOrEqual(390);
    expect(box.height).toBeGreaterThanOrEqual(44);
  }
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(390);
});

for (const width of [390, 768]) test(`every page fits a ${width} px screen with its provenance text wrapped, not clipped`, async ({ page }) => {
  await page.setViewportSize({ width, height: 800 });
  for (const entry of pages) {
    await page.goto(`./${entry.hash}`);
    await expectPageChrome(page, entry.name);
    if (entry.name === 'Forecast') await expect(page.locator('.parameter-citations')).toBeAttached();
    // Expanded, too: the Method and parameters table must scroll in its own wrapper, not extend past the panel.
    await page.evaluate(() => document.querySelectorAll('main details').forEach((d) => { (d as HTMLDetailsElement).open = true; }));
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(width);
    // Panels clip overflow, so also require every visible element to end inside the viewport.
    // Content of a horizontal scroll container (.table-scroll) scrolls rather than clips; the container itself must fit.
    const clipped = await page.evaluate((w) => [...document.querySelectorAll('main *')]
      .filter((el) => (el as HTMLElement).offsetParent !== null && !el.parentElement?.closest('.table-scroll'))
      .filter((el) => el.getBoundingClientRect().right > w + 0.5)
      .map((el) => `${el.tagName.toLowerCase()}.${String(el.className)}`), width);
    expect(clipped, entry.name).toEqual([]);
  }
  // The Census file URLs and SHA-256 hashes are shown in full on the Sources page.
  await page.goto('./#/sources');
  const firstFile = page.locator('.census-files li').first();
  await expect(firstFile).toContainText(/SHA-256 [0-9a-f]{64}/);
});
