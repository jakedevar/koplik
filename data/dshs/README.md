# Texas DSHS outbreak vintage manifest

`vintage-manifest.json` lists every distinct version of the Texas DSHS 2025 West Texas measles
outbreak report held as a snapshot (Internet Archive captures and live fetches), with its
provenance: the input to the E5 backtest's "only data available at each forecast date".
`unmapped.json` lists county names that did not map to a Census FIPS code (empty).

Re-create (needs a snapshot store holding the snapshots; `fetch` commands are in
`data/fixtures/dshs/export-fixtures.sh`):

```
koplik-ingest parse dshs-cases --out /tmp/dshs
cp /tmp/dshs/vintage-manifest.json /tmp/dshs/unmapped.json data/dshs/
```

`koplik-ingest parse dshs-cases` also writes `cumulative.json`, `intervals.json` and `weekly.json`
(contracts v3 `WeeklyCaseCount` rows, `case_definition: confirmed`) to its `--out` directory; those are pipeline outputs, not
committed. How versions are grouped and the series derived: `SOURCES.md`, "Texas DSHS 2025 West
Texas outbreak".
