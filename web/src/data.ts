import Ajv from 'ajv/dist/2020';
import addFormats from 'ajv-formats';
import geographySchema from '../../crates/koplik-contracts/schema/v1/Geography.schema.json';
import casesSchema from '../../crates/koplik-contracts/schema/v1/WeeklyCaseCount.schema.json';
import coverageSchema from '../../crates/koplik-contracts/schema/v1/KindergartenMmrCoverage.schema.json';
import rtSchema from '../../crates/koplik-contracts/schema/v1/RtEstimate.schema.json';
import type { Geography } from './generated/Geography';
import type { WeeklyCaseCount } from './generated/WeeklyCaseCount';
import type { KindergartenMmrCoverage } from './generated/KindergartenMmrCoverage';
import type { RtEstimate } from './generated/RtEstimate';
import type { FeatureCollection, Polygon, MultiPolygon } from 'geojson';

export type Boundaries = FeatureCollection<Polygon | MultiPolygon, { GEOID: string }>;
export interface Dataset {
  geographies: Geography[];
  cases: WeeklyCaseCount[];
  coverage: KindergartenMmrCoverage[];
  rt: RtEstimate[];
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
const validators = {
  geographies: ajv.compile(geographySchema),
  cases: ajv.compile(casesSchema),
  coverage: ajv.compile(coverageSchema),
  rt: ajv.compile(rtSchema),
};

export function parseRows<T extends keyof typeof validators>(kind: T, input: unknown): Dataset[T] {
  if (!Array.isArray(input)) throw new Error(`${kind}: expected an array of v1 rows`);
  const validate = validators[kind];
  const keys = new Set<string>();
  for (const row of input) {
    if (!validate(row)) throw new Error(`${kind}: invalid v1 row: ${ajv.errorsText(validate.errors)}`);
    if (kind === 'geographies' && (row.level !== (row.id.length === 2 ? 'state' : 'county') || !row.name.trim())) {
      throw new Error('geographies: invalid level or empty name');
    }
    if (kind === 'rt') {
      const bounds = [row.mean, row.lower, row.upper];
      if (row.status === 'insufficient_data' ? bounds.some((x) => x != null) :
        bounds.some((x) => typeof x !== 'number' || !Number.isFinite(x)) || row.lower > row.upper) {
        throw new Error('rt: status and bounds disagree');
      }
    }
    if (kind === 'coverage') {
      const start = Number(row.school_year.slice(0, 4));
      if (start < 1900 || start > 2199 || row.school_year.slice(5) !== String((start + 1) % 100).padStart(2, '0')) {
        throw new Error('coverage: invalid school year');
      }
      if (row.imputed ? row.coverage.status === 'missing' || !row.imputation_method?.trim() : row.imputation_method != null) {
        throw new Error('coverage: inconsistent imputation metadata');
      }
    }
    const key = kind === 'geographies' ? row.id : `${row.geography}:${kind === 'coverage' ? row.school_year : `${row.week.year}:${row.week.week}`}`;
    if (keys.has(key)) throw new Error(`${kind}: duplicate row ${key}`);
    keys.add(key);
  }
  return input as Dataset[T];
}

export function parseBoundaries(input: unknown, level: 'state' | 'county'): Boundaries {
  const collection = input as Boundaries;
  if (collection?.type !== 'FeatureCollection' || !Array.isArray(collection.features)) {
    throw new Error(`${level}: expected GeoJSON FeatureCollection`);
  }
  const ids = new Set<string>();
  for (const feature of collection.features) {
    const id = feature.properties?.GEOID;
    if (feature.type !== 'Feature' || !new RegExp(level === 'state' ? '^\\d{2}$' : '^48\\d{3}$').test(id || '') ||
      ids.has(id) || !['Polygon', 'MultiPolygon'].includes(feature.geometry?.type)) {
      throw new Error(`${level}: invalid or duplicate GeoJSON boundary`);
    }
    ids.add(id);
  }
  return collection;
}

/** Read only static, same-origin artifacts; no external source or tile requests. */
export async function loadDataset(base: string, synthetic = false, read: typeof fetch = fetch): Promise<Dataset> {
  const root = `${base.replace(/\/$/, '')}/data/${synthetic ? 'synthetic-v1' : 'v1'}/`;
  async function json(name: string): Promise<unknown> {
    const response = await read(`${root}${synthetic ? 'synthetic-' : ''}${name}.json`);
    if (!response.ok) throw new Error(`${name}: artifact unavailable (${response.status})`);
    return response.json();
  }
  const [geographies, cases, coverage, rt, states, counties] = await Promise.all([
    json('geographies'), json('weekly-cases'), json('coverage'), json('rt'), json('us-states'), json('texas-counties'),
  ]);
  const result: Dataset = {
    geographies: parseRows('geographies', geographies), cases: parseRows('cases', cases),
    coverage: parseRows('coverage', coverage), rt: parseRows('rt', rt),
    states: parseBoundaries(states, 'state'), counties: parseBoundaries(counties, 'county'), synthetic,
  };
  const ids = new Set(result.geographies.map((g) => g.id));
  for (const row of [...result.cases, ...result.coverage, ...result.rt]) {
    if (!ids.has(row.geography)) throw new Error(`Unknown geography ${row.geography}`);
  }
  // A production build cannot accidentally present development fixtures as observations.
  if (!synthetic && [...result.geographies, ...result.cases, ...result.coverage, ...result.rt]
    .some((row) => row.provenance.some((p) => p.source_id.startsWith('synthetic')))) {
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
  if (!rows.length || rows.some((r) => r.confirmed.status === 'missing')) return { value: null, label: 'No data', detail: 'Missing or incomplete weekly reports' };
  const total = rows.reduce((sum, r) => sum + (r.confirmed.status === 'reported' ? r.confirmed.count : 0), 0);
  return { value: total, label: total.toLocaleString('en-US'),
    detail: `Reported weeks ${rows[0].week.week}–${rows.at(-1)!.week.week}, MMWR ${year} (${rows.length} weeks); not a full-year total` };
}

export function compareWeeks(a: { week: { year: number; week: number } }, b: { week: { year: number; week: number } }): number {
  return a.week.year - b.week.year || a.week.week - b.week.week;
}

export const metricLabels: Record<Metric, string> = {
  'cases-2025': 'Cases · 2025', 'cases-2026': 'Cases · 2026 to date', coverage: 'Kindergarten MMR coverage',
};
