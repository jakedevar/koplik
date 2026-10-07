import Ajv from 'ajv/dist/2020';
import addFormats from 'ajv-formats';
import schema from '../../crates/koplik-contracts/schema/v1/ScenarioInput.schema.json';
import type { ScenarioInput } from './generated/ScenarioInput';

export const gaines = '48165';
// UI representation only: the v1 wire contract still requires a JSON u64 number.
export type Scenario = Omit<ScenarioInput, 'seed'> & { seed: string };

const ajv = new Ajv({ strict: false, allErrors: true });
addFormats(ajv);
for (const [format, maximum] of [['uint8', 255], ['uint16', 65535], ['uint32', 4294967295], ['uint64', Number.MAX_SAFE_INTEGER]] as const) {
  ajv.addFormat(format, { type: 'number', validate: (value: number) => Number.isSafeInteger(value) && value >= 0 && value <= maximum });
}
ajv.addFormat('double', { type: 'number', validate: Number.isFinite });
const validate = ajv.compile(schema);

/** Tokenize before parsing so the root u64 seed never goes through a JS number. */
export function parseScenario(raw: string, synthetic = false): Scenario {
  const tokens = [...raw.matchAll(/"(?:[^"\\]|\\.)*"|-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?|[{}\[\]:,]|true|false|null/g)];
  let depth = 0;
  const seeds: RegExpMatchArray[] = [];
  for (let i = 0; i < tokens.length; i++) {
    const token = tokens[i][0];
    if (token === '{' || token === '[') depth++;
    else if (token === '}' || token === ']') depth--;
    else if (depth === 1 && token.startsWith('"') && JSON.parse(token) === 'seed' && tokens[i + 1]?.[0] === ':') seeds.push(tokens[i + 2]);
  }
  if (seeds.length !== 1 || !/^(0|[1-9]\d*)$/.test(seeds[0]?.[0] || '') || BigInt(seeds[0][0]) > 18446744073709551615n) {
    throw new Error('Scenario requires one exact unsigned u64 seed');
  }
  const seed = seeds[0];
  const scenario = JSON.parse(raw.slice(0, seed.index) + JSON.stringify(seed[0]) + raw.slice(seed.index! + seed[0].length)) as Scenario;
  // Seed bounds were checked losslessly above; validate the rest against the released schema.
  if (!validate({ ...scenario, seed: 0 })) throw new Error(`Invalid v1 scenario: ${ajv.errorsText(validate.errors)}`);
  const node = scenario.nodes.find((n) => n.id === gaines);
  if (scenario.start_week.year !== 2025 || !node) throw new Error('Expected Gaines County 2025 scenario');
  if (node.baseline_coverage.status !== 'reported' || node.baseline_coverage.imputed) {
    if (!synthetic || !scenario.coverage_overrides.some((o) => o.geography === gaines)) throw new Error('Measured Gaines County coverage not yet available');
  }
  const provenance = scenario.nodes.flatMap((n) => [...n.provenance, ...(n.baseline_coverage.status === 'reported' ? n.baseline_coverage.provenance : [])]);
  if (!synthetic && provenance.some((p) => p.source_id.startsWith('synthetic') || p.url.includes('example.invalid'))) {
    throw new Error('Synthetic scenario requires explicit development fixture mode');
  }
  return scenario;
}

export function scenarioJson(scenario: Scenario, coverage: number): string {
  if (!Number.isFinite(coverage) || coverage < 0 || coverage > 100) throw new Error('Coverage must be between 0 and 100');
  const { seed, ...rest } = scenario;
  if (!/^(0|[1-9]\d*)$/.test(seed) || BigInt(seed) > 18446744073709551615n) throw new Error('Invalid seed');
  const baseline = scenario.nodes.find((n) => n.id === gaines)!.baseline_coverage;
  const overrides = scenario.coverage_overrides.filter((o) => o.geography !== gaines);
  if (baseline.status === 'missing' || coverage !== baseline.coverage_pct) overrides.push({ geography: gaines, coverage_pct: coverage });
  overrides.sort((a, b) => a.geography.localeCompare(b.geography, 'en'));
  // Seed is the last root field, inserted as validated decimal text, never JSON.stringify(number).
  return JSON.stringify({ ...rest, coverage_overrides: overrides, run_count: 1000 }).slice(0, -1) + `,"seed":${seed}}`;
}

export async function loadScenario(base: string, synthetic = false, read: typeof fetch = fetch): Promise<Scenario | null> {
  const path = `${base.replace(/\/$/, '')}/data/scenarios/${synthetic ? 'synthetic-scenario' : 'gaines-2025'}.json`;
  const response = await read(path);
  if (response.status === 404) return null;
  if (!response.ok) throw new Error(`Scenario artifact unavailable (${response.status})`);
  return parseScenario(await response.text(), synthetic);
}
