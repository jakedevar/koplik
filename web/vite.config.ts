import { defineConfig } from 'vitest/config';
import { fileURLToPath } from 'node:url';
import { syntheticFixtures } from './build/synthetic-fixtures';
import { productionDataGuard } from './build/production-data';
import { preloadExplorerData } from './build/preload-data';

export default defineConfig({
  base: process.env.KOPLIK_BASE_PATH || '/',
  plugins: [productionDataGuard(), preloadExplorerData(), syntheticFixtures(fileURLToPath(new URL('../data/fixtures/web/synthetic-v1/', import.meta.url)), process.env.VITE_SYNTHETIC_FIXTURES === '1')],
  server: {
    fs: { allow: [fileURLToPath(new URL('.', import.meta.url)), fileURLToPath(new URL('../pkg/web/', import.meta.url))] },
  },
  test: {
    environment: 'jsdom',
    include: ['src/**/*.test.ts'],
    maxWorkers: 2,
  },
});
