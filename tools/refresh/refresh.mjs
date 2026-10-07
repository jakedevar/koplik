#!/usr/bin/env node
import { cp, mkdir, readFile, realpath, rm, writeFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { homedir } from 'node:os';
import { join, resolve } from 'node:path';
import { randomUUID } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { buildData, cargo, command, digest, files, verifyStore } from './data.mjs';
import { scanHistory } from './scans.mjs';

const gitEnvironment = (env) => Object.fromEntries(Object.entries(env).filter(([key]) =>
  !key.startsWith('GIT_') && !['KOPLIK_ALLOW_NETWORK_TESTS', 'KOPLIK_CENSUS_CONTACT', 'PUBLISH_REMOTE', 'PUBLISH_DRY_RUN'].includes(key)));

export async function pathGuard(root, run = command) {
  const names = new Set();
  for (const args of [
    ['diff', '--name-only', '--no-renames', '-z'],
    ['diff', '--cached', '--name-only', '--no-renames', '-z'],
    ['ls-files', '--others', '--exclude-standard', '-z'],
  ]) for (const path of (await run('git', args, root)).split('\0').filter(Boolean)) names.add(path);
  if ([...names].some((path) => !path.startsWith('data/release/') || path.includes('/../'))) {
    throw new Error('Data-only path guard failed');
  }
  for (const path of await files(join(root, 'data/release'))) {
    if (!/^(blobs\/[a-f0-9]{2}\/[a-f0-9]{64}|retrievals\.jsonl|ingest\.manifest\.json|qa\/[a-zA-Z0-9._-]+\.json)$/.test(path)) {
      throw new Error('Unexpected release path');
    }
  }
  return [...names].sort();
}

export function ingestGreen(previous, current) {
  const required = Object.entries(previous.items).filter(([, item]) => item.status === 'present').map(([id]) => id);
  if (!required.length || required.some((id) => current.items[id]?.status !== 'present')
    || Object.values(current.items).some((item) => item.status === 'missing' || (item.gaps || 0) > 0)
    || current.notes.some((note) => note.includes('fetch failed'))) throw new Error('Ingest incomplete');
}

export async function failure({ state, runId, phase, run = command, env = process.env }) {
  // Phase identifiers are controlled by this script, never child output or private input.
  const body = `Weekly refresh ${runId} failed at ${phase}.\nNo further promotion or publication was attempted.\nInspect the refresh journal and dedicated worktree; rerun full QA before retrying.\n`;
  await mkdir(state, { recursive: true });
  const artifact = join(state, `FAILED-${runId}.md`);
  if (env.RSI_SESSION_TOKEN) {
    const params = join(state, `failure-${runId}.json`);
    await writeFile(params, JSON.stringify({ title: `Weekly refresh failed: ${phase}`, body, labels: ['refresh-failure'], idempotency_key: `koplik-refresh-${runId}` }), { mode: 0o600 });
    try { await run('rsi-rpc', ['AgentCreateIssue', '--params', `@${params}`], state, env); return; }
    catch { /* A refused/unavailable Issue endpoint still leaves a durable local report. */ }
    finally { await rm(params, { force: true }); }
  }
  await writeFile(artifact, body, { mode: 0o600 });
  try { await run('notify-send', ['Koplik refresh failed', `Phase: ${phase}. See the refresh journal.`], state, env); } catch { /* headless hub */ }
}

export async function refresh({ shared = join(homedir(), 'koplik'), state = join(homedir(), '.rsi/koplik-refresh'),
  patterns = join(homedir(), '.config/koplik/pii-patterns'), run = command,
  ingest, qa, build = buildData, scan = scanHistory, dryRun = false, env = process.env } = {}) {
  const runId = `${new Date().toISOString().slice(0, 10)}-${randomUUID()}`;
  let phase = 'setup';
  let locked = false;
  let work;
  const safeEnv = gitEnvironment(env);
  const execute = (program, args, cwd, extra = {}) => run(program, args, cwd, { ...safeEnv, ...extra });
  try {
    state = resolve(state);
    shared = await realpath(shared);
    if (state.includes('/sandboxes/') || state === shared || state.startsWith(`${shared}/`)) throw new Error('Refresh requires a dedicated directory');
    await mkdir(state, { recursive: true, mode: 0o700 });
    state = await realpath(state);
    if (state.includes('/sandboxes/') || state === shared || state.startsWith(`${shared}/`)) throw new Error('Refresh requires a dedicated directory');
    await mkdir(join(state, 'lock')); locked = true;
    // A fresh detached clone per run avoids deleting or resetting any previous failed work.
    work = join(state, runId, 'worktree');
    await mkdir(join(state, runId));
    const urls = (await execute('git', ['remote', 'get-url', '--push', '--all', 'origin'], shared)).split('\n');
    if (urls.length !== 1) throw new Error('Exactly one local origin required');
    const origin = await realpath(resolve(shared, urls[0]));
    if (await execute('git', ['rev-parse', '--is-bare-repository'], origin) !== 'true') throw new Error('Local bare origin required');
    await execute('git', ['clone', '--no-local', '--no-checkout', origin, work], state);
    await execute('git', ['checkout', '--detach', 'origin/rolling'], work);
    const git = (args, extra = {}) => execute('git', args, work, extra);
    if (await git(['status', '--porcelain=v1', '--untracked-files=all'])) throw new Error('Worktree is dirty');
    const base = await git(['rev-parse', 'HEAD']);
    await git(['config', 'user.name', 'Koplik refresh']);
    await git(['config', 'user.email', 'refresh@example.invalid']);
    const release = join(work, 'data/release');
    const before = new Map();
    for (const path of await files(release)) before.set(path, await readFile(join(release, path)));
    const previous = JSON.parse(before.get('ingest.manifest.json'));
    const runtime = { ...safeEnv, CARGO_TARGET_DIR: join(work, 'target'), KOPLIK_ENV_LOCAL: join(shared, '.env.local') };
    // Refuse a missing operator config rather than silently fall back to a general Census contact.
    if (!existsSync(runtime.KOPLIK_ENV_LOCAL)) throw new Error('Shared Census config required');
    phase = 'ingest'; console.log(`Refresh ${runId}: ingest`);
    const stageWork = join(work, 'data/pipeline');
    if (ingest) await ingest({ work, release, stageWork, env: runtime });
    else await cargo(['run', '--locked', '--release', '-p', 'koplik-pipeline', '--', 'ingest', '--store', release, '--work', stageWork], work, runtime);
    const current = JSON.parse(await readFile(join(stageWork, 'ingest.manifest.json'), 'utf8'));
    ingestGreen(previous, current);
    await cp(join(stageWork, 'ingest.manifest.json'), join(release, 'ingest.manifest.json'));
    phase = 'integrity';
    for (const [path, bytes] of before) {
      const after = await readFile(join(release, path));
      if (path === 'ingest.manifest.json') continue;
      if (path === 'retrievals.jsonl' ? !after.subarray(0, bytes.length).equals(bytes) : !after.equals(bytes)) throw new Error('Existing release data changed');
    }
    await verifyStore(release);
    // SnapshotStore's tmp directory must be empty; it is not a publication artifact.
    await rm(join(release, 'tmp'), { recursive: true, force: true });
    await pathGuard(work, (program, args, cwd) => execute(program, args, cwd));
    phase = 'qa'; console.log(`Refresh ${runId}: full QA`);
    const gates = ['check', 'test', 'web-test', 'determinism'];
    if (qa) await qa({ work, env: runtime, gates });
    else for (const gate of gates) await execute('make', [gate], work, runtime);
    phase = 'reproducibility';
    const hashes = async (out) => Promise.all((await files(out)).map(async (path) => [path, digest(await readFile(join(out, path)))]));
    const builds = [];
    for (const n of [1, 2]) {
      const out = join(work, 'target', `refresh-out-${n}`);
      await build({ root: work, work: join(work, 'target', `refresh-work-${n}`), out,
        run: (program, args, cwd, childEnv) => execute(join(work, 'tools/offline-test.sh'), [program, ...args], cwd, childEnv), env: runtime });
      builds.push(await hashes(out));
    }
    if (JSON.stringify(builds[0]) !== JSON.stringify(builds[1])) throw new Error('Pipeline nondeterministic');
    phase = 'scans';
    const scans = await scan(work, patterns, (program, args, cwd) => execute(program, args, cwd));
    const receipt = { run_id: runId, base_sha: base, gates, output_hashes: builds[0], scans };
    await mkdir(join(release, 'qa'), { recursive: true });
    await writeFile(join(release, 'qa', `${runId}.json`), `${JSON.stringify(receipt)}\n`);
    phase = 'commit';
    const paths = await pathGuard(work, (program, args, cwd) => execute(program, args, cwd));
    await git(['add', '--', ...paths]);
    await git(['commit', '-m', `data: weekly refresh ${runId.slice(0, 10)}`, '--trailer', `Koplik-Refresh: ${runId}`]);
    const sha = await git(['rev-parse', 'HEAD']);
    const committed = (await git(['diff', '--name-only', '--no-renames', '-z', base, sha])).split('\0').filter(Boolean);
    if (committed.some((path) => !path.startsWith('data/release/'))) throw new Error('Committed path guard failed');
    await scan(work, patterns, (program, args, cwd) => execute(program, args, cwd));
    phase = 'publish-dry-run';
    const preview = join(state, runId, 'preview.git');
    await execute('git', ['init', '--bare', preview], state);
    await execute('make', ['publish'], work, { ...runtime, PUBLISH_REMOTE: preview, PUBLISH_DRY_RUN: '1' });
    if (await execute('git', ['for-each-ref', '--format=%(refname)'], preview)) throw new Error('Publish dry-run changed refs');
    phase = 'rolling-race';
    await git(['fetch', 'origin']);
    if (await git(['rev-parse', 'origin/rolling']) !== base) throw new Error('Rolling advanced; skip and rerun QA on new tip');
    // Fail before any push if main cannot be fast-forwarded to the candidate.
    await git(['merge-base', '--is-ancestor', 'origin/main', sha]);
    if (dryRun) { console.log(`Refresh dry-run green ${sha}; no push or publication`); return { sha, work, receipt }; }
    phase = 'promotion';
    // Atomic FF-only update avoids promoting just one branch if either ref races/refuses.
    await git(['push', '--atomic', 'origin', `${sha}:refs/heads/rolling`, `${sha}:refs/heads/main`], { KOPLIK_PROMOTE_MAIN: '1' });
    phase = 'publication';
    await execute('make', ['publish'], work, { ...runtime, PUBLISH_REMOTE: 'origin', PUBLISH_DRY_RUN: '0' });
    console.log(`Refresh green ${sha}`);
    return { sha, work, receipt };
  } catch (error) {
    console.error(`Refresh failed at ${phase}; command details suppressed`);
    await failure({ state, runId, phase, run, env: safeEnv });
    throw new Error(`Refresh failed at ${phase}`);
  } finally {
    if (locked) await rm(join(state, 'lock'), { recursive: true });
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  refresh({ shared: process.env.KOPLIK_REFRESH_SHARED, state: process.env.KOPLIK_REFRESH_STATE,
    patterns: process.env.KOPLIK_PII_PATTERNS, dryRun: process.argv.includes('--dry-run') })
    .catch(() => { process.exitCode = 1; });
}
