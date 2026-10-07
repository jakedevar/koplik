# Koplik web

Static TypeScript + Vite + MapLibre; vanilla DOM and SVG charts. The blank map
style uses only local GeoJSON (no tiles, remote sprites, glyphs, keys or accounts).
Data loading requests only the site's static artifacts.

Build the WASM bindings with `make wasm` before running Vite directly. `make serve`
and `make web-test` build them automatically. Unit tests render actual v2 WASM
output from the committed SEIR fixture through an injected Worker adapter.

Run `npm ci` in `web/` once. `TMPDIR=/tmp make web-test` runs Vitest and the
Playwright smoke test and the local publishing test inside
`tools/offline-test.sh`'s network namespace, with loopback enabled and external
TCP blocked. Dependency installation and WASM compilation happen first.
Publishing-test scratch builds live under `CARGO_TARGET_DIR/publish-tests`;
the browser still uses `TMPDIR=/tmp` to keep its Unix socket path short.
Namespace setup failure refuses execution unless explicitly opted out with
`KOPLIK_ALLOW_NETWORK_TESTS=1` (see `AGENTS.md`; that is an unisolated run).
`make serve` builds
and previews the production site on localhost.
`make serve` always uses `/`. `KOPLIK_BASE_PATH=/koplik/` applies to direct
`npm run build` and `npm run dev` commands. Serve `dist/` with any static host.

`make publish` archives the exact source commit at HEAD into a temporary workspace,
builds with `/koplik/`, and adds `.nojekyll`. Dirty edits and untracked/ignored files
(including caller `pkg/web`, `node_modules` and `web/public/data`) are ignored.
WASM is compiled inside that workspace with its own Cargo target directory;
only an existing version-matched binding tool may be reused. Web dependencies are
installed from the archived lockfile. Before the web build, publishing runs the
offline pipeline from that archive's committed real-byte `data/fixtures/`, with a
scratch work directory and blank `KOPLIK_CONTACT`. It writes `web/public/data` in
the archive, preserving each fixture's recorded retrieval time and provenance.
The pipeline uses the caller's `CARGO_TARGET_DIR` when set, otherwise the scratch
target directory. Pipeline failure or a missing manifest stops publication; the
built site must contain `data/manifest.json` and `data/v6/*.json` before any push.
It creates a commit recording that exact source SHA in an isolated Git repository whose parent
is the fetched `gh-pages` tip (or a root commit on first publication), then pushes
only `HEAD:refs/heads/gh-pages`. A rejected push refetches and retries up to three
times. The caller's files, index, refs and branch are preserved, including dirty edits.
Dirty working trees receive a warning identifying the committed SHA being published.
The temporary workspace is removed after success, failure, SIGINT or SIGTERM;
on interruption, active build commands and their descendants are terminated first.

`PUBLISH_DRY_RUN=1 make publish` builds and prints the proposed commit, parent and
destination without pushing. `PUBLISH_REMOTE=/absolute/path/to/test.git` overrides
the destination for offline testing; destinations must be local bare repositories.
The default is `origin`'s single push URL. Network URLs, including direct GitHub
destinations, are refused. Public mirroring is operator-controlled by the bare
origin's hooks: once enabled, publishing to origin also publishes publicly.
Workers must test against temporary bare repositories, never the real origin.
`make web-test` includes the offline publishing test after the web tests.

## Pipeline artifact layout (contracts v6)

`koplik-pipeline build` (`make pipeline`, or `make pipeline-fixtures` offline from the
committed real-byte fixtures) writes the following files under `web/public/data/v6/`,
plus `web/public/data/manifest.json`: the build manifest with the sha256 of every input
and output, each source's snapshot and retrieval, and what is missing or skipped. Each
observation file contains a versioned envelope:

| File | Published schema |
| --- | --- |
| `geographies.json` | `schema/v6/GeographyArtifact.schema.json` |
| `weekly-cases.json` | `schema/v6/WeeklyCaseCountArtifact.schema.json` (expanded v3 rows with `cases` and `case_definition`) |
| `coverage.json` | `schema/v6/KindergartenMmrCoverageArtifact.schema.json` |
| `rt.json` | `schema/v6/RtEstimateArtifact.schema.json` |
| `us-states.json` | GeoJSON FeatureCollection, Polygon/MultiPolygon, `properties.GEOID` = state FIPS |
| `texas-counties.json` | GeoJSON FeatureCollection, Polygon/MultiPolygon, `properties.GEOID` = Texas county FIPS |

The four observation files now use v6 `*Artifact` envelopes (schemas in
`koplik-contracts/schema/v6/`): `{ contract_version: 6, provenance: [...], rows: [...] }`.
Each row's non-empty `provenance` is an array of indices into that file's table.
The loader validates the envelope, checks reference bounds, then expands records
before applying the released row validators. Every provenance drawer receives the
same source URL, retrieval time, snapshot hash and licence as before. GeoJSON
boundaries retain their existing shapes. Forecast rows use the same v6 envelope
at `data/forecasts/weekly-cases.json`; their v5 companion is unchanged.

Include geography metadata for every boundary and observation. Boundaries use
WGS84 longitude/latitude; FIPS remain zero-padded strings. Additional GeoJSON
properties are allowed. Supply boundary `properties.provenance` as an array of
v1 `Provenance` records for its geometry. Attribution displays those source and
licence ids and updates on drill-down; absent provenance is explicitly labelled
unavailable, with no inferred Census/public-domain claim. Observation keys are
unique: geography + week for cases, geography + week + interval_level for R_t,
and geography + school year for coverage. Empty row arrays in the envelope mean unavailable data; missing counts/coverage use the
contract's explicit `missing` variant. Do not insert zeros for missing weeks.
All six files are required so a failed or incomplete build fails visibly instead
of silently rendering stale partial data. Production never falls back to fixtures.
The boundary files are the Census 2024 cartographic boundaries converted by
`koplik-ingest` (52 states and 254 Texas counties, `properties.GEOID`, `NAME` and
provenance on every feature). When a boundary snapshot is absent from the store the
pipeline writes an explicitly empty FeatureCollection and records it as missing in
the manifest. `koplik-pipeline validate` builds the what-if scenario
(`data/scenarios/gaines-2025.json`) and its provenance companion
(`data/scenarios/gaines-2025.provenance.json`); `build` publishes both or neither, and
reports `gaines-2025` missing in the manifest when an input is absent from the store.
`geographies.json` rows carry `centroid` (the Census Gazetteer internal point) for the
state and Texas county geographies the Gazetteer covers. No `population` artifact is
published: nothing in the web reads one.

At load time Ajv validates rows against the committed v1 schemas and the client
checks contract cross-field semantics, duplicates and geography references.
`npm run generate:types` regenerates all TypeScript types from those same schemas;
tests and builds run `npm run check:types` to detect drift. No Rust contracts changed.

Case rows are contracts v3: `cases` plus a required `case_definition`. The web
app says in words which cases a number counts: `confirmed_or_unknown_status` (CDC
NNDSS state counts) is shown as "confirmed or unknown-status cases" and is never
called "confirmed cases"; `confirmed` (Texas DSHS county counts) is "confirmed
cases". Every case number, axis label, legend, table header and provenance label
names it, and weeks with different definitions are never summed. A v1 case row
(`confirmed`, no `case_definition`) is accepted only through the lossless
conversion `upgradeV1Case` (v1 counted confirmed cases), mirroring
`From<v1::WeeklyCaseCount>` in `koplik-contracts`. Types for v3 case rows are
generated into `src/generated/v3/`.

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

## Terms and attribution

`SOURCES.md` records each source's terms and attribution text (ruled 2026-10-07: CDC and
Census are US federal public domain, 17 USC 105; Texas DSHS is public information used with
attribution and a link; Internet Archive captures fall under the Archive's terms plus the
publisher's). `src/attribution.ts` is the web copy, keyed by the licence id recorded in rows;
ids that say "unconfirmed" are immutable data and are mapped to the ruling there, never
rewritten. The dashboard shows a "Data sources and attribution" section, and the provenance
drawer shows the ruling, terms and attribution next to each record's licence id. A unit test
checks the table against `SOURCES.md`; edit both together.

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
only in Vite development; production reads only `data/v6/` and rejects rows with
synthetic source ids. Production build checks reject synthetic filenames,
directories or provenance in both `public/` before copying and `dist/` afterward;
Vitest also checks these trees and exercises rejection with contaminated outputs.
If upgrading a sandbox that ran the old generator, remove its generated
`public/data/synthetic-v1/` and `dist/data/synthetic-v1/` before building.

## Gaines County hypothetical-introduction what-if panel

The panel is a **hypothetical introduction**: what could happen if one infectious
person arrived in Gaines County, given its population and kindergarten MMR coverage.
It is **not a reconstruction or forecast of the 2025 outbreak** and not a fitted
model, and nothing in it is compared with reported cases. (The artifact keeps the
name `gaines-2025` for the 2025 population and coverage vintages it uses.) A
historical replay was withdrawn because a cumulative DSHS count is not current
infectious prevalence and the retained report vintages are too far apart to repair
that; see `thoughts/shared/notes/gaines-2025-scenario-replay.md`.

The pipeline writes `web/public/data/scenarios/gaines-2025.json`: one
unwrapped **v1 ScenarioInput**, with Gaines FIPS `48165`, a 2025 reference start
week, measured (`reported`, `imputed: false`) baseline coverage and its provenance,
plus population, centroid, the introduced infectious/exposed counts (1 and 0 by
default: a stated assumption, configurable in the pipeline) and all model
parameters. It is built by a rule committed before its first run
(`crates/koplik-pipeline/src/scenario.rs`). This artifact is independent of the
map artifacts; the panel can load even when surveillance reports are unavailable.
Missing or invalid scenario data leaves a visible unavailable state; there is no
production fixture fallback.

Beside it, `gaines-2025.provenance.json` (**contract v4** `ScenarioProvenance`,
schema `crates/koplik-contracts/schema/v4/ScenarioProvenance.schema.json`, types
generated into `src/generated/v4/`, validated by `src/scenario-provenance.ts`) states
the seeding as an assumption and cites every parameter (#1400): `{ contract_version: 4,
scenario, statement, seed (decimal text), run_count, seeding: { geography,
initial_infectious, initial_exposed, start_week, assumption, start_week_basis,
limitation }, parameters: [{ parameter, value, source, url, note }], nodes[],
excluded_nodes[], neighbourhood_note }`. The panel refuses a scenario whose companion
is absent or does not describe it (same seed, run count, start week, node set,
seeding and parameter values), shows the companion's statement as its plain-words
notice, the introduced people marked as a stated assumption (click the number), and a
table citing each parameter (click a value). Parameters are literature citations,
shown in the provenance drawer as published sources rather than snapshot records.

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

## Forecast panel ("Where next?")

The pipeline's `forecast` stage (#1465) runs `koplik_epi::forecast::forecast_weekly` on the
validated weekly case series with the **pre-registered defaults, unchanged** (3-week window, 3-week
look-back, at least 11 cases in the window, 8 weeks ahead, 1,000 members, the 23 hub quantile
levels) and the recorded seed `20250101`, from an origin week two provisional weeks before the
latest data. `build` publishes three files under `data/forecasts/`, or none:
`weekly-cases.json` (v1 `Forecast` rows), `weekly-cases.provenance.json` (**contract v7**
`ForecastProvenance`, schema `crates/koplik-contracts/schema/v7/ForecastProvenance.schema.json`,
types generated into `src/generated/v7/`) and the committed backtest reports the skills were
read from, byte for byte: `backtest-west-texas-2025.json` and `backtest-cdc-states.json`. The companion lists **every series**
considered, each forecast or `insufficient_data` with the estimator's reason, the method and every
parameter with its citation, the seed, run count, quantile levels and origin week, the sha256 of the
input series, and the measured skills of the two backtests with their scope. `src/forecast.ts` validates both
files against the committed schemas and refuses a pair where the companion does not describe the
rows (the cross-checks mirror `ForecastProvenance::check_against`); a missing companion is an
error, never a bare forecast. No file means a visible "not yet available" state.

`src/forecast-view.ts` draws one series at a time (keyboard-accessible selector; it follows the
dashboard's selected geography): reported weekly counts as bars, the forecast median and the 50%
and 90% bands, the origin as a dashed line, and the provisional weeks (reported but still being
revised, and not used) as dashed bars. The exact values are in a table with provenance on every
number. A series that does not meet the method's minimum-count rule says **insufficient data** and
why, in words that name the counts; it never gets a number.

A forecast is introduced, **first, above its chart**, by what is measured about *its own series*
(#1503). Each series carries `skill` in the companion, and the page refuses any other value or a
value its backtest's entry does not support:

- `measured`: the pseudo-real-time backtest of the CDC NNDSS state series scored at least 40
  targets of this series from at least 10 origin weeks (a floor fixed before any score). The page
  says so in plain words with the numbers exactly as measured ("In a pseudo-real-time (revised
  counts truncated at each forecast date) backtest on this series, 90% intervals contained the true
  count 35.1% of the time (101 of 288) ...", mean CRPS against carrying the latest count forward,
  the origin weeks it rests on, and that it is not real-time), each number opening a provenance
  drawer that names the report and the snapshot it was read from.
- `insufficient data for a measured skill`: the backtest ran on this series but scored too few
  targets or origin weeks. "No measured skill for this series. The pseudo-real-time ... backtest
  ... scored N forecasts ... too little to state a skill ... Treat the bands as illustrative."
  No other series' number is shown for it.
- `not backtested; no measured skill` (for example the Texas DSHS county series): "No measured skill
  for this series. This forecast method has not been tested on this data; treat the bands as
  illustrative, not as calibrated uncertainty."
- `backtested`: the series is the one the report-vintage backtest scored (Texas DSHS outbreak
  total); see below.

No skill number and no calibration adjective sits in or next to the chart of a series without a
measured skill.

The tests are reported in their own section, **How we evaluate forecasts**, after the forecast,
each with its own scope and basis and neither standing in for the other. The **CDC state series**
test (pseudo-real-time: one retrieval of the source is held and CDC publishes no revision history,
so counts are revised counts truncated at each forecast date, never called real-time) gives the
pooled result over the series where the method's minimum-count rule held ("pooled over 22 series and
239 forecasts, 90% intervals contained the true count 39.0% of the time (682 of 1748)"; the pooled
mean is dominated by the largest projections and is not any one series' skill), the table of the
series with a measured skill, the pooled scores by horizon, the limitations and the exact report.
The **West Texas** test (48 targets from 7 forecast dates and 5 origin weeks, real-time by report
vintage) keeps its own headline, per-horizon table and limitations, and does not measure the
published state series. Synthetic dev mode serves an invented, clearly labelled forecast pair
(`data/fixtures/web/synthetic-v1/synthetic-forecast*.json`) from `npm run fixtures`.

## Integration events

`mountDashboard` emits a bubbling `koplik:selection` CustomEvent with
`{ geography, metric, year, data }` when selection changes. The what-if panel and
provenance drawer can attach there; exact report rows also have `data-geography`
and `data-week` attributes. The map's native state/county selector remains usable
when WebGL is unavailable. The disclaimer is also rendered on loading/error views.

## Number provenance and browser smoke test

Underlined numeric values open the provenance drawer with mouse, Enter or Space.
Chart marks and whole charts also support keyboard activation. The native modal
dialog moves focus to its close button, traps Tab/Shift-Tab and closes with Escape,
returning focus to the triggering number. The drawer shows only artifact fields:
snapshot sha256, source identifier, source URL, retrieval time and licence/terms
identifier. Empty or absent fields say `Missing`; absent records are explicitly
labelled missing. HTTP(S) source URLs are links; other URIs (including the synthetic
SEIR fixture's local `file:` URI) remain exact plain text. No raw snapshot URL,
licence text or source attribution is guessed. Fixture records are labelled
**synthetic, not a published source** inside the drawer as well as on the page.

Case period totals list every input weekly record's provenance. R_t rows and chart
segments list the provenance attached to those derived artifact rows, rather than
looking up nearby case reports. Ensemble values and fingerprints list the union
of linked population/centroid and baseline-coverage provenance for **all counties**
in the exact result replay scenario. Exact duplicate records are shown once;
differing URLs, licence ids or retrieval times are retained even for the same hash.
The v1 scenario has no separate source links for model parameters, initial
seeding or overrides. The drawer states that limitation; seeds, model settings,
user-selected coverage and locally measured update times are configuration or local
measurements, not published observations. Follow-up #1400 asks pipeline #1359 to
link parameter and initial-seeding sources without modifying released contracts.

The geography selector keeps native keyboard selection. Its numeric summaries
are now in the expandable **Compare geography values and sources** table, where
every value can independently open its provenance.

`make web-test` builds WASM and runs the units followed by **one Playwright smoke
test**. It generates the web fixture artifacts from the committed synthetic
source, starts and stops a dev server under `/koplik/`, blocks external page
requests and verifies rendered map readiness, a keyboard slider change, changed
ensemble bands/fingerprint, number and SVG activation, all drawer fields, focus
trapping and Escape focus restoration. Synthetic artifacts stay outside `public/`
and never enter the production build. Failed browser tests keep a local trace in
ignored `web/test-results/`.

The smoke test uses `CHROME_BIN` when set, otherwise system Chrome at
`/opt/google/chrome/chrome`, otherwise an already-installed Playwright Chromium.
It fails explicitly if no executable is available; it never skips or downloads a
browser during tests. Install a browser separately if needed. Always set
`TMPDIR=/tmp` in RSI sandboxes to avoid Chromium's long Unix socket path (#1367).
To run only the smoke test after `make wasm`, use
`cd web && TMPDIR=/tmp ../tools/offline-test.sh npm run test:smoke`.
