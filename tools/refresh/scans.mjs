import { readFile } from 'node:fs/promises';
import { command } from './data.mjs';

// Deliberately constructed fragments keep the scanner's own source from matching itself.
const secretPatterns = [
  new RegExp('-----BEGIN ' + '(?:RSA |EC |OPENSSH |DSA )?PRIVATE KEY-----'),
  new RegExp('gh[pousr]_' + '[A-Za-z0-9]{36,}'),
  new RegExp('github_pat_' + '[A-Za-z0-9_]{60,}'),
  new RegExp('sk-' + '(?:[A-Za-z0-9]{48}(?![A-Za-z0-9])|(?:proj-|svcacct-)[A-Za-z0-9_-]{48,}|ant-api[0-9]{2}-[A-Za-z0-9_-]{60,})'),
  new RegExp('AKIA' + '[A-Z0-9]{16}'),
  new RegExp('xox[baprs]-' + '[A-Za-z0-9-]{20,}'),
  new RegExp('RSI_SESSION_TOKEN\\s*[:=]\\s*["\x27]?' + '[A-Za-z0-9_./+=-]{32,}'),
];
export function scanText(text, personalPatterns) {
  if (personalPatterns.some((pattern) => text.includes(pattern))) throw new Error('Personal-data scan failed (details suppressed)');
  const rule = secretPatterns.findIndex((pattern) => pattern.test(text));
  if (rule !== -1) throw new Error(`Secrets scan failed (rule ${rule + 1}; details suppressed)`);
}
export async function loadPatterns(patternsFile) {
  // Literal patterns are normalized once for preflight, scanning and redaction.
  const text = new TextDecoder('utf-8', { fatal: true }).decode(await readFile(patternsFile));
  const patterns = text.split('\n').map((line) => line.trim()).filter((line) => line && !line.startsWith('#'));
  if (!patterns.length) throw new Error('Personal-data patterns file has no usable patterns');
  return patterns;
}
export function redact(text, personalPatterns = [], contact) {
  // Decode URL escapes before masking, including mixed escaped/plain text.
  // Malformed UTF-8 stays unchanged; literal encoded variants are masked below.
  text = text.replace(/(?:%[a-f0-9]{2})+/gi, (value) => {
    try { return decodeURIComponent(value); } catch { return value; }
  });
  // Mask complete credentials first: a shorter personal pattern must not hide
  // a token prefix from the secrets matcher and expose the remaining token.
  for (const pattern of secretPatterns) text = text.replace(new RegExp(pattern.source, 'gi'), '[REDACTED]');
  const values = [...personalPatterns, contact].filter(Boolean).flatMap((value) =>
    [value, encodeURIComponent(value), encodeURIComponent(value).replace(/%20/g, '+'), value.replace(/ /g, '+')]);
  for (const value of [...new Set(values)].sort((a, b) => b.length - a.length)) {
    const literal = value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    text = text.replace(new RegExp(literal, 'gi'), '[REDACTED]');
  }
  return text;
}
export async function scanTree(root, sha, patternsFile, run = command) {
  const patterns = await loadPatterns(patternsFile);
  const tree = await run('git', ['ls-tree', '-r', '-z', sha], root);
  const objects = new Set();
  for (const entry of tree.split('\0').filter(Boolean)) {
    const [header, path] = entry.split('\t');
    const [mode, type, object] = header.split(' ');
    if (path.split('/').some((part) => /^\.env/.test(part))) throw new Error('Environment file in publication');
    scanText(path, patterns);
    if (mode === '160000' || type !== 'blob') throw new Error('Unscannable publication entry');
    objects.add(object);
  }
  for (const object of objects) scanText(await run('git', ['cat-file', 'blob', object], root), patterns);
  return { blobs: objects.size, personal_matches: 0, secret_matches: 0 };
}
export async function scanHistory(root, patternsFile, run = command) {
  const patterns = await loadPatterns(patternsFile);
  const git = (args) => run('git', args, root);
  const tracked = (await git(['ls-files', '-z'])).split('\0').filter(Boolean);
  if (tracked.some((path) => path.split('/').some((part) => /^\.env/.test(part)))) throw new Error('Tracked environment file');
  for (const path of tracked) scanText((await readFile(`${root}/${path}`)).toString('utf8'), patterns);
  const commits = (await git(['rev-list', 'HEAD'])).split('\n').filter(Boolean);
  const objects = new Set();
  for (const sha of commits) {
    scanText(await git(['show', '-s', '--format=raw', sha]), patterns);
    const tree = await git(['ls-tree', '-r', '-z', sha]);
    for (const entry of tree.split('\0').filter(Boolean)) {
      const [header, path] = entry.split('\t');
      if (path.split('/').some((part) => /^\.env/.test(part))) throw new Error('Environment file in history');
      scanText(path, patterns);
      const [mode, type, object] = header.split(' ');
      if (type === 'blob') objects.add(object);
      if (mode === '160000') throw new Error('Submodules cannot be scanned');
    }
  }
  for (const object of objects) scanText(await git(['cat-file', 'blob', object]), patterns);
  return { commits: commits.length, blobs: objects.size, personal_matches: 0, secret_matches: 0 };
}
