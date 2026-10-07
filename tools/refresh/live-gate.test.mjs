import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { cp, mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { randomUUID } from 'node:crypto';
import { homedir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import { publishSite } from '../publish.mjs';
import { buildData, command, verifyStore } from './data.mjs';
import { project, seedFixtureStore } from './fixture-store.mjs';
import { refresh } from './refresh.mjs';

// Included only by refresh-test, not web-test: the candidate's real web-test
// gate must not recursively launch another full refresh QA integration test.
test('offline-prepared LIVE candidate passes real refresh QA including make web-test', { timeout: 900000 }, async () => {
  const scratch = await mkdtemp(join(homedir(), '.rsi/koplik-refresh-live-test-'));
  const targets = join(project, 'target/live-gate');
  await mkdir(targets, { recursive: true });
  const taskTarget = await mkdtemp(join(targets, 'run-'));
  const shared = join(scratch, 'shared');
  const env = { ...process.env, CARGO_NET_OFFLINE: 'true', npm_config_offline: 'true' };
  delete env.RSI_SESSION_TOKEN;
  for (const key of Object.keys(env)) if (key.startsWith('GIT_')) delete env[key];
  const git = (cwd, ...args) => command('git', args, cwd, env);
  try {
    await mkdir(shared);
    const archive = join(taskTarget, 'source.tar');
    await git(project, 'archive', '--format=tar', `--output=${archive}`, 'HEAD');
    await command('tar', ['-xf', archive, '-C', shared], project, env);
    await rm(join(shared, 'data/release'), { recursive: true });
    await seedFixtureStore({ store: join(shared, 'data/release'), work: join(taskTarget, 'initial-ingest') });
    await git(shared, 'init', '--initial-branch=rolling');
    await git(shared, 'config', 'user.name', 'Live gate test');
    await git(shared, 'config', 'user.email', 'live-gate@example.invalid');
    const paths = (await git(project, 'ls-tree', '--name-only', 'HEAD')).split('\n');
    await git(shared, 'add', '--', ...paths);
    await git(shared, 'commit', '-m', 'Offline live gate source');
    const base = await git(shared, 'rev-parse', 'HEAD');
    const origin = join(scratch, 'origin.git');
    await git(shared, 'clone', '--bare', shared, origin);
    await git(origin, 'update-ref', 'refs/heads/main', base);
    await git(shared, 'remote', 'add', 'origin', origin);
    await writeFile(join(shared, '.env.local'), 'KOPLIK_CENSUS_CONTACT=live-gate@example.invalid\n');
    const patterns = join(scratch, 'patterns');
    await writeFile(patterns, `unavailable-private-pattern-${randomUUID()}\n`);
    const before = await git(origin, 'for-each-ref', '--format=%(refname) %(objectname)');
    const completed = [];
    // This compiler target belongs solely to the candidate clone, and stays on
    // disk under this sandbox instead of /tmp's shared tmpfs.
    const candidateTarget = join(taskTarget, 'candidate-target');
    const run = async (program, args, cwd, childEnv) => {
      if (program.endsWith('/tools/offline-test.sh') && args[0] === 'make' && args[1] === 'publish') {
        assert.equal(childEnv.PUBLISH_DRY_RUN, '1');
        const phase = join(taskTarget, 'prepared-pages');
        await mkdir(phase);
        // QA below includes real publisher builds. Here use the real candidate
        // pipeline bytes to test refresh's dry-run decision/object validation.
        const dist = join(phase, 'site');
        await cp(join(cwd, 'target/refresh-out-3'), join(dist, 'data'), { recursive: true });
        await publishSite({ scratch: phase, dist, target: origin, source: await git(cwd, 'rev-parse', 'HEAD'),
          identity: ['', 'Live gate test', 'live-gate@example.invalid'], env: childEnv, dryRun: '1',
          prepare: { root: cwd, output: childEnv.PUBLISH_PREPARE_OUTPUT, parent: childEnv.PUBLISH_EXPECTED_PARENT } });
        return '';
      }
      if (program === 'git' && args[0] === 'push') throw new Error('Live gate must not push');
      return command(program, args, cwd, childEnv);
    };
    const result = await refresh({ shared, state: join(scratch, 'state'), patterns, env, run, dryRun: true,
      ingest: async ({ release, stageWork }) => {
        await seedFixtureStore({ store: release, work: stageWork, mode: 'live' });
        const manifest = JSON.parse(await readFile(join(stageWork, 'ingest.manifest.json')));
        // Fixture captures intentionally omit the optional direct county-code
        // endpoint; fake live completion retains only actual captured sources.
        for (const [id, item] of Object.entries(manifest.items)) if (item.status === 'missing') delete manifest.items[id];
        await writeFile(join(stageWork, 'ingest.manifest.json'), `${JSON.stringify(manifest)}\n`);
      },
      build: (options) => buildData({ ...options, env: { ...options.env, CARGO_TARGET_DIR: candidateTarget } }),
      qa: async ({ work, env: runtime, gates }) => {
        assert.equal(JSON.parse(await readFile(join(work, 'data/release/ingest.manifest.json'))).mode, 'live');
        assert.equal((await verifyStore(join(work, 'data/release'))).length, 44);
        assert.equal(await git(work, 'status', '--porcelain'), '');
        await mkdir(join(work, 'target/tools/bin'), { recursive: true });
        await cp(join(project, 'target/tools/bin/wasm-bindgen'), join(work, 'target/tools/bin/wasm-bindgen'));
        for (const gate of gates) {
          console.log(`LIVE candidate QA: make ${gate}`);
          await new Promise((accept, reject) => {
            // These test-only contacts/inputs are public fixtures and dummy
            // config. Keep foreground gate output in the parent test's tee log.
            const child = spawn('make', [gate], { cwd: work, stdio: 'inherit', env: {
              ...runtime, CARGO_TARGET_DIR: candidateTarget, CARGO_NET_OFFLINE: 'true',
              npm_config_offline: 'true', TMPDIR: taskTarget } });
            child.on('error', reject);
            child.on('close', (code) => code === 0 ? accept() : reject(new Error(`LIVE QA make ${gate} failed (${code})`)));
          });
          completed.push(gate);
          console.log(`LIVE candidate QA passed: make ${gate}`);
        }
      },
    });
    assert.deepEqual(completed, ['check', 'test', 'web-test', 'determinism']);
    assert.equal(JSON.parse(await git(result.work, 'show', `${result.pages}:data/manifest.json`)).mode, 'live');
    assert.equal(await git(origin, 'for-each-ref', '--format=%(refname) %(objectname)'), before);
    assert.ok((await git(result.work, 'diff', '--name-only', base, result.sha)).split('\n').every((path) => path.startsWith('data/release/')));
    console.log('LIVE candidate: all four real QA gates green; dry-run left all origin refs unchanged');
  } finally {
    await rm(scratch, { recursive: true, force: true });
    await rm(taskTarget, { recursive: true, force: true });
  }
});
