// Runs the worker locally (wrangler dev, with a local database) and checks
// the API: npm test
import { spawn, execFileSync } from 'node:child_process';
import { createServer } from 'node:http';
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
// Google's token endpoint, standing in: any code gives these tokens.
const GOOGLE_PORT = 8798;
const google = createServer((request, response) => {
  response.setHeader('Content-Type', 'application/json');
  response.end(JSON.stringify({ refresh_token: 'refresh-1', access_token: 'access-1', expires_in: 3599, scope: 'https://www.googleapis.com/auth/drive.appdata' }));
}).listen(GOOGLE_PORT, '127.0.0.1');
let log = '';
function launch(vars = []) {
  const args = ['dev', '--port', String(PORT), '--persist-to', persist, '--var', 'GOOGLE_CLIENT_SECRET:test-secret', '--var', `GOOGLE_TOKEN_URL:http://127.0.0.1:${GOOGLE_PORT}/token`];
  for (const variable of vars) args.push('--var', variable);
  const worker = spawn(wrangler, args, { cwd, stdio: ['ignore', 'pipe', 'pipe'], env: { ...process.env, CI: '1' } });
  worker.stdout.on('data', (chunk) => (log += chunk));
  worker.stderr.on('data', (chunk) => (log += chunk));
  return worker;
}
let server = launch();

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
// Each run, and each check of the rate limits, comes from addresses of its
// own: the limiter's counts outlast a run.
const [a, b] = randomBytes(2);
const address = (n) => `10.${a}.${b}.${n}`;
const from = (n, headers = {}) => ({ ...headers, 'CF-Connecting-IP': address(n) });
// Other requests each from an address of their own, so they stay within
// the limits.
let next = 20;
const plainFetch = globalThis.fetch;
globalThis.fetch = (url, init = {}) =>
  plainFetch(url, { ...init, headers: { 'CF-Connecting-IP': address(next++ % 230 + 20), ...init.headers } });
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
  assert.equal((await put(key, new Uint8Array(1024 * 1024 + 1), { 'If-Match': '"2"' })).status, 413);
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

  // Google sign-in: the start sends the browser to Google with our client,
  // and the session and where to reply as state; nothing waits yet.
  const session = randomBytes(16).toString('hex');
  const start = (query) => fetch(`${BASE}/v1/google/start?${query}`, { redirect: 'manual' });
  response = await start(`session=${session}&reply=49152`);
  assert.equal(response.status, 302);
  const auth = new URL(response.headers.get('Location'));
  assert.equal(auth.host, 'accounts.google.com');
  assert.equal(auth.searchParams.get('state'), `${session}.49152`);
  assert.equal(auth.searchParams.get('access_type'), 'offline');
  assert.match(auth.searchParams.get('scope'), /drive\.appdata/);
  assert.equal(auth.searchParams.get('redirect_uri'), `${BASE}/v1/google/callback`);
  response = await start(`session=${session}&reply=w0`);
  assert.equal(new URL(response.headers.get('Location')).searchParams.get('state'), `${session}.w0`);
  assert.equal((await start('session=nope&reply=w0')).status, 400);
  // An app from before the reply, or a reply nobody can receive: refused.
  for (const query of [`session=${session}`, `session=${session}&reply=w9`, `session=${session}&reply=80`, `session=${session}&reply=https://evil.example`]) {
    response = await start(query);
    assert.equal(response.status, 400, query);
  }
  assert.match(await (await start(`session=${session}`)).text(), /Update the app/);
  // Pick-up codes are the server's own: a session id picks up nothing.
  assert.equal((await fetch(`${BASE}/v1/google/session/${session}`, { method: 'POST' })).status, 404);
  assert.equal((await fetch(`${BASE}/v1/google/session/ABCDE12345`, { method: 'POST' })).status, 404);
  assert.equal((await fetch(`${BASE}/v1/google/session/ABCDEFGHIL`, { method: 'POST' })).status, 404);
  assert.equal((await fetch(`${BASE}/v1/google/refresh`, { method: 'POST', body: '{}' })).status, 400);
  response = await fetch(`${BASE}/v1/google/callback?error=access_denied&state=${session}.w0`);
  assert.equal(response.status, 400);
  assert.match(await response.text(), /cancelled/);
  response = await fetch(`${BASE}/v1/google/callback?code=x&state=${session}`);
  assert.equal(response.status, 400);

  // Signed in from the desktop: the browser goes to the app's port with the
  // session and a pick-up code the server made; the code gives the tokens,
  // once.
  response = await fetch(`${BASE}/v1/google/callback?code=x&state=${session}.49152`, { redirect: 'manual' });
  assert.equal(response.status, 302);
  const back = new URL(response.headers.get('Location'));
  assert.equal(back.origin, 'http://127.0.0.1:49152');
  assert.equal(back.searchParams.get('session'), session);
  const pickUp = back.searchParams.get('pickup');
  assert.match(pickUp, /^[0-9A-HJKMNP-TV-Z]{10}$/);
  response = await fetch(`${BASE}/v1/google/session/${pickUp}`, { method: 'POST' });
  assert.equal(response.status, 200);
  assert.equal((await response.json()).refresh_token, 'refresh-1');
  assert.equal((await fetch(`${BASE}/v1/google/session/${pickUp}`, { method: 'POST' })).status, 404);

  // Signed in from the web app: the browser goes to the app's own page
  // with the session and the code in the fragment.
  response = await fetch(`${BASE}/v1/google/callback?code=x&state=${session}.w0`, { redirect: 'manual' });
  assert.equal(response.status, 302);
  const page = new URL(response.headers.get('Location'));
  assert.equal(`${page.origin}${page.pathname}`, 'https://malx122.github.io/mx_shooting_calculator/google.html');
  const fields = new URLSearchParams(page.hash.slice(1));
  assert.equal(fields.get('session'), session);
  assert.equal((await fetch(`${BASE}/v1/google/session/${fields.get('pickup')}`, { method: 'POST' })).status, 200);

  // One address may start only two copies a minute.
  for (let i = 0; i < 2; i++) {
    assert.equal((await put(id(), 'x', from(1, { 'If-None-Match': '*' }))).status, 201);
  }
  response = await put(id(), 'x', from(1, { 'If-None-Match': '*' }));
  assert.equal(response.status, 429);
  assert.equal((await put(id(), 'x', from(2, { 'If-None-Match': '*' }))).status, 201);
  // ... make 120 requests a minute, and 20 for Google's sign-in.
  // (The limiter counts in windows of a minute, so the 121st may fall in
  // the next one.)
  const statuses = [];
  for (let i = 0; i < 250; i++) {
    statuses.push((await fetch(copy(key), { headers: from(3) })).status);
  }
  assert.ok(statuses.indexOf(429) >= 120, `first 429 at ${statuses.indexOf(429)}`);
  for (let i = 0; i < 20; i++) {
    assert.equal((await fetch(`${BASE}/v1/google/session/ABCDEFGH12`, { method: 'POST', headers: from(4) })).status, 404);
  }
  assert.equal((await fetch(`${BASE}/v1/google/session/ABCDEFGH12`, { method: 'POST', headers: from(4) })).status, 429);
  const kept = id();
  assert.equal((await put(kept, 'x', from(5, { 'If-None-Match': '*' }))).status, 201);

  // A full database refuses new copies, and keeps the ones it has going.
  server.kill();
  await new Promise((resolve) => server.once('exit', resolve));
  // Until the old worker has let go of the port.
  for (let up = true; up; ) {
    up = await fetch(BASE).then(() => true, () => false);
    await new Promise((resolve) => setTimeout(resolve, 200));
  }
  server = launch(['MAX_DATABASE_BYTES:1']);
  await ready();
  assert.equal((await put(id(), 'x', from(6, { 'If-None-Match': '*' }))).status, 507);
  assert.equal((await put(kept, 'y', from(6, { 'If-Match': '"1"' }))).status, 200);

  console.log('sync server: all checks passed');
} catch (error) {
  console.error(error);
  console.error(log);
  process.exitCode = 1;
} finally {
  server.kill();
  google.close();
}
