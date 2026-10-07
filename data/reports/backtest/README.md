# Forecast backtest reports

Each JSON is the full output of `crates/koplik-epi/examples/backtest_west_texas.rs`: the
truth series, every forecast date with its origin week, status and `R` posterior, every
scored target (quantiles, CRPS, 50%/90% interval coverage, persistence baseline), and the
per-horizon and pooled summaries, exactly as measured. The pre-registered protocol and
parameters are in `koplik_epi::forecast` and `koplik_epi::backtest::run`; the narrative
report is `thoughts/shared/research/backtest-2025-west-texas.md`.

- `west-texas-2025.json`: Texas DSHS 2025 West Texas outbreak total, by report vintage.
  `manifest_sha256` is the SHA-256 of the `data/dshs/vintage-manifest.json` it was run on.

Reproduce (offline; the manifest is committed by `koplik-ingest`, see `data/dshs/README.md`):

```bash
~/.rsi/bin/cargo-slot cargo run -p koplik-epi --example backtest_west_texas -- \
  data/dshs/vintage-manifest.json data/reports/backtest/west-texas-2025.json
```

The run is deterministic (fixed seed, portable RNG, `libm` transcendentals): the same
manifest gives a byte-identical JSON.

The pipeline's `forecast` stage (`koplik-pipeline forecast`, `--reports data/reports`) reads
`west-texas-2025.json` and attaches its pooled and per-horizon scores to the published forecast's
provenance companion (contract v5) only when the report was run with exactly the configuration the
pipeline forecasts with (window, look-back, minimum cases, horizon, members, seed). It also copies the
report unchanged to `forecasts/backtest-west-texas-2025.json` so the numbers on the page can be checked
against it. Re-running the backtest with a different configuration therefore detaches the skill rather
than mislabelling it.


## `cdc-states.json` (#1503)

The same method scored on the CDC NNDSS state series the site publishes
(`confirmed_or_unknown_status`, by CDC report week), written by
`crates/koplik-pipeline/examples/backtest_cdc_states.rs`. It is **pseudo-real-time (revised counts
truncated at each forecast date)**, not real-time: one retrieval of the source is held and CDC
publishes no revision history. It records the snapshot it was run on (`input.sha256`, retrieval
time and URL), the published configuration, the pre-registered floors for a "measured" skill, and
for each of the 56 geographies the truth series, every origin at which a forecast was made (with
every scored target) and why the other origins made none. It is compact JSON; read it with `jq`.
Protocol, amendments and results: `thoughts/shared/research/backtest-cdc-states.md`.

```bash
~/.rsi/bin/cargo-slot cargo run --release -p koplik-pipeline --example backtest_cdc_states -- \
  data/fixtures/cdc/nndss-measles-weekly.retrieval.json data/reports/backtest/cdc-states.json
```

Deterministic: the same snapshot gives the same bytes. The forecast stage reads it and attaches each
state series' measured skill, or its absence, to the companion (contract v7).
