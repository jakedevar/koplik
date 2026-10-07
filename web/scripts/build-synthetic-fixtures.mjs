import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { createHash } from 'node:crypto';

// Development-only, explicitly invented inputs. Never used by the production loader.
const sourceURL = new URL('../../data/fixtures/web/synthetic-source.json', import.meta.url);
const bytes = await readFile(sourceURL);
const source = JSON.parse(bytes);
if (!source.label.startsWith('SYNTHETIC TEST DATA')) throw new Error('Fixture must be explicitly synthetic');
const provenance = [{
  source_id: 'synthetic-web-test',
  url: 'https://example.invalid/synthetic-web-test',
  retrieved_at: source.created_at,
  sha256: createHash('sha256').update(bytes).digest('hex'),
  licence_id: 'synthetic-test-only',
}];
const all = [...source.states, ...source.counties];
const geographies = all.map((g) => ({ id: g.id, name: g.name, level: g.id.length === 2 ? 'state' : 'county', centroid: null, provenance }));
const cases = all.flatMap((g) => [2025, 2026].flatMap((year) => g[`cases${year}`].map((count, i) => ({
  geography: g.id, week: { year, week: i + 1 },
  confirmed: count === null ? { status: 'missing', reason: 'not_reported' } : { status: 'reported', count }, provenance,
}))));
const coverage = all.map((g) => ({ geography: g.id, school_year: '2024-25', imputed: false, imputation_method: null,
  coverage: g.coverage === null ? { status: 'missing', reason: 'not_reported' } : { status: 'reported', coverage_pct: g.coverage, exemption_pct: null }, provenance }));
const rt = all.flatMap((g) => g.rt.map((estimate, i) => ({ geography: g.id, week: { year: 2025, week: i + 1 },
  status: estimate === null ? 'insufficient_data' : 'ok', provisional: i === g.rt.length - 1,
  mean: estimate?.[0] ?? null, lower: estimate?.[1] ?? null, upper: estimate?.[2] ?? null, interval_level: 0.9, provenance,
})));
function boundaries(rows) {
  return { type: 'FeatureCollection', features: rows.map((g) => {
    const [west, south, east, north] = g.rectangle;
    return { type: 'Feature', properties: { GEOID: g.id, provenance }, geometry: { type: 'Polygon', coordinates: [
      [[west, south], [east, south], [east, north], [west, north], [west, south]],
    ] } };
  }) };
}
const artifacts = { geographies, 'weekly-cases': cases, coverage, rt, 'us-states': boundaries(source.states), 'texas-counties': boundaries(source.counties) };
for (const root of [new URL('../../data/fixtures/web/synthetic-v1/', import.meta.url)]) {
  await mkdir(root, { recursive: true });
  for (const [name, value] of Object.entries(artifacts)) await writeFile(new URL(`synthetic-${name}.json`, root), `${JSON.stringify(value, null, 2)}\n`);
}
