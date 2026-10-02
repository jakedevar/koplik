# Koplik: product spec

Koplik spots are the earliest visible sign of measles. Koplik aims to be the earliest
honest signal of a measles outbreak.

**Status:** pre-alpha, built live by an RSI-managed agent team.
**Disclaimer (required on every page and in the README):** "Demonstration project; not medical
or public-health advice; not affiliated with CDC or WHO."

## Why now

US measles cases in 2026 passed the 2025 total, and PAHO's verification commission reviews
US measles elimination status in November 2026. Most 2026 cases are in South Carolina and
Utah. The 2025 West Texas outbreak (Gaines County) is the best-documented recent US outbreak.
This is context for choosing what to build, not data: the site shows only numbers the
pipeline ingested, each with its provenance.

## Principle

A person looking at a US state, or at a Texas county, can answer five questions, and can
check every answer back to its primary source:

1. **What is happening?** Confirmed cases over time from primary public sources: every US
   state for 2025 and 2026 to date, and Texas counties for the 2025 West Texas outbreak.
   CDC publishes measles cases by state, not by county; county detail exists only where a
   state publishes it, and Koplik shows it only there.
2. **How fast?** Effective reproduction number R_t with credible intervals.
3. **What if?** Change a county's MMR coverage and watch a 1,000-run simulated ensemble
   respond in the browser in under 3 s. The flagship scenario is Gaines County, Texas, 2025.
   The same seed gives a bit-identical trajectory natively and in the browser, and the page
   shows that trajectory's fingerprint (sha256).
4. **Where next?** A 4–8 week outbreak forecast, scored by an honest backtest (Should tier).
5. **Why trust it?** Every published number links to a content-addressed (sha256) source
   snapshot, with its URL, retrieval time and licence.

Architecture: static site, no server. Deterministic pipeline artifacts (Parquet/JSON) plus
one Rust epidemic engine compiled both natively (pipeline) and to WebAssembly (in-browser
what-if). Map shapes come from public-domain Census cartographic boundary files rendered as
GeoJSON: no third-party map tiles, accounts or API keys.

## Epics

Each Epic lists its intent and acceptance criteria. The manager splits Epics into Issues,
and each Issue restates the slice of intent and criteria that it delivers.

### E1 Contracts (blocks everything)
Intent: one shared, versioned vocabulary for cases, coverage, population, geography,
estimates, scenarios, forecasts and provenance. Keep it small: E1 gates the fan-out.
- Cargo workspace, Makefile targets from `AGENTS.md`, and CI-equivalent `make check test`.
- `koplik-contracts` v1 types with serde, generated JSON Schema, and a round-trip test per type.
- Epidemiological week is explicit: MMWR weeks (CDC's convention), with conversion tests.
- Geography keys are FIPS codes (state and county), never free-text names.
- A provenance record (source id, URL, retrieved_at RFC3339, sha256, licence id) is attached to every
  derived row.

### E2 Ingest
Intent: fetch primary sources reproducibly into an immutable snapshot store.
- Snapshot store: raw bytes stored under their sha256. It is idempotent: re-fetching unchanged
  bytes is a no-op. Every retrieved version is kept, so "what was published on date D" stays answerable.
- Connectors, each with recorded fixtures and a parser test:
  - CDC weekly measles cases by state (jurisdiction) for 2025 and 2026: the CDC "Measles Cases
    and Outbreaks" data or the NNDSS weekly tables on data.cdc.gov, whichever gives weekly state
    counts reliably. One CDC source is enough.
  - Texas DSHS 2025 West Texas outbreak updates: cases by county over time.
  - US Census cartographic boundary files: states, and Texas counties.
- `SOURCES.md` row per source: URL, licence or terms, cadence, last verified date.
  An unknown or restrictive licence becomes an operator decision before the data is published.
- Polite fetching: identify the client, rate-limit, respect robots.txt and terms.

### E3 Coverage & Population
Intent: the denominators and susceptibility inputs.
- CDC SchoolVaxView kindergarten MMR coverage and exemptions by state.
- Texas DSHS kindergarten vaccination coverage by county, where DSHS publishes it (the what-if
  baseline for Texas counties).
- US Census population estimates (states; Texas counties) and Texas county centroids for
  gravity-model coupling.
- Where coverage is missing, the gap is explicit. Any imputation is flagged in data and UI,
  never silent.

### E4 Engine (SEIR + WASM) (tier: review before merge)
Intent: a fast, deterministic, testable stochastic metapopulation SEIR model that gives the
same answer natively and in the browser.
- County-level SEIR with tau-leaping and a seeded RNG. Parameters are configurable, with defaults cited:
  latent about 10–12 d, infectious about 8 d, R0 prior 12–18, MMR effectiveness 93% (1 dose) and 97% (2 doses).
- Susceptibility is derived from coverage. Coupling uses a gravity model on population and
  distance (the Texas county neighbourhood for the flagship scenario).
- Property tests: S+E+I+R conservation, non-negativity, and a monotone response to coverage.
- `koplik-wasm` facade. **Required test (`make determinism`):** for a fixed seed and scenario,
  the sha256 of the full trajectory (every compartment at every step, as little-endian f64) is
  identical on native and `wasm32`. The what-if panel shows that hash as the engine fingerprint.
- Portability rules. Each one prevents a native/wasm mismatch:
  - Use a seeded, documented-portable RNG (`rand_chacha::ChaCha8Rng` or equivalent).
  - Never use `usize`/`isize` in random draws or in hashed output. `usize` is 32-bit on
    `wasm32`, so the draws differ. Use `u32`/`u64`.
  - Every `exp`/`ln`/`pow`/trig call on the seeded path goes through the `libm` crate,
    including inside samplers (Poisson, binomial). std calls the platform libm natively and a
    bundled port on wasm; a last-bit difference makes trajectories diverge.
  - Never iterate a `HashMap`/`HashSet` on a path that affects output. Use `BTreeMap` or a
    `Vec` sorted by FIPS.
  - No parallel floating-point reductions. Ensemble members may run in parallel natively only
    with per-member derived seeds and results collected in member order.
- Performance: a 1,000-run ensemble for the flagship scenario in < 3 s in the browser (benchmark committed).

### E5 Inference (tier: review before merge)
Intent: honest estimates from noisy, delayed counts.
- R_t via the renewal equation (Cori-style) with a gamma serial interval (mean about 11–12 d),
  with credible intervals. Below the minimum-count threshold the output is "insufficient data".
- The most recent two weeks are marked provisional (reporting delay). A full nowcast is Stretch.
- Should: a 4–8 week forecast, and a backtest on the 2025 West Texas outbreak that uses only data
  available at each forecast date. Use report vintages where they exist (retained snapshots, or
  archived copies of the DSHS updates with their capture URL and time recorded as provenance).
  If only revised counts exist, label the backtest "pseudo-real-time (revised counts truncated
  at each forecast date)". Report CRPS and 50%/90% interval coverage exactly as measured, and
  commit the report.

### E6 Web
Intent: the questions answered on one screen, accessible and fast.
- US state choropleth (cases for 2025 and 2026 to date; kindergarten MMR coverage), a Texas
  county drill-down for the 2025 outbreak, a time series and an R_t ribbon. MapLibre over the
  E2 boundary GeoJSON.
- What-if panel driven by the WASM engine. It opens on the Gaines County 2025 scenario at its
  measured coverage and has a coverage slider. It shows the median and 50%/90% bands from
  1,000 runs, plus the seed, the parameters and the engine fingerprint. It is labelled
  "illustrative model scenario, not a prediction".
- Provenance drawer: click any number → snapshot hash, source URL, retrieval time, licence.
- Disclaimer on every page. Keyboard-navigable.
- Unit tests plus one Playwright smoke test against fixture-built data: the page loads, the map
  renders, the slider changes the ensemble, and the provenance drawer opens.

### E7 Pipeline & Publish
Intent: one reproducible path from sources to a public site.
- `koplik-pipeline` stages: ingest → validate → infer → (forecast) → build. Each stage is
  idempotent, with a content hash of its inputs recorded in the output manifest.
- `make pipeline && make serve` works from a clean checkout.
- `make publish` builds the site for GitHub Pages (base path `/koplik/`, plus a `.nojekyll`
  file) and pushes it to `origin` as a new fast-forward commit on the `gh-pages` branch, never a
  force push. Running it changes nothing public: the operator decides on public publishing (see
  `AGENTS.md`, "Landing").
- Stretch: the daily run expressed as an RSI deterministic topology (daemon-executed). This RSI
  surface has not yet run on this machine; it never blocks publishing.

## Priority if time or budget runs short

- **Must (the demo):** E1; E2 CDC state cases, Texas DSHS county cases and boundaries; E3; E4
  engine, WASM what-if and the determinism fingerprint; E5 R_t; E6 map, Texas drill-down,
  what-if, provenance drawer and smoke test; E7 local build and `make publish`.
- **Should:** E5 forecast and its honest backtest.
- **Stretch:** nowcast; the RSI topology for the daily run; a WHO global view; an Atom alert
  feed; Lighthouse performance and accessibility ≥ 90.
