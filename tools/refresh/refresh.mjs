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
  !key.startsWith('GIT_') && !['KOPLIK_ALLOW_NETWORK_TESTS', 'KOPLIK_CENSUS_CONTACT', 'KOPLIK_ENV_LOCAL', 'PUBLISH_REMOTE', 'PUBLISH_DRY_RUN', 'PUBLISH_PREPARE_OUTPUT', 'PUBLISH_EXPECTED_PARENT'].includes(key)));

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
    // The operator's contact config belongs only to live ingest. QA CLI tests
    // provide their own temporary config; inheriting this override both defeats
    // that isolation and can turn a contact-refusal test into a network attempt.
    delete runtime.KOPLIK_ENV_LOCAL;
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
    const gates = ['check', 'test', 'web-test', 'determinism'];
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
    phase = 'qa'; console.log(`Refresh ${runId}: full QA`);
    if (qa) await qa({ work, env: runtime, gates });
    else for (const gate of gates) {
      phase = `qa-${gate}`;
      console.log(`Refresh ${runId}: ${phase}`);
      await execute('make', [gate], work, runtime);
    }
    // Re-run reproducibility on the exact committed candidate, including its QA receipt.
    phase = 'reproducibility';
    for (const n of [3, 4]) {
      const out = join(work, 'target', `refresh-out-${n}`);
      await build({ root: work, work: join(work, 'target', `refresh-work-${n}`), out,
        run: (program, args, cwd, childEnv) => execute(join(work, 'tools/offline-test.sh'), [program, ...args], cwd, childEnv), env: runtime });
      if (JSON.stringify(await hashes(out)) !== JSON.stringify(receipt.output_hashes)) throw new Error('Candidate pipeline changed');
    }
    phase = 'scans';
    await scan(work, patterns, (program, args, cwd) => execute(program, args, cwd));
    phase = 'rolling-race';
    await git(['fetch', 'origin']);
    if (await git(['rev-parse', 'origin/rolling']) !== base) throw new Error('Rolling advanced; skip and rerun QA on new tip');
    // Fail before any push if main cannot be fast-forwarded to the candidate.
    await git(['merge-base', '--is-ancestor', 'origin/main', sha]);
    const pagesRef = await git(['for-each-ref', '--format=%(objectname)', 'refs/remotes/origin/gh-pages']);
    phase = 'publish-prepare';
    const output = join(state, runId, 'publication.json');
    await execute(join(work, 'tools/offline-test.sh'), ['make', 'publish'], work, {
      ...runtime, PUBLISH_REMOTE: 'origin', PUBLISH_DRY_RUN: '1',
      PUBLISH_PREPARE_OUTPUT: output, PUBLISH_EXPECTED_PARENT: pagesRef || 'root',
      TMPDIR: join(work, 'target'),
    });
    phase = 'publication-validate';
    const publication = JSON.parse(await readFile(output, 'utf8'));
    if (publication.source !== sha || publication.parent !== (pagesRef || null)
      || !/^[a-f0-9]{40,64}$/.test(publication.commit)) throw new Error('Invalid prepared publication');
    const g = publication.commit;
    const parents = await git(['rev-list', '--parents', '-n', '1', g]);
    if (parents !== [g, pagesRef].filter(Boolean).join(' ')) throw new Error('Unexpected publication parent');
    if (await git(['rev-parse', `${g}^{tree}`]) !== publication.tree) throw new Error('Unexpected publication tree');
    const sitePaths = (await git(['ls-tree', '-r', '--name-only', g])).split('\n');
    const required = ['data/manifest.json', ...['coverage', 'geographies', 'rt', 'texas-counties', 'us-states', 'weekly-cases'].map((name) => `data/v6/${name}.json`)];
    if (required.some((path) => !sitePaths.includes(path))) throw new Error('Prepared site is missing v6 artifacts');
    const input = JSON.parse(await git(['show', `${g}:data/publication.json`]));
    if (input.source !== 'data/release' || input.pipeline_manifest_sha256 !== new Map(receipt.output_hashes).get('manifest.json')) {
      throw new Error('Prepared site did not use candidate release data');
    }
    if (await git(['status', '--porcelain=v1', '--untracked-files=all'])) throw new Error('Candidate changed during QA');
    if (dryRun) { console.log(`Refresh dry-run green ${sha}, Pages ${g}; no push or publication`); return { sha, pages: g, work, receipt }; }
    phase = 'atomic-release';
    // All local preparation has succeeded. One FF-only transaction changes all three
    // refs, or none on rejection. No retry/reparent: a later run starts from new tips.
    await git(['push', '--atomic', 'origin', `${sha}:refs/heads/rolling`, `${sha}:refs/heads/main`, `${g}:refs/heads/gh-pages`], { KOPLIK_PROMOTE_MAIN: '1' });
    phase = 'release-verify';
    const advertised = (await git(['ls-remote', '--heads', 'origin', 'refs/heads/rolling', 'refs/heads/main', 'refs/heads/gh-pages']))
      .split('\n').map((line) => line.split(/\s+/));
    const refs = new Map(advertised.map(([value, ref]) => [ref, value]));
    if (refs.get('refs/heads/rolling') !== sha || refs.get('refs/heads/main') !== sha || refs.get('refs/heads/gh-pages') !== g) {
      throw new Error('Released refs differ from prepared transaction');
    }
    console.log(`Refresh green ${sha}`);
    return { sha, pages: g, work, receipt };
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
