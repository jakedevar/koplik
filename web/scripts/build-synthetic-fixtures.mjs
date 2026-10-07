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
  // Contracts v3 rows. Mirrors the real sources: state counts are NNDSS (confirmed or unknown status), county counts are Texas DSHS (confirmed).
  cases: count === null ? { status: 'missing', reason: 'not_reported' } : { status: 'reported', count },
  case_definition: g.id.length === 2 ? 'confirmed_or_unknown_status' : 'confirmed', provenance,
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

// A synthetic forecast pair (contract v1 Forecast rows and the v6 companion) for the web tests and the
// dev server only: invented numbers shaped like the real artifacts, labelled SYNTHETIC throughout. The
// forecast pipeline's own output is tested in koplik-pipeline against the committed fixtures.
const levels = [0.05, 0.25, 0.5, 0.75, 0.95];
const forecastRowsFor = (geography, base) => Array.from({ length: 8 }, (_, i) => {
  const h = i + 1;
  const median = base + h / 2;
  const at = (offset) => Math.max(0, median + offset);
  return { geography, origin_week: { year: 2026, week: 1 }, target_week: { year: 2026, week: 1 + h },
    quantiles: [at(-4), at(-1.5), at(0), at(1.5), at(4)].map((value, j) => ({ level: levels[j], value })),
    seed: 1, run_count: 1000, provenance };
});
// Rows in geography order: Kansas (20) then Texas (48).
const forecastRows = [...forecastRowsFor('20', 6), ...forecastRowsFor('48', 3)];
const parameter = (name, value, note) => ({ parameter: name, value, source: 'SYNTHETIC fixture: an invented value, not a published source', url: null, note });
const hex = (seed) => createHash('sha256').update(seed).digest('hex');
const forecastProvenance = {
  contract_version: 6, artifact: 'weekly-cases',
  statement: 'SYNTHETIC FIXTURE. Invented projections for development; nothing here is a model run on observed counts.',
  method: 'SYNTHETIC FIXTURE: no method was run.',
  origin_week: { year: 2026, week: 1 }, latest_data_week: { year: 2026, week: 3 },
  origin_rule: 'SYNTHETIC FIXTURE: the origin is the first week of 2026, two provisional weeks before the latest.',
  horizon_weeks: 8, run_count: 1000, seed: 1, levels,
  input: { artifact: 'weekly-cases', sha256: createHash('sha256').update(JSON.stringify(cases)).digest('hex'), rows: cases.length },
  parameters: [parameter('window_weeks', 3, 'Estimation window.'), parameter('min_cases', 11, 'Minimum cases in the window.'), parameter('provisional_weeks', 2, 'Recent weeks not used.')],
  series: [
    { geography: '20', case_definition: 'confirmed_or_unknown_status', status: 'forecast', reason: null, cases_in_window: 14, skill: 'insufficient data for a measured skill' },
    { geography: '40', case_definition: 'confirmed_or_unknown_status', status: 'insufficient_data', reason: 'below_threshold', cases_in_window: 2, skill: 'not backtested; no measured skill' },
    { geography: '48', case_definition: 'confirmed_or_unknown_status', status: 'forecast', reason: null, cases_in_window: 15, skill: 'measured' },
    { geography: '35', case_definition: 'confirmed_or_unknown_status', status: 'insufficient_data', reason: 'missing_count', cases_in_window: null, skill: 'not backtested; no measured skill' },
  ].sort((a, b) => a.geography.localeCompare(b.geography)),
  backtest: {
    name: 'the synthetic fixture outbreak', series: 'SYNTHETIC invented outbreak total', geography: '48', case_definition: 'confirmed',
    protocol: 'SYNTHETIC FIXTURE: nothing was backtested.', seed: 1, targets: 4, forecast_dates: 2, origin_weeks: 2,
    mean_crps: 3.5, coverage_50: 0.5, coverage_90: 0.5, mean_persistence_abs_error: 5,
    by_horizon: [{ horizon: 1, n: 2, mean_crps: 3, coverage_50: 0.5, coverage_90: 0.5 }, { horizon: 2, n: 2, mean_crps: 4, coverage_50: 0.5, coverage_90: 0.5 }],
    report_path: 'data/reports/synthetic-backtest.json', report_sha256: hex('synthetic-report'), manifest_sha256: hex('synthetic-manifest'),
    limitations: ['SYNTHETIC FIXTURE: every number here is invented.'],
  },
  // Invented scores shaped like the real evaluation of the CDC state series: a floor of 4 targets from 2 origin weeks
  // (the real floor is 40 from 10), Texas above it, Kansas below it, the other states never forecast.
  series_backtest: {
    name: 'the synthetic fixture state series', series: 'SYNTHETIC invented state counts by report week', case_definition: 'confirmed_or_unknown_status',
    basis: 'pseudo-real-time (revised counts truncated at each forecast date)',
    protocol: 'SYNTHETIC FIXTURE: pseudo-real-time (revised counts truncated at each forecast date); nothing was backtested.',
    seed: 1, provisional_weeks: 2, minimum_targets: 4, minimum_origin_weeks: 2,
    pooled: { forecasts: 4, series: 2, scores: { targets: 8, mean_crps: 9.5, coverage_50: 0.375, coverage_90: 0.75, mean_persistence_abs_error: 6.25,
      by_horizon: [{ horizon: 1, n: 4, mean_crps: 8, coverage_50: 0.25, coverage_90: 0.75 }, { horizon: 2, n: 4, mean_crps: 11, coverage_50: 0.5, coverage_90: 0.75 }] } },
    by_series: [
      { geography: '20', forecasts: 1, origin_weeks: 1, targets: 2, measured: null },
      { geography: '35', forecasts: 0, origin_weeks: 0, targets: 0, measured: null },
      { geography: '40', forecasts: 0, origin_weeks: 0, targets: 0, measured: null },
      { geography: '48', forecasts: 3, origin_weeks: 3, targets: 6, measured: { targets: 6, mean_crps: 12.25, coverage_50: 0.5, coverage_90: 5 / 6, mean_persistence_abs_error: 7,
        by_horizon: [{ horizon: 1, n: 3, mean_crps: 11, coverage_50: 1 / 3, coverage_90: 2 / 3 }, { horizon: 2, n: 3, mean_crps: 13.5, coverage_50: 2 / 3, coverage_90: 1 }] } },
    ],
    report_path: 'data/reports/synthetic-series-backtest.json', report_sha256: hex('synthetic-series-report'), input_sha256: hex('synthetic-series-input'),
    limitations: ['SYNTHETIC FIXTURE: every number here is invented.'],
  },
  scope_note: 'SYNTHETIC FIXTURE: the scores describe invented series, not the forecast beside them.',
};
const artifacts = { geographies, 'weekly-cases': cases, coverage, rt, 'us-states': boundaries(source.states), 'texas-counties': boundaries(source.counties) };
for (const root of [new URL('../../data/fixtures/web/synthetic-v1/', import.meta.url)]) {
  await mkdir(root, { recursive: true });
  for (const [name, value] of Object.entries(artifacts)) await writeFile(new URL(`synthetic-${name}.json`, root), `${JSON.stringify(value, null, 2)}\n`);
  await writeFile(new URL('synthetic-forecast.json', root), `${JSON.stringify(forecastRows, null, 2)}\n`);
  await writeFile(new URL('synthetic-forecast.provenance.json', root), `${JSON.stringify(forecastProvenance, null, 2)}\n`);
}
