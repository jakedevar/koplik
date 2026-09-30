# Koplik: product spec

Koplik spots are the earliest visible sign of measles. Koplik aims to be the earliest
honest signal of a measles outbreak.

**Status:** pre-alpha, built live by an RSI-managed agent team.
**Disclaimer (required on every page and in the README):** "Demonstration project; not medical
or public-health advice; not affiliated with CDC or WHO."

## Principle

A person looking at a county or country can answer five questions, and can check
every answer back to its primary source:

1. **What is happening?** Confirmed cases over time, from primary public sources.
2. **How fast?** Effective reproduction number R_t with credible intervals.
3. **Where is it likely next?** A 4–8 week outbreak-risk forecast.
4. **What if?** Change a county's vaccination coverage and watch a 1,000-run
   simulated ensemble respond in the browser in under 3 s.
5. **Why trust it?** Every published number links to a content-addressed (sha256) source
   snapshot, with its URL, retrieval time and licence.

Architecture: static site, no server. Deterministic pipeline artifacts (Parquet/JSON) plus
one Rust epidemic engine compiled both natively (pipeline forecasts) and to WebAssembly
(in-browser what-if). The same seed gives identical output on both targets.

## Epics

Each Epic lists its intent and acceptance criteria. The manager splits Epics into Issues,
and each Issue restates the slice of intent and criteria that it delivers.

### E1 Contracts (blocks everything)
Intent: one shared, versioned vocabulary for cases, coverage, population, geography,
estimates, forecasts and provenance.
- Cargo workspace, Makefile targets from `AGENTS.md`, and CI-equivalent `make check test`.
- `koplik-contracts` v1 types with serde, generated JSON Schema, and a round-trip test per type.
- Epidemiological week is explicit, with a named MMWR vs ISO choice and conversion tests.
- Geography keys are FIPS (county/state) and ISO 3166 (country), never free-text names.
- A provenance record (source id, URL, retrieved_at RFC3339, sha256, licence id) is attached to every
  derived row.

### E2 Ingest
Intent: fetch primary sources reproducibly into an immutable snapshot store.
- Snapshot store: raw bytes stored under their sha256. It is idempotent: re-fetching unchanged bytes is a no-op.
- Connectors, each with recorded fixtures and a parser test:
  - CDC NNDSS weekly tables on data.cdc.gov (Socrata API), measles by reporting area.
  - CDC "Measles Cases and Outbreaks" weekly national and jurisdiction updates.
  - WHO Immunization Data portal, provisional monthly measles cases by country.
  - Texas DSHS 2025 West Texas outbreak updates (the backtest target).
- `SOURCES.md` row per source: URL, licence or terms, cadence, last verified date.
  An unknown or restrictive licence becomes an operator decision before the data is published.
- Polite fetching: identify the client, rate-limit, respect robots.txt and terms.

### E3 Coverage & Population
Intent: the denominators and susceptibility inputs.
- CDC SchoolVaxView kindergarten MMR coverage and exemptions (state; county where published).
- US Census county population estimates, and county centroids for gravity-model coupling.
- Where county coverage is missing, the gap is explicit. Any imputation is flagged in data and UI,
  never silent.

### E4 Engine (SEIR + WASM) (tier: review before merge)
Intent: a fast, deterministic, testable stochastic metapopulation SEIR model.
- County-level SEIR with tau-leaping and a seeded RNG. Parameters are configurable, with defaults cited:
  latent about 10–12 d, infectious about 8 d, R0 prior 12–18, MMR effectiveness 93% (1 dose) and 97% (2 doses).
- Susceptibility is derived from coverage. Coupling uses a gravity model on population and distance.
- Property tests: S+E+I+R conservation, non-negativity, and a monotone response to coverage.
- `koplik-wasm` facade. **Required test:** identical trajectories native vs wasm for the same seed.
- Performance: 1,000-run ensemble for one county neighbourhood in < 3 s in the browser (benchmark committed).

### E5 Inference (tier: review before merge)
Intent: honest estimates from noisy, delayed counts.
- R_t via the renewal equation (Cori-style) with a gamma serial interval (mean about 11–12 d),
  with credible intervals. Below the minimum-count threshold the output is "insufficient data".
- Reporting-delay nowcast for the most recent weeks, clearly marked provisional.
- Backtest harness: forecast the 2025 West Texas outbreak using only data available
  at each forecast date. Report CRPS and 50%/90% interval coverage exactly as measured.
  Commit the report.

### E6 Web
Intent: the five questions answered on one screen, accessible and fast.
- County choropleth (MapLibre) plus a country view; time series; an R_t ribbon.
- What-if panel driven by the WASM engine.
- Provenance drawer: click any number → snapshot hash, source URL, retrieval time, licence.
- Disclaimer on every page. Keyboard-navigable. Lighthouse performance and accessibility ≥ 90.
- Unit tests plus Playwright e2e against fixture-built data.

### E7 Pipeline & Publish
Intent: one reproducible path from sources to site.
- `koplik-pipeline` stages: ingest → validate → infer → forecast → build. Each stage is idempotent,
  with a content hash of its inputs recorded in the output manifest.
- The daily run is expressed as an RSI deterministic topology (daemon-executed), not a hand-run script.
- `make pipeline && make serve` works from a clean checkout.
- Public publishing (GitHub Pages or similar) is an operator decision; local serving is the default.

## Priority if time or budget runs short

- **Must:** E1; E2 CDC + NNDSS; E3; E5 R_t; E6 county map + provenance; E7 local build.
- **Should:** E4 forecast + WASM what-if + determinism test.
- **Stretch:** E5 backtest; WHO global view; Atom alert feed.
