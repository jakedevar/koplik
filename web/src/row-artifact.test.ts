// @vitest-environment node
import { existsSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it, vi } from 'vitest';
import { parseRows, loadDataset } from './data';
import { parseForecast } from './forecast';
import { expandRowArtifact } from './row-artifact';
import { fixtureJson, fixtureRoot } from './fixtures.test-utils';
import type { Provenance } from './generated/Provenance';

function pack(rows: { provenance: Provenance[] }[]) {
  const provenance: Provenance[] = [];
  return { contract_version: 6, provenance, rows: rows.map((row) => ({ ...row, provenance: row.provenance.map((record) => {
    let index = provenance.findIndex((p) => JSON.stringify(p) === JSON.stringify(record));
    if (index < 0) { index = provenance.length; provenance.push(record); }
    return index;
  }) })) };
}

describe('v6 provenance expansion', () => {
  it('preserves every observation field and ordered provenance record for every fixture row', () => {
    for (const [kind, name] of [['geographies', 'geographies'], ['cases', 'weekly-cases'], ['coverage', 'coverage'], ['rt', 'rt']] as const) {
      const legacy = fixtureJson(name);
      expect(parseRows(kind, pack(legacy))).toEqual(parseRows(kind, legacy));
    }
  });
  it('preserves distinct URLs, retrieval times and licences for the same hash and repeated references', () => {
    const row = fixtureJson('geographies')[0];
    const sameHash = { ...row.provenance[0], url: 'https://example.invalid/other', retrieved_at: '2026-10-02T00:00:00Z', licence_id: 'other-terms' };
    const rows = [{ ...row, provenance: [row.provenance[0], sameHash, row.provenance[0]] }];
    expect(parseRows('geographies', pack(rows))).toEqual(rows);
  });
  it('refuses invalid versions, tables and indices before any row reaches the UI', () => {
    for (const indices of [[], [-1], [0.5], [1], [4294967296], ['0']]) {
      const artifact = pack([fixtureJson('geographies')[0]]);
      artifact.rows[0].provenance = indices as number[];
      expect(() => parseRows('geographies', artifact)).toThrow(/invalid v6 artifact|outside this file's table/);
    }
    const artifact = pack([fixtureJson('geographies')[0]]);
    artifact.provenance[0] = { ...artifact.provenance[0], sha256: 'bad' };
    expect(() => parseRows('geographies', artifact)).toThrow('invalid v6 artifact');
    expect(() => parseRows('geographies', { ...artifact, contract_version: 5 })).toThrow('invalid v6 artifact');
    const status = pack(fixtureJson('rt'));
    Object.assign(status.rows[0], { mean: 1 });
    expect(() => parseRows('rt', status)).toThrow('status and bounds');
    expect(parseRows('cases', pack([]))).toEqual([]);
  });
  it('loads v6 through the production path and still checks expanded synthetic provenance', async () => {
    const read = vi.fn(async (url: RequestInfo | URL) => {
      expect(String(url)).toMatch(/^\/koplik\/data\/v6\//);
      const name = String(url).split('/').at(-1)!.replace('.json', '');
      const input = fixtureJson(name);
      return { ok: true, json: async () => Array.isArray(input) ? pack(input) : input } as Response;
    });
    await expect(loadDataset('/koplik/', false, read)).rejects.toThrow('Synthetic artifacts require');
    expect(read).toHaveBeenCalledTimes(6);
  });
  it('expands forecast rows before checking their v5 provenance companion', () => {
    const read = (name: string) => JSON.parse(readFileSync(resolve(fixtureRoot, `synthetic-v1/${name}`), 'utf8'));
    const rows = read('synthetic-forecast.json');
    const companion = read('synthetic-forecast.provenance.json');
    expect(parseForecast(pack(rows), companion, true)).toEqual(parseForecast(rows, companion, true));
  });
});

const published = resolve('public/data/v6');
// Fixture generation is a separate CLI gate; a clean checkout may not have artifacts yet.
it.skipIf(!existsSync(resolve(published, 'rt.json')))('expands every real published row to its current-contract stage output with identical provenance', () => {
  const read = (path: string) => JSON.parse(readFileSync(path, 'utf8'));
  for (const [kind, name, stage] of [['geographies', 'geographies', 'validate'], ['cases', 'weekly-cases', 'validate'], ['coverage', 'coverage', 'validate'], ['rt', 'rt', 'infer']] as const) {
    const original = read(resolve('../data/pipeline', stage, `${name}.json`));
    const expanded = parseRows(kind, read(resolve(published, `${name}.json`)));
    expect(expanded).toEqual(parseRows(kind, original));
    expanded.forEach((row, i) => expect(row.provenance).toEqual(original[i].provenance));
  }
  const original = read(resolve('../data/pipeline/forecast/forecast.json'));
  const expanded = expandRowArtifact('forecast', read(resolve('public/data/forecasts/weekly-cases.json'))) as typeof original;
  expect(expanded).toEqual(original);
  expanded.forEach((row: { provenance: Provenance[] }, i: number) => expect(row.provenance).toEqual(original[i].provenance));
});
