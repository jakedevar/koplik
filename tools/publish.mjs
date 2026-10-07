import { spawnSync } from 'node:child_process';
import { cp, mkdir, mkdtemp, realpath, rm, writeFile } from 'node:fs/promises';
import { constants, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { basename, join, resolve } from 'node:path';

// Do not let an inherited repository/index override escape the isolated workspace.
const environment = { ...process.env };
for (const key of ['GIT_DIR', 'GIT_WORK_TREE', 'GIT_COMMON_DIR', 'GIT_INDEX_FILE', 'GIT_OBJECT_DIRECTORY', 'GIT_ALTERNATE_OBJECT_DIRECTORIES']) {
  delete environment[key];
}

function run(command, args, cwd, options = {}) {
  const result = spawnSync(command, args, {
    cwd, env: environment, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'], ...options,
  });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(' ')} failed (${result.status})\n${result.stdout || ''}${result.stderr || ''}`);
  }
  return (result.stdout || '').trim();
}

const root = run('git', ['rev-parse', '--show-toplevel'], process.cwd());
const remote = process.env.PUBLISH_REMOTE || 'origin';
const dryRun = process.env.PUBLISH_DRY_RUN || '0';
if (!['0', '1'].includes(dryRun)) throw new Error('PUBLISH_DRY_RUN must be 0 or 1');

// origin is an operator-owned local bare repository; never contact GitHub directly.
// Overrides are local bare paths, so offline tests cannot accidentally use origin.
const urls = remote === 'origin'
  ? run('git', ['remote', 'get-url', '--push', '--all', 'origin'], root).split('\n')
  : [remote];
if (urls.length !== 1 || !existsSync(resolve(root, urls[0]))) {
  throw new Error('Publishing requires exactly one local bare repository, not a network URL');
}
const target = await realpath(resolve(root, urls[0]));
if (run('git', ['rev-parse', '--is-bare-repository'], target) !== 'true') {
  throw new Error('Publishing requires a bare repository');
}

const source = run('git', ['rev-parse', 'HEAD'], root);
const branch = run('git', ['rev-parse', '--abbrev-ref', 'HEAD'], root);
const session = process.env.RSI_SESSION_ID || (branch.startsWith('rsi/') ? branch.slice(4) : '');
const identity = run('git', ['var', 'GIT_AUTHOR_IDENT'], root).match(/^(.*) <([^>]+)> \d+ [+-]\d+$/);
if (!identity) throw new Error('Configure a Git author before publishing');
const scratch = await mkdtemp(join(tmpdir(), 'koplik-publish-'));
try {
  const build = join(scratch, 'build');
  await mkdir(build);
  // Copy current sources and pipeline artifacts without building in the caller's tree.
  for (const path of ['web', 'crates', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'Makefile']) {
    await cp(join(root, path), join(build, path), {
      recursive: true,
      filter: (path) => !['node_modules', 'dist', 'test-results', 'playwright-report'].includes(basename(path)),
    });
  }
  await mkdir(join(build, 'tools/wasm'), { recursive: true });
  await cp(join(root, 'tools/wasm/lock-version.mjs'), join(build, 'tools/wasm/lock-version.mjs'));
  if (existsSync(join(root, 'pkg/web/koplik_wasm.js'))) {
    await cp(join(root, 'pkg/web'), join(build, 'pkg/web'), { recursive: true });
  } else {
    console.log('Building WASM in the temporary workspace');
    run('make', ['wasm'], build, {
      stdio: 'inherit', env: { ...environment, CARGO_TARGET_DIR: join(build, 'target') },
    });
  }
  const web = join(build, 'web');
  if (existsSync(join(root, 'web/node_modules/.package-lock.json'))) {
    await cp(await realpath(join(root, 'web/node_modules')), join(web, 'node_modules'), {
      recursive: true, mode: constants.COPYFILE_FICLONE,
    });
  } else {
    run('npm', ['ci', '--no-audit', '--no-fund'], web, { stdio: 'inherit' });
  }
  run('npm', ['run', 'build'], web, {
    stdio: 'inherit', env: { ...environment, KOPLIK_BASE_PATH: '/koplik/' },
  });
  const dist = join(web, 'dist');
  await writeFile(join(dist, '.nojekyll'), '');

  // An isolated repository and index keep all caller refs, FETCH_HEAD and index intact.
  const gitDir = join(scratch, 'publish.git');
  run('git', ['init', '--bare', gitDir], scratch);
  const git = (args) => run('git', ['--git-dir', gitDir, ...args], scratch);
  git(['config', 'user.name', identity[1]]);
  git(['config', 'user.email', identity[2]]);
  git(['--work-tree', dist, '-c', 'core.excludesFile=/dev/null', 'add', '--', '.']);
  const tree = git(['write-tree']);
  const message = `Publish Koplik from ${source}${session ? `\n\nRsi-Session: ${session}` : ''}`;

  for (let attempt = 1; attempt <= 3; attempt++) {
    const advertised = git(['ls-remote', '--heads', target, 'refs/heads/gh-pages']);
    let parent;
    if (advertised) {
      git(['fetch', '--no-tags', target, 'refs/heads/gh-pages']);
      parent = git(['rev-parse', 'FETCH_HEAD']);
    }
    const commit = git(['commit-tree', tree, ...(parent ? ['-p', parent] : []), '-m', message]);
    git(['update-ref', 'HEAD', commit]);
    console.log(`${dryRun === '1' ? 'Would push' : 'Publishing'} ${commit} to ${target} HEAD:refs/heads/gh-pages (parent ${parent || 'root'})`);
    if (dryRun === '1') break;
    try {
      git(['push', target, 'HEAD:refs/heads/gh-pages']);
      console.log(`Published ${commit}`);
      break;
    } catch (error) {
      if (attempt === 3) throw error;
      // A concurrent publisher may have advanced the branch: fetch, reparent, retry.
      console.error(`Push rejected; refetching gh-pages before retry ${attempt + 1}`);
    }
  }
} finally {
  await rm(scratch, { recursive: true, force: true });
}
