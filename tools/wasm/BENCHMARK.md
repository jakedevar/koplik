# WASM ensemble benchmark, 2026-10-07 UTC

Measured on AMD Ryzen 9 9950X3D 16-Core Processor, `linux 7.2.9-zen1-1-zen x64`, Rust 1.94.1,
release WASM, wasm-bindgen 0.2.129. Node `v26.10.0` (V8 `14.6.202.34-node.34`)
and headless `Chrome/155.0.8059.39` used the identical WASM bytes.

This is the committed **synthetic flagship-sized** workload: 1,000 runs,
7 Texas county keys including Gaines, 180 days, dt=1 day, seed=1353,
with gravity coupling. Invented population, centroid, coverage and seed-case
inputs are explicitly identified in `data/fixtures/seir/synthetic-source.json`.
This is not a calibrated 2025 Gaines County outbreak scenario.

| Runtime | Cold call (ms) | Median of five warm calls (ms) |
| --- | ---: | ---: |
| Node | 496.598168 | 474.4075270000001 |
| Headless Chrome | 494.69999999925494 | 472.5 |

Values above are unrounded clock outputs. All samples, their order, timestamps
and returned fingerprints are committed verbatim in
[Node measurement](measurements/node-2026-10-07.json) and
[Chrome measurement](measurements/chrome-2026-10-07.json).
Final recorded Node and Chrome runs were sequential after initial smoke probes;
this shared development host was not reserved or CPU-isolated.

Each timed call includes input JSON parsing, the ensemble, daily summaries,
full-trajectory hashes for all members, output JSON serialization and JS parsing.
Module loading/initialization and JSON size measurement are outside the timer.
Output is 1016604 bytes. No render time is measured.
Cold and all warm calls meet the 3,000 ms target on this host. This does not
establish a performance bound for other devices or a future observed-data scenario.

- Fixture sha256: `a8c3e92eebc9a7fa9fff5af3c5603e287be72e1237152a8dcd3f6e36b7f13a21`
- WASM sha256 (web and Node): `e26afc833c84562e9adcf0bbb3c317dd8ef774abe366cd96048123a02bba35f0`
- Member 0 full-trajectory fingerprint: `7a7471b1ed6d648d9a376d591ed21be513b90128d5f5e7c759c184689d5c25fb`

Reproduce Node with `make wasm-benchmark`. The shared measurement code is
`benchmark-core.mjs`; it always measures one cold call then five warm calls.
For browser reproduction, follow the local HTTP-server method in
[the facade README](../../crates/koplik-wasm/README.md) and run `benchmark.html`.
The recorded Chrome probe automated that same page and button with puppeteer-core,
`/opt/google/chrome/chrome`, `headless: true`, `TMPDIR=/tmp`,
`--no-sandbox` and `--disable-dev-shm-usage`; it used the real performance clock
(no virtual-time budget or CPU throttling). The JSON retains the exact browser
version and user agent. No browser packages are required by the product.

Verification: `make check`,
`~/.rsi/bin/cargo-slot cargo test --offline --locked --workspace`,
and `make determinism` passed. The determinism gate printed the golden hash
above for both native and WASM, and also matched fractional dt=0.3 trajectories
with seed u64::MAX at
`522a6dbb1b5abc4121565799ea2d7532c4aa8378ea8eccbe9039c02f2a12f49c`.
