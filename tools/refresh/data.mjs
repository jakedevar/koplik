import { StringDecoder } from 'node:string_decoder';
import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { cp, mkdir, readFile, readdir, writeFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { homedir } from 'node:os';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

export const digest = (bytes) => createHash('sha256').update(bytes).digest('hex');
export async function files(root, prefix = '') {
  const result = [];
  for (const entry of await readdir(join(root, prefix), { withFileTypes: true })) {
    const path = prefix ? `${prefix}/${entry.name}` : entry.name;
    if (entry.isSymbolicLink()) throw new Error('Symlinks are forbidden in release data');
    if (entry.isDirectory()) result.push(...await files(root, path));
    else if (entry.isFile()) result.push(path);
    else throw new Error('Nonregular release data');
  }
  return result.sort();
}

export async function command(program, args, cwd, env = process.env) {
  // Suppress command output: live error messages can include private contact/URL text.
  return new Promise((accept, reject) => {
    const child = spawn(program, args, { cwd, env, stdio: ['ignore', 'pipe', 'pipe'] });
    let out = '';
    let tail = '';
    const capture = (bytes) => {
      tail = (tail + bytes).split('\n').slice(-201).join('\n');
    };
    const stdout = new StringDecoder('utf8');
    const stderr = new StringDecoder('utf8');
    child.stdout.on('data', (bytes) => { const text = stdout.write(bytes); out += text; capture(text); });
    child.stderr.on('data', (bytes) => capture(stderr.write(bytes)));
    child.stdout.on('end', () => { const text = stdout.end(); out += text; capture(text); });
    child.stderr.on('end', () => capture(stderr.end()));
    child.on('error', () => reject(new Error(`Command unavailable: ${program}`)));
    child.on('close', (code) => {
      if (code === 0) return accept(out.trim());
      const error = new Error(`Command failed (exit ${code})`);
      // Private until the refresh boundary redacts it; never log this property.
      error.commandTail = tail.replace(/\n$/, '').split('\n').slice(-200).join('\n');
      reject(error);
    });
  });
}

export async function cargo(args, root, env = process.env) {
  const governor = join(homedir(), '.rsi/bin/cargo-slot');
  return command(existsSync(governor) ? governor : 'cargo',
    existsSync(governor) ? ['cargo', ...args] : args, root, env);
}

export async function verifyStore(store) {
  const log = await readFile(join(store, 'retrievals.jsonl'), 'utf8');
  const records = log.trim().split('\n').map((line) => JSON.parse(line));
  if (!records.length) throw new Error('Empty release store');
  for (const record of records) {
    if (!/^[a-f0-9]{64}$/.test(record.sha256)) throw new Error('Invalid snapshot digest');
    const bytes = await readFile(join(store, 'blobs', record.sha256.slice(0, 2), record.sha256));
    if (digest(bytes) !== record.sha256 || bytes.length !== record.bytes) throw new Error('Corrupt release snapshot');
  }
  return records;
}

// Release snapshots retain the ingest manifest which the remaining offline stages consume.
// Using its recorded mode preserves the initial fixture publication byte for byte.
export async function buildData({ root, work, out, run = command, env = process.env }) {
  const release = join(root, 'data/release');
  const source = existsSync(release) ? 'data/release' : 'data/fixtures';
  const target = resolve(root, env.CARGO_TARGET_DIR || 'target');
  const buildEnv = { ...env, CARGO_TARGET_DIR: target, KOPLIK_CONTACT: '' };
  const governor = join(homedir(), '.rsi/bin/cargo-slot');
  await run(existsSync(governor) ? governor : 'cargo',
    [...(existsSync(governor) ? ['cargo'] : []), 'build', '--offline', '--locked', '--release', '-p', 'koplik-pipeline'], root, buildEnv);
  const binary = join(target, 'release/koplik-pipeline');
  if (source === 'data/release') {
    await verifyStore(release);
    const manifest = JSON.parse(await readFile(join(release, 'ingest.manifest.json'), 'utf8'));
    if (!['fixtures', 'live'].includes(manifest.mode)) throw new Error('Invalid release mode');
    await mkdir(work, { recursive: true });
    await cp(join(release, 'ingest.manifest.json'), join(work, 'ingest.manifest.json'));
    for (const stage of ['validate', 'infer', 'forecast', 'build']) {
      await run(binary, [stage, ...(manifest.mode === 'fixtures' ? ['--from-fixtures'] : []),
        '--store', release, '--work', work, '--out', out], root, buildEnv);
    }
  } else {
    await run(binary, ['all', '--from-fixtures', '--fixtures', join(root, source), '--work', work, '--out', out], root, buildEnv);
  }
  // Separate publication manifest keeps all established pipeline artifacts, including their
  // manifest, byte-identical while explicitly naming the selected publication input.
  await writeFile(join(out, 'publication.json'), `${JSON.stringify({ source,
    pipeline_manifest_sha256: digest(await readFile(join(out, 'manifest.json'))) })}\n`);
  return source;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const [root, work, out] = process.argv.slice(2);
  buildData({ root: resolve(root || '.'), work: resolve(work || 'data/pipeline'), out: resolve(out || 'web/public/data') })
    .then((source) => console.log(`Offline publication input: ${source}`))
    .catch((error) => { console.error(error.message); process.exitCode = 1; });
}
