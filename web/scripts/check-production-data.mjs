import { readdir, readFile, realpath, stat } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { fileURLToPath } from 'node:url';

function isSynthetic(value) {
  if (!value || typeof value !== 'object') return false;
  if (typeof value.source_id === 'string' && value.source_id.startsWith('synthetic')) return true;
  if (typeof value.label === 'string' && /^synthetic/i.test(value.label)) return true;
  return Object.values(value).some(isSynthetic);
}

export async function assertNoSyntheticData(root, ancestors = new Set()) {
  let entries;
  let canonical;
  try { canonical = await realpath(root); entries = await readdir(root, { withFileTypes: true }); }
  catch (error) { if (error.code === 'ENOENT') return; throw error; }
  if (ancestors.has(canonical)) throw new Error(`Cyclic production data directory: ${root}`);
  const parents = new Set(ancestors).add(canonical);
  for (const entry of entries) {
    const path = join(root, entry.name);
    if (/synthetic/i.test(entry.name)) throw new Error(`Synthetic data cannot enter a production build: ${path}`);
    const type = entry.isSymbolicLink() ? await stat(path) : entry;
    if (type.isDirectory()) await assertNoSyntheticData(path, parents);
    else if (entry.name.endsWith('.json') || entry.name.endsWith('.geojson')) {
      if (isSynthetic(JSON.parse(await readFile(path, 'utf8')))) throw new Error(`Synthetic provenance in production data: ${path}`);
    }
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  if (!process.argv[2]) throw new Error('Usage: check-production-data.mjs <public|dist>');
  await assertNoSyntheticData(resolve(process.argv[2]));
}
