import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { cp, mkdir, mkdtemp, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import { command } from './data.mjs';
import { failure, ingestGreen, pathGuard, refresh } from './refresh.mjs';
import { scanHistory, scanText } from './scans.mjs';

const project = fileURLToPath(new URL('../../', import.meta.url));
const dummyEnv = { ...process.env };
delete dummyEnv.RSI_SESSION_TOKEN;
for (const key of Object.keys(dummyEnv)) if (key.startsWith('GIT_')) delete dummyEnv[key];
function git(cwd, ...args) {
  const result = spawnSync('git', args, { cwd, env: dummyEnv, encoding: 'utf8' });
  assert.equal(result.status, 0, result.stderr);
  return result.stdout.trim();
}
async function repository() {
  const root = await mkdtemp('/tmp/koplik-refresh-test-');
  const shared = join(root, 'shared');
  await mkdir(shared);
  git(shared, 'init', '--initial-branch=rolling');
  git(shared, 'config', 'user.name', 'Refresh test');
  git(shared, 'config', 'user.email', 'refresh-test@example.invalid');
  await cp(join(project, 'data/release'), join(shared, 'data/release'), { recursive: true });
  await writeFile(join(shared, '.gitignore'), 'target/\ndata/pipeline/\n.env*\n');
  await writeFile(join(shared, 'code.txt'), 'Code unchanged\n');
  git(shared, 'add', '--', 'data/release', '.gitignore', 'code.txt');
  git(shared, 'commit', '-m', 'Refresh test source');
  const base = git(shared, 'rev-parse', 'HEAD');
  const origin = join(root, 'origin.git');
  git(shared, 'init', '--bare', origin);
  git(shared, 'remote', 'add', 'origin', origin);
  git(shared, 'push', 'origin', 'HEAD:refs/heads/rolling', 'HEAD:refs/heads/main');
  await writeFile(join(shared, '.env.local'), 'KOPLIK_CENSUS_CONTACT=refresh-test@example.invalid\n');
  const patterns = join(root, 'patterns');
  await writeFile(patterns, 'private-contact-sentinel\n');
  return { root, shared, origin, base, patterns, state: join(root, 'state'), env: dummyEnv };
}
async function fixtureIngest({ release, stageWork, env }) {
  assert.equal(env.KOPLIK_CENSUS_CONTACT, undefined);
  assert.ok(env.KOPLIK_ENV_LOCAL.endsWith('/shared/.env.local'));
  await mkdir(stageWork, { recursive: true });
  const manifest = JSON.parse(await readFile(join(release, 'ingest.manifest.json'), 'utf8'));
  // The fixture seed has a deliberately unavailable direct county-code source;
  // the fake completed live ingest reports only the actual present captures.
  for (const [id, item] of Object.entries(manifest.items)) if (item.status === 'missing') delete manifest.items[id];
  await writeFile(join(stageWork, 'ingest.manifest.json'), `${JSON.stringify(manifest)}\n`);
}
const fakeBuild = async ({ out }) => { await mkdir(out, { recursive: true }); await writeFile(join(out, 'manifest.json'), 'Committed fixture output\n'); };

test('path guard covers unstaged, staged, untracked, deletion and rename; accepts only release changes', async () => {
  const context = await repository();
  try {
    const root = context.shared;
    await writeFile(join(root, 'data/release/qa.json'), 'unexpected');
    await assert.rejects(pathGuard(root), /Unexpected release path/);
    await rm(join(root, 'data/release/qa.json'));
    await writeFile(join(root, 'code.txt'), 'Changed\n');
    await assert.rejects(pathGuard(root), /path guard/);
    git(root, 'add', '--', 'code.txt');
    await assert.rejects(pathGuard(root), /path guard/);
    await rm(join(root, 'code.txt'));
    await assert.rejects(pathGuard(root), /path guard/);
    await writeFile(join(root, 'code.txt'), 'Changed\n');
    git(root, 'commit', '-m', 'Test guard setup');
    await writeFile(join(root, 'untracked.txt'), 'Untracked\n');
    await assert.rejects(pathGuard(root), /path guard/);
    await rm(join(root, 'untracked.txt'));
    git(root, 'mv', 'code.txt', 'data/release/renamed.txt');
    await assert.rejects(pathGuard(root), /path guard/);
  } finally { await rm(context.root, { recursive: true, force: true }); }
});

test('green/red ingest decision refuses missing, partial and failed captures', () => {
  const previous = { items: { source: { status: 'present' } } };
  const green = { items: { source: { status: 'present' } }, notes: [] };
  assert.doesNotThrow(() => ingestGreen(previous, green));
  for (const red of [
    { items: {}, notes: [] },
    { items: { source: { status: 'missing' } }, notes: [] },
    { items: { source: { status: 'present', gaps: 1 } }, notes: [] },
    { ...green, notes: ['capture: fetch failed'] },
  ]) assert.throws(() => ingestGreen(previous, red), /Ingest incomplete/);
});

test('failure RPC has no token in params; RPC refusal and no-token runs persist fallback and notify', async () => {
  const state = await mkdtemp(join(tmpdir(), 'koplik-refresh-failure-'));
  const calls = [];
  const run = async (program, args) => {
    calls.push([program, args]);
    if (program === 'rsi-rpc') {
      const params = JSON.parse(await readFile(args[2].slice(1), 'utf8'));
      assert.equal(params.labels[0], 'refresh-failure');
      assert.equal(Object.hasOwn(params, 'RSI_SESSION_TOKEN'), false);
      throw new Error('Test refusal');
    }
  };
  try {
    await failure({ state, runId: 'test-rpc', phase: 'qa', run, env: { RSI_SESSION_TOKEN: 'transport-only-example' } });
    await failure({ state, runId: 'test-local', phase: 'integrity', run, env: {} });
    assert.deepEqual((await readdir(state)).sort(), ['FAILED-test-local.md', 'FAILED-test-rpc.md']);
    assert.equal(calls.filter(([program]) => program === 'notify-send').length, 2);
  } finally { await rm(state, { recursive: true, force: true }); }
});

test('full refresh in temporary clone: green, QA red and path-guard red; no live commands or real origin', async () => {
  for (const scenario of ['green', 'qa-red', 'path-red']) {
    const context = await repository();
    const effects = [];
    const run = async (program, args, cwd, env) => {
      if (program === 'make') {
        effects.push({ args, dry: env.PUBLISH_DRY_RUN, cwd });
        assert.equal(args[0], 'publish');
        if (env.PUBLISH_DRY_RUN === '1') assert.ok(env.PUBLISH_REMOTE.startsWith(context.state));
        return '';
      }
      if (program === 'notify-send') return '';
      return command(program, args, cwd, env);
    };
    let observedWork;
    try {
      const options = { ...context, run, dryRun: true, build: fakeBuild,
        scan: async (...args) => { try { return await scanHistory(...args); } catch (error) { console.log(error.message); throw error; } },
        ingest: async (args) => {
          observedWork = args.work;
          await fixtureIngest(args);
          if (scenario === 'path-red') await writeFile(join(args.work, 'code.txt'), 'Changed\n');
        },
        qa: async ({ gates }) => {
          assert.deepEqual(gates, ['check', 'test', 'web-test', 'determinism']);
          if (scenario === 'qa-red') throw new Error('QA failure');
        },
      };
      if (scenario === 'green') {
        const result = await refresh(options);
        assert.equal(result.receipt.scans.personal_matches, 0);
        assert.equal(result.receipt.scans.secret_matches, 0);
        assert.equal(git(result.work, 'status', '--porcelain'), '');
        const paths = git(result.work, 'diff', '--name-only', context.base, result.sha).split('\n');
        assert.ok(paths.every((path) => path.startsWith('data/release/')));
        assert.equal(effects.length, 1);
        assert.equal(effects[0].dry, '1');
      } else {
        await assert.rejects(refresh(options), scenario === 'qa-red' ? /failed at qa/ : /failed at integrity/);
        assert.equal(effects.length, 0);
        const failures = (await readdir(context.state)).filter((path) => path.startsWith('FAILED-'));
        assert.equal(failures.length, 1);
      }
      assert.ok(observedWork.startsWith(context.state));
      assert.equal(git(context.origin, 'rev-parse', 'refs/heads/rolling'), context.base);
      assert.equal(git(context.origin, 'rev-parse', 'refs/heads/main'), context.base);
      assert.equal(git(context.origin, 'for-each-ref', '--format=%(refname)').split('\n').length, 2);
      console.log(`Offline refresh scenario ${scenario}: expected decision, remote refs unchanged`);
    } finally { await rm(context.root, { recursive: true, force: true }); }
  }
});

test('scans refuse private patterns, secrets, tracked environment files and secrets removed from current tree', async () => {
  assert.throws(() => scanText('private-contact-sentinel', ['private-contact-sentinel']), /Personal-data/);
  assert.throws(() => scanText('ghp_' + 'a'.repeat(36), []), /Secrets/);
  const context = await repository();
  try {
    await writeFile(join(context.shared, 'secret.txt'), 'sk-' + 'a'.repeat(48));
    git(context.shared, 'add', '--', 'secret.txt');
    git(context.shared, 'commit', '-m', 'Historical secret test');
    git(context.shared, 'rm', 'secret.txt');
    git(context.shared, 'commit', '-m', 'Remove secret test');
    await assert.rejects(scanHistory(context.shared, context.patterns), /Secrets/);
    await writeFile(join(context.shared, '.env.test'), 'dummy=example.invalid\n');
    git(context.shared, 'add', '-f', '--', '.env.test');
    await assert.rejects(scanHistory(context.shared, context.patterns), /Tracked environment/);
  } finally { await rm(context.root, { recursive: true, force: true }); }
});
