#!/usr/bin/env node
// Installed standalone launcher. All refresh policy runs from fetched rolling,
// not from the shared checkout or this installed copy.
import { spawn } from 'node:child_process';
import { mkdir, mkdtemp, realpath, rm, writeFile } from 'node:fs/promises';
import { homedir } from 'node:os';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { randomUUID } from 'node:crypto';

async function git(args, cwd, env) {
  return new Promise((accept, reject) => {
    const child = spawn('git', args, { cwd, env, stdio: ['ignore', 'pipe', 'pipe'] });
    let out = '';
    child.stdout.on('data', (bytes) => { out += bytes; });
    child.stderr.resume();
    child.on('error', () => reject(new Error('Bootstrap git unavailable')));
    child.on('close', (code) => code === 0 ? accept(out.trim()) : reject(new Error('Bootstrap git failed')));
  });
}
export async function bootstrap({ shared = join(homedir(), 'koplik'), state = join(homedir(), '.rsi/koplik-refresh'), env = process.env } = {}) {
  const cleanEnv = Object.fromEntries(Object.entries(env).filter(([key]) => !key.startsWith('GIT_')));
  let scratch;
  try {
    shared = await realpath(shared);
    state = resolve(state);
    if (state.includes('/sandboxes/') || state === shared || state.startsWith(`${shared}/`)) throw new Error('Dedicated state required');
    await mkdir(state, { recursive: true, mode: 0o700 });
    state = await realpath(state);
    if (state.includes('/sandboxes/') || state === shared || state.startsWith(`${shared}/`)) throw new Error('Dedicated state required');
    const urls = (await git(['remote', 'get-url', '--push', '--all', 'origin'], shared, cleanEnv)).split('\n');
    if (urls.length !== 1) throw new Error('Single local origin required');
    const origin = await realpath(resolve(shared, urls[0]));
    if (await git(['rev-parse', '--is-bare-repository'], origin, cleanEnv) !== 'true') throw new Error('Local bare origin required');
    scratch = await mkdtemp(join(state, 'bootstrap-'));
    const clone = join(scratch, 'code');
    await git(['clone', '--no-local', '--no-checkout', '--single-branch', '--branch', 'rolling', origin, clone], state, cleanEnv);
    await git(['checkout', '--detach', 'origin/rolling'], clone, cleanEnv);
    return await new Promise((accept, reject) => {
      const child = spawn(process.execPath, [join(clone, 'tools/refresh/refresh.mjs')], {
        cwd: clone, env: { ...cleanEnv, KOPLIK_REFRESH_SHARED: shared, KOPLIK_REFRESH_STATE: state }, stdio: ['ignore', 'inherit', 'inherit'],
      });
      child.on('error', () => reject(new Error('Bootstrap execution failed')));
      child.on('close', (code) => accept(code ?? 1));
    });
  } catch {
    const run = `bootstrap-${randomUUID()}`;
    await writeFile(join(state, `FAILED-${run}.md`), 'Weekly refresh failed at bootstrap. No ingest or release was attempted.\n', { mode: 0o600 }).catch(() => {});
    const notice = spawn('notify-send', ['Koplik refresh failed', 'Phase: bootstrap. Inspect the refresh journal.'], { env: cleanEnv, stdio: 'ignore' });
    notice.on('error', () => {});
    console.error('Refresh failed at bootstrap; command details suppressed');
    return 1;
  } finally {
    if (scratch) await rm(scratch, { recursive: true, force: true });
  }
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exitCode = await bootstrap({ shared: process.env.KOPLIK_REFRESH_SHARED, state: process.env.KOPLIK_REFRESH_STATE });
}
