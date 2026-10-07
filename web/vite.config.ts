import { defineConfig } from 'vitest/config';

export default defineConfig({
  base: process.env.KOPLIK_BASE_PATH || '/',
  test: {
    environment: 'jsdom',
    include: ['src/**/*.test.ts'],
    maxWorkers: 2,
  },
});
