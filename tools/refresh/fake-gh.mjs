import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';

// Executable test double on PATH. No socket or GitHub client is ever opened.
export async function fakeGh(root, env) {
  const bin = join(root, 'fake-bin');
  await mkdir(bin);
  await writeFile(join(bin, 'gh'), `#!/usr/bin/env node
const fs = require('node:fs');
const { execFileSync } = require('node:child_process');
const args = process.argv.slice(2);
fs.appendFileSync(process.env.FAKE_GH_LOG, JSON.stringify(args)+'\\n');
if (args[0] !== 'api' || args[1] !== '--hostname' || args[2] !== 'github.com'
  || args[3] !== '--method' || !/^repos\\/example\\/koplik\\/pages\\/builds(?:\\/latest)?$/.test(args[5])) process.exit(2);
if (process.env.FAKE_GH_MODE === 'error' || (process.env.FAKE_GH_MODE === 'get-error' && args[4] === 'GET')) process.exit(1);
const commit = process.env.FAKE_GH_COMMIT || execFileSync('git', ['rev-parse', 'refs/heads/gh-pages'], { cwd: process.env.FAKE_GH_ORIGIN, encoding: 'utf8' }).trim();
if (args[4] === 'POST') console.log(JSON.stringify({ status: 'queued', commit }));
else {
  const count = fs.readFileSync(process.env.FAKE_GH_LOG, 'utf8').trim().split('\\n').filter(line => JSON.parse(line)[4] === 'GET').length;
  const mode = process.env.FAKE_GH_MODE;
  if (mode === 'malformed') console.log('invalid json');
  else console.log(JSON.stringify({ id: 123, commit: mode === 'wrong-built' || (mode !== 'instant' && count === 1) ? '0'.repeat(40) : commit,
    status: mode === 'errored' ? 'errored' : mode === 'never' || (mode !== 'instant' && count === 2) ? 'building' : 'built' }));
}
`, { mode: 0o755 });
  return { ...env, PATH: `${bin}:${env.PATH}`, FAKE_GH_LOG: join(root, 'gh-calls.jsonl') };
}
