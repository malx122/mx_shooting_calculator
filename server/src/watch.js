// Garmin watches paired with a phone (docs/watch.md).
//
// Pairing: the watch sends its public key and gets a code to show and a
// token to keep. The hunter types the code into the app, which fetches the
// watch's key and sends its own key, a writer secret and the profiles, each
// encrypted for the watch. The watch fetches them with its token, asks the
// hunter, and says how it went. A pairing the watch took stays as the link:
// the app replaces the profiles with its writer secret, and the watch
// fetches them with its token whenever it opens.
//
//   POST   /v1/watch/pairings        {key} -> 200 {code, token, expires_in}
//   GET    /v1/watch/pairings/C      200 {key} waiting for the app; 409 the
//                                    app has sent; 410 the watch has
//                                    answered; 404 unknown or expired
//   PUT    /v1/watch/pairings/C      {key, writer, version, items} -> 201;
//                                    404; 409 sent already; 410 answered
//   GET    /v1/watch/link            token: 200 {waiting: true} nothing
//                                    sent yet; 200 {key, version, count}
//   GET    /v1/watch/link/items/I    token: 200 {version, box}
//   POST   /v1/watch/link/done       token: {done, version} -> 200
//                                    {taken: true} when it made the link,
//                                    else {}
//   GET    /v1/watch/link/status     writer: 200 {done, version,
//                                    watch_version, watch_said}
//   PUT    /v1/watch/link            writer: {version, items} -> 200 {};
//                                    409 not newer, or not taken yet
//   DELETE /v1/watch/link            token or writer: 200 {}
//
// The token and the writer go as "Authorization: Bearer <64 hex digits>";
// the server keeps only their SHA-256. Without one: 401. Unknown or expired
// links are 404. A pairing lives ten minutes from the watch's POST, and ten
// more from the app's PUT, for the hunter to answer.
// C is 8 random decimal digits. done is used, declined, unreadable or left;
// the first answer to a pairing stays. The server holds two public keys,
// the encrypted items and their version, and what the watch says it has:
// nothing it could read. A pairing the watch didn't take goes after ten
// minutes; a link, after 400 days unused, or when either side forgets it.
// Lookups by code are limited per address, to slow anyone guessing codes.
//
// Every answer is JSON, errors too, and the watch's successes are all 200:
// Garmin Connect relays other codes and empty bodies unreliably.

// How long a pairing lives; WATCH_TTL_MS: shorter, in the tests only.
const TTL_MS = 10 * 60 * 1000;
const DAY_MS = 24 * 60 * 60 * 1000;
// A link neither side has used for this long is forgotten.
const KEEP_MS = 400 * DAY_MS;
const CODE = /^\d{8}$/;
const BEARER = /^Bearer ([0-9a-f]{64})$/;
// How it went: the hunter took the profiles, said no, they didn't open or
// parse, or the watch stopped waiting first (and anything else).
const OUTCOMES = ['used', 'declined', 'unreadable', 'left'];
// Standard base64 with its padding, and nothing else (atob takes more).
const BASE64 = /^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/;
const HEX = /^[0-9a-f]{64}$/;
// A public key on P-256: X then Y, 32 bytes each.
const KEY_BYTES = 64;
// iv (16) || AES-CBC blocks (16 each) || tag (32).
const BOX_OVERHEAD = 48;
const MAX_BOX = 12 * 1024;
const MAX_ITEMS = 16;
// What a watch's signed 32-bit numbers hold.
const MAX_VERSION = 2 ** 31 - 1;
// Bodies a little over the most items in base64.
const MAX_BODY = 300 * 1024;
// The largest multiple of 10^8 a 32-bit number reaches: draws at or past
// it would favour the low codes.
const CODE_RANGE = 1e8;
const FAIR_LIMIT = Math.floor(2 ** 32 / CODE_RANGE) * CODE_RANGE;

export async function watch(request, env, url, json, tooMany, full) {
  const path = url.pathname;
  const ttl = Number(env.WATCH_TTL_MS) || TTL_MS;
  const now = Date.now();
  // Every lookup below skips expired pairings itself; they are deleted
  // when the next pairing is made and by the clean-up.
  if (path === '/v1/watch/pairings') {
    if (request.method !== 'POST') return json(405, { error: 'method not allowed' });
    if (await tooMany(request, env.WATCH_LIMIT)) {
      return json(429, { error: 'too many new pairings; try again in a minute' });
    }
    const body = await readObject(request);
    const key = body && bytes(body.key);
    if (!key || key.length !== KEY_BYTES) return json(400, { error: 'bad key' });
    // Expired pairings go first, so their codes are free again.
    await forgetExpired(env);
    const token = hex(crypto.getRandomValues(new Uint8Array(32)));
    const tokenHash = await sha256(token);
    for (let tries = 0; tries < 5; tries++) {
      const code = newCode();
      const result = await env.DB.prepare(
        'INSERT INTO watch_links (token_hash, code, watch_key, created_at, used_at) VALUES (?1, ?2, ?3, ?4, ?4) ON CONFLICT DO NOTHING',
      )
        .bind(tokenHash, code, btoa(key), now)
        .run();
      if (result.meta.changes === 1) {
        return json(200, { code, token, expires_in: Math.floor(ttl / 1000) });
      }
    }
    return json(503, { error: 'no free code; try again' });
  }

  const pairing = /^\/v1\/watch\/pairings\/([^/]*)$/.exec(path);
  if (pairing) {
    const code = pairing[1];
    if ((request.method === 'GET' || request.method === 'PUT') && (await tooMany(request, env.WATCH_LOOKUP_LIMIT))) {
      return json(429, { error: 'too many codes tried; try again in a minute' });
    }
    if (request.method === 'GET') {
      if (!CODE.test(code)) return gone(json);
      const row = await byCode(env, code, now - ttl);
      if (!row) return gone(json);
      if (row.done) return json(410, { error: 'the watch has stopped waiting with this code' });
      if (row.app_key) return json(409, { error: 'sent; the watch has not answered yet' });
      return json(200, { key: row.watch_key });
    }
    if (request.method === 'PUT') {
      if (!CODE.test(code)) return gone(json);
      const body = await readObject(request);
      if (!body) return json(400, { error: 'bad body' });
      const key = bytes(body.key);
      if (!key || key.length !== KEY_BYTES) return json(400, { error: 'bad key' });
      if (typeof body.writer !== 'string' || !HEX.test(body.writer)) return json(400, { error: 'bad writer' });
      const version = versionOf(body.version);
      if (version === null) return json(400, { error: 'bad version' });
      const items = itemsOf(body.items);
      if (!items) return json(400, { error: 'bad items' });
      // The same sender again (its answer was lost on the way): the same
      // answer, also when the watch has answered since, and even when the
      // server has filled up meanwhile, since the pairing is already kept.
      const writerHash = await sha256(body.writer);
      const mine = () =>
        env.DB.prepare('SELECT 1 FROM watch_links WHERE writer_hash = ?1 AND app_key = ?2')
          .bind(writerHash, btoa(key))
          .first();
      if (await mine()) return json(201, {});
      if (await full(env)) return json(507, { error: 'the sync server is full' });
      // Only the first sender: the pairing must still be waiting. Its ten
      // minutes start again, for the hunter to answer.
      const result = await env.DB.prepare(
        'UPDATE watch_links SET app_key = ?2, writer_hash = ?3, version = ?4, items = ?5, created_at = ?6, used_at = ?6 WHERE code = ?1 AND app_key IS NULL AND done IS NULL AND created_at > ?7',
      )
        .bind(code, btoa(key), writerHash, version, JSON.stringify(items), now, now - ttl)
        .run();
      if (result.meta.changes === 1) return json(201, {});
      // Two tries from the same sender crossing on the way.
      if (await mine()) return json(201, {});
      const row = await byCode(env, code, now - ttl);
      if (!row) return gone(json);
      if (row.done) return json(410, { error: 'the watch has stopped waiting with this code' });
      return json(409, { error: 'already sent' });
    }
    return json(405, { error: 'method not allowed' });
  }

  const link = /^\/v1\/watch\/link(?:\/(items\/[^/]*|done|status))?$/.exec(path);
  if (!link) return json(404, { error: 'not found' });
  const what = link[1] ?? '';
  const secret = BEARER.exec(request.headers.get('Authorization') ?? '')?.[1];
  if (!secret) return json(401, { error: 'say who you are (Authorization: Bearer ...)' });
  const hash = await sha256(secret);
  // A link, or a pairing within its ten minutes.
  const alive = `(done = 'used' OR created_at > ?2)`;

  if (what === '' && request.method === 'DELETE') {
    // Either side forgets it. The same answer whether there was one or
    // not, so it tells nobody anything.
    await env.DB.prepare('DELETE FROM watch_links WHERE token_hash = ?1 OR writer_hash = ?1').bind(hash).run();
    return json(200, {});
  }

  if (what === 'status' || (what === '' && request.method === 'PUT')) {
    // The app's side, with its writer secret.
    if (what === 'status') {
      if (request.method !== 'GET') return json(405, { error: 'method not allowed' });
      const row = await env.DB.prepare(
        `SELECT done, version, watch_version, watch_said FROM watch_links WHERE writer_hash = ?1 AND ${alive}`,
      )
        .bind(hash, now - ttl)
        .first();
      if (!row) return unknown(json);
      return json(200, {
        done: row.done,
        version: row.version,
        watch_version: row.watch_version,
        watch_said: row.watch_said,
      });
    }
    const body = await readObject(request);
    if (!body) return json(400, { error: 'bad body' });
    const version = versionOf(body.version);
    if (version === null) return json(400, { error: 'bad version' });
    const items = itemsOf(body.items);
    if (!items) return json(400, { error: 'bad items' });
    const result = await env.DB.prepare(
      "UPDATE watch_links SET version = ?2, items = ?3, used_at = ?4 WHERE writer_hash = ?1 AND done = 'used' AND version < ?2",
    )
      .bind(hash, version, JSON.stringify(items), now)
      .run();
    if (result.meta.changes === 1) return json(200, {});
    const row = await env.DB.prepare(`SELECT done FROM watch_links WHERE writer_hash = ?1 AND ${alive}`)
      .bind(hash, now - ttl)
      .first();
    if (!row) return unknown(json);
    if (row.done !== 'used') return json(409, { error: 'the watch has not taken the pairing yet' });
    return json(409, { error: 'not newer than the version there' });
  }

  // The watch's side, with its token.
  const row = await env.DB.prepare(
    `SELECT app_key, version, done, used_at, json_array_length(items) AS count FROM watch_links WHERE token_hash = ?1 AND ${alive}`,
  )
    .bind(hash, now - ttl)
    .first();
  if (what === 'done') {
    if (request.method !== 'POST') return json(405, { error: 'method not allowed' });
    const body = await readObject(request);
    if (!body) return json(400, { error: 'bad body' });
    const done = OUTCOMES.includes(body.done) ? body.done : 'left';
    const version = versionOf(body.version);
    if (row && row.done === null) {
      // The answer to the pairing: taken, it becomes the link and its code
      // goes, and the watch is told so (it keeps the link only then);
      // anything else and the app's key and items go now, the rest with the
      // ten minutes.
      if (done === 'used' && row.app_key) {
        const result = await env.DB.prepare(
          "UPDATE watch_links SET done = 'used', code = NULL, watch_version = ?2, watch_said = 'used', used_at = ?3 WHERE token_hash = ?1 AND done IS NULL",
        )
          .bind(hash, version, now)
          .run();
        if (result.meta.changes === 1) return json(200, { taken: true });
      } else {
        await env.DB.prepare(
          'UPDATE watch_links SET done = ?2, app_key = NULL, items = NULL WHERE token_hash = ?1 AND done IS NULL',
        )
          .bind(hash, done === 'used' ? 'left' : done)
          .run();
      }
    } else if (row && row.done === 'used' && done === 'used' && version !== null) {
      // The watch has a version (also the pairing's, said again when the
      // first answer was lost on the way).
      await env.DB.prepare("UPDATE watch_links SET watch_version = ?2, watch_said = 'used', used_at = ?3 WHERE token_hash = ?1")
        .bind(hash, version, now)
        .run();
      return json(200, { taken: true });
    } else if (row && row.done === 'used' && done === 'unreadable') {
      // It couldn't use the latest it fetched.
      await env.DB.prepare("UPDATE watch_links SET watch_said = 'unreadable', used_at = ?2 WHERE token_hash = ?1")
        .bind(hash, now)
        .run();
    }
    return json(200, {});
  }
  if (request.method !== 'GET') return json(405, { error: 'method not allowed' });
  // A pairing the watch has answered no to is over for the watch.
  if (!row || (row.done && row.done !== 'used')) return unknown(json);
  if (!row.app_key) return json(200, { waiting: true });
  if (what === '') {
    if (row.done === 'used' && row.used_at < now - DAY_MS) {
      await env.DB.prepare('UPDATE watch_links SET used_at = ?2 WHERE token_hash = ?1').bind(hash, now).run();
    }
    return json(200, { key: row.app_key, version: row.version, count: row.count });
  }
  const index = /^items\/(\d{1,2})$/.exec(what);
  if (!index || Number(index[1]) >= row.count) return json(404, { error: 'no such item' });
  const item = await env.DB.prepare('SELECT version, json_extract(items, ?2) AS box FROM watch_links WHERE token_hash = ?1')
    .bind(hash, `$[${Number(index[1])}]`)
    .first();
  if (!item || typeof item.box !== 'string') return json(404, { error: 'no such item' });
  return json(200, { version: item.version, box: item.box });
}

/// Forgets pairings past their ten minutes the watch didn't take, and
/// links unused for 400 days.
export async function forgetExpired(env) {
  const ttl = Number(env.WATCH_TTL_MS) || TTL_MS;
  const now = Date.now();
  await env.DB.prepare("DELETE FROM watch_links WHERE (done IS NULL OR done != 'used') AND created_at <= ?1")
    .bind(now - ttl)
    .run();
  await env.DB.prepare('DELETE FROM watch_links WHERE used_at <= ?1')
    .bind(now - KEEP_MS)
    .run();
}

/// The pairing with `code`, if it was made after `since`.
async function byCode(env, code, since) {
  return env.DB.prepare('SELECT watch_key, app_key, writer_hash, done FROM watch_links WHERE code = ?1 AND created_at > ?2')
    .bind(code, since)
    .first();
}

function gone(json) {
  return json(404, { error: 'no such pairing, or it has expired' });
}

function unknown(json) {
  return json(404, { error: 'no such link' });
}

/// A version: a whole number from 1 to 2^31 - 1; else null.
function versionOf(value) {
  return Number.isInteger(value) && value >= 1 && value <= MAX_VERSION ? value : null;
}

/// 1 to MAX_ITEMS boxes, each standard base64 of an iv, whole blocks and a
/// tag; else null.
function itemsOf(value) {
  if (!Array.isArray(value) || value.length < 1 || value.length > MAX_ITEMS) return null;
  for (const item of value) {
    const box = bytes(item);
    if (!box || box.length < BOX_OVERHEAD + 16 || box.length > MAX_BOX || (box.length - BOX_OVERHEAD) % 16 !== 0) {
      return null;
    }
  }
  return value;
}

/// The request's body if it is a JSON object, else null.
async function readObject(request) {
  const body = await readAtMost(request, MAX_BODY);
  if (!body) return null;
  try {
    const value = JSON.parse(new TextDecoder().decode(body));
    return value && typeof value === 'object' && !Array.isArray(value) ? value : null;
  } catch {
    return null;
  }
}

/// The bytes of standard, padded base64, as a binary string; null for
/// anything else.
function bytes(text) {
  if (typeof text !== 'string' || !BASE64.test(text)) return null;
  return atob(text);
}

/// A code: 8 random decimal digits, each code as likely as the next.
function newCode() {
  const draw = new Uint32Array(1);
  do crypto.getRandomValues(draw);
  while (draw[0] >= FAIR_LIMIT);
  return String(draw[0] % CODE_RANGE).padStart(8, '0');
}

async function sha256(text) {
  return hex(new Uint8Array(await crypto.subtle.digest('SHA-256', new TextEncoder().encode(text))));
}

function hex(bytes) {
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('');
}

/// The request's body, or null if it is larger than `most` bytes. Stops
/// reading there, also when no Content-Length was sent.
export async function readAtMost(request, most) {
  if (Number(request.headers.get('Content-Length') ?? 0) > most) return null;
  if (!request.body) return new Uint8Array(0);
  const reader = request.body.getReader();
  const chunks = [];
  let size = 0;
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    size += value.byteLength;
    if (size > most) {
      await reader.cancel();
      return null;
    }
    chunks.push(value);
  }
  const body = new Uint8Array(size);
  let at = 0;
  for (const chunk of chunks) {
    body.set(chunk, at);
    at += chunk.byteLength;
  }
  return body;
}
