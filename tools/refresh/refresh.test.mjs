import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { access, cp, mkdir, mkdtemp, readFile, readdir, rm, utimes, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import { publishSite } from '../publish.mjs';
import { command, digest } from './data.mjs';
import { seedFixtureStore } from './fixture-store.mjs';
import { failure, ingestGreen, pathGuard, refresh } from './refresh.mjs';
import { loadPatterns, redact, scanHistory, scanText } from './scans.mjs';
import { preflight, pruneRuns } from './safety.mjs';
import { bootstrap } from './bootstrap.mjs';
import { fakeGh } from './fake-gh.mjs';

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
  await seedFixtureStore({ store: join(shared, 'data/release'), work: join(root, 'ingest') });
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
  git(origin, 'remote', 'add', 'github', 'https://github.com/example/koplik.git');
  await writeFile(join(shared, '.env.local'), 'KOPLIK_CENSUS_CONTACT=refresh-test@example.invalid\n');
  const patterns = join(root, 'patterns');
  await writeFile(patterns, 'private-contact-sentinel\n');
  return { root, shared, origin, base, patterns, state: join(root, 'state'), env: { ...await fakeGh(root, dummyEnv), FAKE_GH_ORIGIN: origin } };
}
async function fixtureIngest({ release, stageWork, env }, mode = 'fixtures') {
  assert.equal(env.KOPLIK_CENSUS_CONTACT, undefined);
  assert.ok(env.KOPLIK_ENV_LOCAL.endsWith('/shared/.env.local'));
  await seedFixtureStore({ store: release, work: stageWork, mode });
  const manifest = JSON.parse(await readFile(join(stageWork, 'ingest.manifest.json'), 'utf8'));
  // The fixture seed has a deliberately unavailable direct county-code source;
  // the fake completed live ingest reports only the actual present captures.
  for (const [id, item] of Object.entries(manifest.items)) if (item.status === 'missing') delete manifest.items[id];
  await writeFile(join(stageWork, 'ingest.manifest.json'), `${JSON.stringify(manifest)}\n`);
}
const fakeBuild = async ({ root, out }) => {
  await mkdir(join(out, 'v6'), { recursive: true });
  const mode = JSON.parse(await readFile(join(root, 'data/release/ingest.manifest.json'))).mode;
  const manifest = `${JSON.stringify({ mode })}\n`;
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
    'scan-red-crlf', 'secrets-bundle-red', 'pages-never', 'pages-error', 'pages-get-error', 'pages-errored', 'pages-wrong-built', 'atomic-reject-rolling', 'atomic-reject-main', 'atomic-reject-gh-pages', 'live-green', 'live-qa-red', 'live-atomic-reject-gh-pages'];
  for (const scenario of scenarios) {
    const decision = scenario.replace(/^live-/, '');
    const mode = scenario.startsWith('live-') ? 'live' : 'fixtures';
    const context = await repository({ pages: decision !== 'green' });
    const originalRefs = git(context.origin, 'for-each-ref', '--format=%(refname) %(objectname)');
    if (decision.startsWith('atomic-reject-')) {
      const ref = decision.slice('atomic-reject-'.length);
      await writeFile(join(context.origin, 'hooks/update'), `#!/bin/sh
if [ "$1" = "refs/heads/${ref}" ]; then exit 1; fi
exit 0
`, { mode: 0o755 });
    }
    if (scenario === 'scan-red-crlf') await writeFile(context.patterns, '  private-contact-sentinel  \r\n');
    const effects = [];
    const notices = [];
    let time = 0;
    const run = async (program, args, cwd, env, commandOptions) => {
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
        if (scenario === 'scan-red-crlf') await writeFile(join(dist, 'app.js'), 'private-contact-sentinel');
        if (scenario === 'secrets-bundle-red') await writeFile(join(dist, 'app.js'), 'ghp_' + 'a'.repeat(36));
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
      if (program === 'notify-send') { notices.push(args); return ''; }
      return command(program, args, cwd, env, commandOptions);
    };
    let observedWork;
    try {
      const options = { ...context, run, dryRun: decision === 'green', build: fakeBuild,
        env: { ...context.env, FAKE_GH_MODE: scenario.startsWith('pages-') ? scenario.slice(6) : 'green' },
        pagesOptions: { timeoutMs: 3000, pollMs: 1000, now: () => time, sleep: async (ms) => { time += ms; } },
        ingest: async (args) => {
          observedWork = args.work;
          await fixtureIngest(args, mode);
          if (scenario === 'path-red') await writeFile(join(args.work, 'code.txt'), 'Changed\n');
        },
        qa: async ({ work, gates, env }) => {
          assert.equal(Object.hasOwn(env, 'KOPLIK_ENV_LOCAL'), false, 'ingest config must not override QA fixtures');
          assert.deepEqual(gates, ['check', 'test', 'web-test', 'determinism']);
          assert.equal(git(work, 'status', '--porcelain'), '', 'QA runs on committed D');
          assert.notEqual(git(work, 'rev-parse', 'HEAD'), context.base);
          if (decision === 'qa-red') throw new Error('QA failure');
        },
      };
      if (['green', 'publish-green'].includes(decision)) {
        const result = await refresh(options);
        assert.equal(result.receipt.scans.personal_matches, 0);
        assert.equal(result.receipt.scans.secret_matches, 0);
        assert.equal(git(result.work, 'status', '--porcelain'), '');
        const paths = git(result.work, 'diff', '--name-only', context.base, result.sha).split('\n');
        assert.ok(paths.every((path) => path.startsWith('data/release/')));
        assert.equal(effects.length, scenario === 'publish-green' ? 2 : 1);
        assert.equal(effects[0].dry, '1');
        assert.equal(JSON.parse(await readFile(join(result.work, 'data/release/ingest.manifest.json'))).mode, mode);
        assert.equal((await readFile(join(result.work, 'data/release/retrievals.jsonl'), 'utf8')).trim().split('\n').length, mode === 'live' ? 44 : 22);
        assert.equal(git(result.work, 'rev-list', '--parents', '-n', '1', result.pages),
          decision === 'green' ? result.pages : `${result.pages} ${context.base}`);
        assert.ok(git(result.work, 'ls-tree', '-r', '--name-only', result.pages).includes('data/v6/weekly-cases.json'));
        if (scenario === 'publish-green') {
          await assert.rejects(access(join(result.work, 'target')), { code: 'ENOENT' });
          assert.equal(JSON.parse(await readFile(join(result.work, '../status.json'))).status, 'green');
          assert.deepEqual(JSON.parse(await readFile(join(result.work, '../pages-build.json'))), { commit: result.pages, status: 'built', id: 123 });
          const calls = (await readFile(context.env.FAKE_GH_LOG, 'utf8')).trim().split('\n').map(JSON.parse);
          assert.equal(calls[0][4], 'POST');
          assert.equal(calls.length, 4);
          assert.equal(git(context.origin, 'rev-parse', 'refs/heads/rolling'), result.sha);
          assert.equal(git(context.origin, 'rev-parse', 'refs/heads/main'), result.sha);
          assert.equal(git(context.origin, 'rev-parse', 'refs/heads/gh-pages'), result.pages);
        }
      } else {
        const phase = decision === 'qa-red' ? 'qa' : scenario === 'path-red' ? 'integrity'
          : scenario === 'publish-red' ? 'publish-prepare' : ['scan-red-crlf', 'secrets-bundle-red'].includes(scenario) ? 'publication-scans' : scenario.startsWith('pages-') ? 'pages-verify' : 'atomic-release';
        await assert.rejects(refresh(options), new RegExp(`failed at ${phase}`));
        assert.equal(effects.length, decision.startsWith('atomic-reject-') || scenario.startsWith('pages-') ? 2 : ['publish-red', 'scan-red-crlf', 'secrets-bundle-red'].includes(scenario) ? 1 : 0);
        const failures = (await readdir(context.state)).filter((path) => path.startsWith('FAILED-'));
        assert.equal(failures.length, 1);
        const report = await readFile(join(context.state, failures[0]), 'utf8');
        assert.ok(report.includes(phase));
        if (scenario.startsWith('pages-')) {
          const status = JSON.parse(await readFile(join(observedWork, '../status.json')));
          assert.equal(status.status, 'failed');
          const { sha, pages } = status.released;
          for (const ref of ['rolling', 'main']) assert.equal(git(context.origin, 'rev-parse', `refs/heads/${ref}`), sha);
          assert.equal(git(context.origin, 'rev-parse', 'refs/heads/gh-pages'), pages);
          const retry = `make pages-verify PAGES_COMMIT=${pages}`;
          assert.ok(report.includes('Refs moved:') && report.includes('site is stale') && report.includes(retry));
          assert.ok(report.includes('gh api --hostname github.com --method POST repos/example/koplik/pages/builds'));
          assert.ok(notices[0][1].includes('Refs moved:') && notices[0][1].includes(retry));
        }
      }
      assert.ok(observedWork.startsWith(context.state));
      if (scenario !== 'publish-green' && !scenario.startsWith('pages-')) {
        await assert.rejects(access(context.env.FAKE_GH_LOG), { code: 'ENOENT' });
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
  assert.equal(redact('ghp_' + 'a'.repeat(36), ['ghp_']), '[REDACTED]');
  const contact = 'refresh+test@example.invalid';
  for (const value of [contact, contact.toUpperCase(), encodeURIComponent(contact),
    encodeURIComponent(contact).toUpperCase(), '%72%65fresh%2Btest%40example.invalid']) {
    assert.equal(redact(`error: ${value}`, [], contact), 'error: [REDACTED]');
  }
  for (const value of ['Private Pattern', 'PRIVATE PATTERN', 'Private+Pattern', 'Private%20Pattern']) {
    assert.equal(redact(value, ['private pattern']), '[REDACTED]');
  }
  assert.equal(redact('GHp%5F' + 'a'.repeat(36), []), '[REDACTED]');
  assert.equal(redact('a.b+[x]', ['a.b+[x]']), '[REDACTED]');
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

test('preflight trims CRLF patterns and resolves private contact without accepting empty or invalid config', async () => {
  const context = await repository();
  try {
    const contact = join(context.shared, '.env.local');
    await writeFile(context.patterns, '   # comment\r\n\t\r\n  private-contact-sentinel \r\n');
    await writeFile(contact, 'KOPLIK_CENSUS_CONTACT=\n export KOPLIK_CENSUS_CONTACT = " refresh-test@example.invalid "\r\n');
    assert.deepEqual(await loadPatterns(context.patterns), ['private-contact-sentinel']);
    assert.equal((await preflight(context.patterns, contact)).contact, 'refresh-test@example.invalid');
    const original = git(context.origin, 'for-each-ref', '--format=%(refname) %(objectname)');
    for (const bad of ['blank-patterns', 'comment-patterns', 'missing-patterns', 'missing-contact', 'blank-contact', 'unset-contact', 'invalid-utf8', 'invalid-pattern-utf8', 'unrecognized-bom-key']) {
      await writeFile(context.patterns, 'private-contact-sentinel\n');
      await writeFile(contact, 'KOPLIK_CENSUS_CONTACT=refresh-test@example.invalid\n');
      if (bad === 'blank-patterns') await writeFile(context.patterns, ' \t\r\n\r\n');
      if (bad === 'comment-patterns') await writeFile(context.patterns, ' # comment\r\n');
      if (bad === 'missing-patterns') await rm(context.patterns);
      if (bad === 'missing-contact') await rm(contact);
      if (bad === 'blank-contact') await writeFile(contact, 'KOPLIK_CENSUS_CONTACT="  "\n');
      if (bad === 'unset-contact') await writeFile(contact, 'OTHER=value\n');
      if (bad === 'invalid-utf8') await writeFile(contact, Buffer.from([0xff]));
      if (bad === 'invalid-pattern-utf8') await writeFile(context.patterns, Buffer.from([0xff]));
      if (bad === 'unrecognized-bom-key') await writeFile(contact, '\uFEFFKOPLIK_CENSUS_CONTACT=refresh-test@example.invalid\n');
      let ingests = 0;
      await assert.rejects(refresh({ ...context, ingest: async () => { ingests++; },
        run: (program, args, cwd, env) => program === 'notify-send' ? '' : command(program, args, cwd, env) }), /failed at preflight/);
      assert.equal(ingests, 0, bad);
      assert.equal(git(context.origin, 'for-each-ref', '--format=%(refname) %(objectname)'), original);
      console.log(`Offline refresh red-preflight ${bad}: ingest not invoked, all refs preserved`);
    }
  } finally { await rm(context.root, { recursive: true, force: true }); }
});

test('retention keeps two newest green sources, preserves failed sources and expires their targets after 14 days', async () => {
  const state = await mkdtemp('/tmp/koplik-retention-');
  const now = Date.parse('2026-10-07T00:00:00Z');
  const names = [];
  try {
    for (const [index, status, age] of [[1, 'green', 1], [2, 'green', 7], [3, 'green', 21], [4, 'failed', 14], [5, 'failed', 13], [6, 'running', 15], [7, 'dry-run', 15]]) {
      const name = `2026-10-07-00000000-0000-0000-0000-${String(index).padStart(12, '0')}`;
      names[index] = name;
      const root = join(state, name);
      await mkdir(join(root, 'worktree/target'), { recursive: true });
      await writeFile(join(root, 'worktree/source.txt'), 'Retained source\n');
      await writeFile(join(root, 'worktree/target/output'), 'Disposable build\n');
      await writeFile(join(root, 'status.json'), JSON.stringify({ status, completed_at: new Date(now - age * 86400000).toISOString() }));
    }
    for (const [index, age] of [[8, 15], [9, 13]]) {
      names[index] = `2026-10-07-00000000-0000-0000-0000-${String(index).padStart(12, '0')}`;
      const root = join(state, names[index]);
      await mkdir(join(root, 'worktree/target'), { recursive: true });
      await writeFile(join(root, 'worktree/source.txt'), 'Interrupted source');
      await writeFile(join(root, 'worktree/target/output'), 'Disposable build');
      const modified = new Date(now - age * 86400000);
      await utimes(root, modified, modified);
    }
    await mkdir(join(state, 'operator-notes/target'), { recursive: true });
    await pruneRuns(state, now);
    assert.deepEqual((await readdir(state)).sort(), [1, 2, 4, 5, 6, 7, 8, 9].map((i) => names[i]).concat('operator-notes').sort());
    for (const i of [1, 2, 4, 5, 6, 7, 8, 9]) await access(join(state, names[i], 'worktree/source.txt'));
    for (const i of [4, 6, 7, 8]) await assert.rejects(access(join(state, names[i], 'worktree/target')), { code: 'ENOENT' });
    for (const i of [5, 9]) await access(join(state, names[i], 'worktree/target/output'));
    await access(join(state, 'operator-notes/target'));
  } finally { await rm(state, { recursive: true, force: true }); }
});

test('failing QA persists only a redacted last 200 lines of child output', async () => {
  const context = await repository();
  try {
    await assert.rejects(refresh({ ...context, build: fakeBuild, ingest: fixtureIngest,
      run: (program, args, cwd, env) => program === 'notify-send' ? '' : command(program, args, cwd, env),
      qa: async ({ work, env }) => command(process.execPath, ['-e',
        "for(let i=0;i<210;i++)console.error('line '+i);console.error('refresh-test@example.invalid private-contact-sentinel '+('ghp_'+'a'.repeat(36)));process.exitCode=1"], work, env),
    }), /failed at qa/);
    const report = await readFile(join(context.state, (await readdir(context.state)).find((name) => name.startsWith('FAILED-'))), 'utf8');
    const tail = report.split('Redacted command tail (last 200 lines):\n\n')[1].trimEnd().split('\n');
    assert.equal(tail.length, 200);
    assert.equal(tail[0], '    line 11');
    assert.equal(tail.at(-1), '    [REDACTED] [REDACTED] [REDACTED]');
    assert.equal((await readdir(context.state)).filter((name) => name.startsWith('FAILED-')).length, 1);
  } finally { await rm(context.root, { recursive: true, force: true }); }
});

test('bootstrap executes landed rolling code and removes its clone despite dirty shared code', async () => {
  const context = await repository();
  try {
    await mkdir(join(context.shared, 'tools/refresh'), { recursive: true });
    const script = join(context.shared, 'tools/refresh/refresh.mjs');
    await writeFile(script, "import {writeFile} from 'node:fs/promises';await writeFile(process.env.KOPLIK_REFRESH_STATE+'/landed-code', 'reviewed-tip');\n");
    git(context.shared, 'add', '--', 'tools/refresh/refresh.mjs');
    git(context.shared, 'commit', '-m', 'Landed bootstrap test');
    git(context.shared, 'push', 'origin', 'HEAD:refs/heads/rolling');
    await writeFile(script, "throw new Error('Dirty shared code must not run');\n");
    assert.equal(await bootstrap(context), 0);
    assert.equal(await readFile(join(context.state, 'landed-code'), 'utf8'), 'reviewed-tip');
    assert.deepEqual(await readdir(context.state), ['landed-code']);
    await rm(join(context.origin, 'refs/heads/rolling'));
    assert.equal(await bootstrap(context), 1);
    assert.equal((await readdir(context.state)).filter((name) => name.startsWith('FAILED-bootstrap-')).length, 1);
    assert.deepEqual((await readdir(context.state)).filter((name) => name.startsWith('bootstrap-')), []);
  } finally { await rm(context.root, { recursive: true, force: true }); }
});

test('redacted command diagnostics preserve complete credentials across interleaved pipe chunks', async () => {
  const state = await mkdtemp('/tmp/koplik-command-tail-');
  try {
    await assert.rejects(command(process.execPath, ['-e',
      "process.stderr.write('ghp_');setTimeout(()=>{console.log('interleaved output');setTimeout(()=>{process.stderr.write('a'.repeat(36)+'\\n');process.exitCode=1},20)},20)"], state, dummyEnv),
    (error) => {
      assert.equal(redact(error.commandTail, ['ghp_']), 'interleaved output\n[REDACTED]');
      return true;
    });
  } finally { await rm(state, { recursive: true, force: true }); }
});
