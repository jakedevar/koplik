import { test, expect } from '@playwright/test';
import { outlierRtRows } from './rt-outlier-fixture';

for (const width of [390, 1280]) test(`R_t ordinary weeks remain readable and off-scale values discoverable at ${width} px`, async ({ page }) => {
  await page.setViewportSize({ width, height: 900 });
  const rows = outlierRtRows();
  const outlier = rows[39];
  // Explicitly synthetic stress data, derived from the committed offline fixture.
  await page.route('**/data/synthetic-v1/synthetic-rt.json', (route) => route.fulfill({
    contentType: 'application/json', body: JSON.stringify(rows),
  }));
  await page.goto('./#/explorer');
  await expect(page.locator('.synthetic').first()).toContainText('SYNTHETIC TEST DATA');
  const view = page.locator('.rt-chart-view');
  const toggle = view.getByRole('button', { name: 'Show full range', exact: true });
  await expect(toggle).toHaveAttribute('aria-pressed', 'false');
  const svg = view.locator('svg');
  await expect(svg).toHaveAttribute('data-axis-maximum', '3');
  await expect(view.locator('.rt-reference-label')).toHaveText('R_t = 1');
  await view.scrollIntoViewIfNeeded();
  // Measure in screen pixels: the reference and an ordinary point must stand clear of zero.
  const spacing = await svg.evaluate((node) => {
    const svg = node as SVGSVGElement;
    const scale = svg.getScreenCTM()!.d;
    const baseline = Number(svg.querySelector('.gridline')!.getAttribute('y1'));
    return {
      reference: (baseline - Number(svg.querySelector('.rt-reference')!.getAttribute('y1'))) * scale,
      ordinary: (baseline - Number(svg.querySelector('.rt-point[data-week="2"]')!.getAttribute('cy'))) * scale,
    };
  });
  expect(spacing.reference).toBeGreaterThan(12);
  expect(spacing.ordinary).toBeGreaterThan(15);
  const marker = view.locator('.rt-off-scale[data-week="40"]');
  await expect(marker).toBeVisible();
  await expect(view.locator('.rt-interval[data-week="40"]')).toHaveAttribute('y2', '28');
  const label = view.locator('.rt-off-scale-labels button');
  await expect(label).toContainText('upper bound 66.4, off scale; mean 12.345, off scale');
  await expect(label).toBeVisible();
  const box = (await label.boundingBox())!;
  expect(box.x).toBeGreaterThanOrEqual(0);
  expect(box.x + box.width).toBeLessThanOrEqual(width);
  await view.screenshot({ path: test.info().outputPath('rt-readable.png') });
  await label.click();
  const drawer = page.getByRole('dialog', { name: 'Number provenance' });
  await expect(drawer).toBeVisible();
  await expect(drawer).toContainText('Mean 12.345; 90% interval 0.7–66.4');
  for (const value of Object.values(outlier.provenance[0])) await expect(drawer).toContainText(value);
  await drawer.getByRole('button', { name: 'Close provenance' }).press('Escape');
  await expect(label).toBeFocused();
  // The edge mark itself is also a keyboard-accessible, one-click source trigger.
  await marker.focus();
  await marker.press('Enter');
  await expect(drawer).toContainText('Mean 12.345; 90% interval 0.7–66.4');
  await drawer.getByRole('button', { name: 'Close provenance' }).click();
  await toggle.focus();
  await toggle.press('Space');
  await expect(toggle).toHaveAttribute('aria-pressed', 'true');
  await expect(toggle).toBeFocused();
  await expect(svg).toHaveAttribute('data-axis-maximum', '66.4');
  await expect(view.locator('.rt-range-note')).toContainText('All published means and bounds are shown');
  await expect(view.locator('.rt-point[data-week="40"]')).toHaveAttribute('cy', String(170 - 12.345 / 66.4 * 142));
  await page.getByText('Read exact weekly reports and R_t status', { exact: true }).click();
  const exact = page.locator('tr[data-week="2025-40"] .provenance-number');
  await expect(exact).toContainText('Mean 12.345; 90% interval 0.7–66.4');
  await exact.click();
  await expect(drawer).toContainText(outlier.provenance[0].sha256);
  await drawer.getByRole('button', { name: 'Close provenance' }).click();
  await toggle.click();
  await expect(toggle).toHaveAttribute('aria-pressed', 'false');
  await expect(svg).toHaveAttribute('data-axis-maximum', '3');
  await expect(label).toContainText('upper bound 66.4, off scale');
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(width);
});
