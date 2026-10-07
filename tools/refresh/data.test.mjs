import assert from 'node:assert/strict';
import { cp, mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import { buildData, cargo, command, files, verifyStore } from './data.mjs';

const project = fileURLToPath(new URL('../../', import.meta.url));
test('seeded release reproduces all fixture pipeline bytes; fallback is only on absence and corrupt release refuses', async () => {
  const scratch = await mkdtemp(join(tmpdir(), 'koplik-release-data-test-'));
  // One source tree owns this compiler target; temporary roots below contain only data.
  const target = resolve(project, process.env.CARGO_TARGET_DIR || 'target');
  try {
    await cargo(['build', '--offline', '--locked', '--release', '-p', 'koplik-pipeline'], project, { ...process.env, CARGO_TARGET_DIR: target });
    const root = join(scratch, 'source');
    await mkdir(root);
    await cp(join(project, 'data/fixtures'), join(root, 'data/fixtures'), { recursive: true });
    await cp(join(project, 'data/reports'), join(root, 'data/reports'), { recursive: true });
    // The real pipeline binary runs offline; only redundant compiler invocation is skipped.
    const run = async (program, args, cwd, env) => {
      if (args.includes('build') && args.includes('-p')) return '';
      return command(program, args, cwd, env);
    };
    const options = (name) => ({ root, work: join(scratch, `${name}-work`), out: join(scratch, `${name}-out`), run,
      env: { ...process.env, CARGO_TARGET_DIR: target } });
    const fixture = options('fixture');
    assert.equal(await buildData(fixture), 'data/fixtures');
    await cp(join(project, 'data/release'), join(root, 'data/release'), { recursive: true });
    const release = options('release');
    assert.equal(await buildData(release), 'data/release');
    const paths = (await files(fixture.out)).filter((path) => path !== 'publication.json');
    for (const path of paths) assert.deepEqual(await readFile(join(release.out, path)), await readFile(join(fixture.out, path)), `byte-identical ${path}`);
    assert.equal(JSON.parse(await readFile(join(release.out, 'publication.json'))).source, 'data/release');
    assert.equal(JSON.parse(await readFile(join(fixture.out, 'publication.json'))).source, 'data/fixtures');
    const records = await verifyStore(join(root, 'data/release'));
    assert.equal(records.length, 22);
    await writeFile(join(root, 'data/release/blobs', records[0].sha256.slice(0, 2), records[0].sha256), 'Corruption test');
    await assert.rejects(buildData(options('corrupt')), /Corrupt release snapshot/);
    assert.equal(existsSync(join(scratch, 'corrupt-out/manifest.json')), false);
    console.log(`Measured reproduction: ${paths.length} existing files byte-identical; ${records.length} snapshot receipts verified`);
  } finally { await rm(scratch, { recursive: true, force: true }); }
});
