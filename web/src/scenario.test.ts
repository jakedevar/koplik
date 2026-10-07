import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it, vi } from 'vitest';
import { gaines, loadScenario, parseScenario, scenarioJson } from './scenario';

const fixture = readFileSync(resolve(process.cwd(), '../data/fixtures/seir/synthetic-scenario.json'), 'utf8');

describe('scenario artifact and seed handling', () => {
  it('uses the explicitly labelled fixture override without imputing missing measured coverage', () => {
    const scenario = parseScenario(fixture, true);
    expect(scenario.nodes.find((n) => n.id === gaines)!.baseline_coverage.status).toBe('missing');
    const baseline = scenario.coverage_overrides.find((o) => o.geography === gaines)!.coverage_pct;
    const input = JSON.parse(scenarioJson(scenario, baseline));
    expect(input.coverage_overrides).toEqual(scenario.coverage_overrides);
    expect(input.run_count).toBe(1000);
    expect(parseScenario(scenarioJson(scenario, 95), true).coverage_overrides.find((o) => o.geography === gaines)?.coverage_pct).toBe(95);
  });
  it('preserves u64::MAX exactly through parsing, slider updates and wire serialization', () => {
    const large = fixture.replace(/"seed":\s*1353/, '"seed":18446744073709551615');
    const scenario = parseScenario(large, true);
    expect(scenario.seed).toBe('18446744073709551615');
    const updated = scenarioJson(scenario, 95);
    expect(updated).toContain('"seed":18446744073709551615');
    expect(parseScenario(updated, true).seed).toBe(scenario.seed);
  });
  it('recognizes escaped root seed keys and ignores seed text in provenance strings', () => {
    const changed = fixture.replace('"seed"', '"s\\u0065ed"').replace('synthetic-seir', 'synthetic-seed');
    expect(parseScenario(changed, true).seed).toBe('1353');
  });
  it.each(['-1', '18446744073709551616', '1.5', '1e3', '"1353"'])('rejects invalid seed %s', (seed) => {
    expect(() => parseScenario(fixture.replace(/"seed":\s*1353/, `"seed":${seed}`), true)).toThrow('u64 seed');
  });
  it('rejects duplicate seeds and wrong scenario years', () => {
    expect(() => parseScenario(fixture.replace('"seed": 1353', '"seed": 1353, "seed": 1354'), true)).toThrow('u64 seed');
    expect(() => parseScenario(fixture.replace('"year": 2025', '"year": 2026'), true)).toThrow('Gaines County 2025');
  });
  it('keeps measured coverage missing in production instead of falling back to the fixture override', () => {
    expect(() => parseScenario(fixture)).toThrow('Measured Gaines County coverage not yet available');
  });
  it('uses a reported baseline at startup, replaces only Gaines coverage, and rejects synthetic provenance in production', () => {
    // A labelled test-only reported-variant shape, retaining the fixture's explicit inputs.
    const reported = JSON.parse(fixture);
    const node = reported.nodes.find((n: { id: string }) => n.id === gaines);
    const coverage = reported.coverage_overrides.find((o: { geography: string }) => o.geography === gaines).coverage_pct;
    node.baseline_coverage = { status: 'reported', coverage_pct: coverage, imputed: false, imputation_method: null, provenance: node.provenance };
    const scenario = parseScenario(JSON.stringify(reported), true);
    const baseline = JSON.parse(scenarioJson(scenario, coverage));
    expect(baseline.coverage_overrides).toEqual(reported.coverage_overrides.filter((o: { geography: string }) => o.geography !== gaines));
    expect(() => parseScenario(JSON.stringify(reported))).toThrow('Synthetic scenario');
    node.baseline_coverage.imputed = true;
    expect(() => parseScenario(JSON.stringify(reported))).toThrow('Measured Gaines County coverage');
  });
  it('loads the documented artifact under the configured base and handles absence without fallback', async () => {
    const read = vi.fn().mockResolvedValue({ status: 404, ok: false });
    expect(await loadScenario('/koplik/', false, read)).toBeNull();
    expect(read).toHaveBeenCalledExactlyOnceWith('/koplik/data/scenarios/gaines-2025.json');
    read.mockResolvedValue({ status: 200, ok: true, text: async () => fixture });
    expect((await loadScenario('/koplik/', true, read))?.seed).toBe('1353');
    expect(read).toHaveBeenLastCalledWith('/koplik/data/scenarios/synthetic-scenario.json');
  });
  it('shows artifact errors instead of silently guessing data', async () => {
    await expect(loadScenario('/', false, vi.fn().mockResolvedValue({ ok: false, status: 500 }))).rejects.toThrow('500');
    await expect(loadScenario('/', true, vi.fn().mockResolvedValue({ ok: true, text: async () => '{}' }))).rejects.toThrow();
  });
});
