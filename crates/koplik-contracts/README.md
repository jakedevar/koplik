# koplik-contracts

Shared, versioned data shapes for Koplik: geography keys (FIPS), MMWR weeks, provenance,
case/coverage/population rows, R_t estimates, scenario inputs and forecasts. Serde types
with generated JSON Schema; the web app reads the schema.

Contract v2 re-exports the unchanged v1 inputs/rows and adds `TrajectoryResult` and
`EnsembleResult` for the WASM simulation. Schemas for both versions are committed.
Results retain the exact v1 input JSON (including provenance), decimal-string base
seed, parameters, per-member derived seeds, sampled R0 and full-trajectory hashes.
The ensemble's display fingerprint is explicitly member 0, not a median hash.

## Versioning rule

- **Released versions are immutable.** `v1` is never edited after it lands on `rolling`.
- **A shape change is a new version**: add the next version module (and its `schema/<version>/`), bump
  `CONTRACT_VERSION`, regenerate its JSON Schema. It is the equivalent of a migration.
- Every top-level type's JSON Schema is committed under `schema/<version>/` (one file per
  type). `tests/schema.rs` regenerates the schema and fails if the committed files differ.

## Versions

- **v1** (released, frozen): the original vocabulary. `schema/v1/` and the v1 sources are
  checked byte-for-byte by `tests/frozen.rs`.
- **v2** (released, frozen): v1 plus simulation result types (see above). `schema/v2/` and
  `src/v2/mod.rs` are frozen by `tests/frozen.rs` too.
- **v3** (released, frozen by `tests/frozen.rs`): v2 plus one changed type. `WeeklyCaseCount` replaces
  `confirmed` with `cases` plus a required `case_definition` (`confirmed` |
  `confirmed_or_unknown_status`). Why: CDC's NNDSS publication criteria for measles (event
  code 10140) print cases with *confirmed and unknown* case status, and the weekly data carries
  no case-status field, so those totals must not be typed as confirmed cases (see
  `SOURCES.md`). Every other type is re-exported from v2 unchanged (same Rust type and JSON),
  including `CaseCount`, whose v1 doc comment still says "confirmed-case count": read it as
  "case count". Schema: `schema/v3/`. `impl From<v1::WeeklyCaseCount> for v3::WeeklyCaseCount`
  upgrades a v1 row losslessly (`case_definition: confirmed`); v2 re-exports v1's row, so the
  same impl is also the v2 upgrade. Consumers still on v1/v2 rows (R_t, web loader, Texas DSHS
  connector) migrate in follow-ups.
- **v4** (released, frozen by `tests/frozen.rs`): v3 plus one new type, `ScenarioProvenance` (#1400, #1455): the
  companion of a what-if `ScenarioInput`. It states the seeding as an assumption (not data),
  names where the node inputs came from, and cites every model parameter with the value the
  scenario ran with. `ScenarioProvenance::check_against(&ScenarioInput)` is the rule that a
  scenario is only published beside a companion that describes it (same seed, run count, start
  week, nodes, seeding and parameter values). The scenario input keeps its v1 shape. Every
  other type is re-exported from v3 unchanged. Schema: `schema/v4/`.
- **v5** (released, frozen): v4 plus `ForecastProvenance` and the types it is made of (#1465):
  the companion of a published set of v1 `Forecast` rows. It lists every series considered and
  whether it was forecast or `insufficient_data` (with the estimator's reason), fixes the
  method, every configuration value with its citation, the seed, run count, quantile levels and
  origin week, hashes the input series, and carries the backtest's measured skill (CRPS, 50%/90%
  coverage, per horizon) with the scope it was measured on and a per-series `skill`: `backtested`, or
  `not backtested; no measured skill`.
  `ForecastProvenance::check_against(&[Forecast])` is the rule that forecast rows are only
  published beside a companion that describes them. The forecast rows keep their v1 shape. Every
  other type is re-exported from v4 unchanged. Schema: `schema/v5/`.

- **v6** (`CONTRACT_VERSION` 6): v5 plus `RowArtifact<T>` (geography, weekly cases,
  coverage, R_t and forecast aliases). The wire envelope has `contract_version: 6`,
  a per-file `provenance` table, and `rows` whose `provenance` arrays hold zero-based
  u32 indices. The complete record is deduplicated, preserving row order and the
  ordered provenance of every number, including repeated references. Deserialization
  checks indices and runs the released row validators after expansion. Empty rows
  have an empty table. Companions, GeoJSON and stage outputs retain their current
  contracts; released v1–v5 sources and schemas are unchanged. Schema: `schema/v6/`.

## Regenerate the schema

```bash
make schema        # KOPLIK_REGEN_SCHEMA=1 tools/cargo-test.sh -p koplik-contracts --test schema --test schema_v2 --test schema_v3 --test schema_v4 --test schema_v5 --test schema_v6
git add crates/koplik-contracts/schema
```

## Conventions

- Structs are `deny_unknown_fields`; there are no `usize`/`isize` fields (`wasm32` is 32-bit).
- Geography is keyed by `GeoId` (FIPS string), never by name. Names are display-only.
- A missing value is explicit (`CaseCount::Missing`), never zero.
- Every row carries `Provenances` (at least one record).
- Validated newtypes (`StateFips`, `CountyFips`, `MmwrWeek`, `Sha256Hex`, ...) check on
  construction and on deserialize.
- Contracts hold no model defaults; defaults and citations live in `koplik-epi`.
