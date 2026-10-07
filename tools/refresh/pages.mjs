#!/usr/bin/env node
import { realpath } from 'node:fs/promises';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { setTimeout as delay } from 'node:timers/promises';
import { command } from './data.mjs';

export async function pagesRepository(origin, run = command, env = process.env) {
  const url = await run('git', ['remote', 'get-url', '--push', '--all', 'github'], origin, env);
  // Only the operator-configured GitHub mirror is an authority for the target.
  // Reject extra remotes, credentials, query strings and non-GitHub hosts.
  const match = /^(?:https:\/\/github\.com\/|git@github\.com:|ssh:\/\/git@github\.com\/)([A-Za-z0-9_.-]+)\/([A-Za-z0-9_.-]+?)(?:\.git)?$/.exec(url);
  if (!match || ['.', '..'].includes(match[1]) || ['.', '..'].includes(match[2])) throw new Error('GitHub mirror owner/repository required');
  return `${match[1]}/${match[2]}`;
}

export function pagesRetry(repository, commit) {
  return `gh api --hostname github.com --method POST repos/${repository}/pages/builds\nmake pages-verify PAGES_COMMIT=${commit}`;
}

export async function verifyPages({ repository, commit, cwd, run = command, env = process.env,
  timeoutMs = 600000, pollMs = 5000, now = () => performance.now(), sleep = delay }) {
  if (!/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(repository) || !/^[a-f0-9]{40,64}$/.test(commit)
    || !Number.isFinite(timeoutMs) || timeoutMs <= 0 || !Number.isFinite(pollMs) || pollMs <= 0) throw new Error('Invalid Pages verification options');
  const deadline = now() + timeoutMs;
  const api = async (method, endpoint) => {
    const remaining = deadline - now();
    if (remaining <= 0) throw new Error('Pages build timed out');
    return run('gh', ['api', '--hostname', 'github.com', '--method', method, `repos/${repository}/pages/builds${endpoint}`], cwd,
      { ...env, GH_PROMPT_DISABLED: '1' }, { timeoutMs: Math.min(30000, remaining) });
  };
  await api('POST', '');
  while (now() < deadline) {
    const build = JSON.parse(await api('GET', '/latest'));
    if (build.commit === commit && build.status === 'built') return { commit, status: 'built', id: build.id ?? null };
    if (build.commit === commit && build.status === 'errored') throw new Error('Pages build errored');
    const remaining = deadline - now();
    if (remaining > 0) await sleep(Math.min(pollMs, remaining));
  }
  throw new Error('Pages build timed out');
}

export async function verifyPublishedPages({ root = process.cwd(), commit = process.env.PAGES_COMMIT,
  run = command, env = process.env } = {}) {
  // Match refresh's local bare-origin guard; never fetch or push from this check.
  const url = await run('git', ['remote', 'get-url', '--push', '--all', 'origin'], root, env);
  if (url.includes('\n')) throw new Error('Single local origin required');
  const origin = await realpath(resolve(root, url));
  if (await run('git', ['rev-parse', '--is-bare-repository'], origin, env) !== 'true') throw new Error('Local bare origin required');
  const repository = await pagesRepository(origin, run, env);
  commit ||= await run('git', ['rev-parse', 'refs/heads/gh-pages'], origin, env);
  try {
    const result = await verifyPages({ repository, commit, cwd: root, run, env });
    console.log(`Pages built ${result.commit}`);
    return result;
  } catch {
    throw new Error(`Pages verification failed; site may be stale. Retry:\n${pagesRetry(repository, commit)}`);
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  verifyPublishedPages().catch((error) => { console.error(error.message); process.exitCode = 1; });
}
