import type { Provenance } from './generated/Provenance';
import { gaines, type Scenario } from './scenario';

/**
 * Companion to the what-if scenario (`data/scenarios/gaines-2025.provenance.json`, #1400):
 * the source of the initial seeding and the citation of every model parameter. It is a
 * pipeline artifact, not a shared data contract; this file is its documented web shape
 * (see web/README.md and `ScenarioProvenance` in crates/koplik-pipeline/src/scenario.rs).
 */
export interface ScenarioProvenance {
  artifact_version: 1;
  scenario: string;
  statement: string;
  seed: string;
  run_count: number;
  seeding: {
    rule: string;
    report_date: string;
    report_first_seen_at: string;
    county_name_as_printed: string;
    cell_as_printed: string;
    confirmed_basis: string;
    recorded_confirmed_count: number;
    reporting_multiplier: number;
    exposed_per_infectious: number;
    initial_infectious: number;
    initial_exposed: number;
    start_week: { year: number; week: number };
    provenance: Provenance[];
    skipped_vintages: { report_date: string; reason: string }[];
    limitation: string;
  };
  parameters: { parameter: string; value: unknown; source: string; url: string | null; note: string }[];
  nodes: { geography: string; name: string; population: number; population_basis: string; centroid_basis: string; coverage_school_year: string; coverage_basis: string }[];
  excluded_nodes: { geography: string; name: string; reason: string }[];
  neighbourhood_note: string;
}

const fail = (message: string): never => { throw new Error(`Invalid scenario provenance: ${message}`); };
const isObject = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null && !Array.isArray(value);
const text = (value: unknown, name: string): string => (typeof value === 'string' && value.trim() ? value : fail(`${name} must be non-empty text`));
const count = (value: unknown, name: string): number => (typeof value === 'number' && Number.isSafeInteger(value) && value >= 0 ? value : fail(`${name} must be a whole number`));
// Compare as data, not as text: JSON object key order carries no meaning.
const canonical = (value: unknown): string => JSON.stringify(value, (_key, v: unknown) => (isObject(v) ? Object.fromEntries(Object.entries(v).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))) : v));
const same = (a: unknown, b: unknown): boolean => canonical(a) === canonical(b);

/** Parse the companion and check it describes `scenario`: a mismatched pair is refused, not shown. */
export function parseScenarioProvenance(raw: string, scenario: Scenario): ScenarioProvenance {
  const value: unknown = JSON.parse(raw);
  if (!isObject(value)) return fail('not an object');
  if (value.artifact_version !== 1) fail('artifact_version must be 1');
  text(value.statement, 'statement');
  if (value.seed !== scenario.seed) fail('seed does not match the scenario');
  if (value.run_count !== scenario.run_count) fail('run_count does not match the scenario');
  const seeding = value.seeding;
  if (!isObject(seeding)) return fail('seeding missing');
  for (const name of ['rule', 'report_date', 'report_first_seen_at', 'county_name_as_printed', 'cell_as_printed', 'confirmed_basis', 'limitation'] as const) text(seeding[name], `seeding.${name}`);
  count(seeding.recorded_confirmed_count, 'seeding.recorded_confirmed_count');
  const node = scenario.nodes.find((n) => n.id === gaines)!;
  if (seeding.initial_infectious !== node.initial_infectious || seeding.initial_exposed !== node.initial_exposed) fail('seeding does not match the scenario');
  if (!same(seeding.start_week, scenario.start_week)) fail('start week does not match the scenario');
  if (typeof seeding.reporting_multiplier !== 'number' || typeof seeding.exposed_per_infectious !== 'number') fail('seeding multipliers must be numbers');
  const records = seeding.provenance;
  if (!Array.isArray(records) || !records.length) fail('seeding needs at least one source record');
  for (const record of records as unknown[]) {
    if (!isObject(record)) return fail('source record is not an object');
    for (const name of ['source_id', 'url', 'retrieved_at', 'sha256', 'licence_id']) text(record[name], `source record ${name}`);
  }
  if (!Array.isArray(seeding.skipped_vintages)) fail('skipped_vintages must be a list');
  for (const skipped of seeding.skipped_vintages as unknown[]) { if (!isObject(skipped)) fail('skipped vintage is not an object'); text((skipped as Record<string, unknown>).report_date, 'skipped vintage date'); text((skipped as Record<string, unknown>).reason, 'skipped vintage reason'); }
  const parameters = value.parameters;
  if (!Array.isArray(parameters)) return fail('parameters missing');
  const stated = scenario.parameters as unknown as Record<string, unknown>;
  const cited = new Set<string>();
  for (const parameter of parameters as unknown[]) {
    if (!isObject(parameter)) return fail('parameter is not an object');
    const name = text(parameter.parameter, 'parameter name');
    if (!(name in stated)) fail(`${name} is not a scenario parameter`);
    if (!same(parameter.value, stated[name])) fail(`${name} does not match the scenario`);
    text(parameter.source, `${name} source`); text(parameter.note, `${name} note`);
    if (parameter.url !== null && typeof parameter.url !== 'string') fail(`${name} url must be text or null`);
    cited.add(name);
  }
  for (const name of Object.keys(stated)) if (!cited.has(name)) fail(`${name} has no citation`);
  if (!Array.isArray(value.nodes) || !Array.isArray(value.excluded_nodes)) fail('nodes and excluded_nodes must be lists');
  text(value.neighbourhood_note, 'neighbourhood_note');
  return value as unknown as ScenarioProvenance;
}

export async function loadScenarioProvenance(base: string, scenario: Scenario, synthetic = false, read: typeof fetch = fetch): Promise<ScenarioProvenance | null> {
  const path = `${base.replace(/\/$/, '')}/data/scenarios/${synthetic ? 'synthetic-scenario' : 'gaines-2025'}.provenance.json`;
  const response = await read(path);
  if (response.status === 404) return null;
  if (!response.ok) throw new Error(`Scenario provenance unavailable (${response.status})`);
  return parseScenarioProvenance(await response.text(), scenario);
}
