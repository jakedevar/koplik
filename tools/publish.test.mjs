import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { appendFile, cp, mkdir, mkdtemp, readFile, rm, symlink, writeFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { basename, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

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
    for (const path of ['web', 'crates', 'pkg/web', 'tools', 'Makefile', '.gitignore', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml']) {
      await cp(join(project, path), join(caller, path), {
        recursive: true,
        filter: (path) => !['node_modules', 'dist', 'test-results', 'playwright-report', 'public'].includes(basename(path)),
      });
    }
    // Existing production artifacts are copied verbatim if present; none are invented.
    if (existsSync(join(project, 'web/public'))) {
      await cp(join(project, 'web/public'), join(caller, 'web/public'), { recursive: true });
    }
    await symlink(join(project, 'web/node_modules'), join(caller, 'web/node_modules'), 'dir');
    const git = (...args) => command(['git', ...args], caller);
    git('init', '--initial-branch=worker');
    git('config', 'user.name', 'Publish test');
    git('config', 'user.email', 'publish-test@example.invalid');
    git('add', '--', 'web', 'crates', 'tools', 'Makefile', '.gitignore', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml');
    git('commit', '-m', 'Publishing test source');
    git('init', '--bare', remote);
    git('remote', 'add', 'origin', remote);
    // Both staged and unstaged edits must survive every publish attempt.
    await appendFile(join(caller, 'web/README.md'), '\nStaged publishing test edit.\n');
    git('add', '--', 'web/README.md');
    await appendFile(join(caller, 'web/README.md'), '\nUnstaged publishing test edit.\n');
    await writeFile(join(caller, 'caller-marker'), 'Untracked caller file\n');
    const snapshot = async () => ({
      status: git('status', '--porcelain=v1', '--untracked-files=all'),
      branch: git('symbolic-ref', 'HEAD'),
      refs: git('show-ref'),
      worktrees: git('worktree', 'list', '--porcelain'),
      index: createHash('sha256').update(await readFile(join(caller, '.git/index'))).digest('hex'),
      readme: await readFile(join(caller, 'web/README.md'), 'utf8'),
      marker: await readFile(join(caller, 'caller-marker'), 'utf8'),
      fetchHead: existsSync(join(caller, '.git/FETCH_HEAD')),
      dist: existsSync(join(caller, 'web/dist')),
    });
    const before = await snapshot();
    const publish = (extra = {}) => spawnSync('make', ['publish'], {
      cwd: caller, encoding: 'utf8',
      env: { ...process.env, TMPDIR: scratch, PUBLISH_REMOTE: remote, PUBLISH_DRY_RUN: '0', ...extra },
    });
    const successfulPublish = async (extra) => {
      const result = publish(extra);
      assert.ifError(result.error);
      assert.equal(result.status, 0, `${result.stdout}\n${result.stderr}`);
      assert.deepEqual(await snapshot(), before);
      return result.stdout;
    };
    const refs = () => git('--git-dir', remote, 'for-each-ref', '--format=%(refname) %(objectname)');
    const pages = () => git('--git-dir', remote, 'rev-parse', 'refs/heads/gh-pages');
    const rootDryRun = await successfulPublish({ PUBLISH_DRY_RUN: '1' });
    assert.match(rootDryRun, /Would push .*HEAD:refs\/heads\/gh-pages \(parent root\)/);
    assert.equal(refs(), '');

    await successfulPublish();
    const first = pages();
    assert.equal(git('--git-dir', remote, 'rev-list', '--parents', '-n', '1', first), first);
    assert.equal(git('--git-dir', remote, 'show', `${first}:.nojekyll`), '');
    const html = git('--git-dir', remote, 'show', `${first}:index.html`);
    const assets = [...html.matchAll(/(?:src|href)="([^"]*\/assets\/[^\"]+)"/g)].map((match) => match[1]);
    assert.ok(assets.length >= 2, 'the built page has JavaScript and CSS assets');
    assert.ok(assets.every((asset) => asset.startsWith('/koplik/assets/')));
    assert.equal(refs(), `refs/heads/gh-pages ${first}`);

    await successfulPublish();
    const second = pages();
    assert.equal(git('--git-dir', remote, 'rev-parse', `${second}^`), first);
    git('--git-dir', remote, 'merge-base', '--is-ancestor', first, second);
    const beforeDryRun = refs();
    const dryRun = await successfulPublish({ PUBLISH_DRY_RUN: '1' });
    assert.match(dryRun, new RegExp(`Would push .*HEAD:refs/heads/gh-pages \\(parent ${second}\\)`));
    assert.equal(refs(), beforeDryRun);

    // A rejected push must fail without changing the target or the caller.
    await writeFile(join(remote, 'hooks/pre-receive'), '#!/bin/sh\necho "test rejects publication" >&2\nexit 1\n', { mode: 0o755 });
    const rejected = publish();
    assert.notEqual(rejected.status, 0);
    assert.match(rejected.stderr, /test rejects publication/);
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
    await successfulPublish({ PATH: `${bin}:${process.env.PATH}` });
    const competing = await readFile(raceMarker, 'utf8');
    const final = pages();
    assert.equal(git('--git-dir', remote, 'rev-parse', `${final}^`), competing);
    git('--git-dir', remote, 'merge-base', '--is-ancestor', second, final);
    assert.equal(refs(), `refs/heads/gh-pages ${final}`);

    const forbidden = publish({ PUBLISH_REMOTE: 'https://github.com/example/koplik.git' });
    assert.notEqual(forbidden.status, 0);
    assert.match(forbidden.stderr, /local bare repository/);
    assert.deepEqual(await snapshot(), before);
  } finally {
    await rm(scratch, { recursive: true, force: true });
  }
});
