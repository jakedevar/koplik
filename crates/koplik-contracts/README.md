# koplik-contracts

Shared, versioned data shapes for Koplik: geography keys (FIPS), MMWR weeks, provenance,
case/coverage/population rows, R_t estimates, scenario inputs and forecasts. Serde types
with generated JSON Schema; the web app reads the schema.

## Versioning rule

- **Released versions are immutable.** `v1` is never edited after it lands on `rolling`.
- **A shape change is a new version**: add a `v2` module (and `schema/v2/`), bump
  `CONTRACT_VERSION`, regenerate its JSON Schema. It is the equivalent of a migration.
- Every top-level type's JSON Schema is committed under `schema/<version>/` (one file per
  type). `tests/schema.rs` regenerates the schema and fails if the committed files differ.

## Regenerate the schema

```bash
make schema        # KOPLIK_REGEN_SCHEMA=1 cargo test -p koplik-contracts --test schema
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
