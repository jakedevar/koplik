import { lstat, readFile, readdir, rm } from 'node:fs/promises';
import { join } from 'node:path';
import { loadPatterns } from './scans.mjs';

export async function preflight(patternsFile, contactFile) {
  const patterns = await loadPatterns(patternsFile);
  // Match koplik-ingest::polite::config_value: last assignment wins, optional
  // export prefix and paired quotes, no interpolation or inline comments.
  const text = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true }).decode(await readFile(contactFile));
  const trim = (value) => value.replace(/^\p{White_Space}+|\p{White_Space}+$/gu, '');
  let contact;
  for (let line of text.split(/\r?\n/)) {
    line = trim(line);
    if (!line || line.startsWith('#')) continue;
    if (line.startsWith('export ')) line = line.slice(7).replace(/^\p{White_Space}+/u, '');
    const split = line.indexOf('=');
    if (split < 0 || trim(line.slice(0, split)) !== 'KOPLIK_CENSUS_CONTACT') continue;
    let value = trim(line.slice(split + 1));
    if (value.length >= 2 && ['"', "'"].includes(value[0]) && value.at(-1) === value[0]) value = value.slice(1, -1);
    contact = trim(value);
  }
  if (!contact) throw new Error('Configured Census contact required');
  return { patterns, contact };
}

const runName = /^\d{4}-\d{2}-\d{2}-[a-f0-9-]{36}$/;
const day = 24 * 60 * 60 * 1000;
export async function pruneRuns(state, now = Date.now()) {
  const runs = [];
  for (const entry of await readdir(state, { withFileTypes: true })) {
    if (!entry.isDirectory() || !runName.test(entry.name)) continue;
    const directory = join(state, entry.name);
    let status;
    try { status = JSON.parse(await readFile(join(directory, 'status.json'), 'utf8')); }
    catch (error) {
      if (error.code !== 'ENOENT') continue; // Malformed status needs diagnosis.
      // A crash before status.json was written still leaves compiler output.
      // Only recognized run directories qualify, and only targets are removed.
      status = { status: 'running', started_at: (await lstat(directory)).mtime.toISOString() };
    }
    const time = Date.parse(status.completed_at || status.started_at);
    if (!Number.isFinite(time) || !['green', 'failed', 'running', 'dry-run'].includes(status.status)) continue;
    runs.push({ directory, name: entry.name, time, status: status.status });
  }
  const green = runs.filter((run) => run.status === 'green').sort((a, b) => b.time - a.time || b.name.localeCompare(a.name));
  for (const run of green.slice(2)) await rm(run.directory, { recursive: true });
  for (const run of runs.filter((run) => run.status !== 'green' && now - run.time >= 14 * day)) {
    // Also reclaim compiler output from interrupted/dry diagnostic runs, while
    // preserving their source tree, receipts and failure reports.
    const target = join(run.directory, 'worktree', 'target');
    const parent = await lstat(join(run.directory, 'worktree')).catch(() => null);
    if (parent?.isDirectory()) await rm(target, { recursive: true, force: true });
  }
}
