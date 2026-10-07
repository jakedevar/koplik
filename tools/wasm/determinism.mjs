import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const require = createRequire(import.meta.url);
const { runTrajectory, runEnsemble } = require('../../pkg/node/koplik_wasm.js');
const fixture = readFileSync(new URL('../../data/fixtures/seir/synthetic-scenario.json', import.meta.url), 'utf8');
// Honour CARGO_TARGET_DIR (exported by the Makefile) so a machine-wide cargo target-dir cannot hide the binary.
const targetDir = process.env.CARGO_TARGET_DIR || fileURLToPath(new URL('../../target', import.meta.url));
const nativeBin = `${targetDir}/release/trajectory-native`;
const golden = '7a7471b1ed6d648d9a376d591ed21be513b90128d5f5e7c759c184689d5c25fb';

// Independently encode every count, including initial state and fractional leaps.
function hashTrajectory(result) {
  const hash = createHash('sha256');
  let counts = 0;
  for (const step of result.steps) for (const node of step.nodes) {
    for (const field of ['susceptible', 'exposed', 'infectious', 'recovered']) {
      assert(Number.isInteger(node[field]) && node[field] >= 0 && node[field] <= 2 ** 53);
      const bytes = Buffer.alloc(8);
      bytes.writeDoubleLE(node[field]);
      hash.update(bytes);
      counts++;
    }
  }
  assert(counts > 0);
  const fingerprint = hash.digest('hex');
  assert.equal(fingerprint, result.member.fingerprint, 'exported hash must match ALL compartment bytes');
  return fingerprint;
}

function compare(input, label, expected) {
  const process = spawnSync(nativeBin, ['-'], { input, encoding: 'utf8', maxBuffer: 32 * 1024 * 1024 });
  if (process.error || process.status !== 0) throw new Error(`native failed: ${process.error || process.stderr}`);
  const native = JSON.parse(process.stdout);
  const wasm = JSON.parse(runTrajectory(input, 0));
  const nativeHash = hashTrajectory(native);
  const wasmHash = hashTrajectory(wasm);
  console.log(`${label} native sha256: ${nativeHash}`);
  console.log(`${label} wasm32 sha256: ${wasmHash}`);
  assert.equal(wasmHash, nativeHash, 'DETERMINISM MISMATCH: native vs wasm32 full trajectory');
  if (expected) assert.equal(wasmHash, expected, 'fixture golden fingerprint changed');
  assert.deepEqual(wasm, native, 'trajectory metadata and steps must also match');
}

compare(fixture, 'fixture seed=1353', golden);
const fractional = JSON.parse(fixture);
fractional.parameters.time_step_days = 0.3;
fractional.parameters.horizon_days = 8;
fractional.run_count = 3;
const largeSeed = JSON.stringify(fractional).replace(/"seed":1353/, '"seed":18446744073709551615');
compare(largeSeed, 'fractional dt=0.3 seed=u64::MAX');
assert.equal(JSON.parse(runTrajectory(largeSeed, 0)).seed, '18446744073709551615');
const ensemble = JSON.parse(runEnsemble(largeSeed));
assert.equal(ensemble.members.length, 3);
assert.equal(ensemble.daily.length, 9 * fractional.nodes.length);
assert.equal(ensemble.fingerprint, ensemble.members[0].fingerprint);
assert.equal(ensemble.scenario_json, largeSeed);
for (const member of ensemble.members) {
  const result = JSON.parse(runTrajectory(largeSeed, member.member));
  assert.deepEqual(member, result.member);
}
assert.throws(() => runEnsemble('{}'), /nodes|missing field/);
assert.throws(() => runTrajectory(fixture, 1000), /member/);
fractional.coverage_overrides = [];
assert.throws(() => runEnsemble(JSON.stringify(fractional)), /missing baseline coverage/);
console.log('PASS: offline native/wasm32 determinism and JS facade validation');
