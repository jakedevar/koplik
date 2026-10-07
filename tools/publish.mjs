import { spawn } from 'node:child_process';
import { cp, mkdir, mkdtemp, readdir, realpath, rm, writeFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { homedir, tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

// Do not let an inherited repository/index override escape the isolated workspace.
const environment = { ...process.env };
for (const key of ['GIT_DIR', 'GIT_WORK_TREE', 'GIT_COMMON_DIR', 'GIT_INDEX_FILE', 'GIT_OBJECT_DIRECTORY', 'GIT_ALTERNATE_OBJECT_DIRECTORIES']) {
  delete environment[key];
}
const children = new Set();
let interrupted;

async function run(command, args, cwd, options = {}) {
  if (interrupted) throw new Error(`Publishing interrupted by ${interrupted}`);
  return new Promise((accept, reject) => {
    // Each command owns a process group, including make/cargo/npm descendants.
    const child = spawn(command, args, {
      cwd, env: environment, detached: true, stdio: ['ignore', 'pipe', 'pipe'], ...options,
    });
    children.add(child);
    let stdout = '';
    let stderr = '';
    child.stdout?.on('data', (chunk) => { stdout += chunk; });
    child.stderr?.on('data', (chunk) => { stderr += chunk; });
    child.on('error', reject);
    child.on('close', (code, signal) => {
      children.delete(child);
      if (interrupted || code !== 0) {
        reject(new Error(`${command} ${args.join(' ')} failed (${interrupted || signal || code})\n${stdout}${stderr}`));
      } else accept(stdout.trim());
    });
  });
}

// Keep Git publication independently testable using one real built site.
export async function publishSite({ scratch, dist, target, source, identity, session = '', dryRun = '0', env = environment }) {
  const gitDir = join(scratch, 'publish.git');
  await run('git', ['init', '--bare', gitDir], scratch, { env });
  const git = (args) => run('git', ['--git-dir', gitDir, ...args], scratch, { env });
  await git(['config', 'user.name', identity[1]]);
  await git(['config', 'user.email', identity[2]]);
  await git(['--work-tree', dist, '-c', 'core.excludesFile=/dev/null', 'add', '--', '.']);
  const tree = await git(['write-tree']);
  const message = `Publish Koplik from ${source}${session ? `\n\nRsi-Session: ${session}` : ''}`;

  for (let attempt = 1; attempt <= 3; attempt++) {
    const advertised = await git(['ls-remote', '--heads', target, 'refs/heads/gh-pages']);
    let parent;
    if (advertised) {
      await git(['fetch', '--no-tags', target, 'refs/heads/gh-pages']);
      parent = await git(['rev-parse', 'FETCH_HEAD']);
    }
    const commit = await git(['commit-tree', tree, ...(parent ? ['-p', parent] : []), '-m', message]);
    await git(['update-ref', 'HEAD', commit]);
    const description = `${dryRun === '1' ? 'Would push' : 'Publishing'} ${commit} to ${target} HEAD:refs/heads/gh-pages (parent ${parent || 'root'})`;
    console.log(description);
    if (dryRun === '1') return description;
    try {
      await git(['push', target, 'HEAD:refs/heads/gh-pages']);
      console.log(`Published ${commit}`);
      return description;
    } catch (error) {
      if (interrupted || attempt === 3) throw error;
      // A concurrent publisher may have advanced the branch: fetch, reparent, retry.
      console.error(`Push rejected; refetching gh-pages before retry ${attempt + 1}`);
    }
  }
}

async function main() {
  const root = await run('git', ['rev-parse', '--show-toplevel'], process.cwd());
  const remote = process.env.PUBLISH_REMOTE || 'origin';
  const dryRun = process.env.PUBLISH_DRY_RUN || '0';
  if (!['0', '1'].includes(dryRun)) throw new Error('PUBLISH_DRY_RUN must be 0 or 1');

  // origin is operator-owned and local. Overrides cannot contact GitHub directly.
  const urls = remote === 'origin'
    ? (await run('git', ['remote', 'get-url', '--push', '--all', 'origin'], root)).split('\n')
    : [remote];
  if (urls.length !== 1 || !existsSync(resolve(root, urls[0]))) {
    throw new Error('Publishing requires exactly one local bare repository, not a network URL');
  }
  const target = await realpath(resolve(root, urls[0]));
  if (await run('git', ['rev-parse', '--is-bare-repository'], target) !== 'true') {
    throw new Error('Publishing requires a bare repository');
  }
  const source = await run('git', ['rev-parse', 'HEAD'], root);
  if (await run('git', ['status', '--porcelain=v1', '--untracked-files=all'], root)) {
    console.error(`Warning: local edits are not published; publishing ${source}`);
  }
  const branch = await run('git', ['rev-parse', '--abbrev-ref', 'HEAD'], root);
  const session = process.env.RSI_SESSION_ID || (branch.startsWith('rsi/') ? branch.slice(4) : '');
  const identity = (await run('git', ['var', 'GIT_AUTHOR_IDENT'], root)).match(/^(.*) <([^>]+)> \d+ [+-]\d+$/);
  if (!identity) throw new Error('Configure a Git author before publishing');
  let scratch;
  const interrupt = (signal) => {
    interrupted = signal;
    for (const child of children) {
      if (!child.pid) continue;
      try { process.kill(-child.pid, 'SIGTERM'); }
      catch (error) { if (error.code !== 'ESRCH') throw error; }
    }
  };
  const onInterrupt = () => interrupt('SIGINT');
  const onTerminate = () => interrupt('SIGTERM');
  process.on('SIGINT', onInterrupt);
  process.on('SIGTERM', onTerminate);
  try {
    scratch = await mkdtemp(join(tmpdir(), 'koplik-publish-'));
    const build = join(scratch, 'build');
    await mkdir(build);
    // Pin the archive to the recorded SHA, even if HEAD/worktree changes later.
    const archive = join(scratch, 'source.tar');
    await run('git', ['archive', '--format=tar', `--output=${archive}`, source], root, { stdio: 'inherit' });
    await run('tar', ['-xf', archive, '-C', build], root);

    // Reuse only the compiler tool (version-checked by make), never pkg/web or cargo artifacts.
    const cli = join(root, 'target/tools/bin/wasm-bindgen');
    if (existsSync(cli)) {
      await mkdir(join(build, 'target/tools/bin'), { recursive: true });
      await cp(cli, join(build, 'target/tools/bin/wasm-bindgen'));
    }
    const web = join(build, 'web');
    const data = join(web, 'public/data');
    // Rebuild even if generated data was committed; only this pipeline's output may ship.
    await rm(data, { recursive: true, force: true, maxRetries: 3 });
    const governor = join(homedir(), '.rsi/bin/cargo-slot');
    const pipelineArgs = ['run', '--offline', '--locked', '--release', '-p', 'koplik-pipeline', '--',
      'all', '--from-fixtures', '--fixtures', join(build, 'data/fixtures'),
      '--work', join(scratch, 'work'), '--out', data];
    console.log(`Building offline fixture data from ${source} in the temporary workspace`);
    await run(existsSync(governor) ? governor : 'cargo',
      existsSync(governor) ? ['cargo', ...pipelineArgs] : pipelineArgs, build, {
        stdio: 'inherit', env: { ...environment, KOPLIK_CONTACT: '',
          CARGO_TARGET_DIR: environment.CARGO_TARGET_DIR
            ? resolve(root, environment.CARGO_TARGET_DIR) : join(build, 'target') },
      });
    if (!existsSync(join(data, 'manifest.json'))) {
      throw new Error('Offline pipeline produced no web/public/data/manifest.json; refusing publication');
    }
    console.log(`Building WASM from ${source} in the temporary workspace`);
    await run('make', ['wasm'], build, {
      stdio: 'inherit', env: { ...environment, CARGO_TARGET_DIR: join(build, 'target') },
    });
    // Install the archived lockfile; caller node_modules may also be stale or modified.
    await run('npm', ['ci', '--no-audit', '--no-fund'], web, { stdio: 'inherit' });
    await run('npm', ['run', 'build'], web, {
      stdio: 'inherit', env: { ...environment, KOPLIK_BASE_PATH: '/koplik/' },
    });
    const dist = join(web, 'dist');
    if (!existsSync(join(dist, 'data/manifest.json'))
      || !(await readdir(join(dist, 'data/v1')).catch(() => [])).some((name) => name.endsWith('.json'))) {
      throw new Error('Built site is missing data/manifest.json or data/v1/*.json; refusing publication');
    }
    await writeFile(join(dist, '.nojekyll'), '');
    await publishSite({ scratch, dist, target, source, identity, session, dryRun });
  } finally {
    if (scratch) await rm(scratch, { recursive: true, force: true, maxRetries: 3 });
    process.off('SIGINT', onInterrupt);
    process.off('SIGTERM', onTerminate);
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().catch((error) => {
    console.error(error.message);
    process.exitCode = interrupted === 'SIGINT' ? 130 : interrupted === 'SIGTERM' ? 143 : 1;
  });
}
