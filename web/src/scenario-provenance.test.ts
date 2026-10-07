import { existsSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it, vi } from 'vitest';
import { parseScenario } from './scenario';
import { loadScenarioProvenance, parseScenarioProvenance } from './scenario-provenance';

const read = (path: string) => readFileSync(resolve(process.cwd(), path), 'utf8');
const scenario = parseScenario(read('../data/fixtures/seir/synthetic-scenario.json'), true);
const raw = read('../data/fixtures/seir/synthetic-scenario.provenance.json');
const mutate = (change: (value: Record<string, any>) => void) => { const value = JSON.parse(raw); change(value); return JSON.stringify(value); };

describe('scenario provenance companion (contract v4)', () => {
  it('accepts a companion that describes the scenario and cites every parameter', () => {
    const provenance = parseScenarioProvenance(raw, scenario);
    expect(provenance.parameters.map((p) => p.parameter)).toEqual(Object.keys(scenario.parameters));
    expect(provenance.seeding.initial_infectious).toBe(scenario.nodes.find((n) => n.id === '48165')!.initial_infectious);
  });

  it('refuses a companion that does not match the scenario beside it', () => {
    expect(() => parseScenarioProvenance(mutate((v) => { v.seed = '1'; }), scenario)).toThrow('seed does not match');
    expect(() => parseScenarioProvenance(mutate((v) => { v.seeding.initial_infectious += 1; }), scenario)).toThrow('seeding does not match');
    expect(() => parseScenarioProvenance(mutate((v) => { v.seeding.start_week.week += 1; }), scenario)).toThrow('start week does not match');
    expect(() => parseScenarioProvenance(mutate((v) => { v.seeding.geography = '48003'; }), scenario)).toThrow('seeding does not match');
    expect(() => parseScenarioProvenance(mutate((v) => { v.parameters[0].value = { kind: 'fixed', value: 15 }; }), scenario)).toThrow('does not match the scenario');
    expect(() => parseScenarioProvenance(mutate((v) => { v.parameters.pop(); }), scenario)).toThrow('has no citation');
    expect(() => parseScenarioProvenance(mutate((v) => { v.parameters[1].source = ' '; }), scenario)).toThrow('needs a source and a note');
    expect(() => parseScenarioProvenance(mutate((v) => { v.nodes.pop(); }), scenario)).toThrow('nodes do not match');
  });

  it('refuses a companion that breaks the committed v4 schema', () => {
    expect(() => parseScenarioProvenance(mutate((v) => { v.contract_version = 3; }), scenario)).toThrow('Invalid scenario provenance');
    expect(() => parseScenarioProvenance(mutate((v) => { v.seed = '01353'; }), scenario)).toThrow('Invalid scenario provenance');
    expect(() => parseScenarioProvenance(mutate((v) => { v.extra = true; }), scenario)).toThrow('Invalid scenario provenance');
    expect(() => parseScenarioProvenance(mutate((v) => { delete v.seeding.assumption; }), scenario)).toThrow('Invalid scenario provenance');
    expect(() => parseScenarioProvenance(mutate((v) => { v.statement = '  '; }), scenario)).toThrow('statement must not be empty');
    expect(() => parseScenarioProvenance(mutate((v) => { v.scenario = '  '; }), scenario)).toThrow('scenario must not be empty');
  });

  it('loads from the scenarios folder, treats a missing file as absent and rejects server errors', async () => {
    const ok = vi.fn().mockResolvedValue(new Response(raw, { status: 200 }));
    expect((await loadScenarioProvenance('/koplik/', scenario, true, ok))?.seed).toBe(scenario.seed);
    expect(ok).toHaveBeenLastCalledWith('/koplik/data/scenarios/synthetic-scenario.provenance.json');
    const production = vi.fn().mockResolvedValue(new Response('', { status: 404 }));
    expect(await loadScenarioProvenance('/', scenario, false, production)).toBeNull();
    expect(production).toHaveBeenLastCalledWith('/data/scenarios/gaines-2025.provenance.json');
    await expect(loadScenarioProvenance('/', scenario, false, vi.fn().mockResolvedValue(new Response('', { status: 500 })))).rejects.toThrow('unavailable (500)');
  });

  // Runs when a pipeline build (make pipeline-fixtures) has written web/public/data: the pair
  // the pipeline publishes must be accepted by the web's own checks.
  const published = resolve(process.cwd(), 'public/data/scenarios/gaines-2025.json');
  it.skipIf(!existsSync(published))('accepts the scenario and companion the pipeline published', () => {
    const real = parseScenario(readFileSync(published, 'utf8'));
    const provenance = parseScenarioProvenance(readFileSync(published.replace('.json', '.provenance.json'), 'utf8'), real);
    expect(real.nodes.map((n) => n.id)).toEqual(['48165']);
    expect(provenance.seeding.initial_infectious).toBe(1);
    expect(provenance.seeding.initial_exposed).toBe(0);
    expect(provenance.statement).toBe('Hypothetical: what could happen if one infectious person arrived in Gaines County, given its population and kindergarten MMR coverage. This is not a reconstruction or forecast of the 2025 outbreak.');
  });
});
