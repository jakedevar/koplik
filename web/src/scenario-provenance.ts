import provenanceSchema from '../../crates/koplik-contracts/schema/v4/ScenarioProvenance.schema.json';
import type { ScenarioProvenance } from './generated/v4/ScenarioProvenance';
import { ajv, gaines, type Scenario } from './scenario';

export type { ScenarioProvenance };

/**
 * The companion of the what-if scenario (`data/scenarios/gaines-2025.provenance.json`, contract
 * v4, #1400): the seeding stated as an assumption and the citation of every model parameter.
 * Its shape is the committed v4 JSON Schema; the cross-checks against the scenario below mirror
 * `ScenarioProvenance::check_against` in koplik-contracts.
 */
const validate = ajv.compile(provenanceSchema);

const fail = (message: string): never => { throw new Error(`Invalid scenario provenance: ${message}`); };
const isObject = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null && !Array.isArray(value);
// Compare as data, not as text: JSON object key order carries no meaning.
const canonical = (value: unknown): string => JSON.stringify(value, (_key, v: unknown) => (isObject(v) ? Object.fromEntries(Object.entries(v).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))) : v));
const same = (a: unknown, b: unknown): boolean => canonical(a) === canonical(b);
const blank = (value: string) => !value.trim();

/** Parse the companion and check it describes `scenario`: a mismatched pair is refused, not shown. */
export function parseScenarioProvenance(raw: string, scenario: Scenario): ScenarioProvenance {
  const value: unknown = JSON.parse(raw);
  if (!validate(value)) return fail(ajv.errorsText(validate.errors));
  const provenance = value as unknown as ScenarioProvenance;
  if (provenance.contract_version !== 4) fail('contract_version must be 4');
  for (const [name, text] of [['scenario', provenance.scenario], ['statement', provenance.statement], ['neighbourhood_note', provenance.neighbourhood_note], ['seeding.assumption', provenance.seeding.assumption], ['seeding.start_week_basis', provenance.seeding.start_week_basis], ['seeding.limitation', provenance.seeding.limitation]] as const) {
    if (blank(text)) fail(`${name} must not be empty`);
  }
  if (provenance.seed !== scenario.seed) fail('seed does not match the scenario');
  if (provenance.run_count !== scenario.run_count) fail('run_count does not match the scenario');
  if (!same(provenance.seeding.start_week, scenario.start_week)) fail('start week does not match the scenario');
  const seeded = scenario.nodes.find((n) => n.id === provenance.seeding.geography);
  if (!seeded) fail('the seeded geography is not a node of the scenario');
  if (seeded!.initial_infectious !== provenance.seeding.initial_infectious || seeded!.initial_exposed !== provenance.seeding.initial_exposed) fail('seeding does not match the scenario');
  if (scenario.nodes.some((n) => n.id !== provenance.seeding.geography && (n.initial_infectious > 0 || n.initial_exposed > 0))) fail('another node is seeded');
  if (provenance.nodes.length !== scenario.nodes.length || provenance.nodes.some((n, i) => n.geography !== scenario.nodes[i].id || n.population !== scenario.nodes[i].population)) fail('nodes do not match the scenario');
  if (provenance.seeding.geography !== gaines) fail('the panel needs the introduction in Gaines County');
  const stated = scenario.parameters as unknown as Record<string, unknown>;
  const cited = new Set<string>();
  for (const parameter of provenance.parameters) {
    if (!(parameter.parameter in stated)) fail(`${parameter.parameter} is not a scenario parameter`);
    if (!same(parameter.value, stated[parameter.parameter])) fail(`${parameter.parameter} does not match the scenario`);
    if (blank(parameter.source) || blank(parameter.note)) fail(`${parameter.parameter} needs a source and a note`);
    if (cited.has(parameter.parameter)) fail(`${parameter.parameter} is cited twice`);
    cited.add(parameter.parameter);
  }
  for (const name of Object.keys(stated)) if (!cited.has(name)) fail(`${name} has no citation`);
  return provenance;
}

export async function loadScenarioProvenance(base: string, scenario: Scenario, synthetic = false, read: typeof fetch = fetch): Promise<ScenarioProvenance | null> {
  const path = `${base.replace(/\/$/, '')}/data/scenarios/${synthetic ? 'synthetic-scenario' : 'gaines-2025'}.provenance.json`;
  const response = await read(path);
  if (response.status === 404) return null;
  if (!response.ok) throw new Error(`Scenario provenance unavailable (${response.status})`);
  return parseScenarioProvenance(await response.text(), scenario);
}
