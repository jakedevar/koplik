# Koplik web

Static TypeScript + Vite + MapLibre; vanilla DOM and SVG charts. The blank map
style uses only local GeoJSON (no tiles, remote sprites, glyphs, keys or accounts).
Data loading requests only the site's static artifacts.

Build the WASM bindings with `make wasm` before running Vite directly. `make serve`
and `make web-test` build them automatically. Unit tests render actual v2 WASM
output from the committed SEIR fixture through an injected Worker adapter.

Run `npm ci` in `web/` once. `make web-test` runs Vitest offline against committed
fixtures. `make serve` builds and previews the production site on localhost.
`KOPLIK_BASE_PATH=/koplik/ make serve` sets the GitHub Pages path; the same variable
applies to `npm run build` and `npm run dev`. Serve `dist/` with any static host.

## Pipeline artifact layout (contracts v1)

The pipeline writes the following files under `web/public/data/v1/`. Each row
array contains unwrapped v1 contract objects, not a new shared contract shape:

| File | Row schema in `crates/koplik-contracts/schema/v1/` |
| --- | --- |
| `geographies.json` | `Geography.schema.json` |
| `weekly-cases.json` | `WeeklyCaseCount.schema.json` |
| `coverage.json` | `KindergartenMmrCoverage.schema.json` |
| `rt.json` | `RtEstimate.schema.json` |
| `us-states.json` | GeoJSON FeatureCollection, Polygon/MultiPolygon, `properties.GEOID` = state FIPS |
| `texas-counties.json` | GeoJSON FeatureCollection, Polygon/MultiPolygon, `properties.GEOID` = Texas county FIPS |

Include geography metadata for every boundary and observation. Boundaries use
WGS84 longitude/latitude; FIPS remain zero-padded strings. Additional GeoJSON
properties are allowed. Supply boundary `properties.provenance` as an array of
v1 `Provenance` records for its geometry. Attribution displays those source and
licence ids and updates on drill-down; absent provenance is explicitly labelled
unavailable, with no inferred Census/public-domain claim. Observation keys are
unique: geography + week for cases, geography + week + interval_level for R_t,
and geography + school year for coverage. Empty arrays mean unavailable data; missing counts/coverage use the
contract's explicit `missing` variant. Do not insert zeros for missing weeks.
All six files are required so a failed or incomplete build fails visibly instead
of silently rendering stale partial data. Production never falls back to fixtures.

At load time Ajv validates rows against the committed v1 schemas and the client
checks contract cross-field semantics, duplicates and geography references.
`npm run generate:types` regenerates all TypeScript types from those same schemas;
tests and builds run `npm run check:types` to detect drift. No Rust contracts changed.

The cases map sums the contiguous reported period available within the selected
MMWR year and explicitly labels its week range and number of reports. It does
not claim a full-year total. Missing rows or internal gaps make the aggregate
missing. Coverage selects the latest reported school year for each geography
(including a latest missing row) and displays its year and imputation metadata.
The chart table reports exact values for every supplied R_t interval level.
R_t ribbons are grouped by interval level, with wider levels drawn first; each
sequence never bridges omitted weeks, insufficient-data weeks or provisional weeks.
Insufficient-data weeks have grey hatched bands (I); provisional weeks have dashed
outlines (P), with both marks (IP) when both statuses apply. Each marker has an
accessible week/status label and a visible legend; absent rows remain blank.
Provisional estimates remain withheld. Reported zero cases have baseline ticks,
distinct from missing-week gaps.

## Explicit synthetic development mode

Until ingestion/pipeline artifacts are available, run `npm run dev:synthetic`.
It generates clearly named `synthetic-*.json` files from the committed
`data/fixtures/web/synthetic-source.json` into `data/fixtures/web/synthetic-v1/`,
outside `public/`. An opted-in dev-only Vite middleware serves those files at
`data/synthetic-v1/` under the configured base path. Each fixture row's provenance
hash identifies that source file's exact bytes. The fixture URL is deliberately
`example.invalid`, and the timestamp records fixture creation, not real retrieval.
All numeric values and rectangular boundaries are invented. The UI displays a
persistent synthetic-data banner and map attribution. Fixture mode is enabled
only in Vite development; production reads only `data/v1/` and rejects rows with
synthetic source ids. Production build checks reject synthetic filenames,
directories or provenance in both `public/` before copying and `dist/` afterward;
Vitest also checks these trees and exercises rejection with contaminated outputs.
If upgrading a sandbox that ran the old generator, remove its generated
`public/data/synthetic-v1/` and `dist/data/synthetic-v1/` before building.

## Gaines County 2025 what-if panel

The pipeline must write `web/public/data/scenarios/gaines-2025.json`: one
unwrapped **v1 ScenarioInput**, with Gaines FIPS `48165`, a 2025 start week,
measured (`reported`, `imputed: false`) baseline coverage and its provenance,
plus population, centroids, initial exposed/infectious counts and all model
parameters. Other nodes with missing coverage require explicit overrides under
the existing engine contract. This artifact is independent of the map artifacts;
the panel can load even when surveillance reports are unavailable. Missing or
invalid scenario data leaves a visible unavailable state; there is no production
fixture fallback. Current real coverage/population/centroid inputs are pending
#1351, #1352 and pipeline #1359.

The panel starts at Gaines' measured coverage, removes any pre-existing Gaines
override, and always requests 1,000 runs. Changing the native keyboard-accessible
coverage slider adds only that county's override; restore returns to the baseline.
Parameters, seed and other counties' inputs come from the artifact. The module
Worker initializes `pkg/web` and calls `runEnsemble` off the main thread. Rapid
inputs debounce for 120 ms and coalesce to the latest pending coverage; obsolete
results never replace the current run or leave an old fingerprint visible.

The chart and exact daily table show Gaines' **cumulative infections**: initial
E + I plus new exposures, excluding vaccine immunity. These are simulated
infections, not reported cases. Bands are equal-tail 50% and 90% predictive
intervals across the ensemble. The displayed sha256 identifies **member 0's full
trajectory**, matching `make determinism`, rather than the median line. The panel
shows every supplied model parameter and the exact scenario JSON for replay.
Copy that JSON unchanged to a file and run `target/release/trajectory-native
<file>` after `make determinism` to compare `member.fingerprint`. Seed tokens
remain decimal strings in the UI and Worker protocol; serialization inserts the
validated decimal u64 token into v1 JSON without converting it to a JS number.

In `npm run dev:synthetic`, the opted-in middleware serves the committed
`data/fixtures/seir/synthetic-scenario.json` at
`data/scenarios/synthetic-scenario.json`. This fixture's measured baseline is
explicitly missing; development starts from its **labelled synthetic coverage
override**, with no modification or imputation of the source fixture. The dev
server allows the generated `pkg/web` directory so Worker WASM loading works.
Fixtures stay outside `public/`; the production build guard remains enforced.

After `make wasm` and `npm ci` in `web/`, run the offline browser integration and
timing check from the repository root:

```bash
TMPDIR=/tmp node web/scripts/measure-what-if.mjs
```

It starts and stops its own Vite server on port 5138 (`WHAT_IF_PORT` can override),
uses installed Chrome (`CHROME_BIN` can override `/opt/google/chrome/chrome`),
and blocks external requests. It checks an actual simulation Worker, responsive
main-thread heartbeats, changed bands, restored fingerprints, Node WASM replay,
u64::MAX browser replay and the unavailable-data state. Timing starts at coverage
input and includes debounce, cold Worker/WASM initialization, execution, result
transfer and DOM rendering; `through_paint_ms` includes two animation frames and
automation observation. See [the recorded measurement](measurements/what-if-chrome-2026-10-07.json):
cold 514.5999999977648 ms, warm slider median 601.1999999992549 ms on the
seven-county, 180-day, 1,000-run **synthetic** fixture. Warm through-paint samples
range from 599.1000000014901 to 648.6000000014901 ms. These measurements do not
establish performance on the future real Gaines County artifact.

## Integration events

`mountDashboard` emits a bubbling `koplik:selection` CustomEvent with
`{ geography, metric, year, data }` when selection changes. The what-if panel and
provenance drawer can attach there; exact report rows also have `data-geography`
and `data-week` attributes. The map's native state/county selector remains usable
when WebGL is unavailable. The disclaimer is also rendered on loading/error views.
