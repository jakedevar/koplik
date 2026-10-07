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

