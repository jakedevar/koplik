import assert from 'node:assert/strict';
import { cp, mkdir, mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import { command } from './data.mjs';
import { fakeGh } from './fake-gh.mjs';
import { pagesRepository, verifyPages, verifyPublishedPages } from './pages.mjs';
import { fileURLToPath } from 'node:url';

const commit = 'a'.repeat(40);
test('mirror target parsing accepts GitHub transports and refuses ambiguous or unsafe URLs', async () => {
  for (const url of ['https://github.com/example/koplik.git', 'https://github.com/example/koplik',
    'git@github.com:example/koplik.git', 'ssh://git@github.com/example/koplik.git']) {
    assert.equal(await pagesRepository('.', async () => url), 'example/koplik');
  }
  for (const url of ['https://example.invalid/example/koplik.git', 'https://user@github.com/example/koplik.git',
    'https://github.com/../koplik.git', 'https://github.com/example/koplik.git\nhttps://github.com/example/other.git',
    'https://github.com/example/koplik.git?token=dummy']) await assert.rejects(pagesRepository('.', async () => url));
});

test('fake gh requests a build and waits for built at G, with bounded failures', async () => {
  const root = await mkdtemp(join(tmpdir(), 'koplik-pages-test-'));
  const baseEnv = { ...process.env };
  delete baseEnv.RSI_SESSION_TOKEN;
  const env = await fakeGh(root, baseEnv);
  try {
    for (const mode of ['green', 'never', 'wrong-built', 'error', 'get-error', 'errored', 'malformed']) {
      await rm(env.FAKE_GH_LOG, { force: true });
      let time = 0;
      const options = { repository: 'example/koplik', commit, cwd: root,
        env: { ...env, FAKE_GH_COMMIT: commit, FAKE_GH_MODE: mode },
        timeoutMs: 3000, pollMs: 1000, now: () => time, sleep: async (ms) => { time += ms; } };
      if (mode === 'green') assert.deepEqual(await verifyPages(options), { commit, status: 'built', id: 123 });
      else await assert.rejects(verifyPages(options));
      const calls = (await readFile(env.FAKE_GH_LOG, 'utf8')).trim().split('\n').map(JSON.parse);
      assert.deepEqual(calls[0], ['api', '--hostname', 'github.com', '--method', 'POST', 'repos/example/koplik/pages/builds']);
      assert.ok(calls.slice(1).every((args) => args[4] === 'GET' && args[5] === 'repos/example/koplik/pages/builds/latest'));
      assert.equal(calls.length, mode === 'green' || ['never', 'wrong-built'].includes(mode) ? 4 : mode === 'error' ? 1 : mode === 'errored' ? 3 : 2);
    }
    await assert.rejects(command(process.execPath, ['-e', 'setInterval(()=>{},1000)'], root, env, { timeoutMs: 100 }), /Command failed/);
  } finally { await rm(root, { recursive: true, force: true }); }
});

test('standalone verifier uses the local mirror and gh-pages tip without moving refs', async () => {
  const root = await mkdtemp(join(tmpdir(), 'koplik-pages-manual-'));
  const origin = join(root, 'origin.git');
  await mkdir(origin);
  const calls = [];
  const run = async (program, args, cwd) => {
    calls.push({ program, args, cwd });
    if (program === 'git') {
      if (args.includes('origin')) return origin;
      if (args.includes('--is-bare-repository')) return 'true';
      if (args.includes('github')) return 'git@github.com:example/koplik.git';
      if (args.includes('refs/heads/gh-pages')) return commit;
      assert.fail('Unexpected Git operation');
    }
    assert.equal(program, 'gh');
    return JSON.stringify({ status: 'built', commit });
  };
  try {
    assert.equal((await verifyPublishedPages({ root, run })).commit, commit);
    assert.equal(calls.filter(({ program }) => program === 'gh').length, 2);
    await assert.rejects(verifyPublishedPages({ root, run: (program, args, cwd) => {
      if (program === 'gh') throw new Error('API unavailable');
      return run(program, args, cwd);
    } }), new RegExp(`make pages-verify PAGES_COMMIT=${commit}`));
  } finally { await rm(root, { recursive: true, force: true }); }
});

test('make pages-verify uses fake gh on PATH against a temporary local origin', async () => {
  const root = await mkdtemp(join(tmpdir(), 'koplik-pages-make-'));
  const source = join(root, 'source');
  await mkdir(join(source, 'tools/refresh'), { recursive: true });
  const baseEnv = { ...process.env };
  delete baseEnv.RSI_SESSION_TOKEN;
  delete baseEnv.PAGES_COMMIT;
  for (const key of Object.keys(baseEnv)) if (key.startsWith('GIT_')) delete baseEnv[key];
  const env = { ...await fakeGh(root, baseEnv), FAKE_GH_MODE: 'instant' };
  const git = (cwd, args) => command('git', args, cwd, env);
  const origin = join(root, 'origin.git');
  env.FAKE_GH_ORIGIN = origin;
  try {
    const project = fileURLToPath(new URL('../../', import.meta.url));
    for (const path of ['Makefile', 'tools/refresh/pages.mjs', 'tools/refresh/data.mjs']) await cp(join(project, path), join(source, path));
    await git(source, ['init', '--initial-branch=gh-pages']);
    await git(source, ['-c', 'user.name=Pages test', '-c', 'user.email=pages-test@example.invalid', 'commit', '--allow-empty', '-m', 'Manual publication test']);
    const tip = await git(source, ['rev-parse', 'HEAD']);
    await git(source, ['clone', '--bare', source, origin]);
    await git(source, ['remote', 'add', 'origin', origin]);
    await git(origin, ['remote', 'add', 'github', 'git@github.com:example/koplik.git']);
    const before = await git(origin, ['show-ref']);
    assert.ok((await command('make', ['pages-verify'], source, env)).includes(`Pages built ${tip}`));
    assert.ok((await command('make', ['pages-verify', `PAGES_COMMIT=${tip}`], source, env)).includes(`Pages built ${tip}`));
    assert.equal(await git(origin, ['show-ref']), before);
    await assert.rejects(command('make', ['pages-verify', `PAGES_COMMIT=${tip}`], source, { ...env, FAKE_GH_MODE: 'error' }),
      (error) => error.commandTail.includes(`make pages-verify PAGES_COMMIT=${tip}`));
  } finally { await rm(root, { recursive: true, force: true }); }
});
