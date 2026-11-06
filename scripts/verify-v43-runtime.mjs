// Sends a real loopback HTTP request through an already-running Routa proxy.
// No system proxy settings or external network services are used.
import assert from 'node:assert/strict';
import http from 'node:http';
import { once } from 'node:events';

const proxyPort = Number(process.argv[2]);
assert(Number.isInteger(proxyPort) && proxyPort > 0 && proxyPort <= 65535,
  'Usage: node scripts/verify-v43-runtime.mjs <proxy-port>');
const origin = http.createServer((_request, response) => response.end('routa-v43-loopback-ok'));
origin.listen(0, '127.0.0.1');
await once(origin, 'listening');
try {
  const originPort = origin.address().port;
  const body = await new Promise((resolve, reject) => {
    const request = http.get({ hostname: '127.0.0.1', port: proxyPort,
      path: `http://127.0.0.1:${originPort}/verify`,
      headers: { Host: `127.0.0.1:${originPort}` }, timeout: 5000,
    }, response => {
      let body = '';
      response.setEncoding('utf8');
      response.on('data', chunk => { body += chunk; });
      response.on('end', () => response.statusCode === 200
        ? resolve(body) : reject(new Error(`HTTP ${response.statusCode}: ${body}`)));
    });
    request.on('timeout', () => request.destroy(new Error('Proxy request timed out')));
    request.on('error', reject);
  });
  assert.equal(body, 'routa-v43-loopback-ok');
  console.log(JSON.stringify({ passed: true, proxyPort, originPort, body }));
} finally {
  origin.closeAllConnections();
  origin.close();
}
