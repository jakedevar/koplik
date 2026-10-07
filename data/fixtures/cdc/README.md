# CDC NNDSS weekly measles fixture

`nndss-measles-weekly.json` is the **unmodified** response body of one live fetch made with
`koplik-ingest fetch cdc-cases` (all rows, nothing trimmed, no value edited). Its retrieval
record, as written by the snapshot store's log, is `nndss-measles-weekly.retrieval.json`.

- Source id: `cdc-nndss-weekly-measles` (see `SOURCES.md`)
- Retrieved: 2026-10-07T02:19:30Z
- SHA-256: `c4f6862d093b10c59b3519bdef76864d4d95df10a5068f8c829ad5d95d3f3f0e`
  (the `fixture_is_the_recorded_snapshot` test re-hashes the file)
- Content: 12,740 rows = 2 labels x 70 NNDSS reporting units (56 states and territories, New York City, 9 regions and 4 totals) x 91 reporting weeks
  (2025 weeks 1-53, 2026 weeks 1-38); columns `states, year, week, label, m3, m3_flag`.

The parser tests assert values measured from these bytes. Do not re-record this file in place:
to add a newer snapshot, store it under a new name and add tests for it.
