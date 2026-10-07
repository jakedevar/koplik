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

After v6: same fixtures; four observation envelopes and forecast rows.
Packing validates current-contract JSON and preserves non-provenance fields as raw
JSON to avoid changing decimal values during parsing and reserialization.
GeoJSON boundaries and scenario/forecast companions keep their existing shapes.

| Artifact | After raw bytes | After gzip bytes |
| --- | ---: | ---: |
| forecasts/backtest-west-texas-2025.json | 189272 | 8259 |
| forecasts/weekly-cases.json | 39849 | 3617 |
| forecasts/weekly-cases.provenance.json | 25908 | 4267 |
| manifest.json | 32354 | 6376 |
| scenarios/gaines-2025.json | 1899 | 955 |
| scenarios/gaines-2025.provenance.json | 4997 | 2190 |
| v6/coverage.json | 116496 | 7459 |
| v6/geographies.json | 41886 | 7392 |
| v6/rt.json | 2453176 | 50903 |
| v6/texas-counties.json | 169613 | 23419 |
| v6/us-states.json | 233074 | 73431 |
| v6/weekly-cases.json | 1065557 | 23797 |
| Total | 4374081 | 212065 |

Two final v6 `make pipeline-fixtures` runs produced identical SHA-256 hashes for all 12 published files.
Total raw reduction: 79.1%; gzip reduction: 31.7%. No observations or provenance were removed.
