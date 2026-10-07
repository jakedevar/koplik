import assert from 'node:assert/strict';
import { cp, mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import { buildData, files, verifyStore } from './data.mjs';
import { fixturePipeline, project, seedFixtureStore } from './fixture-store.mjs';

async function dataRoot(scratch) {
  const root = join(scratch, 'source');
  await mkdir(root);
  await cp(join(project, 'data/fixtures'), join(root, 'data/fixtures'), { recursive: true });
  await cp(join(project, 'data/reports'), join(root, 'data/reports'), { recursive: true });
  return root;
}
async function options(scratch, root, name) {
  const { env, run } = await fixturePipeline();
  return { root, work: join(scratch, `${name}-work`), out: join(scratch, `${name}-out`), run, env };
}
const v6 = ['coverage', 'geographies', 'rt', 'texas-counties', 'us-states', 'weekly-cases'].map((name) => `v6/${name}.json`).sort();

test('offline fixture-ingested release stores build in both modes; fallback only on absence and corruption refuses', async () => {
  const scratch = await mkdtemp(join(tmpdir(), 'koplik-release-data-test-'));
  try {
    const root = await dataRoot(scratch);
    const fixture = await options(scratch, root, 'fallback');
    assert.equal(await buildData(fixture), 'data/fixtures');
    assert.equal(JSON.parse(await readFile(join(fixture.out, 'publication.json'))).source, 'data/fixtures');
    const store = join(root, 'data/release');
    for (const mode of ['fixtures', 'live']) {
      await seedFixtureStore({ store, work: join(scratch, `ingest-${mode}`), mode });
      const release = await options(scratch, root, mode);
      assert.equal(await buildData(release), 'data/release');
      const manifest = JSON.parse(await readFile(join(release.out, 'manifest.json')));
      const recordedMode = JSON.parse(await readFile(join(store, 'ingest.manifest.json'))).mode;
      assert.equal(manifest.mode, recordedMode);
      assert.equal(recordedMode, mode);
      assert.deepEqual((await files(release.out)).filter((path) => path.startsWith('v6/')), v6);
      assert.equal(JSON.parse(await readFile(join(release.out, 'publication.json'))).source, 'data/release');
      const repeat = await options(scratch, root, `${mode}-repeat`);
      await buildData(repeat);
      for (const path of await files(release.out)) {
        assert.deepEqual(await readFile(join(repeat.out, path)), await readFile(join(release.out, path)), `deterministic ${mode}/${path}`);
      }
      const records = await verifyStore(store);
      assert.equal(records.length, mode === 'fixtures' ? 22 : 44, 'fake live re-retrieval replays exact fixture receipts without changing raw bytes');
    }
    const [record] = await verifyStore(store);
    await writeFile(join(store, 'blobs', record.sha256.slice(0, 2), record.sha256), 'Corruption test');
    await assert.rejects(buildData(await options(scratch, root, 'corrupt')), /Corrupt release snapshot/);
    assert.equal(existsSync(join(scratch, 'corrupt-out/manifest.json')), false);
  } finally { await rm(scratch, { recursive: true, force: true }); }
});

test('committed initial data/release seed reproduces the fixture build byte-identically', async (t) => {
  const recorded = JSON.parse(await readFile(join(project, 'data/release/ingest.manifest.json')));
  if (recorded.mode === 'live') {
    const reason = 'Initial seed proof skipped: committed data/release has mode live after a real refresh';
    console.log(reason); t.skip(reason); return;
  }
  assert.equal(recorded.mode, 'fixtures');
  const scratch = await mkdtemp(join(tmpdir(), 'koplik-initial-release-proof-'));
  try {
    const root = await dataRoot(scratch);
    const fixture = await options(scratch, root, 'fixture');
    await buildData(fixture);
    // This single, explicitly named proof is the only test consuming the
    // committed initial publication seed. General tests seed their own stores.
    await cp(join(project, 'data/release'), join(root, 'data/release'), { recursive: true });
    const release = await options(scratch, root, 'release');
    await buildData(release);
    const paths = (await files(fixture.out)).filter((path) => path !== 'publication.json');
    for (const path of paths) assert.deepEqual(await readFile(join(release.out, path)), await readFile(join(fixture.out, path)), `byte-identical ${path}`);
    assert.equal((await verifyStore(join(root, 'data/release'))).length, 22);
    console.log(`Initial seed reproduction: ${paths.length} current files byte-identical; 22 snapshot receipts verified`);
  } finally { await rm(scratch, { recursive: true, force: true }); }
});
