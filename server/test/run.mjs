// Runs the worker locally (wrangler dev, with a local database) and checks
// the API: npm test
import { spawn, execFileSync } from 'node:child_process';
import { createServer } from 'node:http';
import assert from 'node:assert/strict';
import { randomBytes } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { limitKey } from '../src/index.js';

// Rate limits count an IPv6 connection's whole /64 as one, however written.
assert.equal(limitKey('2001:db8:85a3:12:abcd::1'), '2001:db8:85a3:12::/64');
assert.equal(limitKey('2001:0db8:85a3:0012:ffff:ffff:ffff:ffff'), '2001:db8:85a3:12::/64');
assert.equal(limitKey('2001:db8::1'), '2001:db8:0:0::/64');
assert.equal(limitKey('203.0.113.7'), '203.0.113.7');
assert.equal(limitKey(null), 'unknown');

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
// Watches: a key is 64 bytes, a box an iv, whole blocks and a tag.
const b64 = (length) => randomBytes(length).toString('base64');
const box = (blocks) => b64(16 + 16 * blocks + 32);
const pairings = `${BASE}/v1/watch/pairings`;
const pairing = (code) => `${pairings}/${code}`;
const link = (path = '') => `${BASE}/v1/watch/link${path}`;
const bearer = (secret, headers = {}) => ({ Authorization: `Bearer ${secret}`, ...headers });
const send = (url, method, body, headers) =>
  fetch(url, { method, body: typeof body === 'string' ? body : JSON.stringify(body), headers: { 'Content-Type': 'application/json', ...headers } });
const pair = async (key = b64(64), headers) => {
  const response = await send(pairings, 'POST', { key }, headers);
  assert.equal(response.status, 200);
  return response.json();
};

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

  // Pairing a watch: the watch gives its key and gets a code and a token.
  const watchKey = b64(64);
  const { code, token, expires_in } = await pair(watchKey);
  assert.match(code, /^\d{8}$/);
  assert.match(token, /^[0-9a-f]{64}$/);
  assert.equal(expires_in, 600);
  // Nothing sent yet: the watch waits; the app gets the watch's key.
  response = await fetch(link(), { headers: bearer(token) });
  assert.equal(response.status, 200);
  assert.equal(response.headers.get('Cache-Control'), 'no-store');
  assert.equal(response.headers.get('Access-Control-Allow-Origin'), '*');
  assert.deepEqual(await response.json(), { waiting: true });
  response = await fetch(pairing(code));
  assert.equal(response.status, 200);
  assert.match(response.headers.get('Content-Type'), /json/);
  assert.deepEqual(await response.json(), { key: watchKey });
  // Bad bodies are refused, and leave the pairing waiting.
  const appKey = b64(64);
  const writer = id();
  const items = [box(3), box(1)];
  const good = { key: appKey, writer, version: 5, items };
  const without = (field) => Object.fromEntries(Object.entries(good).filter(([name]) => name !== field));
  for (const body of [
    { ...good, key: b64(63) },
    { ...good, key: b64(65) },
    { ...good, key: watchKey.replace(/=*$/, '') },
    { ...good, key: `${watchKey.slice(0, 4)}\n${watchKey.slice(4)}` },
    { ...good, key: 42 },
    without('key'),
    without('writer'),
    { ...good, writer: writer.toUpperCase() },
    { ...good, writer: writer.slice(1) },
    without('version'),
    { ...good, version: 0 },
    { ...good, version: 1.5 },
    { ...good, version: 2 ** 31 },
    { ...good, version: '5' },
    without('items'),
    { ...good, items: [] },
    { ...good, items: box(1) },
    { ...good, items: Array(17).fill(box(1)) },
    { ...good, items: [b64(47)] },
    { ...good, items: [b64(48)] },
    { ...good, items: [b64(16 + 15 + 32)] },
    { ...good, items: [b64(12 * 1024 + 16)] },
    { ...good, items: ['not base64!'] },
    { ...good, items: [42] },
  ]) {
    assert.equal((await send(pairing(code), 'PUT', body)).status, 400, JSON.stringify(body).slice(0, 80));
  }
  for (const body of ['not json', '[1, 2]', 'null', '"x"']) {
    assert.equal((await send(pairing(code), 'PUT', body)).status, 400, body);
  }
  assert.equal((await send(pairings, 'POST', 'not json')).status, 400);
  assert.equal((await send(pairings, 'POST', { key: b64(65) })).status, 400);
  assert.equal((await send(pairings, 'POST', { key: b64(32) })).status, 400);
  assert.equal((await send(pairings, 'POST', [watchKey])).status, 400);
  assert.deepEqual(await (await fetch(link(), { headers: bearer(token) })).json(), { waiting: true });
  // The app sends; a second sender, and the key afterwards, are refused.
  assert.equal((await send(pairing(code), 'PUT', good)).status, 201);
  // The same sender again (an answer lost on the way) gets the same answer.
  assert.equal((await send(pairing(code), 'PUT', good)).status, 201);
  assert.equal((await send(pairing(code), 'PUT', { ...good, writer: id() })).status, 409);
  assert.equal((await send(pairing(code), 'PUT', { ...good, key: b64(64) })).status, 409);
  assert.equal((await fetch(pairing(code))).status, 409);
  // The app hears the watch hasn't answered, and can't update yet.
  const status = async (secret) => {
    const answer = await fetch(link('/status'), { headers: bearer(secret) });
    return answer.status === 200 ? answer.json() : answer.status;
  };
  assert.deepEqual(await status(writer), { done: null, version: 5, watch_version: null, watch_said: null });
  response = await send(link(), 'PUT', { version: 6, items }, bearer(writer));
  assert.equal(response.status, 409);
  assert.equal((await response.json()).error, 'the watch has not taken the pairing yet');
  // The watch fetches what there is, and each item, unchanged, as often as
  // it needs to (an answer may be lost on the way).
  const item = async (secret, index) => {
    const answer = await fetch(link(`/items/${index}`), { headers: bearer(secret) });
    return answer.status === 200 ? answer.json() : answer.status;
  };
  for (let i = 0; i < 2; i++) {
    response = await fetch(link(), { headers: bearer(token) });
    assert.equal(response.status, 200);
    assert.equal(response.headers.get('Cache-Control'), 'no-store');
    assert.deepEqual(await response.json(), { key: appKey, version: 5, count: 2 });
    assert.deepEqual(await item(token, 0), { version: 5, box: items[0] });
    assert.deepEqual(await item(token, 1), { version: 5, box: items[1] });
  }
  for (const index of [2, 99, 100, -1, 'x', '']) assert.equal(await item(token, index), 404, String(index));
  // Each side's secret is its own; without one, nothing.
  assert.equal((await fetch(link(), { headers: bearer(writer) })).status, 404);
  assert.equal(await status(token), 404);
  assert.equal(await status(id()), 404);
  // Without a secret at all: 401, so a watch never mistakes it for a link
  // that has gone.
  assert.equal((await fetch(link())).status, 401);
  assert.equal((await fetch(link(), { headers: { Authorization: `Bearer ${token.toUpperCase()}` } })).status, 401);
  assert.equal((await fetch(link(), { headers: { Authorization: token } })).status, 401);
  // The watch takes it: the pairing is now the link, and its code goes.
  const done = (secret, body) => send(link('/done'), 'POST', body, bearer(secret));
  response = await done(token, { done: 'used', version: 5 });
  assert.equal(response.status, 200);
  assert.deepEqual(await response.json(), { taken: true });
  // Said again (the answer lost on the way), the same.
  assert.deepEqual(await (await done(token, { done: 'used', version: 5 })).json(), { taken: true });
  // The app's send repeated after that still gets its answer.
  assert.equal((await send(pairing(code), 'PUT', good)).status, 201);
  assert.deepEqual(await status(writer), { done: 'used', version: 5, watch_version: 5, watch_said: 'used' });
  assert.equal((await fetch(pairing(code))).status, 404);
  assert.equal((await send(pairing(code), 'PUT', { ...good, writer: id() })).status, 404);
  // A second answer doesn't undo it.
  assert.equal((await done(token, { done: 'declined' })).status, 200);
  assert.equal((await status(writer)).done, 'used');
  // The app updates it, with newer versions only.
  const update = (secret, version, items) => send(link(), 'PUT', { version, items }, bearer(secret));
  const changed = [box(2)];
  response = await update(writer, 6, changed);
  assert.equal(response.status, 200);
  assert.deepEqual(await response.json(), {});
  for (const version of [6, 4]) {
    response = await update(writer, version, items);
    assert.equal(response.status, 409, String(version));
    assert.equal((await response.json()).error, 'not newer than the version there');
  }
  assert.equal((await update(writer, 7, [])).status, 400);
  assert.equal((await update(token, 7, changed)).status, 404);
  assert.deepEqual(await (await fetch(link(), { headers: bearer(token) })).json(), { key: appKey, version: 6, count: 1 });
  assert.deepEqual(await item(token, 0), { version: 6, box: changed[0] });
  assert.equal(await item(token, 1), 404);
  // The app hears what the watch made of it.
  assert.equal((await status(writer)).watch_version, 5);
  await done(token, { done: 'used', version: 6 });
  assert.deepEqual(await status(writer), { done: 'used', version: 6, watch_version: 6, watch_said: 'used' });
  await done(token, { done: 'unreadable', version: 6 });
  assert.deepEqual(await status(writer), { done: 'used', version: 6, watch_version: 6, watch_said: 'unreadable' });
  await done(token, { done: 'used' });
  assert.equal((await status(writer)).watch_said, 'unreadable');
  // Either side forgets it: then neither gets anything.
  const forget = (secret) => fetch(link(), { method: 'DELETE', headers: bearer(secret) });
  response = await forget(writer);
  assert.equal(response.status, 200);
  assert.deepEqual(await response.json(), {});
  assert.equal((await fetch(link(), { headers: bearer(token) })).status, 404);
  assert.equal(await item(token, 0), 404);
  assert.equal(await status(writer), 404);
  assert.equal((await update(writer, 8, changed)).status, 404);
  for (const secret of [writer, token, id()]) assert.equal((await forget(secret)).status, 200);
  assert.equal((await fetch(link(), { method: 'DELETE' })).status, 401);
  // Made and taken, then forgotten by the watch; a link that stays for the
  // last checks.
  const linked = async (headers) => {
    const watchSide = await pair(b64(64), headers);
    const appSide = id();
    assert.equal((await send(pairing(watchSide.code), 'PUT', { ...good, writer: appSide }, headers)).status, 201);
    assert.equal((await done(watchSide.token, { done: 'used', version: 5 })).status, 200);
    return { token: watchSide.token, writer: appSide };
  };
  const forgotten = await linked();
  assert.equal((await forget(forgotten.token)).status, 200);
  assert.equal(await status(forgotten.writer), 404);
  const lasting = await linked();
  // Each way a pairing can end without a link: the app hears it, and the
  // watch gets nothing more. Anything else, or nothing, is "left".
  for (const [asked, kept] of [['declined', 'declined'], ['unreadable', 'unreadable'], ['left', 'left'], ['bogus', 'left'], [undefined, 'left']]) {
    const other = await pair();
    const otherWriter = id();
    assert.equal((await send(pairing(other.code), 'PUT', { ...good, writer: otherWriter })).status, 201);
    assert.equal((await done(other.token, asked === undefined ? {} : { done: asked })).status, 200);
    assert.equal((await status(otherWriter)).done, kept, String(asked));
    assert.equal((await fetch(link(), { headers: bearer(other.token) })).status, 404);
    assert.equal(await item(other.token, 0), 404);
    assert.equal((await fetch(pairing(other.code))).status, 410);
    assert.equal((await update(otherWriter, 6, changed)).status, 409);
  }
  // A watch may give up before anything was sent; "used" then is "left".
  for (const asked of ['left', 'used']) {
    const dropped = await pair();
    response = await done(dropped.token, { done: asked, version: 5 });
    assert.equal(response.status, 200);
    assert.deepEqual(await response.json(), {});
    assert.equal((await fetch(pairing(dropped.code))).status, 410);
    assert.equal((await fetch(link(), { headers: bearer(dropped.token) })).status, 404);
    assert.equal((await send(pairing(dropped.code), 'PUT', { ...good, writer: id() })).status, 410);
  }
  // Answers about links nobody has get the same answer.
  response = await done(id(), { done: 'used', version: 5 });
  assert.equal(response.status, 200);
  assert.deepEqual(await response.json(), {});
  assert.equal((await send(link('/done'), 'POST', 'not json', bearer(token))).status, 400);
  // The largest and the smallest items, and the most of them.
  {
    const other = await pair();
    const big = [b64(12 * 1024), b64(64), ...Array(14).fill(box(1))];
    assert.equal((await send(pairing(other.code), 'PUT', { ...good, writer: id(), items: big })).status, 201);
    assert.equal((await (await fetch(link(), { headers: bearer(other.token) })).json()).count, 16);
    assert.equal((await item(other.token, 0)).box, big[0]);
    assert.equal((await item(other.token, 15)).box, big[15]);
  }
  // Unknown and bad codes.
  const unknown = code === '00000000' ? '00000001' : '00000000';
  for (const bad of [unknown, '1234567', '123456789', '1234 5678', 'abcdefgh']) {
    assert.equal((await fetch(pairing(encodeURIComponent(bad)))).status, 404, bad);
    assert.equal((await send(pairing(encodeURIComponent(bad)), 'PUT', { ...good, writer: id() })).status, 404, bad);
  }
  assert.equal((await fetch(pairings)).status, 405);
  assert.equal((await fetch(`${BASE}/v1/watch/deliveries/${token}`)).status, 404);
  response = await fetch(link(), { method: 'OPTIONS' });
  assert.equal(response.status, 204);
  assert.match(response.headers.get('Access-Control-Allow-Headers'), /Authorization/);
  // One address may start ten pairings a minute.
  for (let i = 0; i < 10; i++) await pair(b64(64), from(7));
  response = await send(pairings, 'POST', { key: b64(64) }, from(7));
  assert.equal(response.status, 429);
  assert.ok((await response.json()).error);
  await pair(b64(64), from(8));
  // ... and look up or send to 30 codes a minute, to slow anyone guessing codes.
  for (let i = 0; i < 30; i++) {
    const tried = String(i).padStart(8, '0');
    const status = i % 2 ? (await send(pairing(tried), 'PUT', good, from(10))).status : (await fetch(pairing(tried), { headers: from(10) })).status;
    assert.notEqual(status, 429, `lookup ${i}`);
  }
  response = await fetch(pairing(unknown), { headers: from(10) });
  assert.equal(response.status, 429);
  assert.ok((await response.json()).error);
  assert.equal((await send(pairing(unknown), 'PUT', good, from(10))).status, 429);
  assert.equal((await fetch(pairing(unknown), { headers: from(11) })).status, 404);

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
  server = launch(['MAX_DATABASE_BYTES:1', 'WATCH_TTL_MS:1000']);
  await ready();
  // Pairings expire: then nobody gets anything, and the code is unknown. A
  // full database takes no new links, and keeps the ones it has going.
  const late = await pair(b64(64), from(9));
  assert.equal(late.expires_in, 1);
  assert.equal((await send(pairing(late.code), 'PUT', { ...good, writer: id() }, from(9))).status, 507);
  assert.equal((await update(lasting.writer, 9, changed)).status, 200);
  await new Promise((resolve) => setTimeout(resolve, 1200));
  assert.equal((await fetch(pairing(late.code), { headers: from(9) })).status, 404);
  assert.equal((await fetch(link(), { headers: bearer(late.token, from(9)) })).status, 404);
  assert.deepEqual(await (await fetch(link(), { headers: bearer(lasting.token, from(9)) })).json(), { key: appKey, version: 9, count: 1 });
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
