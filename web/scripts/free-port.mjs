import net from 'node:net';

const server = net.createServer();
server.once('error', (error) => {
  console.error(`Could not find an available loopback port: ${error.message}`);
  process.exitCode = 1;
});
server.listen(0, '127.0.0.1', () => {
  const address = server.address();
  if (!address || typeof address === 'string') {
    console.error('Could not read the allocated loopback port.');
    process.exitCode = 1;
    server.close();
    return;
  }
  server.close((error) => {
    if (error) {
      console.error(`Could not release the port probe: ${error.message}`);
      process.exitCode = 1;
      return;
    }
    console.log(address.port);
  });
});
