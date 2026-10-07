import Ajv from 'ajv/dist/2020';
import { expandRowArtifact } from './row-artifact';
import addFormats from 'ajv-formats';
import geographySchema from '../../crates/koplik-contracts/schema/v1/Geography.schema.json';
import casesV1Schema from '../../crates/koplik-contracts/schema/v1/WeeklyCaseCount.schema.json';
import casesSchema from '../../crates/koplik-contracts/schema/v3/WeeklyCaseCount.schema.json';
import coverageSchema from '../../crates/koplik-contracts/schema/v1/KindergartenMmrCoverage.schema.json';
import rtSchema from '../../crates/koplik-contracts/schema/v1/RtEstimate.schema.json';
import provenanceSchema from '../../crates/koplik-contracts/schema/v1/Provenance.schema.json';
import cumulativeSchema from '../../crates/koplik-contracts/schema/v7/CumulativeCaseReport.schema.json';
import type { Provenance } from './generated/Provenance';
import type { Geography } from './generated/Geography';
import type { WeeklyCaseCount as WeeklyCaseCountV1 } from './generated/WeeklyCaseCount';
import type { WeeklyCaseCount } from './generated/v3/WeeklyCaseCount';
import type { KindergartenMmrCoverage } from './generated/KindergartenMmrCoverage';
import type { RtEstimate } from './generated/RtEstimate';
import type { CumulativeCaseReport } from './generated/v7/CumulativeCaseReport';
import type { FeatureCollection, Polygon, MultiPolygon } from 'geojson';

export type Boundaries = FeatureCollection<Polygon | MultiPolygon, { GEOID: string; provenance?: Provenance[] }>;
export interface Dataset {
  geographies: Geography[];
  cases: WeeklyCaseCount[];
  coverage: KindergartenMmrCoverage[];
  rt: RtEstimate[];
  /** Texas DSHS cumulative counts by report date (contracts v7); never converted to weekly counts. */
  cumulative: CumulativeCaseReport[];
  states: Boundaries;
  counties: Boundaries;
  synthetic: boolean;
}
export type Metric = 'cases-2025' | 'cases-2026' | 'coverage';
export interface Value {
  value: number | null;
  label: string;
  detail: string;
}

const ajv = new Ajv({ strict: false, allErrors: true });
addFormats(ajv);
for (const [format, maximum] of [['uint8', 255], ['uint16', 65535], ['uint32', 4294967295]] as const) {
  ajv.addFormat(format, { type: 'number', validate: (value: number) => Number.isInteger(value) && value >= 0 && value <= maximum });
}
ajv.addFormat('double', { type: 'number', validate: Number.isFinite });
const validateCasesV1 = ajv.compile(casesV1Schema);
const validators = {
  geographies: ajv.compile(geographySchema),
  cases: ajv.compile(casesSchema),
  coverage: ajv.compile(coverageSchema),
  rt: ajv.compile(rtSchema),
  cumulative: ajv.compile(cumulativeSchema),
};
const validateProvenance = ajv.compile(provenanceSchema);

export type CaseDefinition = WeeklyCaseCount['case_definition'];
/** What each definition counts, in words. NNDSS publishes confirmed and unknown-status cases together. */
export const caseDefinitionLabels: Record<CaseDefinition, string> = {
  confirmed: 'confirmed cases',
  confirmed_or_unknown_status: 'confirmed or unknown-status cases',
};

/** Why a weekly count is missing, in plain words (the row keeps the contract's reason). */
export const missingReasonWords: Record<Extract<WeeklyCaseCount['cases'], { status: 'missing' }>['reason'], string> = {
  not_reported: 'not reported',
  suppressed: 'suppressed by the source',
  ambiguous: 'ambiguous: cannot be assigned to this week alone',
};

/** Lossless upgrade, mirroring koplik-contracts `From<v1::WeeklyCaseCount>`: a v1 row counted confirmed cases. */
export function upgradeV1Case(row: WeeklyCaseCountV1): WeeklyCaseCount {
  return { geography: row.geography, week: row.week, cases: row.confirmed, case_definition: 'confirmed', provenance: row.provenance };
}

/** The case definition shared by every row, or null when the rows mix definitions (or there are none). */
export function commonCaseDefinition(rows: WeeklyCaseCount[]): CaseDefinition | null {
  const definitions = new Set(rows.map((r) => r.case_definition));
  return definitions.size === 1 ? [...definitions][0] : null;
}

/** The case definition in words for these rows; never "confirmed" unless every row says confirmed. */
export function caseDefinitionWords(rows: WeeklyCaseCount[]): string {
  const definition = commonCaseDefinition(rows);
  if (definition) return caseDefinitionLabels[definition];
  return rows.length ? 'cases (case definitions differ)' : 'cases';
}

export function parseRows<T extends keyof typeof validators>(kind: T, input: unknown): Dataset[T] {
  input = expandRowArtifact(kind, input);
  if (!Array.isArray(input)) throw new Error(`${kind}: expected an array of rows`);
  const validate = validators[kind];
  const keys = new Set<string>();
  const rows: unknown[] = [];
  for (let inputRow of input) {
    if (kind === 'cases' && inputRow && typeof inputRow === 'object' && !('case_definition' in inputRow) && 'confirmed' in inputRow) {
      // A v1 case row is accepted only through the documented lossless conversion.
      if (!validateCasesV1(inputRow)) throw new Error(`${kind}: invalid v1 row: ${ajv.errorsText(validateCasesV1.errors)}`);
      inputRow = upgradeV1Case(inputRow as unknown as WeeklyCaseCountV1);
    }
    if (!validate(inputRow)) throw new Error(`${kind}: invalid ${kind === 'cases' ? 'v3' : kind === 'cumulative' ? 'v7' : 'v1'} row: ${ajv.errorsText(validate.errors)}`);
    rows.push(inputRow);
    if (kind === 'geographies') {
      const row = inputRow as unknown as Geography;
      if (row.level !== (row.id.length === 2 ? 'state' : 'county') || !row.name.trim()) throw new Error('geographies: invalid level or empty name');
    }
    if (kind === 'rt') {
      const row = inputRow as unknown as RtEstimate;
      const bounds = [row.mean, row.lower, row.upper];
      if (row.status === 'insufficient_data' ? bounds.some((x) => x != null) :
        bounds.some((x) => typeof x !== 'number' || !Number.isFinite(x)) || row.lower! > row.upper!) {
        throw new Error('rt: status and bounds disagree');
      }
    }
    if (kind === 'coverage') {
      const row = inputRow as unknown as KindergartenMmrCoverage;
      const start = Number(row.school_year.slice(0, 4));
      if (start < 1900 || start > 2199 || row.school_year.slice(5) !== String((start + 1) % 100).padStart(2, '0')) {
        throw new Error('coverage: invalid school year');
      }
      if (row.imputed ? row.coverage.status === 'missing' || !row.imputation_method?.trim() : row.imputation_method != null) {
        throw new Error('coverage: inconsistent imputation metadata');
      }
    }
    const weekly = inputRow as unknown as WeeklyCaseCount;
    const observationKey = kind === 'geographies' ? (inputRow as unknown as Geography).id :
      kind === 'cumulative' ? `${(inputRow as unknown as CumulativeCaseReport).geography}:${(inputRow as unknown as CumulativeCaseReport).report_date}` :
      `${weekly.geography}:${kind === 'coverage' ? (inputRow as unknown as KindergartenMmrCoverage).school_year : `${weekly.week.year}:${weekly.week.week}`}`;
    const key = kind === 'rt' ? `${observationKey}:${(inputRow as unknown as RtEstimate).interval_level}` : observationKey;
    if (keys.has(key)) throw new Error(`${kind}: duplicate row ${key}`);
    keys.add(key);
  }
  return rows as Dataset[T];
}

export function parseBoundaries(input: unknown, level: 'state' | 'county'): Boundaries {
  const collection = input as Boundaries;
  if (collection?.type !== 'FeatureCollection' || !Array.isArray(collection.features)) {
    throw new Error(`${level}: expected GeoJSON FeatureCollection`);
  }
  const ids = new Set<string>();
  for (const feature of collection.features) {
    const id = feature.properties?.GEOID;
    const provenance = feature.properties?.provenance;
    if (provenance !== undefined && (!Array.isArray(provenance) || !provenance.length || provenance.some((p) => !validateProvenance(p)))) {
      throw new Error(`${level}: invalid boundary provenance`);
    }
    if (feature.type !== 'Feature' || !new RegExp(level === 'state' ? '^\\d{2}$' : '^48\\d{3}$').test(id || '') ||
      ids.has(id) || !['Polygon', 'MultiPolygon'].includes(feature.geometry?.type)) {
      throw new Error(`${level}: invalid or duplicate GeoJSON boundary`);
    }
    const polygons = feature.geometry.type === 'Polygon' ? [feature.geometry.coordinates] : feature.geometry.coordinates;
    if (!Array.isArray(polygons) || !polygons.length || polygons.some((polygon) => !Array.isArray(polygon) || !polygon.length ||
      polygon.some((ring) => !Array.isArray(ring) || ring.length < 4 || ring.some((point) => !Array.isArray(point) || point.length < 2 ||
        !point.every(Number.isFinite) || Math.abs(point[0]) > 180 || Math.abs(point[1]) > 90) ||
        ring[0][0] !== ring.at(-1)![0] || ring[0][1] !== ring.at(-1)![1]))) {
      throw new Error(`${level}: invalid GeoJSON polygon coordinates`);
    }
    ids.add(id);
  }
  return collection;
}

/** Read only static, same-origin artifacts; no external source or tile requests. */
export async function loadDataset(base: string, synthetic = false, read: typeof fetch = fetch): Promise<Dataset> {
  const dataRoot = `${base.replace(/\/$/, '')}/data/`;
  // Row artifacts live under the contract version of their envelope: v6 for the original files, v7 for the cumulative series.
  async function json(name: string, version: 'v6' | 'v7' = 'v6'): Promise<unknown> {
    const response = await read(`${dataRoot}${synthetic ? 'synthetic-v1/synthetic-' : `${version}/`}${name}.json`);
    if (!response.ok) throw new Error(`${name}: artifact unavailable (${response.status})`);
    return response.json();
  }
  const [geographies, cases, coverage, rt, states, counties, cumulative] = await Promise.all([
    json('geographies'), json('weekly-cases'), json('coverage'), json('rt'), json('us-states'), json('texas-counties'),
    json('cumulative-cases', 'v7'),
  ]);
  const result: Dataset = {
    geographies: parseRows('geographies', geographies), cases: parseRows('cases', cases),
    coverage: parseRows('coverage', coverage), rt: parseRows('rt', rt), cumulative: parseRows('cumulative', cumulative),
    states: parseBoundaries(states, 'state'), counties: parseBoundaries(counties, 'county'), synthetic,
  };
  const ids = new Set(result.geographies.map((g) => g.id));
  for (const feature of [...result.states.features, ...result.counties.features]) {
    if (!ids.has(feature.properties.GEOID)) throw new Error(`Boundary has unknown geography ${feature.properties.GEOID}`);
  }
  for (const row of [...result.cases, ...result.coverage, ...result.rt, ...result.cumulative]) {
    if (!ids.has(row.geography)) throw new Error(`Unknown geography ${row.geography}`);
  }
  // A production build cannot accidentally present development fixtures as observations.
  if (!synthetic && ([...result.geographies, ...result.cases, ...result.coverage, ...result.rt, ...result.cumulative]
    .some((row) => row.provenance.some((p) => p.source_id.startsWith('synthetic'))) ||
    [...result.states.features, ...result.counties.features].some((feature) => feature.properties.provenance?.some((p) => p.source_id.startsWith('synthetic'))))) {
    throw new Error('Synthetic artifacts require explicit development fixture mode');
  }
  return result;
}

export function metricValue(data: Dataset, id: string, metric: Metric): Value {
  if (metric === 'coverage') {
    const row = data.coverage.filter((r) => r.geography === id).sort((a, b) => b.school_year.localeCompare(a.school_year))[0];
    if (!row || row.coverage.status === 'missing') return { value: null, label: 'No data', detail: row?.coverage.status === 'missing' ? row.coverage.reason : 'not reported' };
    return { value: row.coverage.coverage_pct, label: `${row.coverage.coverage_pct}%`,
      detail: `${row.school_year} school year${row.imputed ? ` · Imputed: ${row.imputation_method}` : ''}` };
  }
  const year = metric === 'cases-2025' ? 2025 : 2026;
  const rows = data.cases.filter((r) => r.geography === id && r.week.year === year).sort(compareWeeks);
  // An incomplete series is never silently converted into an annual total.
  const definition = commonCaseDefinition(rows);
  if (rows.length && !definition) return { value: null, label: 'No data', detail: 'Weekly reports use different case definitions; not summed' };
  if (!rows.length || rows.some((r) => r.cases.status === 'missing') || rows.at(-1)!.week.week - rows[0].week.week + 1 !== rows.length) return { value: null, label: 'No data', detail: 'Missing or incomplete weekly reports' };
  const total = rows.reduce((sum, r) => sum + (r.cases.status === 'reported' ? r.cases.count : 0), 0);
  return { value: total, label: total.toLocaleString('en-US'),
    detail: `${caseDefinitionLabels[definition!]} · Reported weeks ${rows[0].week.week}–${rows.at(-1)!.week.week}, MMWR ${year} (${rows.length} weeks); not a full-year total` };
}

const levelOf = (id: string): 'state' | 'county' => id.length === 2 ? 'state' : 'county';

/** The case definitions present among a level's weekly rows, in a fixed order; each gets its own map scale. */
export function caseDefinitionsAt(data: Dataset, level: 'state' | 'county'): CaseDefinition[] {
  const present = new Set(data.cases.filter((r) => levelOf(r.geography) === level).map((r) => r.case_definition));
  return (Object.keys(caseDefinitionLabels) as CaseDefinition[]).filter((definition) => present.has(definition));
}
/** The CDC definition for the states view when present, otherwise the first definition available. */
export function defaultCaseDefinition(definitions: CaseDefinition[]): CaseDefinition | undefined {
  return definitions.includes('confirmed_or_unknown_status') ? 'confirmed_or_unknown_status' : definitions[0];
}
function caseYear(metric: Metric) { return metric === 'cases-2025' ? 2025 : 2026; }
/** The definition a geography's rows report under for the metric's year; null for none or for mixed definitions. */
function geographyDefinition(data: Dataset, id: string, metric: Metric): CaseDefinition | null {
  return commonCaseDefinition(data.cases.filter((r) => r.geography === id && r.week.year === caseYear(metric)));
}
/**
 * The value to colour on the map. A case value is coloured only on the scale of its own case definition:
 * a geography whose rows use any other definition is "No data" here, never placed on this definition's scale.
 */
export function mapMetricValue(data: Dataset, id: string, metric: Metric, definition?: CaseDefinition): Value {
  if (metric !== 'coverage' && definition) {
    const own = geographyDefinition(data, id, metric);
    if (own && own !== definition) return { value: null, label: 'No data',
      detail: `Reports ${caseDefinitionLabels[own]}, not ${caseDefinitionLabels[definition]}; not shown on this scale` };
  }
  return metricValue(data, id, metric);
}
/** Geographies at a level whose rows use a case definition other than the selected one (greyed on the map). */
export function otherDefinitionGeographies(data: Dataset, level: 'state' | 'county', metric: Metric, definition: CaseDefinition): string[] {
  if (metric === 'coverage') return [];
  return [...new Set(data.cases.filter((r) => levelOf(r.geography) === level).map((r) => r.geography))]
    .filter((id) => { const own = geographyDefinition(data, id, metric); return own !== null && own !== definition; });
}

export function compareWeeks(a: { week: { year: number; week: number } }, b: { week: { year: number; week: number } }): number {
  return a.week.year - b.week.year || a.week.week - b.week.week;
}

export const metricLabels: Record<Metric, string> = {
  'cases-2025': 'Cases · 2025', 'cases-2026': 'Cases · 2026 to date', coverage: 'Kindergarten MMR coverage',
};
