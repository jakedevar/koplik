// @vitest-environment node
import { mkdtemp, mkdir, writeFile, rm, symlink } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { ViteDevServer } from 'vite';
import { build } from 'vite';
import { assertNoSyntheticData } from '../scripts/check-production-data.mjs';
import { syntheticFixtures } from '../build/synthetic-fixtures';
import { productionDataGuard } from '../build/production-data';
import { fixtureRoot, fixtureJson } from './fixtures.test-utils';

const temporary: string[] = [];
afterEach(async () => { await Promise.all(temporary.splice(0).map((path) => rm(path, { recursive: true, force: true }))); });
async function output() {
  const path = await mkdtemp(join(tmpdir(), 'koplik-production-check-'));
  temporary.push(path);
  return path;
}

describe('production fixture isolation', () => {
  it('rejects synthetic directories and synthetic provenance even with ordinary artifact names', async () => {
    const named = await output();
    await mkdir(join(named, 'synthetic-v1'));
    await expect(assertNoSyntheticData(named)).rejects.toThrow('Synthetic data cannot enter');
    const disguised = await output();
    await writeFile(join(disguised, 'weekly-cases.json'), JSON.stringify(fixtureJson('weekly-cases')));
    await expect(assertNoSyntheticData(disguised)).rejects.toThrow('Synthetic provenance');
    const linked = await output();
    await symlink(disguised, join(linked, 'data'), 'dir');
    await expect(assertNoSyntheticData(linked)).rejects.toThrow('Synthetic provenance');
  });
  it('rejects contaminated public data even when invoking Vite build directly', async () => {
    const root = await output();
    await writeFile(join(root, 'index.html'), '<!doctype html><title>Production guard test</title>');
    await mkdir(join(root, 'public'));
    await writeFile(join(root, 'public', 'rt.json'), JSON.stringify(fixtureJson('rt')));
    await expect(build({ root, configFile: false, logLevel: 'silent', plugins: [productionDataGuard()] })).rejects.toThrow('Synthetic provenance');
  });
  it('accepts unavailable observations and checks the real public and dist trees', async () => {
    const clean = await output();
    await writeFile(join(clean, 'weekly-cases.json'), '[]');
    await expect(assertNoSyntheticData(clean)).resolves.toBeUndefined();
    await assertNoSyntheticData(resolve('public'));
    await assertNoSyntheticData(resolve('dist'));
  });
  it('serves opted-in development fixtures offline under the configured base, outside public/', async () => {
    const plugin = syntheticFixtures(join(fixtureRoot, 'synthetic-v1'), true);
    expect(plugin.apply).toBe('serve');
    const use = vi.fn();
    const server = { config: { base: '/koplik/' }, middlewares: { use } } as unknown as ViteDevServer;
    (plugin.configureServer as (server: ViteDevServer) => void)(server);
    const middleware = use.mock.calls[0][0];
    const response = { setHeader: vi.fn(), end: vi.fn() };
    const next = vi.fn();
    await middleware({ url: '/koplik/data/synthetic-v1/synthetic-rt.json' }, response, next);
    expect(response.setHeader).toHaveBeenCalledWith('Content-Type', 'application/json');
    expect(JSON.parse(response.end.mock.calls[0][0].toString())).toEqual(fixtureJson('rt'));
    expect(next).toHaveBeenCalledTimes(0);
    await middleware({ url: '/koplik/data/synthetic-v1/../../synthetic-source.json' }, response, next);
    expect(next).toHaveBeenCalledOnce();
    const disabled = syntheticFixtures(fixtureRoot, false);
    use.mockClear();
    (disabled.configureServer as (server: ViteDevServer) => void)(server);
    expect(use).toHaveBeenCalledTimes(0);
  });
});
