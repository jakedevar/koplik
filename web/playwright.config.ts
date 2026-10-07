import { defineConfig, chromium } from '@playwright/test';
import { existsSync } from 'node:fs';

const executablePath = process.env.CHROME_BIN ||
  (existsSync('/opt/google/chrome/chrome') ? '/opt/google/chrome/chrome' : chromium.executablePath());
if (!existsSync(executablePath)) {
  throw new Error(`Chromium unavailable at ${executablePath}. Set CHROME_BIN to an installed Chrome/Chromium executable or install the Playwright Chromium browser. The smoke test cannot be skipped.`);
}

export default defineConfig({
  testDir: './tests',
  workers: 1,
  retries: 0,
  timeout: 30000,
  reporter: 'list',
  use: {
    baseURL: 'http://127.0.0.1:5158/koplik/',
    headless: true,
    launchOptions: { executablePath, args: ['--no-sandbox', '--disable-dev-shm-usage'] },
    trace: 'retain-on-failure',
  },
  webServer: {
    command: 'npm run fixtures && npm run dev -- --host 127.0.0.1 --port 5158 --strictPort',
    url: 'http://127.0.0.1:5158/koplik/',
    env: { VITE_SYNTHETIC_FIXTURES: '1', KOPLIK_BASE_PATH: '/koplik/' },
    reuseExistingServer: false,
    timeout: 20000,
  },
});
