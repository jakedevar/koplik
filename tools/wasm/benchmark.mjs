import { readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { createRequire } from 'node:module';
import os from 'node:os';
import { benchmark } from './benchmark-core.mjs';

const require = createRequire(import.meta.url);
const { runEnsemble } = require('../../pkg/node/koplik_wasm.js');
const fixture = readFileSync(new URL('../../data/fixtures/seir/synthetic-scenario.json', import.meta.url), 'utf8');
const result = benchmark(runEnsemble, fixture);
console.log(JSON.stringify({
  measured_at: new Date().toISOString(),
  runtime: process.version,
  v8: process.versions.v8,
  platform: `${os.platform()} ${os.release()} ${os.arch()}`,
  cpu: os.cpus()[0]?.model,
  fixture_sha256: createHash('sha256').update(fixture).digest('hex'),
  wasm_sha256: createHash('sha256').update(readFileSync(new URL('../../pkg/node/koplik_wasm_bg.wasm', import.meta.url))).digest('hex'),
  ...result,
}, null, 2));
