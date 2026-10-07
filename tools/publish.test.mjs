import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { appendFile, cp, mkdir, mkdtemp, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { basename, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import { publishSite } from './publish.mjs';

const project = fileURLToPath(new URL('../', import.meta.url));
function command(args, cwd, env = {}) {
  const result = spawnSync(args[0], args.slice(1), {
    cwd, encoding: 'utf8', env: { ...process.env, ...env },
  });
  assert.ifError(result.error);
  assert.equal(result.status, 0, `${args.join(' ')}\n${result.stdout}\n${result.stderr}`);
  return result.stdout.trim();
}

test('publish builds Pages offline, preserves the caller and only fast-forwards gh-pages', async () => {
  assert.ok(existsSync(join(project, 'pkg/web/koplik_wasm.js')), 'run make wasm before the publishing test');
  const scratch = await mkdtemp(join(tmpdir(), 'koplik-publish-test-'));
  try {
    const caller = join(scratch, 'caller');
    const remote = join(scratch, 'origin.git');
    await mkdir(caller);
    for (const path of ['web', 'crates', 'tools', 'Makefile', '.gitignore', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml']) {
      await cp(join(project, path), join(caller, path), {
        recursive: true,
        filter: (path) => !['node_modules', 'dist', 'test-results', 'playwright-report', 'public'].includes(basename(path)),
      });
    }
    // Copy only a build tool; both CLI test runs compile the actual WASM in scratch.
    await mkdir(join(caller, 'target/tools/bin'), { recursive: true });
    await cp(join(project, 'target/tools/bin/wasm-bindgen'), join(caller, 'target/tools/bin/wasm-bindgen'));
    const index = join(caller, 'web/index.html');
    const committedMarker = '<meta name="publish-test-source" content="committed-tree">';
    await writeFile(index, (await readFile(index, 'utf8')).replace('</head>', `${committedMarker}</head>`));
    const git = (...args) => command(['git', ...args], caller);
    git('init', '--initial-branch=worker');
    git('config', 'user.name', 'Publish test');
    git('config', 'user.email', 'publish-test@example.invalid');
    git('add', '--', 'web', 'crates', 'tools', 'Makefile', '.gitignore', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml');
    git('commit', '-m', 'Publishing test source');
    const source = git('rev-parse', 'HEAD');
    git('init', '--bare', remote);
    git('remote', 'add', 'origin', remote);
    // Both staged and unstaged edits must survive every publish attempt.
    await appendFile(join(caller, 'web/README.md'), '\nStaged publishing test edit.\n');
    git('add', '--', 'web/README.md');
    await appendFile(join(caller, 'web/README.md'), '\nUnstaged publishing test edit.\n');
    await writeFile(join(caller, 'caller-marker'), 'Untracked caller file\n');
    // Staged/unstaged HTML and ignored assets must not influence the published tree.
    await writeFile(index, (await readFile(index, 'utf8')).replace('committed-tree', 'staged-tree'));
    git('add', '--', 'web/index.html');
    await writeFile(index, (await readFile(index, 'utf8')).replace('staged-tree', 'dirty-tree'));
    await mkdir(join(caller, 'web/public'), { recursive: true });
    await writeFile(join(caller, 'web/public/untracked-asset.txt'), 'Untracked source asset\n');
    // A package with no exports would break the Vite build if reused.
    await mkdir(join(caller, 'pkg/web'), { recursive: true });
    await writeFile(join(caller, 'pkg/web/koplik_wasm.js'), 'throw new Error("Stale WASM package");\n');
    await writeFile(join(caller, 'pkg/web/koplik_wasm_bg.wasm'), 'Stale WASM bytes');
    const snapshot = async () => ({
      status: git('status', '--porcelain=v1', '--untracked-files=all'),
      branch: git('symbolic-ref', 'HEAD'),
      refs: git('show-ref'),
      worktrees: git('worktree', 'list', '--porcelain'),
      index: createHash('sha256').update(await readFile(join(caller, '.git/index'))).digest('hex'),
      readme: await readFile(join(caller, 'web/README.md'), 'utf8'),
      html: await readFile(index, 'utf8'),
      staleWasm: await readFile(join(caller, 'pkg/web/koplik_wasm.js'), 'utf8'),
      marker: await readFile(join(caller, 'caller-marker'), 'utf8'),
      fetchHead: existsSync(join(caller, '.git/FETCH_HEAD')),
      dist: existsSync(join(caller, 'web/dist')),
    });
    const before = await snapshot();
    const publish = (extra = {}) => spawnSync('make', ['publish'], {
      cwd: caller, encoding: 'utf8',
      env: { ...process.env, TMPDIR: scratch, PUBLISH_REMOTE: remote, PUBLISH_DRY_RUN: '0', CARGO_NET_OFFLINE: 'true', npm_config_offline: 'true', ...extra },
    });
    const successfulPublish = async (extra) => {
      const result = publish(extra);
      assert.ifError(result.error);
      assert.equal(result.status, 0, `${result.stdout}\n${result.stderr}`);
      assert.deepEqual(await snapshot(), before);
      assert.deepEqual((await readdir(scratch)).filter((name) => name.startsWith('koplik-publish-')), []);
      return result.stdout;
    };
    const refs = () => git('--git-dir', remote, 'for-each-ref', '--format=%(refname) %(objectname)');
    const pages = () => git('--git-dir', remote, 'rev-parse', 'refs/heads/gh-pages');
    const rootDryRun = await successfulPublish({ PUBLISH_DRY_RUN: '1', GIT_INDEX_FILE: join(caller, '.git/index') });
    assert.match(rootDryRun, /Would push .*HEAD:refs\/heads\/gh-pages \(parent root\)/);
    assert.equal(refs(), '');

    await successfulPublish({ PUBLISH_REMOTE: 'origin' });
    const first = pages();
    assert.equal(git('--git-dir', remote, 'rev-list', '--parents', '-n', '1', first), first);
    assert.equal(git('--git-dir', remote, 'show', `${first}:.nojekyll`), '');
    const html = git('--git-dir', remote, 'show', `${first}:index.html`);
    assert.ok(html.includes(committedMarker), 'published HTML uses the committed source');
    assert.equal(git('--git-dir', remote, 'show', '-s', '--format=%s', first), `Publish Koplik from ${source}`);
    const wasmAsset = git('--git-dir', remote, 'ls-tree', '-r', '--name-only', first).split('\n').find((path) => path.endsWith('.wasm'));
    assert.ok(wasmAsset, 'the fresh WASM package is published');
    const wasm = spawnSync('git', ['--git-dir', remote, 'show', `${first}:${wasmAsset}`], { cwd: caller });
    assert.equal(wasm.status, 0);
    assert.deepEqual([...wasm.stdout.subarray(0, 4)], [0, 97, 115, 109], 'published bytes are a real WASM module');
    const assets = [...html.matchAll(/(?:src|href)="([^"]*\/assets\/[^\"]+)"/g)].map((match) => match[1]);
    assert.ok(assets.length >= 2, 'the built page has JavaScript and CSS assets');
    assert.ok(assets.every((asset) => asset.startsWith('/koplik/assets/')));
    assert.equal(refs(), `refs/heads/gh-pages ${first}`);

    // Exercise Git-only cases on the real built site without repeating Rust compilation.
    const dist = join(scratch, 'built-site');
    await mkdir(dist);
    const siteArchive = join(scratch, 'site.tar');
    git('--git-dir', remote, 'archive', '--format=tar', `--output=${siteArchive}`, first);
    command(['tar', '-xf', siteArchive, '-C', dist], caller);
    const publishBuilt = async (extra = {}) => {
      const phase = await mkdtemp(join(scratch, 'phase-'));
      try {
        return await publishSite({ scratch: phase, dist, target: remote, source,
          identity: ['', 'Publish test', 'publish-test@example.invalid'],
          env: { ...process.env, ...extra }, dryRun: extra.PUBLISH_DRY_RUN || '0' });
      } finally { await rm(phase, { recursive: true, force: true }); }
    };
    await publishBuilt();
    assert.deepEqual(await snapshot(), before);
    const second = pages();
    assert.equal(git('--git-dir', remote, 'rev-parse', `${second}^`), first);
    git('--git-dir', remote, 'merge-base', '--is-ancestor', first, second);
    const beforeDryRun = refs();
    const dryRun = await publishBuilt({ PUBLISH_DRY_RUN: '1' });
    assert.match(dryRun, new RegExp(`Would push .*HEAD:refs/heads/gh-pages \\(parent ${second}\\)`));
    assert.equal(refs(), beforeDryRun);

    // A rejected push must fail without changing the target or the caller.
    await writeFile(join(remote, 'hooks/pre-receive'), '#!/bin/sh\necho "test rejects publication" >&2\nexit 1\n', { mode: 0o755 });
    await assert.rejects(publishBuilt(), /test rejects publication/);
    assert.equal(refs(), beforeDryRun);
    assert.deepEqual(await snapshot(), before);
    await rm(join(remote, 'hooks/pre-receive'));

    // Interpose one real competing local publication immediately before push.
    const bin = join(scratch, 'bin');
    await mkdir(bin);
    const realGit = command(['which', 'git'], caller);
    const raceMarker = join(scratch, 'competing-sha');
    await writeFile(join(bin, 'git'), `#!/usr/bin/env node
const { spawnSync } = require('node:child_process');
const { existsSync, writeFileSync } = require('node:fs');
const args = process.argv.slice(2);
const git = ${JSON.stringify(realGit)};
const marker = ${JSON.stringify(raceMarker)};
const remote = ${JSON.stringify(remote)};
function run(args) {
  const r = spawnSync(git, args, { encoding: 'utf8', env: { ...process.env, GIT_AUTHOR_NAME: 'Competing publisher', GIT_AUTHOR_EMAIL: 'race@example.invalid', GIT_COMMITTER_NAME: 'Competing publisher', GIT_COMMITTER_EMAIL: 'race@example.invalid' } });
  if (r.status !== 0) throw new Error(r.stderr);
  return r.stdout.trim();
}
if (args.includes('push') && !existsSync(marker)) {
  const parent = run(['--git-dir', remote, 'rev-parse', 'refs/heads/gh-pages']);
  const tree = run(['--git-dir', remote, 'rev-parse', parent + '^{tree}']);
  const commit = run(['--git-dir', remote, 'commit-tree', tree, '-p', parent, '-m', 'Competing publication']);
  run(['--git-dir', remote, 'update-ref', 'refs/heads/gh-pages', commit, parent]);
  writeFileSync(marker, commit);
}
const r = spawnSync(git, args, { stdio: 'inherit' });
process.exit(r.status ?? 1);
`, { mode: 0o755 });
    await publishBuilt({ PATH: `${bin}:${process.env.PATH}` });
    const competing = await readFile(raceMarker, 'utf8');
    const final = pages();
    assert.equal(git('--git-dir', remote, 'rev-parse', `${final}^`), competing);
    git('--git-dir', remote, 'merge-base', '--is-ancestor', second, final);
    assert.equal(refs(), `refs/heads/gh-pages ${final}`);

    const forbidden = publish({ PUBLISH_REMOTE: 'https://github.com/example/koplik.git' });
    assert.notEqual(forbidden.status, 0);
    assert.match(forbidden.stderr, /local bare repository/);
    assert.deepEqual(await snapshot(), before);

    // Pause archive extraction with an actual child, then signal the publisher.
    await writeFile(join(bin, 'git'), `#!/usr/bin/env node
const { spawnSync } = require('node:child_process');
const args = process.argv.slice(2);
if (args.includes('archive')) {
  process.stdout.write('ARCHIVE_READY ' + process.pid + '\\n');
  setInterval(() => {}, 1000);
} else {
  const r = spawnSync(${JSON.stringify(realGit)}, args, { stdio: 'inherit' });
  process.exit(r.status ?? 1);
}
`, { mode: 0o755 });
    for (const signal of ['SIGINT', 'SIGTERM']) {
      const child = spawn(process.execPath, ['tools/publish.mjs'], {
        cwd: caller, env: { ...process.env, TMPDIR: scratch, PUBLISH_REMOTE: remote, PATH: `${bin}:${process.env.PATH}` },
        stdio: ['ignore', 'pipe', 'pipe'],
      });
      let subprocess;
      await new Promise((accept, reject) => {
        const timeout = setTimeout(() => { child.kill('SIGTERM'); reject(new Error('Publisher signal test timed out')); }, 15000);
        let stdout = '';
        child.stdout.on('data', (chunk) => {
          stdout += chunk;
          const ready = stdout.match(/ARCHIVE_READY (\d+)/);
          if (ready && !subprocess) { subprocess = Number(ready[1]); child.kill(signal); }
        });
        child.stderr.resume();
        child.on('error', reject);
        child.on('close', (code) => {
          clearTimeout(timeout);
          try { assert.equal(code, signal === 'SIGINT' ? 130 : 143); accept(); }
          catch (error) { reject(error); }
        });
      });
      assert.throws(() => process.kill(subprocess, 0), { code: 'ESRCH' }, 'build subprocess was stopped');
      assert.deepEqual((await readdir(scratch)).filter((name) => name.startsWith('koplik-publish-')), []);
      assert.deepEqual(await snapshot(), before);
      assert.equal(refs(), `refs/heads/gh-pages ${final}`);
    }
  } finally {
    await rm(scratch, { recursive: true, force: true });
  }
});
