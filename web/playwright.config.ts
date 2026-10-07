import { defineConfig, chromium } from '@playwright/test';
import { existsSync } from 'node:fs';

const executablePath = process.env.CHROME_BIN ||
  (existsSync('/opt/google/chrome/chrome') ? '/opt/google/chrome/chrome' : chromium.executablePath());
const port = Number(process.env.KOPLIK_TEST_PORT || 5158);
if (!existsSync(executablePath)) {
  throw new Error(`Chromium unavailable at ${executablePath}. Set CHROME_BIN to an installed Chrome/Chromium executable or install the Playwright Chromium browser. The smoke test cannot be skipped.`);
}
if (!Number.isInteger(port) || port < 1 || port > 65535) {
  throw new Error(`Invalid Playwright test port: ${process.env.KOPLIK_TEST_PORT}`);
}

const baseURL = `http://127.0.0.1:${port}/koplik/`;

export default defineConfig({
  testDir: './tests',
  outputDir: process.env.KOPLIK_TEST_OUTPUT_DIR || 'test-results',
  workers: 1,
  retries: 0,
  timeout: 30000,
  reporter: 'list',
  use: {
    baseURL,
    headless: true,
    launchOptions: { executablePath, args: ['--no-sandbox', '--disable-dev-shm-usage'] },
    trace: 'retain-on-failure',
  },
  webServer: {
    command: `npm run fixtures && npm run dev -- --host 127.0.0.1 --port ${port} --strictPort`,
    url: baseURL,
    env: { VITE_SYNTHETIC_FIXTURES: '1', KOPLIK_BASE_PATH: '/koplik/' },
    reuseExistingServer: false,
    timeout: 20000,
  },
});
