# Issue #1422 published payload measurements

Baseline: cbb9f14; offline committed fixtures via `make pipeline-fixtures`.
Gzip uses Node zlib level 9 on exact published bytes, including the manifest.
These are reproducible local compression sizes, not a live HTTP measurement.

| Artifact | Before raw bytes | Before gzip bytes |
| --- | ---: | ---: |
| forecasts/backtest-west-texas-2025.json | 189272 | 8259 |
| forecasts/weekly-cases.json | 61422 | 3868 |
| forecasts/weekly-cases.provenance.json | 25908 | 4267 |
| manifest.json | 32356 | 6143 |
| scenarios/gaines-2025.json | 1899 | 955 |
| scenarios/gaines-2025.provenance.json | 4997 | 2190 |
| v1/coverage.json | 498959 | 10295 |
| v1/geographies.json | 220390 | 8624 |
| v1/rt.json | 14302232 | 113569 |
| v1/texas-counties.json | 169613 | 23419 |
| v1/us-states.json | 233074 | 73431 |
| v1/weekly-cases.json | 5193035 | 55543 |
| Total | 20933157 | 310563 |
