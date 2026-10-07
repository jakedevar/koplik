// Offline browser integration and timing against the committed, labelled synthetic input.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { readFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import puppeteer from 'puppeteer-core';

const webRoot = fileURLToPath(new URL('../', import.meta.url));
const port = process.env.WHAT_IF_PORT || '5138';
const server = spawn(process.execPath, ['node_modules/vite/bin/vite.js', '--host', '127.0.0.1', '--port', port, '--strictPort'], {
  cwd: webRoot,
  env: { ...process.env, VITE_SYNTHETIC_FIXTURES: '1', KOPLIK_BASE_PATH: '/koplik/' },
  stdio: ['ignore', 'pipe', 'pipe'],
});
let browser;
try {
  await new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error('Vite did not start within 15 s')), 15000);
    let log = '';
    server.stdout.on('data', (chunk) => { log += chunk; if (log.includes('Local:')) { clearTimeout(timer); resolve(); } });
    server.stderr.on('data', (chunk) => { log += chunk; });
    server.on('error', reject);
    server.on('exit', (code) => { clearTimeout(timer); reject(new Error(`Vite exited ${code}: ${log}`)); });
  });
  browser = await puppeteer.launch({
    executablePath: process.env.CHROME_BIN || '/opt/google/chrome/chrome', headless: true,
    env: { ...process.env, TMPDIR: '/tmp' },
    args: ['--no-sandbox', '--disable-dev-shm-usage', '--disable-background-timer-throttling'],
  });
  const page = await browser.newPage();
  const errors = [];
  const failedRequests = [];
  page.on('pageerror', (error) => errors.push(String(error)));
  page.on('response', (response) => { if (response.status() >= 400) failedRequests.push(`${response.status()} ${response.url()}`); });
  const origin = `http://127.0.0.1:${port}`;
  await page.setRequestInterception(true);
  page.on('request', (request) => {
    // Keep the integration run offline, including accidental external map requests.
    if (request.url().startsWith(origin) || /^(data|blob):/.test(request.url())) request.continue();
    else { errors.push(`External request: ${request.url()}`); request.abort(); }
  });
  await page.evaluateOnNewDocument(() => {
    window.whatIfHeartbeat = 0;
    setInterval(() => { window.whatIfHeartbeat++; }, 20);
  });
  await page.goto(`${origin}/koplik/`);
  async function completion() {
    try {
      await page.waitForFunction(() => document.querySelector('.what-if')?.getAttribute('aria-busy') === 'false' && !!document.querySelector('.engine-fingerprint'), { timeout: 15000 });
    } catch (error) {
      throw new Error(`${error.message}\n${await page.$eval('body', (node) => node.textContent)}\n${[...errors, ...failedRequests].join('\n')}`);
    }
    await page.evaluate(() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve))));
    return page.evaluate(() => ({
      elapsed_ms: Number(document.querySelector('.what-if').dataset.updateMs),
      fingerprint: document.querySelector('.engine-fingerprint').textContent,
      coverage: document.querySelector('#what-if-coverage').value,
      replay: [...document.querySelectorAll('.what-if-result pre')][0].textContent,
      chart: document.querySelector('.ensemble-median').getAttribute('points'),
      heartbeat: window.whatIfHeartbeat,
    }));
  }
  const cold = await completion();
  assert.equal(cold.coverage, '70');
  assert.equal(cold.fingerprint, '7a7471b1ed6d648d9a376d591ed21be513b90128d5f5e7c759c184689d5c25fb');
  assert.equal(page.workers().filter((worker) => worker.url().includes('simulation.worker.ts')).length, 1, 'engine must run in a Web Worker');
  const { runTrajectory } = createRequire(import.meta.url)('../../pkg/node/koplik_wasm.js');
  const warm = [];
  for (const coverage of [75, 80, 85, 90, 95]) {
    const before = await page.evaluate((coverage) => {
      const slider = document.querySelector('#what-if-coverage');
      slider.value = String(coverage);
      const start = performance.now();
      const heartbeat = window.whatIfHeartbeat;
      slider.dispatchEvent(new Event('input', { bubbles: true }));
      return { start, heartbeat };
    }, coverage);
    const sample = await completion();
    sample.through_paint_ms = await page.evaluate((start) => performance.now() - start, before.start);
    sample.heartbeat_ticks_during_update = sample.heartbeat - before.heartbeat;
    assert(sample.heartbeat_ticks_during_update > 0, 'page must remain responsive while WASM runs');
    assert.equal(sample.coverage, String(coverage));
    assert.equal(JSON.parse(runTrajectory(sample.replay, 0)).member.fingerprint, sample.fingerprint);
    warm.push(sample);
  }
  assert.notEqual(warm.at(-1).chart, cold.chart, 'coverage change must change the ensemble');
  assert.notEqual(warm.at(-1).fingerprint, cold.fingerprint);
  await page.click('.what-if-controls button');
  assert.equal((await completion()).fingerprint, cold.fingerprint, 'restoring coverage must reproduce the fingerprint');

  // Full-width seed in the actual browser/Worker path, not only a unit serializer.
  const fixture = await readFile(new URL('../../data/fixtures/seir/synthetic-scenario.json', import.meta.url), 'utf8');
  const large = fixture.replace(/"seed":\s*1353/, '"seed":18446744073709551615');
  // The provenance companion must name the same seed or the panel refuses the pair.
  const companion = (await readFile(new URL('../../data/fixtures/seir/synthetic-scenario.provenance.json', import.meta.url), 'utf8'))
    .replace(/"seed":\s*"1353"/, '"seed": "18446744073709551615"');
  page.removeAllListeners('request');
  page.on('request', (request) => {
    if (request.url().endsWith('/data/scenarios/synthetic-scenario.json')) request.respond({ status: 200, contentType: 'application/json', body: large });
    else if (request.url().endsWith('/data/scenarios/synthetic-scenario.provenance.json')) request.respond({ status: 200, contentType: 'application/json', body: companion });
    else if (request.url().startsWith(origin) || /^(data|blob):/.test(request.url())) request.continue();
    else { errors.push(`External request: ${request.url()}`); request.abort(); }
  });
  await page.reload();
  const largeSeed = await completion();
  assert.equal(JSON.parse(runTrajectory(largeSeed.replay, 0)).seed, '18446744073709551615');
  assert.equal(largeSeed.fingerprint, JSON.parse(runTrajectory(largeSeed.replay, 0)).member.fingerprint);
  assert((await page.$eval('.what-if-metadata', (node) => node.textContent)).includes('18446744073709551615'));

  // An absent production artifact is a clear disabled state, even without map artifacts.
  page.removeAllListeners('request');
  page.on('request', (request) => {
    if (request.url().includes('/data/')) request.respond({ status: 404, contentType: 'text/plain', body: '' });
    else if (request.url().startsWith(origin) || /^(data|blob):/.test(request.url())) request.continue();
    else { errors.push(`External request: ${request.url()}`); request.abort(); }
  });
  await page.reload();
  await page.waitForFunction(() => document.querySelector('.what-if-status')?.textContent.includes('Scenario data not yet available'));
  assert(await page.$eval('#what-if-coverage', (node) => node.disabled));
  assert.deepEqual(errors, []);
  const compact = ({ replay, chart, heartbeat, ...sample }) => sample;
  const median = warm.map((s) => s.elapsed_ms).sort((a, b) => a - b)[2];
  console.log(JSON.stringify({
    measured_at: new Date().toISOString(), browser_version: await browser.version(),
    user_agent: await page.evaluate(() => navigator.userAgent), headless: true, synthetic: true,
    fixture: 'data/fixtures/seir/synthetic-scenario.json', runs: 1000, counties: 7, horizon_days: 180, seed: '1353',
    measured_scope: 'Coverage input through debounce, worker init (cold only), WASM ensemble, message transfer and DOM render. through_paint_ms additionally includes two animation frames and automation observation.',
    cold: compact(cold), warm: warm.map(compact), warm_median_ms: median,
    target_ms: 3000, warm_median_meets_target: median < 3000,
    checks: 'Worker, responsive heartbeat, changed ensemble, Node WASM member-0 replay, restored fingerprint, u64::MAX browser replay, unavailable-data state, same-origin requests',
  }, null, 2));
} finally {
  await browser?.close();
  server.kill('SIGTERM');
}
