# Coverage gaps from the pinned source fixtures

Each JSON is an array of v1 `KindergartenMmrCoverage` rows whose coverage is missing.
Nothing is zero-filled or imputed. The records retain snapshot URL, retrieval time, SHA-256
and missing reason. See SOURCES.md for the baseline choice and source terms.

- CDC 2023–24/2024–25: 3 gaps (Montana both years; West Virginia 2024–25).
- Texas 2023–24 baseline: 4 gaps (Crane, Loving, Real, Stonewall).
- Texas 2024–25: 1 gap (Loving).

These reports were produced by the offline CLI from the corresponding fixture snapshots:

```bash
~/.rsi/bin/cargo-slot cargo run -p koplik-ingest --offline -- parse cdc-coverage --out /tmp/cdc-coverage.json --gaps data/reports/coverage/cdc-2023-25-gaps.json
~/.rsi/bin/cargo-slot cargo run -p koplik-ingest --offline -- parse texas-coverage --year 2023 --out /tmp/texas-2023-24.json --gaps data/reports/coverage/texas-2023-24-gaps.json
~/.rsi/bin/cargo-slot cargo run -p koplik-ingest --offline -- parse texas-coverage --year 2024 --out /tmp/texas-2024-25.json --gaps data/reports/coverage/texas-2024-25-gaps.json
```

The store must contain the pinned retrievals for those commands to reproduce these reports.
`tests/coverage_parser.rs` seeds a temporary store from committed fixtures without network
access, parses both connectors, and compares these reports to the regenerated missing rows.
