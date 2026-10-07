import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it, vi } from 'vitest';
import { loadDataset, metricValue, parseBoundaries, parseRows } from './data';
import { fixtureDataset, fixtureJson, fixtureRoot, pairedRtRows } from './fixtures.test-utils';

describe('contracts and artifact loading', () => {
  it('validates all committed synthetic v1 fixtures and traces rows to their source bytes', () => {
    const data = fixtureDataset();
    const bytes = readFileSync(resolve(fixtureRoot, 'synthetic-source.json'));
    const hash = createHash('sha256').update(bytes).digest('hex');
    for (const row of [...data.geographies, ...data.cases, ...data.coverage, ...data.rt]) {
      expect(row.provenance[0].sha256).toBe(hash);
      expect(row.provenance[0].source_id).toBe('synthetic-web-test');
    }
    expect(data.counties.features.map((f) => f.properties.GEOID)).toContain('48165');
  });
  it('requests only artifacts under the configured base path, offline', async () => {
    const read = vi.fn(async (url: RequestInfo | URL) => {
      const path = String(url);
      expect(path.startsWith('/koplik/data/synthetic-v1/synthetic-')).toBe(true);
      const name = path.split('/').at(-1)!.replace('synthetic-', '').replace('.json', '');
      return { ok: true, json: async () => fixtureJson(name) } as Response;
    });
    const data = await loadDataset('/koplik/', true, read);
    expect(data.geographies).toHaveLength(6);
    expect(read).toHaveBeenCalledTimes(6);
  });
  it('loads paired 50%/95% R_t rows and rejects a duplicate of the same interval level', async () => {
    const paired = pairedRtRows();
    const read = vi.fn(async (url: RequestInfo | URL) => {
      const name = String(url).split('/').at(-1)!.replace('synthetic-', '').replace('.json', '');
      return { ok: true, json: async () => name === 'rt' ? paired : fixtureJson(name) } as Response;
    });
    const data = await loadDataset('/koplik/', true, read);
    expect(data.rt).toEqual(paired);
    expect(data.rt.map((row) => row.interval_level)).toEqual([0.5, 0.95, 0.5, 0.95]);
    expect(() => parseRows('rt', [...data.rt, { ...data.rt[0] }])).toThrow('duplicate row 48:2025:2:0.5');
  });
  it('fails visibly for absent artifacts and refuses synthetic observations in production', async () => {
    const unavailable = vi.fn(async () => ({ ok: false, status: 404 }) as Response);
    await expect(loadDataset('/', false, unavailable)).rejects.toThrow('artifact unavailable');
    const synthetic = vi.fn(async (url: RequestInfo | URL) => ({ ok: true,
      json: async () => fixtureJson(String(url).split('/').at(-1)!.replace('.json', '')),
    }) as Response);
    await expect(loadDataset('/', false, synthetic)).rejects.toThrow('Synthetic artifacts require');
  });
  it('rejects invalid schema rows, duplicate keys and inconsistent R_t status/bounds', () => {
    const data = fixtureDataset();
    expect(() => parseRows('cases', [{ ...data.cases[0], confirmed: { status: 'reported', count: -1 } }])).toThrow('invalid v1');
    expect(() => parseRows('cases', [data.cases[0], data.cases[0]])).toThrow('duplicate');
    expect(() => parseRows('rt', [{ ...data.rt[0], mean: 1 }])).toThrow('status and bounds');
    expect(() => parseRows('rt', [{ ...data.rt[1], lower: 2, upper: 1 }])).toThrow('status and bounds');
    expect(() => parseRows('coverage', [{ ...data.coverage[0], school_year: '2024-26' }])).toThrow('school year');
    expect(() => parseRows('coverage', [{ ...data.coverage[0], imputed: true }])).toThrow('imputation');
    expect(() => parseBoundaries(data.states, 'county')).toThrow('invalid');
    const broken = structuredClone(data.states);
    broken.features[0].geometry.coordinates = [];
    expect(() => parseBoundaries(broken, 'state')).toThrow('polygon coordinates');
  });
});

describe('map values', () => {
  it('keeps real zero, explicit missing and absent data distinct', () => {
    const data = fixtureDataset();
    expect(metricValue(data, '40', 'cases-2025').value).toBe(0);
    expect(metricValue(data, '35', 'cases-2025').label).toBe('No data');
    expect(metricValue(data, '20', 'cases-2025').value).toBeNull();
    expect(metricValue(data, '35', 'coverage').label).toBe('No data');
    expect(metricValue(data, '48', 'cases-2026').value).toBe(3);
    expect(metricValue(data, '48', 'cases-2025').detail).toContain('not a full-year total');
  });
  it('marks an unreported internal week missing instead of summing across the gap', () => {
    const data = fixtureDataset();
    data.cases = data.cases.filter((r) => r.week.week !== 2);
    expect(metricValue(data, '48', 'cases-2025').value).toBeNull();
  });
  it('shows the latest school year even when its value is missing; labels supplied imputation', () => {
    const data = fixtureDataset();
    data.coverage.push({ ...data.coverage[0], school_year: '2025-26', coverage: { status: 'missing', reason: 'suppressed' } });
    expect(metricValue(data, '48', 'coverage').detail).toBe('suppressed');
    data.coverage.pop();
    data.coverage[0].imputed = true;
    data.coverage[0].imputation_method = 'Synthetic test method';
    expect(metricValue(data, '48', 'coverage').detail).toContain('Imputed: Synthetic test method');
  });
});
