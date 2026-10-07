import assert from 'node:assert/strict';
import { createConnection, createServer } from 'node:net';
import test from 'node:test';

// Run only via offline-test.sh, inside the namespace, before the requested tests.
function connect(host, port) {
  return new Promise((resolve, reject) => {
    const socket = createConnection({ host, port });
    socket.setTimeout(1000, () => socket.destroy(new Error('TCP connect timed out; isolation did not fail immediately')));
    socket.once('error', reject);
    socket.once('connect', () => { socket.destroy(); resolve(); });
  });
}

test('offline gate rejects outbound TCP immediately with ENETUNREACH', async () => {
  // RFC 5737 TEST-NET-1: a numeric address avoids DNS and proxy configuration.
  await assert.rejects(connect('192.0.2.1', 443), { code: 'ENETUNREACH' });
  console.log('Offline gate: 192.0.2.1:443 -> ENETUNREACH');
});

test('offline gate permits a real loopback TCP connection', async () => {
  const server = createServer((socket) => socket.end());
  try {
    await new Promise((resolve, reject) => {
      server.once('error', reject);
      server.listen(0, '127.0.0.1', resolve);
    });
    await connect('127.0.0.1', server.address().port);
  } finally {
    await new Promise((resolve) => server.close(resolve));
  }
});
