import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { cp, mkdir, mkdtemp, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import { publishSite } from '../publish.mjs';
import { command, digest } from './data.mjs';
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
async function repository({ pages = false } = {}) {
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
  if (pages) git(shared, 'push', 'origin', 'HEAD:refs/heads/gh-pages');
  git(origin, 'config', 'receive.denyNonFastForwards', 'true');
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
const fakeBuild = async ({ out }) => {
  await mkdir(join(out, 'v6'), { recursive: true });
  const manifest = '{"mode":"fixtures"}\n';
  await writeFile(join(out, 'manifest.json'), manifest);
  for (const name of ['coverage', 'geographies', 'rt', 'texas-counties', 'us-states', 'weekly-cases']) {
    await writeFile(join(out, 'v6', `${name}.json`), '{"fixture":true}\n');
  }
  await writeFile(join(out, 'publication.json'), `${JSON.stringify({ source: 'data/release', pipeline_manifest_sha256: digest(manifest) })}\n`);
};

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

test('temporary-clone refresh prepares exact Pages objects and releases all refs or none', async () => {
  const scenarios = ['green', 'publish-green', 'qa-red', 'path-red', 'publish-red',
    'atomic-reject-rolling', 'atomic-reject-main', 'atomic-reject-gh-pages'];
  for (const scenario of scenarios) {
    const context = await repository({ pages: scenario !== 'green' });
    const originalRefs = git(context.origin, 'for-each-ref', '--format=%(refname) %(objectname)');
    if (scenario.startsWith('atomic-reject-')) {
      const ref = scenario.slice('atomic-reject-'.length);
      await writeFile(join(context.origin, 'hooks/update'), `#!/bin/sh
if [ "$1" = "refs/heads/${ref}" ]; then exit 1; fi
exit 0
`, { mode: 0o755 });
    }
    const effects = [];
    const run = async (program, args, cwd, env) => {
      if (program.endsWith('/tools/offline-test.sh')) {
        assert.deepEqual(args, ['make', 'publish']);
        program = 'make'; args = ['publish'];
      }
      if (program === 'make') {
        effects.push({ args, dry: env.PUBLISH_DRY_RUN, cwd });
        assert.equal(args[0], 'publish');
        assert.equal(env.PUBLISH_DRY_RUN, '1');
        assert.equal(env.PUBLISH_REMOTE, 'origin');
        if (scenario === 'publish-red') throw new Error('Build failed before push');
        const scratch = join(cwd, 'target', 'publisher');
        await mkdir(scratch, { recursive: true });
        // Fake build/QA, real Git commit preparation, object import and atomic push.
        const dist = join(scratch, 'site');
        await cp(join(cwd, 'target/refresh-out-3'), join(dist, 'data'), { recursive: true });
        await publishSite({ scratch, dist,
          target: context.origin, source: git(cwd, 'rev-parse', 'HEAD'),
          identity: ['', 'Refresh test', 'refresh-test@example.invalid'], env, dryRun: '1',
          prepare: { root: cwd, output: env.PUBLISH_PREPARE_OUTPUT, parent: env.PUBLISH_EXPECTED_PARENT } });
        await rm(scratch, { recursive: true, force: true });
        return '';
      }
      if (program === 'git' && args[0] === 'push') {
        assert.equal(env.KOPLIK_PROMOTE_MAIN, '1');
        assert.equal(args[1], '--atomic');
        assert.equal(args.length, 6);
        effects.push({ push: args });
      }
      if (program === 'notify-send') return '';
      return command(program, args, cwd, env);
    };
    let observedWork;
    try {
      const options = { ...context, run, dryRun: scenario === 'green', build: fakeBuild,
        ingest: async (args) => {
          observedWork = args.work;
          await fixtureIngest(args);
          if (scenario === 'path-red') await writeFile(join(args.work, 'code.txt'), 'Changed\n');
        },
        qa: async ({ work, gates }) => {
          assert.deepEqual(gates, ['check', 'test', 'web-test', 'determinism']);
          assert.equal(git(work, 'status', '--porcelain'), '', 'QA runs on committed D');
          assert.notEqual(git(work, 'rev-parse', 'HEAD'), context.base);
          if (scenario === 'qa-red') throw new Error('QA failure');
        },
      };
      if (['green', 'publish-green'].includes(scenario)) {
        const result = await refresh(options);
        assert.equal(result.receipt.scans.personal_matches, 0);
        assert.equal(result.receipt.scans.secret_matches, 0);
        assert.equal(git(result.work, 'status', '--porcelain'), '');
        const paths = git(result.work, 'diff', '--name-only', context.base, result.sha).split('\n');
        assert.ok(paths.every((path) => path.startsWith('data/release/')));
        assert.equal(effects.length, scenario === 'publish-green' ? 2 : 1);
        assert.equal(effects[0].dry, '1');
        assert.equal(git(result.work, 'rev-list', '--parents', '-n', '1', result.pages),
          scenario === 'green' ? result.pages : `${result.pages} ${context.base}`);
        assert.ok(git(result.work, 'ls-tree', '-r', '--name-only', result.pages).includes('data/v6/weekly-cases.json'));
        if (scenario === 'publish-green') {
          assert.equal(git(context.origin, 'rev-parse', 'refs/heads/rolling'), result.sha);
          assert.equal(git(context.origin, 'rev-parse', 'refs/heads/main'), result.sha);
          assert.equal(git(context.origin, 'rev-parse', 'refs/heads/gh-pages'), result.pages);
        }
      } else {
        const phase = scenario === 'qa-red' ? 'qa' : scenario === 'path-red' ? 'integrity'
          : scenario === 'publish-red' ? 'publish-prepare' : 'atomic-release';
        await assert.rejects(refresh(options), new RegExp(`failed at ${phase}`));
        assert.equal(effects.length, scenario.startsWith('atomic-reject-') ? 2 : scenario === 'publish-red' ? 1 : 0);
        const failures = (await readdir(context.state)).filter((path) => path.startsWith('FAILED-'));
        assert.equal(failures.length, 1);
        assert.ok((await readFile(join(context.state, failures[0]), 'utf8')).includes(phase));
      }
      assert.ok(observedWork.startsWith(context.state));
      if (scenario !== 'publish-green') {
        assert.equal(git(context.origin, 'for-each-ref', '--format=%(refname) %(objectname)'), originalRefs,
          'all refs preserved on dry-run or any rejected transaction');
      }
      console.log(`Offline refresh scenario ${scenario}: expected decision and all local remote refs verified`);
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
