// Test-only stores are written by real offline fixture ingest, never copied from
// mutable publication data. Live-mode metadata is an explicit test simulation;
// every raw byte, URL, digest and retrieval time still comes from fixtures.
import { appendFile, cp, mkdir, readFile, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { cargo, command } from './data.mjs';

export const project = fileURLToPath(new URL('../../', import.meta.url));
let pipeline;
export function fixturePipeline() {
  pipeline ??= (async () => {
    const target = resolve(project, process.env.CARGO_TARGET_DIR || 'target');
    const env = { ...process.env, CARGO_TARGET_DIR: target, CARGO_NET_OFFLINE: 'true', KOPLIK_CONTACT: '' };
    await cargo(['build', '--offline', '--locked', '--release', '-p', 'koplik-pipeline'], project, env);
    return { binary: join(target, 'release/koplik-pipeline'), env,
      // Temporary data-only roots have no Rust source. Execute the already-built
      // real binary there; compiler artifacts remain owned by the project tree.
      run: (program, args, cwd, childEnv) => args.includes('build') && args.includes('-p')
        ? Promise.resolve('') : command(program, args, cwd, childEnv) };
  })();
  return pipeline;
}

export async function seedFixtureStore({ store, work, mode = 'fixtures' }) {
  if (!['fixtures', 'live'].includes(mode)) throw new Error('Invalid test store mode');
  const { binary, env } = await fixturePipeline();
  await command(binary, ['ingest', '--from-fixtures', '--fixtures', join(project, 'data/fixtures'),
    '--store', store, '--work', work], project, env);
  const path = join(work, 'ingest.manifest.json');
  if (mode === 'live') {
    // Manager-authorized offline live candidate: only orchestration metadata
    // changes. The immutable fixture snapshots and real receipts are untouched.
    // Fixture ingest is idempotent. Explicitly simulate a live re-retrieval
    // by replaying its exact receipts in this temporary test store. No raw
    // bytes, URLs, hashes or retrieval times are invented or modified.
    await appendFile(join(store, 'retrievals.jsonl'), await readFile(join(store, 'retrievals.jsonl')));
    const manifest = JSON.parse(await readFile(path, 'utf8'));
    manifest.mode = 'live';
    manifest.notes.push('Offline-prepared live-mode test candidate; recorded fixture bytes');
    await writeFile(path, `${JSON.stringify(manifest)}\n`);
  }
  await mkdir(store, { recursive: true });
  await cp(path, join(store, 'ingest.manifest.json'));
}
