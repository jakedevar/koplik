# koplik-wasm

`make wasm` builds release WASM and installs the exact Cargo.lock wasm-bindgen
CLI version in `target/tools` if needed. The ignored `pkg/web/` has ESM JS, WASM
and TypeScript declarations; `pkg/node/` has Node bindings to the same engine.
Generated bindings are rebuilt on every invocation, so a changed engine cannot
leave stale WASM. Run Cargo via the resource governor, as the Makefile does.

From `web/src/`, the what-if panel can import:

```ts
import init, { runEnsemble, runTrajectory } from '../../pkg/web/koplik_wasm.js';
await init();
const ensemble = JSON.parse(runEnsemble(scenarioJson));
const member = JSON.parse(runTrajectory(scenarioJson, 0));
```

Inputs are **v1 ScenarioInput JSON strings**, validated by contracts and the engine.
Invalid inputs, absent coverage or out-of-range members throw JS Errors. No data
lookups, defaults or missing-value imputation happen here. Pass the raw input
string: `JSON.stringify` on a parsed u64 seed above 2^53 would round it in JS.
Output base seeds are decimal strings; replay the exact `scenario_json` directly.
Do not round-trip that string through JS numbers when retaining full-width seeds.

Outputs are **contract v2**, with committed JSON Schema in
`crates/koplik-contracts/schema/v2`. V2 re-exports unchanged v1 inputs and adds
simulation results. Each output retains input JSON, source provenance, parameters
and seeds. Trajectories contain day zero and every leap in canonical FIPS order.
Ensembles contain daily median and equal-tail 50%/90% predictive bands (type 7),
plus sampled R0, derived seed bytes and full-trajectory hash for each member.
`EnsembleResult.fingerprint` is member 0's trajectory, explicitly not the median.
Recovered includes vaccine immunity; new exposures are not reported cases.

`make determinism` uses the committed synthetic fixture with seed 1353, computes
the trajectory natively and under WASM in Node, independently SHA256-hashes every
compartment at every step as little-endian f64 and prints both hashes. It checks
the engine's golden digest, full output equality, fractional steps, a u64::MAX
seed and JS errors. Any mismatch fails the command. Tests need no source network.

`make wasm-benchmark` times a 1,000-member ensemble on the seven-county, 180-day
fixture: one cold call and five warm calls, including JSON and every member hash.
Module initialization is excluded. The fixture is an artificial flagship-sized
workload, not a calibrated 2025 Gaines County scenario. Population, centroids,
coverage, seeding and coupling are synthetic and must not be displayed as data.
The benchmark reports measured values even if it misses the 3-second target.

For a browser measurement, build with `make wasm`, serve the repo root with
`python3 -m http.server 8000 --bind 127.0.0.1`, then open
`http://127.0.0.1:8000/tools/wasm/benchmark.html` in a foreground tab and click
the button. Copy the JSON with its user agent, timestamp and all six samples.
The browser runs the identical benchmark core through the web bindings. For
headless Chromium in RSI sandboxes, set `TMPDIR=/tmp`. Node timings alone do not
establish that a browser meets the target. See `tools/wasm/BENCHMARK.md` for the
committed measurement and host details.
