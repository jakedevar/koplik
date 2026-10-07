// A minimal static server for a built site under a base path (default /koplik/), used by the screenshot and paint
// measurement scripts. Serves files only from the given directory; no network access beyond loopback.
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { extname, join, normalize, resolve } from 'node:path';
import { gzipSync } from 'node:zlib';

const types = { '.html': 'text/html; charset=utf-8', '.js': 'text/javascript', '.mjs': 'text/javascript', '.css': 'text/css', '.json': 'application/json',
  '.wasm': 'application/wasm', '.svg': 'image/svg+xml', '.png': 'image/png' };

/** Resolves with `{ url, close }`; `url` ends with the base path. */
export async function serve(dir, { base = '/koplik/', port = 0 } = {}) {
  const root = resolve(dir);
  const server = createServer(async (request, response) => {
    try {
      const path = decodeURIComponent(new URL(request.url, 'http://x').pathname);
      if (!path.startsWith(base)) { response.writeHead(404).end(); return; }
      let file = normalize(join(root, path.slice(base.length)));
      if (!file.startsWith(root)) { response.writeHead(403).end(); return; }
      if (path.endsWith('/')) file = join(file, 'index.html');
      const body = await readFile(file);
      // GitHub Pages gzips text assets; do the same so transfer sizes (and throttled timings) match what visitors get.
      const type = types[extname(file)] || 'application/octet-stream';
      const gzip = /text|json|javascript|wasm|svg/.test(type) && /\bgzip\b/.test(request.headers['accept-encoding'] || '');
      response.writeHead(200, { 'content-type': type, 'cache-control': 'no-store', ...(gzip ? { 'content-encoding': 'gzip' } : {}) }).end(gzip ? gzipSync(body) : body);
    } catch { response.writeHead(404).end(); }
  });
  await new Promise((done) => server.listen(port, '127.0.0.1', done));
  return { url: `http://127.0.0.1:${server.address().port}${base}`, close: () => new Promise((done) => server.close(done)) };
}
