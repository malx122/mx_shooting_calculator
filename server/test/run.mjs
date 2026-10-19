// Runs the worker locally (wrangler dev, with a local database) and checks
// the API: npm test
import { spawn, execFileSync } from 'node:child_process';
import assert from 'node:assert/strict';
import { randomBytes } from 'node:crypto';
import { fileURLToPath } from 'node:url';

const PORT = 8799;
const BASE = `http://127.0.0.1:${PORT}`;
const here = (path) => fileURLToPath(new URL(path, import.meta.url));
const wrangler = here('../node_modules/.bin/wrangler');
const cwd = here('..');
const persist = here('../.wrangler/test-state');

execFileSync(wrangler, ['d1', 'migrations', 'apply', 'mxsc-sync', '--local', '--persist-to', persist], {
  cwd,
  stdio: 'ignore',
  env: { ...process.env, CI: '1' },
});
const server = spawn(wrangler, ['dev', '--port', String(PORT), '--persist-to', persist, '--var', 'GOOGLE_CLIENT_SECRET:test-secret'], {
  cwd,
  stdio: ['ignore', 'pipe', 'pipe'],
  env: { ...process.env, CI: '1' },
});
let log = '';
server.stdout.on('data', (chunk) => (log += chunk));
server.stderr.on('data', (chunk) => (log += chunk));

async function ready() {
  for (let i = 0; i < 100; i++) {
    try {
      if ((await fetch(BASE)).ok) return;
    } catch {}
    await new Promise((resolve) => setTimeout(resolve, 200));
  }
  throw new Error(`the worker didn't start:\n${log}`);
}

const id = () => randomBytes(32).toString('hex');
const copy = (key) => `${BASE}/v1/copies/${key}`;
const put = (key, body, headers) => fetch(copy(key), { method: 'PUT', body, headers });

try {
  await ready();
  const key = id();

  // Nothing yet.
  assert.equal((await fetch(copy(key))).status, 404);
  // Writes must say what they replace.
  assert.equal((await put(key, 'x')).status, 428);

  // The first copy.
  const first = new Uint8Array([0, 1, 2, 250, 255]);
  let response = await put(key, first, { 'If-None-Match': '*' });
  assert.equal(response.status, 201);
  assert.equal(response.headers.get('ETag'), '"1"');
  // A second "first copy" loses.
  assert.equal((await put(key, first, { 'If-None-Match': '*' })).status, 412);

  // Reading gives the bytes back, unchanged, with the version.
  response = await fetch(copy(key));
  assert.equal(response.status, 200);
  assert.equal(response.headers.get('ETag'), '"1"');
  assert.deepEqual(new Uint8Array(await response.arrayBuffer()), first);

  // Replacing version 1 works once; a device still on version 1 loses.
  response = await put(key, 'second', { 'If-Match': '"1"' });
  assert.equal(response.status, 200);
  assert.equal(response.headers.get('ETag'), '"2"');
  assert.equal((await put(key, 'stale', { 'If-Match': '"1"' })).status, 412);
  assert.equal(await (await fetch(copy(key))).text(), 'second');

  // Size limits and bad ids.
  assert.equal((await put(key, new Uint8Array(256 * 1024 + 1), { 'If-Match': '"2"' })).status, 413);
  assert.equal((await put(key, '', { 'If-Match': '"2"' })).status, 400);
  assert.equal((await fetch(`${BASE}/v1/copies/not-an-id`)).status, 404);

  // Browsers may ask first.
  response = await fetch(copy(key), { method: 'OPTIONS' });
  assert.equal(response.status, 204);
  assert.equal(response.headers.get('Access-Control-Allow-Origin'), '*');
  assert.match(response.headers.get('Access-Control-Allow-Headers'), /If-Match/);
  response = await fetch(copy(key));
  assert.equal(response.headers.get('Access-Control-Expose-Headers'), 'ETag');

  // Deleting: later reads and writes are told so, so no device restarts it.
  assert.equal((await fetch(copy(key), { method: 'DELETE' })).status, 204);
  assert.equal((await fetch(copy(key))).status, 410);
  assert.equal((await put(key, 'again', { 'If-Match': '"3"' })).status, 410);
  assert.equal((await put(key, 'again', { 'If-None-Match': '*' })).status, 410);
  const never = id();
  assert.equal((await fetch(copy(never), { method: 'DELETE' })).status, 204);
  assert.equal((await put(never, 'x', { 'If-None-Match': '*' })).status, 410);

  // Google sign-in: the start sends the browser to Google with our client
  // and the session as state; nothing is waiting to be picked up yet.
  const session = randomBytes(16).toString('hex');
  response = await fetch(`${BASE}/v1/google/start?session=${session}`, { redirect: 'manual' });
  assert.equal(response.status, 302);
  const auth = new URL(response.headers.get('Location'));
  assert.equal(auth.host, 'accounts.google.com');
  assert.equal(auth.searchParams.get('state'), session);
  assert.equal(auth.searchParams.get('access_type'), 'offline');
  assert.match(auth.searchParams.get('scope'), /drive\.appdata/);
  assert.equal(auth.searchParams.get('redirect_uri'), `${BASE}/v1/google/callback`);
  assert.equal((await fetch(`${BASE}/v1/google/start?session=nope`, { redirect: 'manual' })).status, 400);
  assert.equal((await fetch(`${BASE}/v1/google/session/${session}`, { method: 'POST' })).status, 404);
  assert.equal((await fetch(`${BASE}/v1/google/refresh`, { method: 'POST', body: '{}' })).status, 400);
  response = await fetch(`${BASE}/v1/google/callback?error=access_denied&state=${session}`);
  assert.equal(response.status, 400);
  assert.match(await response.text(), /cancelled/);

  console.log('sync server: all checks passed');
} catch (error) {
  console.error(error);
  console.error(log);
  process.exitCode = 1;
} finally {
  server.kill();
}
