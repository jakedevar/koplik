import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const gate = fileURLToPath(new URL('./offline-test.sh', import.meta.url));

test('unavailable isolation refuses execution unless explicitly opted out', async () => {
  const scratch = await mkdtemp(join(tmpdir(), 'koplik-offline-gate-'));
  try {
    await writeFile(join(scratch, 'unshare'), '#!/bin/sh\necho "namespace disabled for gate test" >&2\nexit 1\n', { mode: 0o755 });
    const args = [gate, process.execPath, '-e', 'console.log("command executed")'];
    const env = { ...process.env, PATH: `${scratch}:${process.env.PATH}`, KOPLIK_ALLOW_NETWORK_TESTS: '' };
    const refused = spawnSync('bash', args, { env, encoding: 'utf8' });
    assert.ifError(refused.error);
    assert.equal(refused.status, 1);
    assert.equal(refused.stdout, '');
    assert.match(refused.stderr, /namespace disabled for gate test/);
    assert.match(refused.stderr, /Refusing to run tests/);

    const optedOut = spawnSync('bash', args, {
      env: { ...env, KOPLIK_ALLOW_NETWORK_TESTS: '1' }, encoding: 'utf8',
    });
    assert.ifError(optedOut.error);
    assert.equal(optedOut.status, 0);
    assert.equal(optedOut.stdout, 'command executed\n');
    assert.match(optedOut.stderr, /WITH NETWORK ACCESS/);
  } finally {
    await rm(scratch, { recursive: true, force: true });
  }
});

test('isolated command failures keep their exit status even with opt-out enabled', () => {
  const result = spawnSync('bash', [gate, process.execPath, '-e', 'process.exit(47)'], {
    env: { ...process.env, KOPLIK_ALLOW_NETWORK_TESTS: '1' }, encoding: 'utf8',
  });
  assert.ifError(result.error);
  assert.equal(result.status, 47);
  assert.match(result.stdout, /192\.0\.2\.1:443 -> ENETUNREACH/);
});

test('isolated command receives arguments unchanged', () => {
  const args = ['space in argument', '$(literal)', 'semi;colon'];
  const result = spawnSync('bash', [gate, process.execPath, '-e',
    `require('node:assert/strict').deepEqual(process.argv.slice(1), ${JSON.stringify(args)})`,
    ...args], { encoding: 'utf8' });
  assert.ifError(result.error);
  assert.equal(result.status, 0, result.stderr);
});
