// Screenshots of every page (desktop 1280 and mobile 390) and first/largest contentful paint, from a built site.
//   node scripts/capture-ui.mjs <dist-dir> <out-dir> <label> [--paint] [--shots]
// Screenshots go to <out-dir>/<label>-<page>-<desktop|mobile>.png; paint numbers to <out-dir>/<label>-paint.json.
// Run from web/ with TMPDIR=/tmp. Chrome comes from CHROME_BIN or /opt/google/chrome/chrome.
import { chromium } from '@playwright/test';
import { existsSync } from 'node:fs';
import { mkdir, writeFile } from 'node:fs/promises';
import { serve } from './static-server.mjs';

const [dist, out, label, ...flags] = process.argv.slice(2);
if (!dist || !out || !label) throw new Error('usage: capture-ui.mjs <dist-dir> <out-dir> <label> [--paint] [--shots]');
const executablePath = process.env.CHROME_BIN || (existsSync('/opt/google/chrome/chrome') ? '/opt/google/chrome/chrome' : chromium.executablePath());
const pages = ['explorer', 'forecast', 'what-if', 'sources'];
const viewports = { desktop: { width: 1280, height: 800 }, mobile: { width: 390, height: 844 } };
const median = (values) => [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)];

await mkdir(out, { recursive: true });
const site = await serve(dist);
const browser = await chromium.launch({ executablePath, args: ['--no-sandbox', '--disable-dev-shm-usage'] });
try {
  if (flags.includes('--shots')) {
    for (const [name, viewport] of Object.entries(viewports)) {
      const context = await browser.newContext({ viewport, deviceScaleFactor: 1 });
      const page = await context.newPage();
      for (const id of pages) {
        await page.goto(`${site.url}#/${id}`, { waitUntil: 'load' });
        // Same page state for before and after: wait for data, the map (Explorer) and the what-if result.
        await page.waitForFunction(() => !document.querySelector('[role="status"]')?.textContent?.includes('Loading pipeline'), null, { timeout: 30000 });
        if (id === 'explorer') await page.locator('.map').waitFor();
        if (id === 'explorer') await page.waitForFunction(() => ['ready', 'error'].includes(document.querySelector('.map')?.dataset.mapState ?? ''), null, { timeout: 30000 }).catch(() => {});
        if (id === 'what-if') await page.waitForFunction(() => document.querySelector('.what-if')?.getAttribute('aria-busy') === 'false', null, { timeout: 60000 }).catch(() => {});
        if (id === 'forecast') await page.waitForFunction(() => !document.querySelector('.forecast-status')?.textContent?.includes('Loading'), null, { timeout: 30000 }).catch(() => {});
        await page.waitForTimeout(800);
        await page.screenshot({ path: `${out}/${label}-${id}-${name}.png`, fullPage: true });
      }
      await context.close();
    }
  }
  if (flags.includes('--paint')) {
    const profiles = {
      // Loopback, no throttling: what a fast machine sees.
      unthrottled: null,
      // Roughly a mid-range phone on a good mobile connection: 4x slower CPU, 1.6 Mbit/s down, 150 ms round trip.
      mobile_throttled: { cpu: 4, network: { offline: false, latency: 150, downloadThroughput: 1.6 * 1024 * 1024 / 8, uploadThroughput: 750 * 1024 / 8 } },
    };
    const result = {};
    for (const [profile, throttle] of Object.entries(profiles)) {
      const runs = [];
      for (let i = 0; i < 7; i++) {
        const context = await browser.newContext({ viewport: viewports[throttle ? 'mobile' : 'desktop'] });
        const page = await context.newPage();
        const client = await context.newCDPSession(page);
        if (throttle) {
          await client.send('Network.enable');
          await client.send('Network.emulateNetworkConditions', throttle.network);
          await client.send('Emulation.setCPUThrottlingRate', { rate: throttle.cpu });
        }
        await page.addInitScript(() => {
          window.__paint = { fcp: null, lcp: null, cls: 0 };
          new PerformanceObserver((list) => { for (const e of list.getEntries()) if (e.name === 'first-contentful-paint') window.__paint.fcp = e.startTime; }).observe({ type: 'paint', buffered: true });
          new PerformanceObserver((list) => { for (const e of list.getEntries()) window.__paint.lcp = e.startTime; }).observe({ type: 'largest-contentful-paint', buffered: true });
          new PerformanceObserver((list) => { for (const e of list.getEntries()) if (!e.hadRecentInput) window.__paint.cls += e.value; }).observe({ type: 'layout-shift', buffered: true });
        });
        await page.goto(site.url, { waitUntil: 'load' });
        await page.waitForFunction(() => document.querySelector('.headline-value button') !== null, null, { timeout: 60000 });
        await page.waitForTimeout(1500);
        runs.push(await page.evaluate(() => ({ ...window.__paint, transferred: performance.getEntriesByType('resource').reduce((n, e) => n + e.transferSize, 0) })));
        await context.close();
      }
      result[profile] = { runs, median_fcp_ms: median(runs.map((r) => r.fcp)), median_lcp_ms: median(runs.map((r) => r.lcp)), median_cls: median(runs.map((r) => r.cls)) };
    }
    await writeFile(`${out}/${label}-paint.json`, `${JSON.stringify(result, null, 1)}\n`);
    for (const [profile, r] of Object.entries(result)) console.log(`${label} ${profile}: FCP ${r.median_fcp_ms.toFixed(0)} ms, LCP ${r.median_lcp_ms.toFixed(0)} ms, CLS ${r.median_cls.toFixed(3)}`);
  }
} finally { await browser.close(); await site.close(); }
